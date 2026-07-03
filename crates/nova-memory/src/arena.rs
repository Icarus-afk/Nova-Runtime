use std::cell::Cell;
use nova_core::{Result, RuntimeError};

const DEFAULT_ALIGNMENT: usize = 8;

fn align_up(size: usize, alignment: usize) -> usize {
    (size + alignment - 1) & !(alignment - 1)
}

pub struct Arena {
    name: String,
    data: Vec<u8>,
    cursor: Cell<usize>,
    capacity: usize,
}

impl Arena {
    pub fn new(name: String, capacity: usize) -> Result<Self> {
        let mut data = Vec::new();
        data.try_reserve_exact(capacity)
            .map_err(|e| RuntimeError::OutOfMemory(format!("Failed to allocate arena '{}': {}", name, e)))?;
        data.resize(capacity, 0);
        Ok(Arena { name, data, cursor: Cell::new(0), capacity })
    }

    pub fn allocate(&mut self, size: usize) -> Result<*mut u8> {
        if size == 0 {
            return Err(RuntimeError::InvalidArgument(
                "Arena allocation size must be greater than zero".into(),
            ));
        }
        let aligned = align_up(size, DEFAULT_ALIGNMENT);
        let current = self.cursor.get();
        match current.checked_add(aligned) {
            Some(new_cursor) if new_cursor <= self.capacity => {
                self.cursor.set(new_cursor);
                // SAFETY: The following invariants hold:
                // - `current` is within bounds of `self.data` (0 <= current < self.capacity)
                // - `aligned` is non-zero and properly aligned (aligned_up ensures this)
                // - `current + aligned` <= `self.capacity` (checked above)
                // - The returned pointer is valid for reads/writes of `aligned` bytes
                // - The pointer provenance is from `self.data`, ensuring proper lifetime
                Ok(unsafe { self.data.as_mut_ptr().add(current) })
            }
            _ => Err(RuntimeError::OutOfMemory(format!(
                "Arena '{}': allocation of {} bytes at offset {} exceeds capacity {}",
                self.name, aligned, current, self.capacity
            ))),
        }
    }

    pub fn allocate_zeroed(&mut self, size: usize) -> Result<*mut u8> {
        let ptr = self.allocate(size)?;
        let aligned = align_up(size, DEFAULT_ALIGNMENT);
        // SAFETY: The following invariants hold:
        // - `ptr` is valid for `aligned` bytes (guaranteed by `self.allocate`)
        // - `ptr` is properly aligned (guaranteed by `align_up` and `DEFAULT_ALIGNMENT`)
        // - The memory is initialized to zero, ensuring safe access after this operation
        // - No other references to this memory exist (exclusive access guaranteed by arena)
        unsafe { std::ptr::write_bytes(ptr, 0, aligned); }
        Ok(ptr)
    }

    pub fn reset(&mut self) {
        self.cursor.set(0);
        self.data.fill(0);
    }

    pub fn used(&self) -> usize {
        self.cursor.get()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn remaining(&self) -> usize {
        self.capacity - self.cursor.get()
    }

    pub fn contains(&self, ptr: *const u8) -> bool {
        let start = self.data.as_ptr();
        // SAFETY: The following invariants hold:
        // - `self.capacity` is the exact length of `self.data`
        // - `start` is valid for `self.capacity` bytes (guaranteed by Vec)
        // - The resulting pointer `end` is one-past-the-end, valid for comparison
        let end = unsafe { start.add(self.capacity) };
        ptr >= start && ptr < end
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocate_basic() {
        let mut arena = Arena::new("test".into(), 4096).unwrap();
        let ptr = arena.allocate(64).unwrap();
        assert!(!ptr.is_null());
        assert!(arena.contains(ptr));
    }

    #[test]
    fn allocate_zeroed() {
        let mut arena = Arena::new("test".into(), 4096).unwrap();
        let ptr = arena.allocate_zeroed(64).unwrap();
        assert!(!ptr.is_null());
        unsafe {
            for i in 0..64 {
                assert_eq!(*ptr.add(i), 0);
            }
        }
    }

    #[test]
    fn arena_reset() {
        let mut arena = Arena::new("test".into(), 4096).unwrap();
        let ptr = arena.allocate(1024).unwrap();
        assert!(!ptr.is_null());
        assert_eq!(arena.used(), 1024);
        arena.reset();
        assert_eq!(arena.used(), 0);
        assert_eq!(arena.capacity(), 4096);
        let ptr2 = arena.allocate(1024).unwrap();
        assert!(!ptr2.is_null());
    }

    #[test]
    fn multiple_allocations() {
        let mut arena = Arena::new("test".into(), 4096).unwrap();
        let ptr1 = arena.allocate(64).unwrap();
        let ptr2 = arena.allocate(64).unwrap();
        assert_ne!(ptr1, ptr2);
        assert!(arena.contains(ptr1));
        assert!(arena.contains(ptr2));
        unsafe {
            std::ptr::write(ptr1 as *mut u64, 0xDEADBEEF);
            std::ptr::write(ptr2 as *mut u64, 0xCAFEBABE);
            assert_eq!(std::ptr::read(ptr1 as *mut u64), 0xDEADBEEF);
            assert_eq!(std::ptr::read(ptr2 as *mut u64), 0xCAFEBABE);
        }
    }

    #[test]
    fn allocation_exhaustion() {
        let mut arena = Arena::new("test".into(), 128).unwrap();
        arena.allocate(128).unwrap();
        let result = arena.allocate(1);
        assert!(result.is_err());
        assert!(matches!(result, Err(RuntimeError::OutOfMemory(_))));
    }

    #[test]
    fn allocate_size_zero() {
        let mut arena = Arena::new("test".into(), 4096).unwrap();
        let result = arena.allocate(0);
        assert!(result.is_err());
        assert!(matches!(result, Err(RuntimeError::InvalidArgument(_))));
    }

    #[test]
    fn contains_ptr() {
        let mut arena = Arena::new("test".into(), 4096).unwrap();
        let ptr = arena.allocate(64).unwrap();
        assert!(arena.contains(ptr));
        assert!(!arena.contains(std::ptr::null()));
    }

    #[test]
    fn used_and_remaining() {
        let mut arena = Arena::new("test".into(), 4096).unwrap();
        assert_eq!(arena.used(), 0);
        assert_eq!(arena.remaining(), 4096);
        arena.allocate(64).unwrap();
        assert_eq!(arena.used(), 64);
        assert_eq!(arena.remaining(), 4032);
        arena.allocate(128).unwrap();
        assert_eq!(arena.used(), 192);
        assert_eq!(arena.remaining(), 3904);
    }

    #[test]
    fn alignment_guarantee() {
        let mut arena = Arena::new("test".into(), 4096).unwrap();
        let ptr = arena.allocate(1).unwrap();
        assert_eq!(ptr as usize % 8, 0);
        let ptr2 = arena.allocate(3).unwrap();
        assert_eq!(ptr2 as usize % 8, 0);
    }

    #[test]
    fn large_allocation() {
        let mut arena = Arena::new("test".into(), 8192).unwrap();
        let ptr = arena.allocate(7000).unwrap();
        assert!(!ptr.is_null());
        assert_eq!(arena.used(), 7000);
        unsafe {
            std::ptr::write_bytes(ptr, 0xFF, 7000);
        }
    }

    #[test]
    fn allocate_after_reset_reuses_memory() {
        let mut arena = Arena::new("test".into(), 256).unwrap();
        let ptr1 = arena.allocate(128).unwrap();
        unsafe { std::ptr::write(ptr1 as *mut u64, 42); }
        arena.reset();
        let ptr2 = arena.allocate(128).unwrap();
        assert_eq!(ptr1, ptr2);
    }
}
