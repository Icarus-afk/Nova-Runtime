use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use parking_lot::RwLock;
use tokio::task::JoinHandle;

use super::{matches_glob, CacheBackend, CacheKey, CacheValue};
use crate::error::Result;

pub struct TtlBackend {
    inner: Box<dyn CacheBackend>,
    expires: RwLock<HashMap<CacheKey, Instant>>,
}

impl TtlBackend {
    pub fn new(inner: Box<dyn CacheBackend>) -> Self {
        Self {
            inner,
            expires: RwLock::new(HashMap::new()),
        }
    }

    pub fn start_ttl_sweeper(self: Arc<Self>, interval: Duration) -> JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(interval).await;
                self.evict_expired().await;
            }
        })
    }

    async fn evict_expired(&self) {
        let now = Instant::now();
        let expired: Vec<CacheKey> = {
            let map = self.expires.read();
            map.iter()
                .filter(|(_, expiry)| **expiry <= now)
                .map(|(k, _)| k.clone())
                .collect()
        };
        for k in &expired {
            let _ = self.inner.delete(k).await;
            self.expires.write().remove(k);
        }
    }
}

#[async_trait]
impl CacheBackend for TtlBackend {
    async fn get(&self, key: &CacheKey) -> Result<Option<CacheValue>> {
        let expired = {
            let map = self.expires.read();
            map.get(key).map(|expiry| *expiry <= Instant::now()).unwrap_or(false)
        };
        if expired {
            let _ = self.inner.delete(key).await;
            self.expires.write().remove(key);
            return Ok(None);
        }
        self.inner.get(key).await
    }

    async fn set(&self, key: CacheKey, value: CacheValue, ttl: Option<Duration>) -> Result<()> {
        match ttl {
            Some(d) if d.is_zero() || d.as_nanos() == 0 => {
                self.expires.write().insert(key.clone(), Instant::now());
            }
            Some(d) => {
                self.expires.write().insert(key.clone(), Instant::now() + d);
            }
            None => {
                self.expires.write().remove(&key);
            }
        }
        self.inner.set(key, value, ttl).await
    }

    async fn delete(&self, key: &CacheKey) -> Result<bool> {
        self.expires.write().remove(key);
        self.inner.delete(key).await
    }

    async fn exists(&self, key: &CacheKey) -> Result<bool> {
        let expired = {
            let map = self.expires.read();
            map.get(key).map(|expiry| *expiry <= Instant::now()).unwrap_or(false)
        };
        if expired {
            let _ = self.inner.delete(key).await;
            self.expires.write().remove(key);
            return Ok(false);
        }
        self.inner.exists(key).await
    }

    async fn flush(&self) -> Result<()> {
        self.expires.write().clear();
        self.inner.flush().await
    }

    async fn len(&self) -> Result<usize> {
        self.inner.len().await
    }

    async fn keys(&self) -> Result<Vec<CacheKey>> {
        self.inner.keys().await
    }

    async fn delete_matching(&self, pattern: &str) -> Result<usize> {
        let matched: Vec<CacheKey> = {
            let map = self.expires.read();
            map.keys()
                .filter(|k| matches_glob(pattern, k))
                .cloned()
                .collect()
        };
        let mut count = 0;
        for k in matched {
            self.expires.write().remove(&k);
            if self.inner.delete(&k).await? {
                count += 1;
            }
        }
        Ok(count)
    }

    async fn get_many(&self, keys: &[CacheKey]) -> Result<Vec<(CacheKey, CacheValue)>> {
        let mut results = Vec::with_capacity(keys.len());
        for key in keys {
            if let Some(value) = self.get(key).await? {
                results.push((key.clone(), value));
            }
        }
        Ok(results)
    }

    async fn set_many(&self, items: Vec<(CacheKey, CacheValue, Option<Duration>)>) -> Result<()> {
        for (key, value, ttl) in items {
            self.set(key, value, ttl).await?;
        }
        Ok(())
    }

    async fn delete_many(&self, keys: &[CacheKey]) -> Result<usize> {
        let mut count = 0;
        for key in keys {
            if self.delete(key).await? {
                count += 1;
            }
        }
        Ok(count)
    }
}
