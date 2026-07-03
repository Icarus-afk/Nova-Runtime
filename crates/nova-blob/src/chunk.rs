use sha2::{Digest, Sha256};

pub struct ChunkManager {
    chunk_size: usize,
}

impl ChunkManager {
    pub fn new(chunk_size: usize) -> Self {
        Self { chunk_size }
    }

    pub fn chunk_size(&self) -> usize {
        self.chunk_size
    }

    pub fn split(&self, data: &[u8]) -> (Vec<Vec<u8>>, Vec<String>) {
        let mut chunks = Vec::new();
        let mut hashes = Vec::new();
        let mut offset = 0;
        while offset < data.len() {
            let end = std::cmp::min(offset + self.chunk_size, data.len());
            let chunk = data[offset..end].to_vec();
            let hash = Self::hash(&chunk);
            chunks.push(chunk);
            hashes.push(hash);
            offset = end;
        }
        if chunks.is_empty() {
            let hash = Self::hash(b"");
            chunks.push(Vec::new());
            hashes.push(hash);
        }
        (chunks, hashes)
    }

    pub fn hash(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hex::encode(hasher.finalize())
    }

    pub fn chunk_count_for_size(&self, size: u64) -> u32 {
        if size == 0 {
            return 1;
        }
        ((size + self.chunk_size as u64 - 1) / self.chunk_size as u64) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_small_data() {
        let cm = ChunkManager::new(1024 * 1024);
        let data = b"hello world";
        let (chunks, hashes) = cm.split(data);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], data);
        assert_eq!(hashes.len(), 1);
    }

    #[test]
    fn test_split_empty_data() {
        let cm = ChunkManager::new(1024 * 1024);
        let (chunks, hashes) = cm.split(b"");
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].is_empty());
        assert_eq!(hashes.len(), 1);
    }

    #[test]
    fn test_split_multiple_chunks() {
        let cm = ChunkManager::new(10);
        let data = b"abcdefghijklmnopqrstuvwxyz";
        let (chunks, hashes) = cm.split(data);
        assert_eq!(chunks.len(), 3);
        assert_eq!(hashes.len(), 3);
        assert_eq!(chunks[0], b"abcdefghij");
        assert_eq!(chunks[1], b"klmnopqrst");
        assert_eq!(chunks[2], b"uvwxyz");
    }

    #[test]
    fn test_hash_consistency() {
        let h1 = ChunkManager::hash(b"test data");
        let h2 = ChunkManager::hash(b"test data");
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64);
    }

    #[test]
    fn test_chunk_count() {
        let cm = ChunkManager::new(1024 * 1024);
        assert_eq!(cm.chunk_count_for_size(0), 1);
        assert_eq!(cm.chunk_count_for_size(1), 1);
        assert_eq!(cm.chunk_count_for_size(1024 * 1024), 1);
        assert_eq!(cm.chunk_count_for_size(1024 * 1024 + 1), 2);
    }
}
