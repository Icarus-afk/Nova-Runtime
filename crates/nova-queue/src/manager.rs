use crate::backend::QueueBackend;
use crate::config::QueueConfig;
use crate::scanner::QueueScanner;
use crate::types::*;
use crate::error::Result;
use std::sync::Arc;

/// Central manager for the queue subsystem.
pub struct QueueManager {
    backend: Arc<dyn QueueBackend>,
    config: QueueConfig,
}

impl QueueManager {
    pub fn new(backend: Arc<dyn QueueBackend>, config: QueueConfig) -> Self {
        Self { backend, config }
    }

    pub fn backend(&self) -> &Arc<dyn QueueBackend> {
        &self.backend
    }

    pub fn config(&self) -> &QueueConfig {
        &self.config
    }

    pub async fn create_queue(&self, name: &str) -> Result<()> {
        let cfg = crate::types::IndividualQueueConfig::new(name);
        self.backend.create_queue(cfg).await
    }

    pub async fn delete_queue(&self, name: &str) -> Result<()> {
        self.backend.delete_queue(name).await
    }

    pub async fn list_queues(&self) -> Result<Vec<QueueSummary>> {
        self.backend.list_queues().await
    }

    pub async fn enqueue(&self, queue_name: &str, body: Vec<u8>) -> Result<()> {
        let msg = QueueMessage::new(queue_name, body);
        self.backend.enqueue(msg).await
    }

    pub async fn dequeue(&self, queue_name: &str, max_messages: u32) -> Result<Vec<QueueMessage>> {
        self.backend.dequeue(queue_name, max_messages).await
    }

    pub async fn ack(&self, queue_name: &str, receipt_handle: &str) -> Result<()> {
        self.backend.ack(queue_name, receipt_handle).await
    }

    pub async fn stats(&self, queue_name: &str) -> Result<QueueStats> {
        self.backend.stats(queue_name).await
    }

    /// Create a scanner for running background tasks.
    pub fn create_scanner(&self, shutdown_rx: tokio::sync::watch::Receiver<bool>) -> QueueScanner {
        QueueScanner::new(
            self.backend.clone(),
            self.config.clone(),
            shutdown_rx,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::StorageQueueBackend;
    use nova_core::StorageEngine;

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
    async fn test_manager_new() {
        let backend = Arc::new(StorageQueueBackend::new(Arc::new(MockStorage)));
        let config = QueueConfig::default();
        let manager = QueueManager::new(backend, config);
        let queues = manager.list_queues().await.unwrap();
        assert!(queues.is_empty());
    }

    #[tokio::test]
    async fn test_manager_create_and_list() {
        let backend = Arc::new(StorageQueueBackend::new(Arc::new(MockStorage)));
        let config = QueueConfig::default();
        let manager = QueueManager::new(backend, config);
        assert!(manager.create_queue("test-q").await.is_ok());
    }
}
