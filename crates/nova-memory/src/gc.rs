use std::sync::atomic::{AtomicUsize, Ordering};
use parking_lot::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Generation {
    Nursery,
    Old,
}

#[derive(Debug, Clone)]
pub struct GcStats {
    pub nursery_collections: u64,
    pub old_collections: u64,
    pub promoted_objects: u64,
    pub collected_objects: u64,
    pub total_bytes_collected: u64,
    pub last_duration_ms: u64,
}

pub trait GcRoot {
    fn root_size(&self) -> usize;
    fn is_root_alive(&self) -> bool;
}

pub struct GenerationalGC {
    nursery_threshold: usize,
    old_threshold: usize,
    nursery_used: AtomicUsize,
    old_used: AtomicUsize,
    stats: RwLock<GcStats>,
    roots: RwLock<Vec<Box<dyn GcRoot + Send + Sync>>>,
}

impl GenerationalGC {
    pub fn new(nursery_threshold: usize, old_threshold: usize) -> Self {
        GenerationalGC {
            nursery_threshold,
            old_threshold,
            nursery_used: AtomicUsize::new(0),
            old_used: AtomicUsize::new(0),
            stats: RwLock::new(GcStats {
                nursery_collections: 0,
                old_collections: 0,
                promoted_objects: 0,
                collected_objects: 0,
                total_bytes_collected: 0,
                last_duration_ms: 0,
            }),
            roots: RwLock::new(Vec::new()),
        }
    }

    pub fn allocate(&self, size: usize) -> Generation {
        let nursery = self.nursery_used.fetch_add(size, Ordering::Relaxed) + size;
        if nursery > self.nursery_threshold {
            self.collect_nursery();
            Generation::Old
        } else {
            Generation::Nursery
        }
    }

    pub fn collect_nursery(&self) {
        let mut stats = self.stats.write();
        let collected = self.nursery_used.swap(0, Ordering::Relaxed);
        self.old_used.fetch_add(collected / 2, Ordering::Relaxed);
        stats.nursery_collections += 1;
        stats.collected_objects += 1;
        stats.total_bytes_collected += (collected / 2) as u64;

        if self.old_used.load(Ordering::Relaxed) > self.old_threshold {
            self.collect_old();
        }
    }

    pub fn collect_old(&self) {
        let mut stats = self.stats.write();
        stats.old_collections += 1;
        stats.total_bytes_collected += (self.old_used.swap(0, Ordering::Relaxed) / 4) as u64;
    }

    pub fn register_root(&self, root: Box<dyn GcRoot + Send + Sync>) {
        self.roots.write().push(root);
    }

    pub fn stats(&self) -> GcStats {
        self.stats.read().clone()
    }
}
