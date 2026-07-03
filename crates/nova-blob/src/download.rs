use std::sync::Arc;

use tracing::debug;

use crate::backend::BlobStore;
use crate::chunk::ChunkManager;
use crate::error::{BlobError, Result};

pub struct DownloadHandler {
    store: Arc<dyn BlobStore>,
}

impl DownloadHandler {
    pub fn new(store: Arc<dyn BlobStore>, _chunk_size: usize) -> Self {
        Self {
            store,
        }
    }

    pub async fn download(&self, blob_id: &str) -> Result<Vec<u8>> {
        let meta = self.store.get_metadata(blob_id).await?;
        let mut all_data = Vec::with_capacity(meta.size as usize);

        for chunk_hash in &meta.chunk_hashes {
            let chunk_data = self.store.get_chunk(chunk_hash).await?;
            let actual_hash = ChunkManager::hash(&chunk_data);
            if actual_hash != *chunk_hash {
                return Err(BlobError::ChecksumMismatch {
                    expected: chunk_hash.clone(),
                    actual: actual_hash,
                });
            }
            all_data.extend_from_slice(&chunk_data);
        }

        let full_hash = ChunkManager::hash(&all_data);
        if full_hash != meta.sha256 {
            return Err(BlobError::ChecksumMismatch {
                expected: meta.sha256,
                actual: full_hash,
            });
        }

        debug!("downloaded blob {} ({} bytes)", blob_id, meta.size);
        Ok(all_data)
    }

    pub async fn download_range(
        &self,
        blob_id: &str,
        offset: u64,
        length: u64,
    ) -> Result<Vec<u8>> {
        let meta = self.store.get_metadata(blob_id).await?;

        if offset >= meta.size {
            return Err(BlobError::InvalidRange(format!(
                "offset {} exceeds blob size {}",
                offset, meta.size
            )));
        }

        let end = std::cmp::min(offset + length, meta.size);
        let actual_length = end - offset;

        let chunk_size = meta.chunk_size as u64;
        let start_chunk = (offset / chunk_size) as usize;
        let end_chunk = ((end - 1) / chunk_size) as usize;

        let mut result = Vec::with_capacity(actual_length as usize);

        for chunk_idx in start_chunk..=end_chunk {
            let chunk_hash = &meta.chunk_hashes[chunk_idx];
            let chunk_data = self.store.get_chunk(chunk_hash).await?;

            let chunk_start = if chunk_idx == start_chunk {
                (offset % chunk_size) as usize
            } else {
                0
            };

            let chunk_end = if chunk_idx == end_chunk {
                let end_in_chunk = ((end - 1) % chunk_size) as usize + 1;
                std::cmp::min(end_in_chunk, chunk_data.len())
            } else {
                chunk_data.len()
            };

            if chunk_start < chunk_end {
                result.extend_from_slice(&chunk_data[chunk_start..chunk_end]);
            }
        }

        debug!(
            "downloaded range blob {} (offset={}, length={}) -> {} bytes",
            blob_id,
            offset,
            length,
            result.len()
        );
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::filesystem::FilesystemBackend;
    use crate::config::BlobConfig;
    use crate::metadata::{BlobMetadata, UploadState};
    use std::collections::HashMap;
    use std::sync::Arc;

    struct TestStore {
        store: FilesystemBackend,
        blob_id: String,
        size: u64,
        _dir: tempfile::TempDir,
    }

    async fn setup_test_store() -> TestStore {
        let dir = tempfile::tempdir().unwrap();
        let mut config = BlobConfig::default();
        config.data_dir = dir.path().to_str().unwrap().to_string();
        let store = FilesystemBackend::new(&config);
        store.init().await.unwrap();

        let data = b"Hello, World! This is test blob data for download testing.";
        let cm = ChunkManager::new(1024 * 1024);
        let (chunks, chunk_hashes) = cm.split(data);
        let sha256 = ChunkManager::hash(data);

        let blob_id = "test-blob-download";
        let meta = BlobMetadata {
            id: blob_id.to_string(),
            namespace: "test-ns".to_string(),
            size: data.len() as u64,
            content_type: "text/plain".to_string(),
            sha256,
            merkle_root: "unused".to_string(),
            chunk_size: 1024 * 1024,
            chunk_count: chunks.len() as u32,
            chunk_hashes,
            upload_id: None,
            upload_state: UploadState::Completed,
            metadata: HashMap::new(),
            created_at: 0,
            expires_at: None,
        };

        store.put_metadata(&meta).await.unwrap();
        for (chunk, hash) in chunks.iter().zip(&meta.chunk_hashes) {
            store.put_chunk(hash, chunk).await.unwrap();
        }

        TestStore {
            store,
            blob_id: blob_id.to_string(),
            size: data.len() as u64,
            _dir: dir,
        }
    }

    #[tokio::test]
    async fn test_download_full() {
        let ts = setup_test_store().await;
        let handler = DownloadHandler::new(Arc::new(ts.store), 1024 * 1024);
        let result = handler.download(&ts.blob_id).await.unwrap();
        assert_eq!(result.len() as u64, ts.size);
        assert_eq!(&result, b"Hello, World! This is test blob data for download testing.");
    }

    #[tokio::test]
    async fn test_download_range_middle() {
        let ts = setup_test_store().await;
        let handler = DownloadHandler::new(Arc::new(ts.store), 1024 * 1024);
        let result = handler.download_range(&ts.blob_id, 7, 5).await.unwrap();
        assert_eq!(result, b"World");
    }

    #[tokio::test]
    async fn test_download_range_exact_end() {
        let ts = setup_test_store().await;
        let handler = DownloadHandler::new(Arc::new(ts.store), 1024 * 1024);
        let result = handler.download_range(&ts.blob_id, 0, ts.size).await.unwrap();
        assert_eq!(result.len() as u64, ts.size);
    }

    #[tokio::test]
    async fn test_download_range_invalid_offset() {
        let ts = setup_test_store().await;
        let handler = DownloadHandler::new(Arc::new(ts.store), 1024 * 1024);
        let result = handler.download_range(&ts.blob_id, ts.size + 1, 5).await;
        assert!(result.is_err());
    }
}
