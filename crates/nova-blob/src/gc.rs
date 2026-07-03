use std::sync::Arc;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info};

use crate::backend::BlobStore;
use crate::config::BlobConfig;
use crate::dedup::DeduplicationEngine;
use crate::error::Result;

pub struct GarbageCollector {
    store: Arc<dyn BlobStore>,
    dedup: Arc<DeduplicationEngine>,
    config: BlobConfig,
    running: Arc<std::sync::atomic::AtomicBool>,
}

impl GarbageCollector {
    pub fn new(
        store: Arc<dyn BlobStore>,
        dedup: Arc<DeduplicationEngine>,
        config: &BlobConfig,
    ) -> Self {
        Self {
            store,
            dedup,
            config: config.clone(),
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub async fn run_once(&self) -> Result<usize> {
        let mut total = 0usize;

        let unreferenced = self
            .dedup
            .collect_unreferenced(self.config.gc_grace_period_secs);
        let count = unreferenced.len();
        info!("GC: found {} unreferenced chunks to collect", count);

        for hash in &unreferenced {
            debug!("GC: deleting chunk {}", hash);
            if let Err(e) = self.store.delete_chunk(hash).await {
                error!("GC: failed to delete chunk {}: {}", hash, e);
            } else {
                self.dedup.remove_tracked(hash);
            }
        }
        total += count;

        let expired = self.cleanup_expired_blobs().await?;
        info!("GC: cleaned up {} expired blobs", expired);
        total += expired as usize;

        Ok(total)
    }

    async fn cleanup_expired_blobs(&self) -> Result<u64> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let namespaces = self.store.list_namespaces().await?;
        let mut count = 0u64;

        for ns in &namespaces {
            let blobs = self.store.list_blobs(ns).await?;
            for blob_id in &blobs {
                if let Ok(meta) = self.store.get_metadata(blob_id).await {
                    if let Some(expires_at) = meta.expires_at {
                        if expires_at <= now {
                            for chunk_hash in &meta.chunk_hashes {
                                let ref_count = self.dedup.release_chunk(chunk_hash);
                                if ref_count == 0 {
                                    if let Err(e) = self.store.delete_chunk(chunk_hash).await {
                                        error!("GC TTL: failed to delete chunk {}: {}", chunk_hash, e);
                                    } else {
                                        self.dedup.remove_tracked(chunk_hash);
                                    }
                                }
                            }
                            if let Err(e) = self.store.delete_metadata(blob_id).await {
                                error!("GC TTL: failed to delete metadata {}: {}", blob_id, e);
                            } else {
                                count += 1;
                                info!("GC TTL: deleted expired blob {} (namespace {})", blob_id, ns);
                            }
                        }
                    }
                }
            }
        }

        Ok(count)
    }

    pub fn start_background(self: Arc<Self>, interval: Duration, cancel: CancellationToken) -> JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = tokio::time::sleep(interval) => {
                        if let Err(e) = self.run_once().await {
                            error!("GC background run failed: {}", e);
                        }
                    }
                    _ = cancel.cancelled() => {
                        info!("GC shutting down");
                        break;
                    }
                }
            }
        })
    }

    pub fn shutdown(&self) {
        self.running.store(false, std::sync::atomic::Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::filesystem::FilesystemBackend;
    use crate::config::BlobConfig;

    #[tokio::test]
    async fn test_gc_no_unreferenced() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = BlobConfig::default();
        config.data_dir = dir.path().to_str().unwrap().to_string();
        config.gc_grace_period_secs = 0;
        let store = FilesystemBackend::new(&config);
        store.init().await.unwrap();
        let dedup = Arc::new(DeduplicationEngine::new());
        let gc = GarbageCollector::new(Arc::new(store), dedup, &config);
        let count = gc.run_once().await.unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_gc_collects_unreferenced() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = BlobConfig::default();
        config.data_dir = dir.path().to_str().unwrap().to_string();
        config.gc_grace_period_secs = 0;
        let store = Arc::new(FilesystemBackend::new(&config));
        store.init().await.unwrap();

        store.put_chunk("testhash123", b"data").await.unwrap();

        let dedup = Arc::new(DeduplicationEngine::new());
        dedup.record_chunk("testhash123", 4);
        dedup.release_chunk("testhash123");

        let gc = GarbageCollector::new(store.clone(), dedup.clone(), &config);
        let count = gc.run_once().await.unwrap();
        assert!(count >= 1);

        let result = store.get_chunk("testhash123").await;
        assert!(result.is_err());
    }
}
