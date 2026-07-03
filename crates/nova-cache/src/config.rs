use serde::{Deserialize, Serialize};
use crate::policy::EvictionPolicy;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BackendType {
    #[default]
    HashMap,
    Redis,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CacheConfig {
    #[serde(default = "default_max_size")]
    pub max_size: usize,
    #[serde(default = "default_ttl_secs")]
    pub default_ttl_secs: u64,
    #[serde(default)]
    pub eviction_policy: EvictionPolicy,
    #[serde(default)]
    pub backend_type: BackendType,
    #[serde(default)]
    pub redis_url: Option<String>,
}

fn default_max_size() -> usize { 128 * 1024 * 1024 }
fn default_ttl_secs() -> u64 { 300 }

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_size: 128 * 1024 * 1024,
            default_ttl_secs: 300,
            eviction_policy: EvictionPolicy::Lru,
            backend_type: BackendType::HashMap,
            redis_url: None,
        }
    }
}
