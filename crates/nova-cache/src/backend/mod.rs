use async_trait::async_trait;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use crate::error::Result;

pub type CacheKey = String;
pub type CacheValue = Vec<u8>;

#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub value: CacheValue,
    pub created_at: std::time::Instant,
    pub expires_at: Option<std::time::Instant>,
    pub access_count: u64,
}

#[async_trait]
pub trait CacheBackend: Send + Sync {
    async fn get(&self, key: &CacheKey) -> Result<Option<CacheValue>>;
    async fn set(&self, key: CacheKey, value: CacheValue, ttl: Option<Duration>) -> Result<()>;
    async fn delete(&self, key: &CacheKey) -> Result<bool>;
    async fn exists(&self, key: &CacheKey) -> Result<bool>;
    async fn flush(&self) -> Result<()>;
    async fn len(&self) -> Result<usize>;
    async fn is_empty(&self) -> Result<bool> {
        self.len().await.map(|l| l == 0)
    }

    async fn get_or_insert_with(
        &self,
        key: CacheKey,
        f: Box<dyn FnOnce() -> Pin<Box<dyn Future<Output = Result<CacheValue>> + Send>> + Send>,
        ttl: Option<Duration>,
    ) -> Result<CacheValue> {
        if let Some(value) = self.get(&key).await? {
            return Ok(value);
        }
        let value = f().await?;
        let cloned = value.clone();
        self.set(key, cloned, ttl).await?;
        Ok(value)
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

    async fn keys(&self) -> Result<Vec<CacheKey>> {
        Ok(Vec::new())
    }

    async fn delete_matching(&self, pattern: &str) -> Result<usize> {
        let mut count = 0;
        for key in self.keys().await? {
            if crate::backend::matches_glob(pattern, &key) {
                if self.delete(&key).await? {
                    count += 1;
                }
            }
        }
        Ok(count)
    }

    fn start_ttl_sweeper(self: Arc<Self>, interval: Duration) -> tokio::task::JoinHandle<()> {
        let _ = interval;
        tokio::spawn(async {
            std::future::pending::<()>().await;
        })
    }
}

pub(crate) fn matches_glob(pattern: &str, key: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let k: Vec<char> = key.chars().collect();
    fn rec(p: &[char], k: &[char]) -> bool {
        match (p.first(), k.first()) {
            (None, None) => true,
            (Some('*'), _) => rec(&p[1..], k) || (!k.is_empty() && rec(p, &k[1..])),
            (Some('?'), Some(_)) => rec(&p[1..], &k[1..]),
            (Some(c1), Some(c2)) if c1 == c2 => rec(&p[1..], &k[1..]),
            _ => false,
        }
    }
    rec(&p, &k)
}

pub mod hashmap;
pub mod ttl;
pub use hashmap::HashMapBackend;
pub use ttl::TtlBackend;
