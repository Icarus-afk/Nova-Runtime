use std::sync::Arc;
use std::time::Duration;

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
}

impl CacheManager {
    pub fn new(backend: Arc<dyn CacheBackend>, config: CacheConfig) -> Self {
        Self {
            backend,
            config,
            metrics: Arc::new(CacheMetrics::default()),
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
                .unwrap_or_default()
                .as_millis() as u64,
            active: true,
            consumer_group: None,
            sender: tx,
        };

        bus.subscribe(sub)
            .map_err(|e| CacheError::Internal(e.to_string()))?;

        let backend = Arc::clone(&self.backend);
        tokio::spawn(async move {
            loop {
                let result = tokio::task::spawn_blocking({
                    let rx = rx.clone();
                    move || rx.recv()
                })
                .await;

                match result {
                    Ok(Ok(event)) => {
                        let key = event.metadata.event_type.canonical;
                        if let Some(k) = key.strip_prefix("cache.invalidate.") {
                            if let Err(e) = backend.delete(&k.to_string()).await {
                                tracing::warn!(key = %k, error = %e, "failed to invalidate cache entry");
                            }
                        }
                    }
                    _ => break,
                }
            }
        });

        Ok(())
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
        ));
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
        ));
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
        ));
        let mut config = CacheConfig::default();
        config.default_ttl_secs = 0;
        let manager = CacheManager::new(backend, config);

        manager.set("key1", b"v".to_vec(), None).await.unwrap();
        let result = manager.get("key1").await.unwrap();
        assert_eq!(result, Some(b"v".to_vec()));
    }
}
