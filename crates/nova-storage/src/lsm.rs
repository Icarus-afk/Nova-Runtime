use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write, Seek, SeekFrom};
use std::time::{SystemTime, UNIX_EPOCH};
use xxhash_rust::xxh3::xxh3_64;
use nova_core::types::*;
use nova_core::error::*;

const SSTABLE_MAGIC: u32 = 0x53535442;
const SSTABLE_VERSION: u32 = 1;
const DEFAULT_BLOOM_BITS_PER_KEY: u32 = 10;
const DATA_BLOCK_TARGET_SIZE: usize = 64 * 1024;

#[derive(Clone)]
pub struct MemTable {
    data: BTreeMap<Vec<u8>, MemTableEntry>,
    size: usize,
    immutable: bool,
}

#[derive(Clone)]
struct MemTableEntry {
    value: Value,
    flags: u8,
}

impl MemTable {
    pub fn new() -> Self {
        MemTable {
            data: BTreeMap::new(),
            size: 0,
            immutable: false,
        }
    }

    pub fn get(&self, key: &Key) -> Option<Value> {
        let entry = self.data.get(key.as_bytes())?;
        if entry.flags & 0x01 != 0 {
            return None;
        }
        Some(entry.value.clone())
    }

    pub fn insert(&mut self, key: Key, value: Value) {
        let entry = MemTableEntry {
            value: value.clone(),
            flags: 0,
        };
        let key_bytes = key.as_bytes().to_vec();
        let key_len = key_bytes.len();
        let val_len = value.len();
        if !self.data.contains_key(&key_bytes) {
            self.size += key_len + val_len + 8;
        } else {
            let old = self.data.get(&key_bytes).unwrap();
            self.size = self.size.saturating_sub(old.value.len());
            self.size += val_len;
        }
        self.data.insert(key_bytes, entry);
    }

    pub fn delete(&mut self, key: &Key) {
        let entry = MemTableEntry {
            value: Value::new(vec![]),
            flags: 0x01,
        };
        let key_bytes = key.as_bytes().to_vec();
        if !self.data.contains_key(&key_bytes) {
            self.size += key_bytes.len() + 8;
        }
        self.data.insert(key_bytes, entry);
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (Key, Value)> + '_ {
        self.data.iter().filter_map(|(k, entry)| {
            if entry.flags & 0x01 != 0 {
                None
            } else {
                Some((Key::new(k.clone()), entry.value.clone()))
            }
        })
    }

    pub fn set_immutable(&mut self) {
        self.immutable = true;
    }

    pub fn is_immutable(&self) -> bool {
        self.immutable
    }
}

#[derive(Debug, Clone)]
pub struct BloomFilter {
    bits: Vec<u64>,
    num_hashes: u32,
    num_bits: u64,
    num_keys: u32,
}

impl BloomFilter {
    pub fn new(num_keys: u32, bits_per_key: u32) -> Self {
        let bits_per = if bits_per_key == 0 { DEFAULT_BLOOM_BITS_PER_KEY } else { bits_per_key };
        let raw_bits = num_keys as u64 * bits_per as u64;
        let num_bits = if raw_bits < 64 { 64 } else { raw_bits.next_power_of_two() };
        let num_hashes = ((num_bits as f64 / num_keys as f64) * 0.69) as u32;
        let num_hashes = num_hashes.max(1).min(30);
        let bit_len = (num_bits / 64).max(1) as usize;
        BloomFilter {
            bits: vec![0u64; bit_len],
            num_hashes,
            num_bits,
            num_keys,
        }
    }

    pub fn insert(&mut self, key: &[u8]) {
        let h1 = xxh3_64(key);
        let h2 = xxh3_64(&Self::flip_bit(key));
        for i in 0..self.num_hashes {
            let idx = (h1.wrapping_add(i as u64 * h2)) % self.num_bits;
            let word = (idx / 64) as usize;
            let bit = idx % 64;
            if word < self.bits.len() {
                self.bits[word] |= 1u64 << bit;
            }
        }
    }

    pub fn may_contain(&self, key: &[u8]) -> bool {
        if self.bits.is_empty() {
            return true;
        }
        let h1 = xxh3_64(key);
        let h2 = xxh3_64(&Self::flip_bit(key));
        for i in 0..self.num_hashes {
            let idx = (h1.wrapping_add(i as u64 * h2)) % self.num_bits;
            let word = (idx / 64) as usize;
            let bit = idx % 64;
            if word >= self.bits.len() {
                return false;
            }
            if self.bits[word] & (1u64 << bit) == 0 {
                return false;
            }
        }
        true
    }

    fn flip_bit(data: &[u8]) -> Vec<u8> {
        data.iter().map(|&b| !b).collect()
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.num_hashes.to_le_bytes());
        buf.extend_from_slice(&self.num_bits.to_le_bytes());
        buf.extend_from_slice(&self.num_keys.to_le_bytes());
        buf.extend_from_slice(&(self.bits.len() as u32).to_le_bytes());
        for word in &self.bits {
            buf.extend_from_slice(&word.to_le_bytes());
        }
        buf
    }

    pub fn decode(data: &[u8]) -> Result<Self> {
        if data.len() < 20 {
            return Err(RuntimeError::CorruptData("Bloom filter data too short".into()));
        }
        let mut off = 0;
        let num_hashes = u32::from_le_bytes(data[off..off + 4].try_into().unwrap());
        off += 4;
        let num_bits = u64::from_le_bytes(data[off..off + 8].try_into().unwrap());
        off += 8;
        let num_keys = u32::from_le_bytes(data[off..off + 4].try_into().unwrap());
        off += 4;
        let bit_len = u32::from_le_bytes(data[off..off + 4].try_into().unwrap()) as usize;
        off += 4;
        let expected = off + bit_len * 8;
        if data.len() < expected {
            return Err(RuntimeError::CorruptData("Bloom filter truncated".into()));
        }
        let mut bits = Vec::with_capacity(bit_len);
        for _ in 0..bit_len {
            bits.push(u64::from_le_bytes(data[off..off + 8].try_into().unwrap()));
            off += 8;
        }
        Ok(BloomFilter { bits, num_hashes, num_bits, num_keys })
    }
}

#[derive(Debug, Clone)]
pub struct SSTable {
    pub id: u64,
    pub path: PathBuf,
    pub key_min: Key,
    pub key_max: Key,
    pub level: u8,
    pub size: u64,
    pub created_at: i64,
    bloom_filter: BloomFilter,
}

struct DataBlockHeader {
    offset: u64,
    length: u64,
    uncompressed_length: u64,
    first_key: Vec<u8>,
    last_key: Vec<u8>,
}

impl SSTable {
    pub fn create(dir: &Path, id: u64, level: u8, entries: Vec<(Key, Value)>) -> Result<Self> {
        fs::create_dir_all(dir)?;
        let filename = format!("lsm_{:06}_l{}.sst", id, level);
        let path = dir.join(&filename);
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .read(true)
            .open(&path)?;

        let mut bloom = BloomFilter::new(entries.len() as u32, DEFAULT_BLOOM_BITS_PER_KEY);
        let mut data_blocks: Vec<DataBlockHeader> = Vec::new();
        let mut current_block_data: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
        let mut current_block_size = 0usize;
        let mut data_offset = 0u64;

        let key_min = entries.first().map(|(k, _)| k.clone()).unwrap_or_else(|| Key::new(vec![]));
        let key_max = entries.last().map(|(k, _)| k.clone()).unwrap_or_else(|| Key::new(vec![]));

        for (key, value) in &entries {
            bloom.insert(key.as_bytes());
            let entry_len = 4 + key.len() + 4 + value.len();
            if current_block_size + entry_len > DATA_BLOCK_TARGET_SIZE && !current_block_data.is_empty() {
                let first_k = current_block_data.first().unwrap().0.clone();
                let last_k = current_block_data.last().unwrap().0.clone();
                let compressed = Self::compress_block(&current_block_data)?;
                let uncompressed_len = current_block_size as u64;
                let block_offset = data_offset;
                let block_len = compressed.len() as u64;

                file.seek(SeekFrom::Start(data_offset))?;
                file.write_all(&compressed)?;

                data_blocks.push(DataBlockHeader {
                    offset: block_offset,
                    length: block_len,
                    uncompressed_length: uncompressed_len,
                    first_key: first_k,
                    last_key: last_k,
                });
                data_offset += block_len;
                current_block_data.clear();
                current_block_size = 0;
            }
            current_block_data.push((key.as_bytes().to_vec(), value.as_bytes().to_vec()));
            current_block_size += entry_len;
        }

        if !current_block_data.is_empty() {
            let first_k = current_block_data.first().unwrap().0.clone();
            let last_k = current_block_data.last().unwrap().0.clone();
            let compressed = Self::compress_block(&current_block_data)?;
            let uncompressed_len = current_block_size as u64;

            file.seek(SeekFrom::Start(data_offset))?;
            file.write_all(&compressed)?;

            data_blocks.push(DataBlockHeader {
                offset: data_offset,
                length: compressed.len() as u64,
                uncompressed_length: uncompressed_len,
                first_key: first_k,
                last_key: last_k,
            });
            data_offset += compressed.len() as u64;
        }

        let bloom_data = bloom.encode();
        let bloom_offset = data_offset;
        file.seek(SeekFrom::Start(bloom_offset))?;
        file.write_all(&(bloom_data.len() as u32).to_le_bytes())?;
        file.write_all(&bloom_data)?;
        data_offset += 4 + bloom_data.len() as u64;

        let mut index_buf = Vec::new();
        index_buf.extend_from_slice(&(data_blocks.len() as u32).to_le_bytes());
        for block in &data_blocks {
            index_buf.extend_from_slice(&block.offset.to_le_bytes());
            index_buf.extend_from_slice(&block.length.to_le_bytes());
            index_buf.extend_from_slice(&block.uncompressed_length.to_le_bytes());
            index_buf.extend_from_slice(&(block.first_key.len() as u32).to_le_bytes());
            index_buf.extend_from_slice(&block.first_key);
            index_buf.extend_from_slice(&(block.last_key.len() as u32).to_le_bytes());
            index_buf.extend_from_slice(&block.last_key);
        }
        let index_offset = data_offset;
        let index_len = index_buf.len() as u64;
        file.seek(SeekFrom::Start(index_offset))?;
        file.write_all(&index_buf)?;
        let footer_checksum = crc32c::crc32c(&index_buf);
        let mut footer = Vec::with_capacity(56);
        footer.extend_from_slice(&SSTABLE_MAGIC.to_le_bytes());
        footer.extend_from_slice(&SSTABLE_VERSION.to_le_bytes());
        footer.extend_from_slice(&index_offset.to_le_bytes());
        footer.extend_from_slice(&index_len.to_le_bytes());
        footer.extend_from_slice(&bloom_offset.to_le_bytes());
        footer.extend_from_slice(&(bloom_data.len() as u64).to_le_bytes());
        footer.extend_from_slice(&(data_blocks.len() as u32).to_le_bytes());
        let total_uncompressed: u64 = data_blocks.iter().map(|b| b.uncompressed_length).sum();
        footer.extend_from_slice(&total_uncompressed.to_le_bytes());
        footer.extend_from_slice(&footer_checksum.to_le_bytes());
        file.seek(SeekFrom::End(0))?;
        file.write_all(&footer)?;
        file.sync_all()?;

        let metadata = file.metadata()?;
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as i64;

        Ok(SSTable {
            id,
            path,
            key_min,
            key_max,
            level,
            size: metadata.len(),
            created_at,
            bloom_filter: bloom,
        })
    }

    fn compress_block(entries: &[(Vec<u8>, Vec<u8>)]) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        for (key, value) in entries {
            buf.extend_from_slice(&(key.len() as u32).to_le_bytes());
            buf.extend_from_slice(key);
            buf.extend_from_slice(&(value.len() as u32).to_le_bytes());
            buf.extend_from_slice(value);
        }
        compress_data(&buf, CompressionCodec::Snappy)
    }

    fn decompress_block(data: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
        let raw = decompress_data(data, CompressionCodec::Snappy)?;
        let mut entries = Vec::new();
        let mut off = 0;
        while off < raw.len() {
            if off + 4 > raw.len() {
                break;
            }
            let key_len = u32::from_le_bytes(raw[off..off + 4].try_into().unwrap()) as usize;
            off += 4;
            if off + key_len > raw.len() {
                break;
            }
            let key = raw[off..off + key_len].to_vec();
            off += key_len;
            if off + 4 > raw.len() {
                break;
            }
            let val_len = u32::from_le_bytes(raw[off..off + 4].try_into().unwrap()) as usize;
            off += 4;
            if off + val_len > raw.len() {
                break;
            }
            let value = raw[off..off + val_len].to_vec();
            off += val_len;
            entries.push((key, value));
        }
        Ok(entries)
    }

    pub fn open(path: &Path) -> Result<Self> {
        let mut file = File::open(path)?;
        let metadata = file.metadata()?;
        let file_size = metadata.len();
        if file_size < 56 {
            return Err(RuntimeError::CorruptData("SSTable too small".into()));
        }
        file.seek(SeekFrom::End(-56))?;
        let mut footer = vec![0u8; 56];
        file.read_exact(&mut footer)?;

        let magic = u32::from_le_bytes(footer[0..4].try_into().unwrap());
        if magic != SSTABLE_MAGIC {
            return Err(RuntimeError::CorruptData("SSTable invalid magic".into()));
        }
        let _version = u32::from_le_bytes(footer[4..8].try_into().unwrap());
        let index_offset = u64::from_le_bytes(footer[8..16].try_into().unwrap());
        let index_length = u64::from_le_bytes(footer[16..24].try_into().unwrap());
        let bloom_offset = u64::from_le_bytes(footer[24..32].try_into().unwrap());
        let bloom_length = u64::from_le_bytes(footer[32..40].try_into().unwrap());
        let _data_blocks_count = u32::from_le_bytes(footer[40..44].try_into().unwrap());
        let _total_uncompressed = u64::from_le_bytes(footer[44..52].try_into().unwrap());
        let _footer_checksum = u32::from_le_bytes(footer[52..56].try_into().unwrap());

        file.seek(SeekFrom::Start(index_offset))?;
        let mut index_data = vec![0u8; index_length as usize];
        file.read_exact(&mut index_data)?;

        let _actual_checksum = crc32c::crc32c(&index_data);
        let num_blocks = u32::from_le_bytes(index_data[0..4].try_into().unwrap()) as usize;
        let mut key_min: Option<Key> = None;
        let mut key_max: Option<Key> = None;
        let mut off = 4;
        for _ in 0..num_blocks {
            if off + 32 > index_data.len() {
                break;
            }
            let _block_off = u64::from_le_bytes(index_data[off..off + 8].try_into().unwrap());
            off += 8;
            let _block_len = u64::from_le_bytes(index_data[off..off + 8].try_into().unwrap());
            off += 8;
            let _uncomp_len = u64::from_le_bytes(index_data[off..off + 8].try_into().unwrap());
            off += 8;
            let fk_len = u32::from_le_bytes(index_data[off..off + 4].try_into().unwrap()) as usize;
            off += 4;
            if off + fk_len > index_data.len() {
                break;
            }
            let fk = index_data[off..off + fk_len].to_vec();
            off += fk_len;
            if key_min.is_none() {
                key_min = Some(Key::new(fk));
            }
            let lk_len = u32::from_le_bytes(index_data[off..off + 4].try_into().unwrap()) as usize;
            off += 4;
            if off + lk_len > index_data.len() {
                break;
            }
            let lk = index_data[off..off + lk_len].to_vec();
            off += lk_len;
            key_max = Some(Key::new(lk));
        }

        let filename = path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("0")
            .to_string();
        let id = filename.split('_').nth(1).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
        let level = filename.split('_').nth(2)
            .and_then(|s| s.strip_prefix('l'))
            .and_then(|s| s.parse::<u8>().ok())
            .unwrap_or(0);

        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as i64;

        let bloom_filter = if bloom_length > 0 {
            file.seek(SeekFrom::Start(bloom_offset))?;
            let mut bloom_len_buf = [0u8; 4];
            file.read_exact(&mut bloom_len_buf)?;
            let bloom_data_len = u32::from_le_bytes(bloom_len_buf) as usize;
            let mut bloom_data = vec![0u8; bloom_data_len];
            file.read_exact(&mut bloom_data)?;
            BloomFilter::decode(&bloom_data)?
        } else {
            BloomFilter::new(1000, 10)
        };

        Ok(SSTable {
            id,
            path: path.to_path_buf(),
            key_min: key_min.unwrap_or_else(|| Key::new(vec![])),
            key_max: key_max.unwrap_or_else(|| Key::new(vec![])),
            level,
            size: file_size,
            created_at,
            bloom_filter,
        })
    }

    pub fn get(&self, key: &Key) -> Result<Option<Value>> {
        if !self.bloom_filter.may_contain(key.as_bytes()) {
            return Ok(None);
        }
        let mut file = File::open(&self.path)?;
        let file_size = file.metadata()?.len();
        if file_size < 56 {
            return Ok(None);
        }
        file.seek(SeekFrom::End(-56))?;
        let mut footer = vec![0u8; 56];
        file.read_exact(&mut footer)?;
        let index_offset = u64::from_le_bytes(footer[8..16].try_into().unwrap());
        let index_length = u64::from_le_bytes(footer[16..24].try_into().unwrap());
        let num_blocks = u32::from_le_bytes(footer[40..44].try_into().unwrap()) as usize;

        file.seek(SeekFrom::Start(index_offset))?;
        let mut index_data = vec![0u8; index_length as usize];
        file.read_exact(&mut index_data)?;

        let mut off = 4;
        for _ in 0..num_blocks {
            if off + 32 > index_data.len() {
                break;
            }
            let block_off = u64::from_le_bytes(index_data[off..off + 8].try_into().unwrap());
            off += 8;
            let block_len = u64::from_le_bytes(index_data[off..off + 8].try_into().unwrap());
            off += 8;
            let _uncomp_len = u64::from_le_bytes(index_data[off..off + 8].try_into().unwrap());
            off += 8;
            let fk_len = u32::from_le_bytes(index_data[off..off + 4].try_into().unwrap()) as usize;
            off += 4;
            let fk = index_data[off..off + fk_len].to_vec();
            off += fk_len;
            let lk_len = u32::from_le_bytes(index_data[off..off + 4].try_into().unwrap()) as usize;
            off += 4;
            let lk = index_data[off..off + lk_len].to_vec();
            off += lk_len;

            if key.as_bytes() >= fk.as_slice() && key.as_bytes() <= lk.as_slice() {
                file.seek(SeekFrom::Start(block_off))?;
                let mut block_data = vec![0u8; block_len as usize];
                file.read_exact(&mut block_data)?;
                let entries = Self::decompress_block(&block_data)?;
                for (ek, ev) in entries {
                    if ek == key.as_bytes() {
                        return Ok(Some(Value::new(ev)));
                    }
                }
            }
        }
        Ok(None)
    }

    pub fn scan(&self, range: &std::ops::Range<Key>) -> Result<Vec<(Key, Value)>> {
        let mut results = Vec::new();
        let mut file = File::open(&self.path)?;
        let file_size = file.metadata()?.len();
        if file_size < 56 {
            return Ok(results);
        }
        file.seek(SeekFrom::End(-56))?;
        let mut footer = vec![0u8; 56];
        file.read_exact(&mut footer)?;
        let index_offset = u64::from_le_bytes(footer[8..16].try_into().unwrap());
        let index_length = u64::from_le_bytes(footer[16..24].try_into().unwrap());
        let num_blocks = u32::from_le_bytes(footer[40..44].try_into().unwrap()) as usize;

        file.seek(SeekFrom::Start(index_offset))?;
        let mut index_data = vec![0u8; index_length as usize];
        file.read_exact(&mut index_data)?;

        let mut off = 4;
        for _ in 0..num_blocks {
            if off + 32 > index_data.len() {
                break;
            }
            let block_off = u64::from_le_bytes(index_data[off..off + 8].try_into().unwrap());
            off += 8;
            let block_len = u64::from_le_bytes(index_data[off..off + 8].try_into().unwrap());
            off += 8;
            let _uncomp_len = u64::from_le_bytes(index_data[off..off + 8].try_into().unwrap());
            off += 8;
            let fk_len = u32::from_le_bytes(index_data[off..off + 4].try_into().unwrap()) as usize;
            off += 4;
            let fk = index_data[off..off + fk_len].to_vec();
            off += fk_len;
            let lk_len = u32::from_le_bytes(index_data[off..off + 4].try_into().unwrap()) as usize;
            off += 4;
            let lk = index_data[off..off + lk_len].to_vec();
            off += lk_len;

            if lk.as_slice() < range.start.as_bytes() {
                continue;
            }
            if fk.as_slice() >= range.end.as_bytes() {
                break;
            }

            file.seek(SeekFrom::Start(block_off))?;
            let mut block_data = vec![0u8; block_len as usize];
            file.read_exact(&mut block_data)?;
            let entries = Self::decompress_block(&block_data)?;
            for (ek, ev) in entries {
                if ek.as_slice() >= range.start.as_bytes() && ek.as_slice() < range.end.as_bytes() {
                    results.push((Key::new(ek), Value::new(ev)));
                }
            }
        }
        Ok(results)
    }
}

pub fn compression_for_level(level: u8) -> CompressionCodec {
    match level {
        0 | 1 | 2 => CompressionCodec::Snappy,
        3 => CompressionCodec::Zstd { level: 3 },
        4 => CompressionCodec::Zstd { level: 5 },
        5 => CompressionCodec::Zstd { level: 10 },
        _ => CompressionCodec::Zstd { level: 16 },
    }
}

pub fn compression_for_type(page_type: u16) -> CompressionCodec {
    match page_type {
        0 | 1 => CompressionCodec::None,
        2 => CompressionCodec::Snappy,
        _ => CompressionCodec::Snappy,
    }
}

pub fn compress_data(data: &[u8], codec: CompressionCodec) -> Result<Vec<u8>> {
    match codec {
        CompressionCodec::None => Ok(data.to_vec()),
        CompressionCodec::Snappy => {
            let mut encoder = snap::raw::Encoder::new();
            encoder.compress_vec(data).map_err(|e| RuntimeError::Io(e.to_string()))
        }
        CompressionCodec::Zstd { level } => {
            zstd::encode_all(std::io::Cursor::new(data), level)
                .map_err(|e| RuntimeError::Io(e.to_string()))
        }
    }
}

pub fn decompress_data(data: &[u8], codec: CompressionCodec) -> Result<Vec<u8>> {
    match codec {
        CompressionCodec::None => Ok(data.to_vec()),
        CompressionCodec::Snappy => {
            let mut decoder = snap::raw::Decoder::new();
            decoder.decompress_vec(data).map_err(|e| RuntimeError::Io(e.to_string()))
        }
        CompressionCodec::Zstd { level: _ } => {
            zstd::decode_all(std::io::Cursor::new(data))
                .map_err(|e| RuntimeError::Io(e.to_string()))
        }
    }
}
