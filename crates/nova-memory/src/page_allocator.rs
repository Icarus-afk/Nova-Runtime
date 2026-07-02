use std::alloc::{alloc, dealloc, Layout};
use std::collections::BTreeMap;
use nova_core::{Result, RuntimeError, PAGE_SIZE};

pub struct PageAllocator {
    allocations: BTreeMap<*mut u8, usize>,
    total_allocated: usize,
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

    #[allow(unused_variables)]
    pub fn free_pages(&mut self, ptr: *mut u8, count: usize) {
        if let Some(&size) = self.allocations.get(&ptr) {
            let layout = Layout::from_size_align(size, PAGE_SIZE).expect("valid layout");
            unsafe { dealloc(ptr, layout) }
            self.allocations.remove(&ptr);
            self.total_allocated -= size;
        }
    }

    pub fn total_allocated(&self) -> usize {
        self.total_allocated
    }
}
