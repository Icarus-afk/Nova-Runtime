use std::sync::Arc;
use std::time::Duration;

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

        Ok(count)
    }

    pub async fn start_background(&'static self) {
        self.running.store(true, std::sync::atomic::Ordering::Relaxed);
        let interval = Duration::from_secs(self.config.gc_interval_secs);

        while self.is_running() {
            tokio::time::sleep(interval).await;
            if let Err(e) = self.run_once().await {
                error!("GC background run failed: {}", e);
            }
        }
    }

    pub fn stop(&self) {
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
