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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_stats_zero() {
        let gc = GenerationalGC::new(1024 * 1024, 10 * 1024 * 1024);
        let stats = gc.stats();
        assert_eq!(stats.nursery_collections, 0);
        assert_eq!(stats.old_collections, 0);
        assert_eq!(stats.promoted_objects, 0);
        assert_eq!(stats.collected_objects, 0);
        assert_eq!(stats.total_bytes_collected, 0);
        assert_eq!(stats.last_duration_ms, 0);
    }

    #[test]
    fn allocate_nursery() {
        let gc = GenerationalGC::new(100, 1000);
        let generation = gc.allocate(50);
        assert_eq!(generation, Generation::Nursery);
        let stats = gc.stats();
        assert_eq!(stats.nursery_collections, 0);
    }

    #[test]
    fn allocate_triggers_nursery_collection() {
        let gc = GenerationalGC::new(100, 1000);
        gc.allocate(60);
        let generation = gc.allocate(60);
        assert_eq!(generation, Generation::Old);
        let stats = gc.stats();
        assert_eq!(stats.nursery_collections, 1);
        assert_eq!(stats.collected_objects, 1);
    }

    #[test]
    fn collect_nursery_updates_stats() {
        let gc = GenerationalGC::new(100, 1000);
        gc.allocate(50);
        gc.allocate(30);
        let generation = gc.allocate(30);
        assert_eq!(generation, Generation::Old);
        let stats = gc.stats();
        assert_eq!(stats.nursery_collections, 1);
        assert!(stats.total_bytes_collected > 0);
    }

    #[test]
    fn collect_old() {
        let gc = GenerationalGC::new(100, 200);
        gc.allocate(80);
        gc.allocate(80);
        gc.allocate(80);
        gc.allocate(80);
        gc.allocate(80);
        gc.allocate(80);
        let stats = gc.stats();
        assert_eq!(stats.nursery_collections, 3);
        assert!(stats.old_collections >= 1);
        assert!(stats.total_bytes_collected > 0);
    }

    #[test]
    fn register_root_no_crash() {
        struct TestRoot(usize);
        impl GcRoot for TestRoot {
            fn root_size(&self) -> usize { self.0 }
            fn is_root_alive(&self) -> bool { true }
        }

        let gc = GenerationalGC::new(100, 1000);
        gc.register_root(Box::new(TestRoot(42)));
        let stats = gc.stats();
        assert_eq!(stats.nursery_collections, 0);
    }

    #[test]
    fn multiple_nursery_collections() {
        let gc = GenerationalGC::new(50, 1000);
        gc.allocate(60);
        gc.allocate(60);
        gc.allocate(60);
        let stats = gc.stats();
        assert!(stats.nursery_collections >= 3);
    }

    #[test]
    fn stats_accumulate() {
        let gc = GenerationalGC::new(50, 1000);
        gc.allocate(100);
        let stats = gc.stats();
        let collected_before = stats.total_bytes_collected;
        assert!(collected_before > 0);
        gc.allocate(100);
        let stats2 = gc.stats();
        assert!(stats2.total_bytes_collected > collected_before);
    }
}
