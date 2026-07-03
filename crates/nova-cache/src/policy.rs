use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum EvictionPolicy {
    #[default]
    Lru,
    Lfu,
    Ttl,
    LruWithTtl,
    NoEviction,
}
