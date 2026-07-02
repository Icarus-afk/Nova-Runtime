use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use parking_lot::RwLock;
use nova_core::types::*;
use nova_core::error::*;

pub const MIN_BLOB_SIZE: u64 = 1024 * 1024;
pub const MAX_BLOB_SIZE: u64 = 5_497_558_138_880;
pub const BLOB_HEADER_PAGE_TYPE: u16 = 5;
pub const BLOB_DATA_PAGE_TYPE: u16 = 6;

#[derive(Debug, Clone)]
pub struct BlobRecord {
    pub blob_id: u128,
    pub size: u64,
    pub num_pages: u32,
    pub first_page: u32,
    pub checksum: u32,
    pub created_at: u64,
    pub ttl: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct BlobStats {
    pub total_blobs: usize,
    pub active_blobs: usize,
    pub total_size: u64,
    pub region_start: u64,
    pub region_end: u64,
}

pub struct BlobStore {
    pub min_blob_size: u64,
    pub max_blob_size: u64,
    pub region_start: u64,
    pub region_end: AtomicU64,
    blob_index: RwLock<HashMap<u128, BlobRecord>>,
}

impl BlobStore {
    pub fn new(region_start: u64) -> Self {
        BlobStore {
            min_blob_size: MIN_BLOB_SIZE,
            max_blob_size: MAX_BLOB_SIZE,
            region_start,
            region_end: AtomicU64::new(region_start + 1),
            blob_index: RwLock::new(HashMap::new()),
        }
    }

    pub fn put(&self, data: &[u8], ttl: Option<u64>) -> Result<u128> {
        let size = data.len() as u64;
        if size < self.min_blob_size {
            return Err(RuntimeError::InvalidArgument(format!(
                "blob too small: {} < min {}",
                size, self.min_blob_size
            )));
        }
        if size > self.max_blob_size {
            return Err(RuntimeError::InvalidArgument(format!(
                "blob too large: {} > max {}",
                size, self.max_blob_size
            )));
        }

        let num_pages = ((size + PAGE_SIZE as u64 - 1) / PAGE_SIZE as u64) as u32;
        let first_page = self.region_end.fetch_add(num_pages as u64 + 1, Ordering::SeqCst) as u32;

        let blob_id = uuid::Uuid::now_v7().as_u128();
        let checksum = crc32c::crc32c(data);
        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let record = BlobRecord {
            blob_id,
            size,
            num_pages,
            first_page,
            checksum,
            created_at,
            ttl,
        };

        let mut index = self.blob_index.write();
        index.insert(blob_id, record);
        Ok(blob_id)
    }

    pub fn get(&self, blob_id: u128) -> Result<Option<Vec<u8>>> {
        let index = self.blob_index.read();
        let record = match index.get(&blob_id) {
            Some(r) => r.clone(),
            None => return Ok(None),
        };

        if let Some(ttl) = record.ttl {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            if now > record.created_at + ttl {
                return Ok(None);
            }
        }

        let checksum = crc32c::crc32c(&[]);
        if checksum != record.checksum {
            return Err(RuntimeError::ChecksumMismatch {
                expected: record.checksum,
                actual: checksum,
            });
        }

        Ok(Some(Vec::new()))
    }

    pub fn delete(&self, blob_id: u128) -> Result<()> {
        let mut index = self.blob_index.write();
        index.remove(&blob_id);
        Ok(())
    }

    pub fn metadata(&self, blob_id: u128) -> Result<Option<BlobRecord>> {
        let index = self.blob_index.read();
        Ok(index.get(&blob_id).cloned())
    }

    pub fn stats(&self) -> BlobStats {
        let index = self.blob_index.read();
        let total_blobs = index.len();
        let total_size: u64 = index.values().map(|r| r.size).sum();
        BlobStats {
            total_blobs,
            active_blobs: total_blobs,
            total_size,
            region_start: self.region_start,
            region_end: self.region_end.load(Ordering::Relaxed),
        }
    }
}
