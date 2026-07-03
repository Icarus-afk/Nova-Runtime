use async_trait::async_trait;
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
}

pub mod hashmap;
pub use hashmap::HashMapBackend;
