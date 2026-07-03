use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug)]
pub struct CacheMetrics {
    pub hits: AtomicU64,
    pub misses: AtomicU64,
    pub evictions: AtomicU64,
    pub sets: AtomicU64,
    pub deletes: AtomicU64,
}

impl Default for CacheMetrics {
    fn default() -> Self {
        Self {
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            evictions: AtomicU64::new(0),
            sets: AtomicU64::new(0),
            deletes: AtomicU64::new(0),
        }
    }
}

impl CacheMetrics {
    pub fn hits(&self) -> u64 {
        self.hits.load(Ordering::Relaxed)
    }

    pub fn misses(&self) -> u64 {
        self.misses.load(Ordering::Relaxed)
    }

    pub fn evictions(&self) -> u64 {
        self.evictions.load(Ordering::Relaxed)
    }

    pub fn sets(&self) -> u64 {
        self.sets.load(Ordering::Relaxed)
    }

    pub fn deletes(&self) -> u64 {
        self.deletes.load(Ordering::Relaxed)
    }

    pub fn hit_rate(&self) -> f64 {
        let total = self.hits() + self.misses();
        if total == 0 {
            0.0
        } else {
            self.hits() as f64 / total as f64
        }
    }
}
