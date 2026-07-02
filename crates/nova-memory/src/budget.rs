use std::alloc::Layout;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use nova_core::{Result, RuntimeError, PAGE_SIZE};
use tracing::warn;

#[derive(Debug, Clone)]
pub struct MemoryConfig {
    pub max_memory: u64,
    pub pressure_threshold_pct: u8,
    pub critical_threshold_pct: u8,
    pub emergency_reserve: u64,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        MemoryConfig {
            max_memory: 1024 * 1024 * 1024,
            pressure_threshold_pct: 80,
            critical_threshold_pct: 95,
            emergency_reserve: 32 * 1024 * 1024,
        }
    }
}

pub struct MemoryBudget {
    name: String,
    max_bytes: u64,
    used_bytes: AtomicU64,
    peak_bytes: AtomicU64,
}

impl MemoryBudget {
    pub fn new(name: &str, max_bytes: u64) -> Self {
        MemoryBudget {
            name: name.to_string(),
            max_bytes,
            used_bytes: AtomicU64::new(0),
            peak_bytes: AtomicU64::new(0),
        }
    }

    pub fn reserve(&self, size: u64) -> Result<()> {
        let prev = self.used_bytes.fetch_add(size, Ordering::Relaxed);
        let new = prev + size;
        if new > self.max_bytes {
            self.used_bytes.fetch_sub(size, Ordering::Relaxed);
            let pct = (prev as f64 / self.max_bytes as f64) * 100.0;
            warn!(
                budget = %self.name,
                used = prev,
                max = self.max_bytes,
                requested = size,
                utilization_pct = pct,
                "Memory budget exceeded"
            );
            return Err(RuntimeError::OutOfMemory(format!(
                "Budget '{}' exceeded: {}/{} (requested {})",
                self.name, prev, self.max_bytes, size
            )));
        }
        let peak = self.peak_bytes.load(Ordering::Relaxed);
        if new > peak {
            self.peak_bytes.store(new, Ordering::Relaxed);
        }
        Ok(())
    }

    pub fn release(&self, size: u64) {
        self.used_bytes.fetch_sub(size, Ordering::Relaxed);
    }

    pub fn used(&self) -> u64 {
        self.used_bytes.load(Ordering::Relaxed)
    }

    pub fn peak(&self) -> u64 {
        self.peak_bytes.load(Ordering::Relaxed)
    }

    pub fn available(&self) -> u64 {
        self.max_bytes.saturating_sub(self.used_bytes.load(Ordering::Relaxed))
    }

    pub fn utilization_pct(&self) -> f64 {
        let used = self.used_bytes.load(Ordering::Relaxed) as f64;
        let max = self.max_bytes as f64;
        if max == 0.0 {
            0.0
        } else {
            (used / max) * 100.0
        }
    }

    pub fn reset_peak(&self) {
        self.peak_bytes.store(self.used_bytes.load(Ordering::Relaxed), Ordering::Relaxed);
    }

    pub fn is_pressured(&self, threshold_pct: u8) -> bool {
        self.utilization_pct() >= threshold_pct as f64
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryPressureLevel {
    Normal,
    Pressure,
    Critical,
}

pub type PressureCallback = Arc<dyn Fn(MemoryPressureLevel) + Send + Sync>;

pub struct MemoryManager {
    budgets: Vec<Arc<MemoryBudget>>,
    global_max: u64,
    total_used: AtomicU64,
    pressure_threshold_pct: u8,
    critical_threshold_pct: u8,
    emergency_reserve: u64,
    pressure_callbacks: Vec<PressureCallback>,
}

impl MemoryManager {
    pub fn new(config: &MemoryConfig) -> Self {
        MemoryManager {
            budgets: Vec::new(),
            global_max: config.max_memory,
            total_used: AtomicU64::new(0),
            pressure_threshold_pct: config.pressure_threshold_pct,
            critical_threshold_pct: config.critical_threshold_pct,
            emergency_reserve: config.emergency_reserve,
            pressure_callbacks: Vec::new(),
        }
    }

    pub fn register_budget(&mut self, name: &str, max_bytes: u64) -> Arc<MemoryBudget> {
        let budget = Arc::new(MemoryBudget::new(name, max_bytes));
        self.budgets.push(Arc::clone(&budget));
        budget
    }

    pub fn total_used(&self) -> u64 {
        self.total_used.load(Ordering::Relaxed)
    }

    pub fn global_utilization_pct(&self) -> f64 {
        let used = self.total_used.load(Ordering::Relaxed) as f64;
        let max = self.global_max as f64;
        if max == 0.0 {
            0.0
        } else {
            (used / max) * 100.0
        }
    }

    pub fn is_global_pressured(&self) -> bool {
        self.global_utilization_pct() >= self.pressure_threshold_pct as f64
    }

    pub fn is_global_critical(&self) -> bool {
        self.global_utilization_pct() >= self.critical_threshold_pct as f64
    }

    pub fn emergency_reserve_size(&self) -> u64 {
        self.emergency_reserve
    }

    pub fn allocate(&mut self, size: usize) -> Result<Vec<u8>> {
        let new_used = self.total_used.fetch_add(size as u64, Ordering::Relaxed) + size as u64;
        let effective_max = self.global_max.saturating_sub(self.emergency_reserve);
        if new_used > effective_max {
            self.total_used.fetch_sub(size as u64, Ordering::Relaxed);
            return Err(RuntimeError::OutOfMemory(format!(
                "Global memory limit would be exceeded: {} bytes requested, {} available",
                size,
                effective_max.saturating_sub(new_used - size as u64),
            )));
        }

        let layout = Layout::array::<u8>(size)
            .map_err(|_| RuntimeError::InvalidArgument("invalid allocation size".into()))?;
        let ptr = if size <= PAGE_SIZE {
            unsafe { std::alloc::alloc_zeroed(layout) }
        } else {
            let pages = size.div_ceil(PAGE_SIZE);
            let page_layout = Layout::from_size_align(pages * PAGE_SIZE, PAGE_SIZE)
                .expect("page layout is valid");
            unsafe { std::alloc::alloc_zeroed(page_layout) }
        };

        if ptr.is_null() {
            self.total_used.fetch_sub(size as u64, Ordering::Relaxed);
            return Err(RuntimeError::OutOfMemory("system allocator returned null".into()));
        }

        Ok(unsafe { Vec::from_raw_parts(ptr, size, layout.size()) })
    }

    pub fn on_pressure(&mut self, callback: PressureCallback) {
        self.pressure_callbacks.push(callback);
    }

    pub fn check_pressure(&self) -> MemoryPressureLevel {
        let used = self.total_used.load(Ordering::Relaxed);
        let used_pct = if self.global_max == 0 {
            0.0
        } else {
            (used as f64 / self.global_max as f64) * 100.0
        };

        if used_pct >= self.critical_threshold_pct as f64 {
            MemoryPressureLevel::Critical
        } else if used_pct >= self.pressure_threshold_pct as f64 {
            MemoryPressureLevel::Pressure
        } else {
            MemoryPressureLevel::Normal
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_budget() {
        let budget = MemoryBudget::new("test", 1024);
        assert_eq!(budget.used(), 0);
        assert_eq!(budget.peak(), 0);
        assert_eq!(budget.available(), 1024);
    }

    #[test]
    fn reserve_within_budget() {
        let budget = MemoryBudget::new("test", 1024);
        assert!(budget.reserve(512).is_ok());
        assert_eq!(budget.used(), 512);
    }

    #[test]
    fn reserve_exceeds_budget() {
        let budget = MemoryBudget::new("test", 100);
        let result = budget.reserve(200);
        assert!(result.is_err());
        assert_eq!(budget.used(), 0);
    }

    #[test]
    fn release_frees_memory() {
        let budget = MemoryBudget::new("test", 1024);
        budget.reserve(500).unwrap();
        assert_eq!(budget.used(), 500);
        budget.release(200);
        assert_eq!(budget.used(), 300);
    }

    #[test]
    fn peak_tracking() {
        let budget = MemoryBudget::new("test", 1024);
        assert_eq!(budget.peak(), 0);
        budget.reserve(100).unwrap();
        assert_eq!(budget.peak(), 100);
        budget.reserve(200).unwrap();
        assert_eq!(budget.peak(), 300);
        budget.release(300);
        assert_eq!(budget.peak(), 300);
    }

    #[test]
    fn available_tracking() {
        let budget = MemoryBudget::new("test", 1000);
        assert_eq!(budget.available(), 1000);
        budget.reserve(300).unwrap();
        assert_eq!(budget.available(), 700);
        budget.release(100);
        assert_eq!(budget.available(), 800);
    }

    #[test]
    fn utilization_pct() {
        let budget = MemoryBudget::new("test", 1000);
        assert!((budget.utilization_pct() - 0.0).abs() < f64::EPSILON);
        budget.reserve(250).unwrap();
        assert!((budget.utilization_pct() - 25.0).abs() < f64::EPSILON);
        budget.reserve(250).unwrap();
        assert!((budget.utilization_pct() - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn is_pressured() {
        let budget = MemoryBudget::new("test", 100);
        assert!(!budget.is_pressured(50));
        budget.reserve(60).unwrap();
        assert!(budget.is_pressured(50));
        assert!(!budget.is_pressured(70));
    }

    #[test]
    fn reset_peak() {
        let budget = MemoryBudget::new("test", 1024);
        budget.reserve(500).unwrap();
        assert_eq!(budget.peak(), 500);
        budget.release(500);
        budget.reset_peak();
        assert_eq!(budget.peak(), 0);
    }

    #[test]
    fn budget_zero_max() {
        let budget = MemoryBudget::new("test", 0);
        assert_eq!(budget.available(), 0);
        assert!(budget.reserve(1).is_err());
        assert!((budget.utilization_pct() - 100.0).abs() < f64::EPSILON);
        assert!(budget.is_pressured(0));
    }

    #[test]
    fn multiple_budget_operations() {
        let budget = MemoryBudget::new("test", 1000);
        for _ in 0..10 {
            assert!(budget.reserve(50).is_ok());
        }
        assert_eq!(budget.used(), 500);
        assert!(budget.reserve(501).is_err());
        assert_eq!(budget.used(), 500);
        budget.release(500);
        assert_eq!(budget.used(), 0);
    }

    #[test]
    fn manager_new() {
        let config = MemoryConfig::default();
        let mut manager = MemoryManager::new(&config);
        assert_eq!(manager.total_used(), 0);
        assert_eq!(manager.emergency_reserve_size(), config.emergency_reserve);
    }

    #[test]
    fn manager_register_budget() {
        let config = MemoryConfig::default();
        let mut manager = MemoryManager::new(&config);
        let budget = manager.register_budget("workers", 500 * 1024 * 1024);
        assert_eq!(budget.used(), 0);
        assert_eq!(budget.available(), 500 * 1024 * 1024);
    }

    #[test]
    fn manager_pressure_levels() {
        let config = MemoryConfig {
            max_memory: 1000,
            pressure_threshold_pct: 80,
            critical_threshold_pct: 95,
            emergency_reserve: 100,
        };
        let manager = MemoryManager::new(&config);
        assert_eq!(manager.check_pressure(), MemoryPressureLevel::Normal);
        assert!(!manager.is_global_pressured());
        assert!(!manager.is_global_critical());
    }

    #[test]
    fn manager_allocate_small() {
        let config = MemoryConfig {
            max_memory: 1024 * 1024,
            pressure_threshold_pct: 80,
            critical_threshold_pct: 95,
            emergency_reserve: 4096,
        };
        let mut manager = MemoryManager::new(&config);
        let vec = manager.allocate(64).unwrap();
        assert_eq!(vec.len(), 64);
        assert_eq!(manager.total_used(), 64);
    }
}
