use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use lru::LruCache;
use parking_lot::RwLock;
use tracing::instrument;

use super::{CacheBackend, CacheEntry, CacheKey, CacheValue};
use crate::error::Result;
use crate::metrics::CacheMetrics;

pub struct HashMapBackend {
    cache: RwLock<LruCache<CacheKey, CacheEntry>>,
    max_bytes: usize,
    current_bytes: AtomicU64,
    metrics: Arc<CacheMetrics>,
}

impl HashMapBackend {
    pub fn new(max_bytes: usize, metrics: Arc<CacheMetrics>) -> Self {
        let max_entries = std::cmp::max(max_bytes, 100).min(1_000_000);
        Self {
            cache: RwLock::new(LruCache::new(NonZeroUsize::new(max_entries).unwrap())),
            max_bytes,
            current_bytes: AtomicU64::new(0),
            metrics,
        }
    }

    fn entry_size(key: &CacheKey, value: &CacheValue) -> u64 {
        (key.len() + value.len() + std::mem::size_of::<CacheEntry>()) as u64
    }
}

#[async_trait]
impl CacheBackend for HashMapBackend {
    #[instrument(skip(self))]
    async fn get(&self, key: &CacheKey) -> Result<Option<CacheValue>> {
        let mut cache = self.cache.write();
        match cache.get_mut(key) {
            Some(entry) => {
                if let Some(expires_at) = entry.expires_at {
                    if Instant::now() > expires_at {
                        cache.pop(key);
                        self.metrics.misses.fetch_add(1, Ordering::Relaxed);
                        return Ok(None);
                    }
                }
                entry.access_count += 1;
                let value = entry.value.clone();
                self.metrics.hits.fetch_add(1, Ordering::Relaxed);
                Ok(Some(value))
            }
            None => {
                self.metrics.misses.fetch_add(1, Ordering::Relaxed);
                Ok(None)
            }
        }
    }

    #[instrument(skip(self))]
    async fn set(&self, key: CacheKey, value: CacheValue, ttl: Option<Duration>) -> Result<()> {
        let new_size = Self::entry_size(&key, &value);
        let expires_at = ttl.map(|d| Instant::now() + d);

        let mut cache = self.cache.write();

        if let Some(existing) = cache.peek(&key) {
            let old_size = Self::entry_size(&key, &existing.value);
            self.current_bytes.fetch_sub(old_size, Ordering::Relaxed);
        }

        while self.current_bytes.load(Ordering::Relaxed) + new_size > self.max_bytes as u64 {
            match cache.pop_lru() {
                Some((evicted_key, evicted_entry)) => {
                    let evicted_size = Self::entry_size(&evicted_key, &evicted_entry.value);
                    self.current_bytes.fetch_sub(evicted_size, Ordering::Relaxed);
                    self.metrics.evictions.fetch_add(1, Ordering::Relaxed);
                }
                None => break,
            }
        }

        let entry = CacheEntry {
            value,
            created_at: Instant::now(),
            expires_at,
            access_count: 0,
        };

        cache.put(key, entry);
        self.current_bytes.fetch_add(new_size, Ordering::Relaxed);
        self.metrics.sets.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    #[instrument(skip(self))]
    async fn delete(&self, key: &CacheKey) -> Result<bool> {
        let mut cache = self.cache.write();
        match cache.pop(key) {
            Some(entry) => {
                let size = Self::entry_size(key, &entry.value);
                self.current_bytes.fetch_sub(size, Ordering::Relaxed);
                self.metrics.deletes.fetch_add(1, Ordering::Relaxed);
                Ok(true)
            }
            None => Ok(false),
        }
    }

    #[instrument(skip(self))]
    async fn exists(&self, key: &CacheKey) -> Result<bool> {
        let mut cache = self.cache.write();
        match cache.get_mut(key) {
            Some(entry) => {
                if let Some(expires_at) = entry.expires_at {
                    if Instant::now() > expires_at {
                        cache.pop(key);
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            None => Ok(false),
        }
    }

    #[instrument(skip(self))]
    async fn flush(&self) -> Result<()> {
        let mut cache = self.cache.write();
        cache.clear();
        self.current_bytes.store(0, Ordering::Relaxed);
        Ok(())
    }

    #[instrument(skip(self))]
    async fn len(&self) -> Result<usize> {
        let cache = self.cache.read();
        Ok(cache.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_basic_put_get() {
        let metrics = Arc::new(CacheMetrics::default());
        let backend = HashMapBackend::new(1024 * 1024, metrics);

        backend.set("key1".into(), b"value1".to_vec(), None).await.unwrap();
        let result = backend.get(&"key1".into()).await.unwrap();
        assert_eq!(result, Some(b"value1".to_vec()));
    }

    #[tokio::test]
    async fn test_get_missing() {
        let metrics = Arc::new(CacheMetrics::default());
        let backend = HashMapBackend::new(1024 * 1024, metrics);

        let result = backend.get(&"nonexistent".into()).await.unwrap();
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_delete() {
        let metrics = Arc::new(CacheMetrics::default());
        let backend = HashMapBackend::new(1024 * 1024, metrics);

        backend.set("key1".into(), b"value1".to_vec(), None).await.unwrap();
        assert!(backend.delete(&"key1".into()).await.unwrap());
        assert!(!backend.delete(&"key1".into()).await.unwrap());
        let result = backend.get(&"key1".into()).await.unwrap();
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_ttl_expiry() {
        let metrics = Arc::new(CacheMetrics::default());
        let backend = HashMapBackend::new(1024 * 1024, metrics);

        backend.set("key1".into(), b"value1".to_vec(), Some(Duration::from_millis(10))).await.unwrap();
        assert!(backend.get(&"key1".into()).await.unwrap().is_some());
        tokio::time::sleep(Duration::from_millis(50)).await;
        let result = backend.get(&"key1".into()).await.unwrap();
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_lru_eviction() {
        let metrics = Arc::new(CacheMetrics::default());
        let backend = HashMapBackend::new(200, metrics);

        backend.set("key1".into(), b"x".to_vec(), None).await.unwrap();
        backend.set("key2".into(), b"y".to_vec(), None).await.unwrap();
        backend.set("key3".into(), b"z".to_vec(), None).await.unwrap();

        let result = backend.get(&"key1".into()).await.unwrap();
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_flush() {
        let metrics = Arc::new(CacheMetrics::default());
        let backend = HashMapBackend::new(1024 * 1024, metrics);

        backend.set("key1".into(), b"v1".to_vec(), None).await.unwrap();
        backend.set("key2".into(), b"v2".to_vec(), None).await.unwrap();
        assert_eq!(backend.len().await.unwrap(), 2);

        backend.flush().await.unwrap();
        assert_eq!(backend.len().await.unwrap(), 0);
        assert!(backend.is_empty().await.unwrap());
    }

    #[tokio::test]
    async fn test_overwrite() {
        let metrics = Arc::new(CacheMetrics::default());
        let backend = HashMapBackend::new(1024 * 1024, metrics);

        backend.set("key1".into(), b"old".to_vec(), None).await.unwrap();
        backend.set("key1".into(), b"new".to_vec(), None).await.unwrap();
        let result = backend.get(&"key1".into()).await.unwrap();
        assert_eq!(result, Some(b"new".to_vec()));
    }

    #[tokio::test]
    async fn test_concurrent_access() {
        let metrics = Arc::new(CacheMetrics::default());
        let backend = Arc::new(HashMapBackend::new(1024 * 1024, metrics));

        let mut handles = Vec::new();
        for i in 0..10 {
            let b = Arc::clone(&backend);
            handles.push(tokio::spawn(async move {
                let key = format!("key{}", i);
                let val = format!("val{}", i);
                b.set(key.clone(), val.into_bytes(), None).await.unwrap();
                let result = b.get(&key).await.unwrap();
                assert!(result.is_some());
            }));
        }

        for h in handles {
            h.await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_metrics_tracking() {
        let metrics = Arc::new(CacheMetrics::default());
        let backend = HashMapBackend::new(1024 * 1024, Arc::clone(&metrics));

        backend.get(&"miss".into()).await.unwrap();
        assert_eq!(metrics.misses(), 1);

        backend.set("hit".into(), b"v".to_vec(), None).await.unwrap();
        backend.get(&"hit".into()).await.unwrap();
        assert_eq!(metrics.hits(), 1);
        assert_eq!(metrics.sets(), 1);

        backend.delete(&"hit".into()).await.unwrap();
        assert_eq!(metrics.deletes(), 1);
    }
}
