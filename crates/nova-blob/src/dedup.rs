use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::RwLock;
use tracing::debug;

use crate::metadata::ChunkRecord;

pub struct DeduplicationEngine {
    chunks: Arc<RwLock<HashMap<String, ChunkRecord>>>,
}

impl DeduplicationEngine {
    pub fn new() -> Self {
        Self {
            chunks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn record_chunk(&self, hash: &str, size: u32) -> bool {
        let mut chunks = self.chunks.write();
        if let Some(record) = chunks.get_mut(hash) {
            record.ref_count += 1;
            debug!("chunk {} ref_count incremented to {}", hash, record.ref_count);
            true
        } else {
            let record = ChunkRecord {
                hash: hash.to_string(),
                size,
                ref_count: 1,
                created_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64,
            };
            chunks.insert(hash.to_string(), record);
            debug!("chunk {} created with ref_count 1", hash);
            false
        }
    }

    pub fn release_chunk(&self, hash: &str) -> u64 {
        let mut chunks = self.chunks.write();
        if let Some(record) = chunks.get_mut(hash) {
            if record.ref_count > 0 {
                record.ref_count -= 1;
            }
            debug!("chunk {} ref_count decremented to {}", hash, record.ref_count);
            record.ref_count
        } else {
            0
        }
    }

    pub fn is_duplicate(&self, hash: &str) -> bool {
        self.chunks.read().contains_key(hash)
    }

    pub fn get_ref_count(&self, hash: &str) -> u64 {
        self.chunks.read().get(hash).map(|r| r.ref_count).unwrap_or(0)
    }

    pub fn collect_unreferenced(&self, grace_period_secs: u64) -> Vec<String> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let chunks = self.chunks.read();
        chunks
            .values()
            .filter(|r| {
                r.ref_count == 0 && (now - r.created_at) as u64 >= grace_period_secs
            })
            .map(|r| r.hash.clone())
            .collect()
    }

    pub fn remove_tracked(&self, hash: &str) {
        self.chunks.write().remove(hash);
    }

    pub fn tracked_chunks(&self) -> Vec<ChunkRecord> {
        self.chunks.read().values().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_new_chunk() {
        let dedup = DeduplicationEngine::new();
        assert!(!dedup.record_chunk("abc", 1024));
        assert_eq!(dedup.get_ref_count("abc"), 1);
    }

    #[test]
    fn test_record_duplicate_chunk() {
        let dedup = DeduplicationEngine::new();
        dedup.record_chunk("abc", 1024);
        assert!(dedup.record_chunk("abc", 1024));
        assert_eq!(dedup.get_ref_count("abc"), 2);
    }

    #[test]
    fn test_release_chunk() {
        let dedup = DeduplicationEngine::new();
        dedup.record_chunk("abc", 1024);
        dedup.record_chunk("abc", 1024);
        assert_eq!(dedup.release_chunk("abc"), 1);
        assert_eq!(dedup.release_chunk("abc"), 0);
    }

    #[test]
    fn test_is_duplicate() {
        let dedup = DeduplicationEngine::new();
        assert!(!dedup.is_duplicate("abc"));
        dedup.record_chunk("abc", 1024);
        assert!(dedup.is_duplicate("abc"));
    }
}
