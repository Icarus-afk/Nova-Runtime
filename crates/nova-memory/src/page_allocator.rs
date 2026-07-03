use std::alloc::{alloc, dealloc, Layout};
use std::collections::BTreeMap;
use nova_core::{Result, RuntimeError, PAGE_SIZE};

pub struct PageAllocator {
    allocations: BTreeMap<*mut u8, usize>,
    total_allocated: usize,
}

impl Default for PageAllocator {
    fn default() -> Self {
        Self::new()
    }
}

impl PageAllocator {
    pub fn new() -> Self {
        PageAllocator {
            allocations: BTreeMap::new(),
            total_allocated: 0,
        }
    }

    pub fn allocate_pages(&mut self, count: usize) -> Result<*mut u8> {
        if count == 0 {
            return Err(RuntimeError::InvalidArgument(
                "Page count must be greater than zero".into(),
            ));
        }
        let size = count
            .checked_mul(PAGE_SIZE)
            .ok_or_else(|| RuntimeError::OutOfMemory("Page allocation overflow".into()))?;
        let layout =
            Layout::from_size_align(size, PAGE_SIZE).expect("PAGE_SIZE alignment is a power of two");
        // SAFETY: The following invariants hold:
        // - `layout` is valid (size > 0, alignment is power of two)
        // - `size` is a multiple of `PAGE_SIZE` (guaranteed by caller)
        // - The returned pointer is either null or valid for `size` bytes
        // - The memory is uninitialized but safe to write
        let ptr = unsafe { alloc(layout) };
        if ptr.is_null() {
            return Err(RuntimeError::OutOfMemory(format!(
                "Failed to allocate {} pages ({} bytes)",
                count, size
            )));
        }
        self.allocations.insert(ptr, size);
        self.total_allocated += size;
        Ok(ptr)
    }

    #[allow(unused_variables)] // `count` is unused but kept for API symmetry with `allocate_pages(count)`
    pub unsafe fn free_pages(&mut self, ptr: *mut u8, count: usize) {
        if let Some(&size) = self.allocations.get(&ptr) {
            let layout = Layout::from_size_align(size, PAGE_SIZE).expect("valid layout");
            // SAFETY: The following invariants hold:
            // - `ptr` was allocated with `layout` (guaranteed by `self.allocations`)
            // - `layout` is the same as used for allocation
            // - No other threads are accessing the memory
            // - The memory is not accessed after deallocation
            unsafe { dealloc(ptr, layout) }
            self.allocations.remove(&ptr);
            self.total_allocated -= size;
        }
    }

    pub fn total_allocated(&self) -> usize {
        self.total_allocated
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nova_core::PAGE_SIZE;

    #[test]
    fn allocate_pages_returns_non_null() {
        let mut allocator = PageAllocator::new();
        let ptr = allocator.allocate_pages(1).unwrap();
        assert!(!ptr.is_null());
        assert_eq!(allocator.total_allocated(), PAGE_SIZE);
        unsafe { allocator.free_pages(ptr, 1); }
    }

    #[test]
    fn free_pages_reclaims_memory() {
        let mut allocator = PageAllocator::new();
        let ptr = allocator.allocate_pages(1).unwrap();
        unsafe { allocator.free_pages(ptr, 1); }
        assert_eq!(allocator.total_allocated(), 0);
    }

    #[test]
    fn total_allocated_tracking() {
        let mut allocator = PageAllocator::new();
        assert_eq!(allocator.total_allocated(), 0);
        let p1 = allocator.allocate_pages(2).unwrap();
        assert_eq!(allocator.total_allocated(), 2 * PAGE_SIZE);
        let p2 = allocator.allocate_pages(3).unwrap();
        assert_eq!(allocator.total_allocated(), 5 * PAGE_SIZE);
        unsafe { allocator.free_pages(p1, 2); }
        assert_eq!(allocator.total_allocated(), 3 * PAGE_SIZE);
        unsafe { allocator.free_pages(p2, 3); }
        assert_eq!(allocator.total_allocated(), 0);
    }

    #[test]
    fn allocate_zero_pages_returns_error() {
        let mut allocator = PageAllocator::new();
        let result = allocator.allocate_pages(0);
        assert!(result.is_err());
        assert!(matches!(result, Err(RuntimeError::InvalidArgument(_))));
    }

    #[test]
    fn write_to_allocated_pages() {
        let mut allocator = PageAllocator::new();
        let ptr = allocator.allocate_pages(1).unwrap();
        unsafe {
            std::ptr::write_bytes(ptr, 0xAA, PAGE_SIZE);
            for i in 0..PAGE_SIZE {
                assert_eq!(*ptr.add(i), 0xAA);
            }
        }
        unsafe { allocator.free_pages(ptr, 1); }
    }

    #[test]
    fn multiple_allocations() {
        let mut allocator = PageAllocator::new();
        let mut ptrs = Vec::new();
        for _ in 0..5 {
            ptrs.push(allocator.allocate_pages(1).unwrap());
        }
        assert_eq!(allocator.total_allocated(), 5 * PAGE_SIZE);
        for ptr in ptrs {
            unsafe { allocator.free_pages(ptr, 1); }
        }
        assert_eq!(allocator.total_allocated(), 0);
    }

    #[test]
    fn large_page_allocation() {
        let mut allocator = PageAllocator::new();
        let ptr = allocator.allocate_pages(100).unwrap();
        assert!(!ptr.is_null());
        assert_eq!(allocator.total_allocated(), 100 * PAGE_SIZE);
        unsafe { allocator.free_pages(ptr, 100); }
    }
}
