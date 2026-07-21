use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use tokio::task::JoinHandle;
use tracing::instrument;
use uuid::Uuid;

use crate::backend::{CacheBackend, CacheKey, CacheValue};
use crate::config::CacheConfig;
use crate::error::{CacheError, Result};
use crate::metrics::CacheMetrics;

pub struct CacheManager {
    backend: Arc<dyn CacheBackend>,
    config: CacheConfig,
    metrics: Arc<CacheMetrics>,
    event_handle: Mutex<Option<JoinHandle<()>>>,
}

impl CacheManager {
    pub fn new(backend: Arc<dyn CacheBackend>, config: CacheConfig) -> Self {
        Self {
            backend,
            config,
            metrics: Arc::new(CacheMetrics::default()),
            event_handle: Mutex::new(None),
        }
    }

    pub fn with_metrics(
        backend: Arc<dyn CacheBackend>,
        config: CacheConfig,
        metrics: Arc<CacheMetrics>,
    ) -> Self {
        Self {
            backend,
            config,
            metrics,
            event_handle: Mutex::new(None),
        }
    }

    pub fn metrics(&self) -> Arc<CacheMetrics> {
        Arc::clone(&self.metrics)
    }

    pub fn config(&self) -> &CacheConfig {
        &self.config
    }

    #[instrument(skip(self))]
    pub async fn get(&self, key: &str) -> Result<Option<CacheValue>> {
        self.backend.get(&key.to_string()).await
    }

    #[instrument(skip(self, key))]
    pub async fn set(&self, key: impl Into<CacheKey> + std::fmt::Debug, value: CacheValue, ttl: Option<Duration>) -> Result<()> {
        let ttl = ttl.or_else(|| {
            if self.config.default_ttl_secs > 0 {
                Some(Duration::from_secs(self.config.default_ttl_secs))
            } else {
                None
            }
        });
        self.backend.set(key.into(), value, ttl).await
    }

    #[instrument(skip(self))]
    pub async fn delete(&self, key: &str) -> Result<bool> {
        self.backend.delete(&key.to_string()).await
    }

    #[instrument(skip(self))]
    pub async fn exists(&self, key: &str) -> Result<bool> {
        self.backend.exists(&key.to_string()).await
    }

    #[instrument(skip(self))]
    pub async fn flush(&self) -> Result<()> {
        self.backend.flush().await
    }

    #[instrument(skip(self))]
    pub async fn len(&self) -> Result<usize> {
        self.backend.len().await
    }

    #[instrument(skip(self))]
    pub async fn keys(&self) -> Result<Vec<CacheKey>> {
        self.backend.keys().await
    }

    pub async fn get_or_insert_with(
        &self,
        key: CacheKey,
        f: Box<dyn FnOnce() -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<CacheValue>> + Send>> + Send>,
        ttl: Option<Duration>,
    ) -> Result<CacheValue> {
        self.backend.get_or_insert_with(key, f, ttl).await
    }

    pub async fn get_many(&self, keys: &[CacheKey]) -> Result<Vec<(CacheKey, CacheValue)>> {
        self.backend.get_many(keys).await
    }

    pub async fn set_many(&self, items: Vec<(CacheKey, CacheValue, Option<Duration>)>) -> Result<()> {
        self.backend.set_many(items).await
    }

    pub async fn delete_many(&self, keys: &[CacheKey]) -> Result<usize> {
        self.backend.delete_many(keys).await
    }

    pub fn attach_event_bus(&self, bus: &nova_event::EventBus) -> Result<()> {
        let topic = nova_event::TopicPattern::new("cache.invalidate.*")
            .map_err(|e| CacheError::Internal(e.to_string()))?;

        let (tx, rx) = crossbeam::channel::bounded::<nova_event::Event>(1024);

        let sub = nova_event::Subscription {
            id: Uuid::new_v4(),
            subscriber: nova_event::SubscriberId {
                id: "nova-cache".into(),
                subsystem: nova_event::Subsystem::System,
                name: "cache-manager".into(),
            },
            topic,
            content_filter: None,
            delivery_guarantee: nova_event::DeliveryGuarantee::AtMostOnce,
            max_retries: 0,
            retry_backoff_ms: 0,
            max_backoff_ms: 0,
            queue_capacity: 1024,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock before UNIX_EPOCH")
                .as_millis() as u64,
            active: true,
            consumer_group: None,
            sender: tx,
        };

        bus.subscribe(sub)
            .map_err(|e| CacheError::Internal(e.to_string()))?;

        let backend = Arc::clone(&self.backend);
        let handle = tokio::spawn(async move {
            loop {
                let result = tokio::task::spawn_blocking({
                    let rx = rx.clone();
                    move || rx.recv()
                })
                .await;

                match result {
                    Ok(Ok(event)) => {
                        let canonical = &event.metadata.event_type.canonical;
                        if let Some(rest) = canonical.strip_prefix("cache.invalidate.") {
                            if let Some(pattern) = rest.strip_prefix("pattern.") {
                                if let Err(e) = backend.delete_matching(pattern).await {
                                    tracing::warn!(pattern = %pattern, error = %e, "pattern invalidation failed");
                                }
                            } else {
                                if let Err(e) = backend.delete(&rest.to_string()).await {
                                    tracing::warn!(key = %rest, error = %e, "failed to invalidate cache entry");
                                }
                            }
                        }
                    }
                    _ => break,
                }
            }
        });

        if let Ok(mut guard) = self.event_handle.lock() {
            *guard = Some(handle);
        }

        Ok(())
    }

    pub fn shutdown(&self) {
        if let Ok(mut guard) = self.event_handle.lock() {
            if let Some(handle) = guard.take() {
                handle.abort();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::hashmap::HashMapBackend;

    #[tokio::test]
    async fn test_manager_basic_operations() {
        let backend = Arc::new(HashMapBackend::new(
            1024 * 1024,
            Arc::new(CacheMetrics::default()),
        ).unwrap());
        let config = CacheConfig::default();
        let manager = CacheManager::new(backend, config);

        manager.set("key1", b"value1".to_vec(), None).await.unwrap();
        let result = manager.get("key1").await.unwrap();
        assert_eq!(result, Some(b"value1".to_vec()));

        assert!(manager.exists("key1").await.unwrap());
        assert!(!manager.exists("key2").await.unwrap());

        assert!(manager.delete("key1").await.unwrap());
        assert!(!manager.delete("key1").await.unwrap());
    }

    #[tokio::test]
    async fn test_manager_metrics() {
        let metrics = Arc::new(CacheMetrics::default());
        let backend = Arc::new(HashMapBackend::new(
            1024 * 1024,
            Arc::clone(&metrics),
        ).unwrap());
        let manager = CacheManager::with_metrics(backend, CacheConfig::default(), Arc::clone(&metrics));

        manager.get("miss").await.unwrap();
        manager.set("hit", b"v".to_vec(), None).await.unwrap();
        manager.get("hit").await.unwrap();

        assert_eq!(metrics.hits(), 1);
        assert_eq!(metrics.misses(), 1);
    }

    #[tokio::test]
    async fn test_manager_default_ttl() {
        let backend = Arc::new(HashMapBackend::new(
            1024 * 1024,
            Arc::new(CacheMetrics::default()),
        ).unwrap());
        let mut config = CacheConfig::default();
        config.default_ttl_secs = 0;
        let manager = CacheManager::new(backend, config);

        manager.set("key1", b"v".to_vec(), None).await.unwrap();
        let result = manager.get("key1").await.unwrap();
        assert_eq!(result, Some(b"v".to_vec()));
    }
}
