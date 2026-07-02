use std::path::{Path, PathBuf};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use nova_core::types::*;
use nova_core::error::*;

const WAL_MAGIC: u32 = 0x4E4F5641;
const MAX_PAYLOAD_SIZE: u32 = 65535;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalRecordType {
    Begin = 0,
    Commit = 1,
    Rollback = 2,
    Insert = 3,
    Update = 4,
    Delete = 5,
    Checkpoint = 6,
}

impl WalRecordType {
    pub fn to_u16(self) -> u16 {
        self as u16
    }

    pub fn from_u16(v: u16) -> Option<Self> {
        match v {
            0 => Some(WalRecordType::Begin),
            1 => Some(WalRecordType::Commit),
            2 => Some(WalRecordType::Rollback),
            3 => Some(WalRecordType::Insert),
            4 => Some(WalRecordType::Update),
            5 => Some(WalRecordType::Delete),
            6 => Some(WalRecordType::Checkpoint),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct WalRecord {
    pub lsn: Lsn,
    pub tx_id: TransactionId,
    pub record_type: WalRecordType,
    pub key: Key,
    pub value: Option<Value>,
    pub checksum: Checksum,
    pub timestamp: i64,
}

pub struct WalWriter {
    dir: PathBuf,
    seg_file: File,
    seg_num: u64,
    offset: u64,
    current_lsn: u64,
    policy: FsyncPolicy,
}

impl WalWriter {
    pub fn open(dir: &Path, policy: FsyncPolicy) -> Result<Self> {
        fs::create_dir_all(dir)?;
        let (seg_num, max_lsn) = Self::find_highest_segment(dir)?;
        let path = dir.join(format!("{:018}.wal", seg_num));
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(&path)?;
        let offset = file.metadata()?.len();
        Ok(WalWriter {
            dir: dir.to_path_buf(),
            seg_file: file,
            seg_num,
            offset,
            current_lsn: max_lsn,
            policy,
        })
    }

    fn find_highest_segment(dir: &Path) -> Result<(u64, u64)> {
        let mut max_seg = 0u64;
        let mut max_lsn = 0u64;
        if dir.exists() {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name.ends_with(".wal") {
                    if let Some(stem) = name.strip_suffix(".wal") {
                        if let Ok(num) = stem.parse::<u64>() {
                            if num > max_seg {
                                max_seg = num;
                                max_lsn = Self::recover_lsn_from_segment(&entry.path())?;
                            }
                        }
                    }
                }
            }
        }
        if max_seg == 0 {
            Ok((1, 0))
        } else {
            Ok((max_seg, max_lsn))
        }
    }

    fn recover_lsn_from_segment(path: &Path) -> Result<u64> {
        let mut f = File::open(path)?;
        let mut max_lsn = 0u64;
        loop {
            let mut header = [0u8; 24];
            match f.read_exact(&mut header) {
                Ok(()) => {}
                Err(_) => break,
            }
            let length = u32::from_le_bytes(header[8..12].try_into().unwrap()) as usize;
            let lsn = u64::from_le_bytes(header[12..20].try_into().unwrap());
            let _rtype = u16::from_le_bytes(header[20..22].try_into().unwrap());
            if lsn > max_lsn {
                max_lsn = lsn;
            }
            if length > 0 {
                let mut payload = vec![0u8; length];
                if f.read_exact(&mut payload).is_err() {
                    break;
                }
            }
        }
        Ok(max_lsn)
    }

    pub fn append(&mut self, record: &WalRecord) -> Result<Lsn> {
        self.current_lsn += 1;
        let lsn = Lsn::new(self.current_lsn);
        let payload = encode_payload(record);
        let payload_len = payload.len() as u32;
        if payload_len > MAX_PAYLOAD_SIZE {
            return Err(RuntimeError::InvalidArgument("WAL record payload too large".into()));
        }
        let checksum = crc32c::crc32c(&payload);
        let mut header = [0u8; 24];
        header[0..4].copy_from_slice(&WAL_MAGIC.to_le_bytes());
        header[4..8].copy_from_slice(&checksum.to_le_bytes());
        header[8..12].copy_from_slice(&payload_len.to_le_bytes());
        header[12..20].copy_from_slice(&lsn.value().to_le_bytes());
        header[20..22].copy_from_slice(&record.record_type.to_u16().to_le_bytes());
        header[22..24].copy_from_slice(&0u16.to_le_bytes());
        self.seg_file.write_all(&header)?;
        self.seg_file.write_all(&payload)?;
        self.offset += 24 + payload_len as u64;
        if matches!(self.policy, FsyncPolicy::EveryWrite) {
            self.seg_file.flush()?;
        }
        Ok(lsn)
    }

    pub fn flush(&mut self) -> Result<()> {
        self.seg_file.flush()?;
        if matches!(self.policy, FsyncPolicy::EveryWrite | FsyncPolicy::EveryNMs(_)) {
            self.seg_file.sync_all()?;
        }
        Ok(())
    }

    pub fn switch_segment(&mut self) -> Result<()> {
        self.seg_file.flush()?;
        self.seg_file.sync_all()?;
        self.seg_num += 1;
        self.offset = 0;
        let path = self.dir.join(format!("{:018}.wal", self.seg_num));
        self.seg_file = OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(&path)?;
        Ok(())
    }

    pub fn current_lsn(&self) -> Lsn {
        Lsn::new(self.current_lsn)
    }

    pub fn close(&mut self) -> Result<()> {
        self.flush()?;
        Ok(())
    }
}

fn encode_payload(record: &WalRecord) -> Vec<u8> {
    let key_bytes = record.key.as_bytes();
    let value_bytes = match &record.value {
        Some(v) => v.as_bytes(),
        None => &[],
    };
    let mut buf = Vec::with_capacity(8 + 4 + key_bytes.len() + 4 + value_bytes.len() + 8);
    buf.extend_from_slice(&record.tx_id.value().to_le_bytes());
    buf.extend_from_slice(&(key_bytes.len() as u32).to_le_bytes());
    buf.extend_from_slice(key_bytes);
    buf.extend_from_slice(&(value_bytes.len() as u32).to_le_bytes());
    buf.extend_from_slice(value_bytes);
    buf.extend_from_slice(&record.timestamp.to_le_bytes());
    buf
}

fn decode_payload(data: &[u8]) -> Result<(TransactionId, Key, Option<Value>, i64)> {
    if data.len() < 24 {
        return Err(RuntimeError::CorruptData("WAL payload too short".into()));
    }
    let mut off = 0;
    let tx_id = TransactionId::new(u64::from_le_bytes(data[off..off + 8].try_into().unwrap()));
    off += 8;
    let key_len = u32::from_le_bytes(data[off..off + 4].try_into().unwrap()) as usize;
    off += 4;
    if off + key_len > data.len() {
        return Err(RuntimeError::CorruptData("WAL payload key truncated".into()));
    }
    let key = Key::new(data[off..off + key_len].to_vec());
    off += key_len;
    if off + 4 > data.len() {
        return Err(RuntimeError::CorruptData("WAL payload value length truncated".into()));
    }
    let value_len = u32::from_le_bytes(data[off..off + 4].try_into().unwrap()) as usize;
    off += 4;
    let value = if value_len > 0 {
        if off + value_len > data.len() {
            return Err(RuntimeError::CorruptData("WAL payload value truncated".into()));
        }
        Some(Value::new(data[off..off + value_len].to_vec()))
    } else {
        None
    };
    off += value_len;
    let timestamp = if off + 8 <= data.len() {
        i64::from_le_bytes(data[off..off + 8].try_into().unwrap())
    } else {
        0
    };
    Ok((tx_id, key, value, timestamp))
}

pub struct WalReader {
    dir: PathBuf,
    current_seg: u64,
    max_seg: u64,
    file: Option<File>,
}

impl WalReader {
    pub fn open(dir: &Path) -> Result<Self> {
        fs::create_dir_all(dir)?;
        let mut max_seg = 0u64;
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.ends_with(".wal") {
                if let Some(stem) = name.strip_suffix(".wal") {
                    if let Ok(num) = stem.parse::<u64>() {
                        if num > max_seg {
                            max_seg = num;
                        }
                    }
                }
            }
        }
        if max_seg == 0 {
            max_seg = 1;
        }
        let mut reader = WalReader {
            dir: dir.to_path_buf(),
            current_seg: 1,
            max_seg,
            file: None,
        };
        reader.open_segment(1)?;
        Ok(reader)
    }

    fn open_segment(&mut self, num: u64) -> Result<()> {
        let path = self.dir.join(format!("{:018}.wal", num));
        let file = OpenOptions::new().read(true).open(&path)?;
        self.file = Some(file);
        Ok(())
    }

    pub fn read_next(&mut self) -> Result<Option<WalRecord>> {
        let file = match self.file.as_mut() {
            Some(f) => f,
            None => return Ok(None),
        };
        let mut header = [0u8; 24];
        match file.read_exact(&mut header) {
            Ok(()) => {}
            Err(ref e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                return self.try_next_segment();
            }
            Err(e) => return Err(RuntimeError::Io(e.to_string())),
        }
        let magic = u32::from_le_bytes(header[0..4].try_into().unwrap());
        if magic != WAL_MAGIC {
            return Err(RuntimeError::CorruptData("WAL invalid magic".into()));
        }
        let checksum_val = u32::from_le_bytes(header[4..8].try_into().unwrap());
        let length = u32::from_le_bytes(header[8..12].try_into().unwrap()) as usize;
        let lsn = Lsn::new(u64::from_le_bytes(header[12..20].try_into().unwrap()));
        let rtype = u16::from_le_bytes(header[20..22].try_into().unwrap());
        let record_type = WalRecordType::from_u16(rtype)
            .ok_or_else(|| RuntimeError::CorruptData("WAL invalid record type".into()))?;
        let mut payload = vec![0u8; length];
        if length > 0 {
            file.read_exact(&mut payload)?;
        }
        let actual_checksum = crc32c::crc32c(&payload);
        if actual_checksum != checksum_val {
            return Err(RuntimeError::ChecksumMismatch {
                expected: checksum_val,
                actual: actual_checksum,
            });
        }
        let (tx_id, key, value, timestamp) = if length > 0 {
            decode_payload(&payload)?
        } else {
            (TransactionId::ZERO, Key::new(vec![]), None, 0)
        };
        Ok(Some(WalRecord {
            lsn,
            tx_id,
            record_type,
            key,
            value,
            checksum: Checksum::new(checksum_val),
            timestamp,
        }))
    }

    fn try_next_segment(&mut self) -> Result<Option<WalRecord>> {
        if self.current_seg >= self.max_seg {
            self.file = None;
            return Ok(None);
        }
        self.current_seg += 1;
        self.open_segment(self.current_seg)?;
        self.read_next()
    }

    pub fn seek(&mut self, _lsn: Lsn) -> Result<()> {
        self.current_seg = 1;
        self.open_segment(1)?;
        Ok(())
    }

    pub fn read_from(&mut self, lsn: Lsn) -> Result<Vec<WalRecord>> {
        self.seek(lsn)?;
        let mut records = Vec::new();
        loop {
            match self.read_next()? {
                Some(record) => records.push(record),
                None => break,
            }
        }
        Ok(records)
    }

    pub fn close(&self) -> Result<()> {
        Ok(())
    }
}

fn now_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as i64
}

pub fn make_record(
    record_type: WalRecordType,
    tx_id: TransactionId,
    key: Key,
    value: Option<Value>,
) -> WalRecord {
    WalRecord {
        lsn: Lsn::ZERO,
        tx_id,
        record_type,
        key,
        value,
        checksum: Checksum::new(0),
        timestamp: now_timestamp(),
    }
}

pub struct GroupCommit {
    batch_interval: Duration,
    pending: parking_lot::RwLock<Vec<WalRecord>>,
}

impl GroupCommit {
    pub fn new(interval_ms: u64) -> Self {
        GroupCommit {
            batch_interval: Duration::from_millis(interval_ms),
            pending: parking_lot::RwLock::new(Vec::new()),
        }
    }

    pub fn submit(&self, record: WalRecord) {
        self.pending.write().push(record);
    }

    pub fn run_once(&self, wal: &mut WalWriter) -> Result<()> {
        let mut pending = self.pending.write();
        if pending.is_empty() {
            return Ok(());
        }
        let mut batch: Vec<WalRecord> = std::mem::take(&mut *pending);
        drop(pending);

        batch.sort_by_key(|r| r.lsn.value());
        batch.dedup_by_key(|r| r.key.clone());

        for record in &batch {
            wal.append(record)?;
        }
        wal.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("nova_wal_test_{}_{}", name, std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    fn cleanup(dir: &std::path::Path) {
        let _ = std::fs::remove_dir_all(dir);
    }

    // ── WalRecordType tests ──

    #[test]
    fn test_wal_record_type_to_u16() {
        assert_eq!(WalRecordType::Begin.to_u16(), 0);
        assert_eq!(WalRecordType::Commit.to_u16(), 1);
        assert_eq!(WalRecordType::Rollback.to_u16(), 2);
        assert_eq!(WalRecordType::Insert.to_u16(), 3);
        assert_eq!(WalRecordType::Update.to_u16(), 4);
        assert_eq!(WalRecordType::Delete.to_u16(), 5);
        assert_eq!(WalRecordType::Checkpoint.to_u16(), 6);
    }

    #[test]
    fn test_wal_record_type_from_u16() {
        assert_eq!(WalRecordType::from_u16(0), Some(WalRecordType::Begin));
        assert_eq!(WalRecordType::from_u16(1), Some(WalRecordType::Commit));
        assert_eq!(WalRecordType::from_u16(2), Some(WalRecordType::Rollback));
        assert_eq!(WalRecordType::from_u16(3), Some(WalRecordType::Insert));
        assert_eq!(WalRecordType::from_u16(4), Some(WalRecordType::Update));
        assert_eq!(WalRecordType::from_u16(5), Some(WalRecordType::Delete));
        assert_eq!(WalRecordType::from_u16(6), Some(WalRecordType::Checkpoint));
        assert_eq!(WalRecordType::from_u16(99), None);
    }

    // ── make_record tests ──

    #[test]
    fn test_make_record_insert() {
        let record = make_record(
            WalRecordType::Insert,
            TransactionId::new(42),
            Key::from("test_key"),
            Some(Value::new(b"test_val".to_vec())),
        );
        assert_eq!(record.record_type, WalRecordType::Insert);
        assert_eq!(record.tx_id, TransactionId::new(42));
        assert_eq!(record.key, Key::from("test_key"));
        assert_eq!(record.value, Some(Value::new(b"test_val".to_vec())));
        assert!(record.timestamp > 0);
    }

    #[test]
    fn test_make_record_delete() {
        let record = make_record(
            WalRecordType::Delete,
            TransactionId::ZERO,
            Key::from("del_key"),
            None,
        );
        assert_eq!(record.record_type, WalRecordType::Delete);
        assert_eq!(record.tx_id, TransactionId::ZERO);
        assert!(record.value.is_none());
    }

    // ── WalWriter / WalReader integration tests ──

    #[test]
    fn test_wal_write_and_read_single_record() {
        let dir = temp_dir("single_record");
        let mut writer = WalWriter::open(&dir, FsyncPolicy::EveryWrite).unwrap();
        let record = make_record(
            WalRecordType::Insert,
            TransactionId::new(1),
            Key::from("hello"),
            Some(Value::new(b"world".to_vec())),
        );
        let lsn = writer.append(&record).unwrap();
        assert!(lsn.value() > 0);
        writer.close().unwrap();

        let mut reader = WalReader::open(&dir).unwrap();
        let read = reader.read_next().unwrap().unwrap();
        assert_eq!(read.record_type, WalRecordType::Insert);
        assert_eq!(read.tx_id, TransactionId::new(1));
        assert_eq!(read.key, Key::from("hello"));
        assert_eq!(read.value, Some(Value::new(b"world".to_vec())));

        assert!(reader.read_next().unwrap().is_none());
        cleanup(&dir);
    }

    #[test]
    fn test_wal_write_and_read_multiple_records() {
        let dir = temp_dir("multi_record");
        let mut writer = WalWriter::open(&dir, FsyncPolicy::EveryWrite).unwrap();
        for i in 0u8..5 {
            let record = make_record(
                WalRecordType::Insert,
                TransactionId::new(i as u64),
                Key::new(vec![i]),
                Some(Value::new(vec![i + 100])),
            );
            writer.append(&record).unwrap();
        }
        writer.close().unwrap();

        let mut reader = WalReader::open(&dir).unwrap();
        for i in 0u8..5 {
            let read = reader.read_next().unwrap().unwrap();
            assert_eq!(read.record_type, WalRecordType::Insert);
            assert_eq!(read.tx_id, TransactionId::new(i as u64));
            assert_eq!(read.key, Key::new(vec![i]));
        }
        assert!(reader.read_next().unwrap().is_none());
        cleanup(&dir);
    }

    #[test]
    fn test_wal_write_read_all_types() {
        let dir = temp_dir("all_types");
        let mut writer = WalWriter::open(&dir, FsyncPolicy::EveryWrite).unwrap();
        let types = [
            WalRecordType::Begin,
            WalRecordType::Commit,
            WalRecordType::Rollback,
            WalRecordType::Insert,
            WalRecordType::Update,
            WalRecordType::Delete,
            WalRecordType::Checkpoint,
        ];
        for rt in &types {
            let record = make_record(*rt, TransactionId::new(1), Key::from("k"), None);
            writer.append(&record).unwrap();
        }
        writer.close().unwrap();

        let mut reader = WalReader::open(&dir).unwrap();
        for rt in &types {
            let read = reader.read_next().unwrap().unwrap();
            assert_eq!(read.record_type, *rt);
        }
        cleanup(&dir);
    }

    #[test]
    fn test_wal_append_increments_lsn() {
        let dir = temp_dir("lsn_inc");
        let mut writer = WalWriter::open(&dir, FsyncPolicy::EveryWrite).unwrap();
        let lsn1 = writer.append(&make_record(WalRecordType::Insert, TransactionId::new(1), Key::from("a"), None)).unwrap();
        let lsn2 = writer.append(&make_record(WalRecordType::Insert, TransactionId::new(2), Key::from("b"), None)).unwrap();
        assert!(lsn2.value() > lsn1.value());
        writer.close().unwrap();
        cleanup(&dir);
    }

    #[test]
    fn test_wal_switch_segment() {
        let dir = temp_dir("switch_seg");
        let mut writer = WalWriter::open(&dir, FsyncPolicy::EveryWrite).unwrap();
        writer.append(&make_record(WalRecordType::Insert, TransactionId::new(1), Key::from("pre"), None)).unwrap();
        writer.switch_segment().unwrap();
        writer.append(&make_record(WalRecordType::Insert, TransactionId::new(2), Key::from("post"), None)).unwrap();
        writer.close().unwrap();

        let mut reader = WalReader::open(&dir).unwrap();
        let r1 = reader.read_next().unwrap().unwrap();
        assert_eq!(r1.key, Key::from("pre"));
        let r2 = reader.read_next().unwrap().unwrap();
        assert_eq!(r2.key, Key::from("post"));
        cleanup(&dir);
    }

    #[test]
    fn test_wal_flush() {
        let dir = temp_dir("flush");
        let mut writer = WalWriter::open(&dir, FsyncPolicy::Async).unwrap();
        writer.append(&make_record(WalRecordType::Insert, TransactionId::new(1), Key::from("x"), None)).unwrap();
        writer.flush().unwrap();
        writer.close().unwrap();

        let mut reader = WalReader::open(&dir).unwrap();
        assert!(reader.read_next().unwrap().is_some());
        cleanup(&dir);
    }

    #[test]
    fn test_wal_current_lsn() {
        let dir = temp_dir("current_lsn");
        let mut writer = WalWriter::open(&dir, FsyncPolicy::Async).unwrap();
        writer.append(&make_record(WalRecordType::Insert, TransactionId::new(1), Key::from("a"), None)).unwrap();
        assert!(writer.current_lsn().value() > 0);
        writer.close().unwrap();
        cleanup(&dir);
    }

    // ── GroupCommit tests ──

    #[test]
    fn test_group_commit_submit_and_run() {
        let dir = temp_dir("group_commit");
        let mut writer = WalWriter::open(&dir, FsyncPolicy::EveryWrite).unwrap();
        let gc = GroupCommit::new(100);

        let record = make_record(WalRecordType::Insert, TransactionId::new(1), Key::from("gc_key"), Some(Value::new(b"gc_val".to_vec())));
        gc.submit(record);
        gc.run_once(&mut writer).unwrap();
        writer.close().unwrap();

        let mut reader = WalReader::open(&dir).unwrap();
        let read = reader.read_next().unwrap().unwrap();
        assert_eq!(read.key, Key::from("gc_key"));
        assert_eq!(read.value, Some(Value::new(b"gc_val".to_vec())));
        cleanup(&dir);
    }

    #[test]
    fn test_group_commit_empty_run_is_noop() {
        let dir = temp_dir("gc_empty");
        let mut writer = WalWriter::open(&dir, FsyncPolicy::EveryWrite).unwrap();
        let gc = GroupCommit::new(100);
        gc.run_once(&mut writer).unwrap();
        writer.close().unwrap();

        let mut reader = WalReader::open(&dir).unwrap();
        assert!(reader.read_next().unwrap().is_none());
        cleanup(&dir);
    }

    #[test]
    fn test_group_commit_batch_sort_and_dedup() {
        let dir = temp_dir("gc_dedup");
        let mut writer = WalWriter::open(&dir, FsyncPolicy::EveryWrite).unwrap();
        let gc = GroupCommit::new(100);

        gc.submit(make_record(WalRecordType::Insert, TransactionId::new(2), Key::from("b"), None));
        gc.submit(make_record(WalRecordType::Insert, TransactionId::new(1), Key::from("a"), None));
        gc.submit(make_record(WalRecordType::Insert, TransactionId::new(3), Key::from("b"), None));
        gc.run_once(&mut writer).unwrap();
        writer.close().unwrap();

        let mut reader = WalReader::open(&dir).unwrap();
        let r1 = reader.read_next().unwrap().unwrap();
        // After sort-by-lsn and dedup-by-key: "b" (txn 2/3) should be deduped; "a" comes first since all LSNs are 0
        assert_eq!(r1.key, Key::from("b"));
        let r2 = reader.read_next().unwrap().unwrap();
        assert_eq!(r2.key, Key::from("a"));
        assert!(reader.read_next().unwrap().is_none());
        cleanup(&dir);
    }

    // ── Error handling ──

    #[test]
    fn test_wal_payload_too_large() {
        let dir = temp_dir("large_payload");
        let mut writer = WalWriter::open(&dir, FsyncPolicy::EveryWrite).unwrap();
        let large_value = Value::new(vec![0u8; MAX_PAYLOAD_SIZE as usize + 1]);
        let record = make_record(WalRecordType::Insert, TransactionId::new(1), Key::from("k"), Some(large_value));
        let result = writer.append(&record);
        assert!(result.is_err());
        writer.close().unwrap();
        cleanup(&dir);
    }

    #[test]
    fn test_wal_reader_open_nonexistent_dir() {
        let dir = std::env::temp_dir().join("nonexistent_wal_dir_should_not_exist");
        let _ = std::fs::remove_dir_all(&dir);
        // WalReader::open creates the dir
        let reader = WalReader::open(&dir).unwrap();
        assert!(reader.file.is_some());
    }

    #[test]
    fn test_wal_reader_open_creates_dir() {
        let dir = std::env::temp_dir().join("wal_reader_create_dir_test");
        let _ = std::fs::remove_dir_all(&dir);
        let reader = WalReader::open(&dir).unwrap();
        assert!(dir.exists());
        assert!(reader.file.is_some());
        cleanup(&dir);
    }
}

pub fn wal_group_commit_loop(
    gc: &GroupCommit,
    wal: &mut WalWriter,
    shutdown: &std::sync::atomic::AtomicBool,
) -> Result<()> {
    let interval = gc.batch_interval;
    loop {
        if shutdown.load(std::sync::atomic::Ordering::Relaxed) {
            gc.run_once(wal)?;
            return Ok(());
        }
        std::thread::sleep(interval);
        gc.run_once(wal)?;
    }
}
