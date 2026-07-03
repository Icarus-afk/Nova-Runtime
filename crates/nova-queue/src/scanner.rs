use crate::backend::QueueBackend;
use crate::config::QueueConfig;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;

/// Runs periodic scanner tasks for a queue subsystem.
pub struct QueueScanner {
    backend: Arc<dyn QueueBackend>,
    config: QueueConfig,
    shutdown_rx: watch::Receiver<bool>,
}

impl QueueScanner {
    pub fn new(
        backend: Arc<dyn QueueBackend>,
        config: QueueConfig,
        shutdown_rx: watch::Receiver<bool>,
    ) -> Self {
        Self {
            backend,
            config,
            shutdown_rx,
        }
    }

    /// Start all scanner loops. Runs until shutdown signal is received.
    pub async fn run(&mut self) {
        let interval = Duration::from_millis(self.config.scanner_interval_ms);
        let mut interval_timer = tokio::time::interval(interval);

        loop {
            tokio::select! {
                _ = interval_timer.tick() => {
                    self.scan_all().await;
                }
                _ = self.shutdown_rx.changed() => {
                    if *self.shutdown_rx.borrow() {
                        tracing::info!("Queue scanner shutting down");
                        break;
                    }
                }
            }
        }
    }

    async fn scan_all(&self) {
        let queues = match self.backend.list_queues().await {
            Ok(q) => q,
            Err(e) => {
                tracing::error!("Failed to list queues for scanning: {}", e);
                return;
            }
        };

        let now_ms = chrono::Utc::now().timestamp_millis();

        for summary in &queues {
            // 1. Release expired inflight messages
            match self.backend.release_expired_messages(&summary.name, now_ms).await {
                Ok(count) => {
                    if count > 0 {
                        tracing::debug!("Released {} expired messages from queue '{}'", count, summary.name);
                    }
                }
                Err(e) => tracing::error!("Failed to release expired messages for '{}': {}", summary.name, e),
            }

            // 2. Move excess-receive messages to DLQ
            if self.config.enable_dlq {
                match self.backend.move_to_dlq(&summary.name, now_ms).await {
                    Ok(count) => {
                        if count > 0 {
                            tracing::debug!("Moved {} messages to DLQ from queue '{}'", count, summary.name);
                        }
                    }
                    Err(e) => tracing::error!("Failed to move messages to DLQ for '{}': {}", summary.name, e),
                }
            }

            // 3. Purge expired TTL messages
            match self.backend.purge_expired_messages(&summary.name, now_ms).await {
                Ok(count) => {
                    if count > 0 {
                        tracing::debug!("Purged {} expired messages from queue '{}'", count, summary.name);
                    }
                }
                Err(e) => tracing::error!("Failed to purge expired messages for '{}': {}", summary.name, e),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::StorageQueueBackend;
    use crate::error::QueueError;
    use crate::types::*;
    use nova_core::StorageEngine;
    use std::sync::Arc;

    struct MockStorage;

    impl StorageEngine for MockStorage {
        fn get(&self, _key: &nova_core::Key) -> nova_core::Result<Option<nova_core::Value>> {
            Ok(None)
        }
        fn set(&self, _key: &nova_core::Key, _value: nova_core::Value) -> nova_core::Result<()> {
            Ok(())
        }
        fn delete(&self, _key: &nova_core::Key) -> nova_core::Result<bool> {
            Ok(true)
        }
        fn scan(&self, _range: std::ops::Range<nova_core::Key>) -> nova_core::Result<Vec<(nova_core::Key, nova_core::Value)>> {
            Ok(Vec::new())
        }
        fn batch(&self, _ops: Vec<nova_core::WriteOperation>) -> nova_core::Result<()> {
            Ok(())
        }
        fn flush(&self) -> nova_core::Result<()> {
            Ok(())
        }
        fn sync(&self) -> nova_core::Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_scanner_new() {
        let backend = Arc::new(StorageQueueBackend::new(Arc::new(MockStorage)));
        let config = QueueConfig::default();
        let (_tx, rx) = watch::channel(false);
        let scanner = QueueScanner::new(backend, config, rx);
        // Just verifies construction doesn't panic
        assert!(scanner.config.scanner_interval_ms > 0);
    }
}
