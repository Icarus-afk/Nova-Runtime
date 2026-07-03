use nova_core::{Result, RuntimeError};

pub const SLAB_SIZE_CLASSES: &[usize] = &[32, 64, 128, 256, 512, 1024, 2048, 4096, 8192, 16384];
pub const SLAB_PAGE_SIZE: usize = 4096;

struct SlabPage {
    data: Vec<u8>,
    num_slots: usize,
}

impl SlabPage {
    fn new(object_size: usize) -> Self {
        let raw = std::cmp::max(SLAB_PAGE_SIZE, object_size);
        let page_size = raw.div_ceil(object_size) * object_size;
        let num_slots = page_size / object_size;
        let data = vec![0u8; page_size];
        SlabPage { data, num_slots }
    }

    fn slot_ptr(&self, slot: usize, object_size: usize) -> *mut u8 {
        // SAFETY: The following invariants hold:
        // - `slot` is within bounds (0 <= slot < self.num_slots)
        // - `object_size` is valid for the slab (guaranteed by Slab::new)
        // - `slot * object_size` does not overflow (guaranteed by page size constraints)
        // - The returned pointer is valid for `object_size` bytes
        unsafe { self.data.as_ptr().add(slot * object_size) as *mut u8 }
    }

    fn contains(&self, ptr: *const u8) -> bool {
        let start = self.data.as_ptr();
        // SAFETY: The following invariants hold:
        // - `self.data.len()` is the exact length of the allocation
        // - `start` is valid for `self.data.len()` bytes
        // - The resulting pointer `end` is one-past-the-end, valid for comparison
        let end = unsafe { start.add(self.data.len()) };
        ptr >= start && ptr < end
    }

    fn slot_index(&self, ptr: *const u8, object_size: usize) -> usize {
        // SAFETY: The following invariants hold:
        // - `ptr` is within `self.data` (guaranteed by `contains` check in caller)
        // - `self.data.as_ptr()` is valid and aligned
        // - The offset calculation is valid for in-bounds pointers
        let offset = unsafe { ptr.offset_from(self.data.as_ptr()) };
        offset as usize / object_size
    }
}

pub struct Slab {
    object_size: usize,
    pages: Vec<SlabPage>,
    free_list: Vec<(usize, usize)>,
    allocated_count: usize,
    total_capacity: usize,
}

impl Slab {
    pub fn new(object_size: usize, initial_capacity: usize) -> Result<Self> {
        if !SLAB_SIZE_CLASSES.contains(&object_size) {
            return Err(RuntimeError::InvalidArgument(format!(
                "Invalid slab object size: {}. Valid sizes: {:?}",
                object_size, SLAB_SIZE_CLASSES
            )));
        }

        let template = SlabPage::new(object_size);
        let slots_per_page = template.num_slots;
        let pages_needed = if initial_capacity == 0 || slots_per_page == 0 {
            0
        } else {
            initial_capacity.div_ceil(slots_per_page)
        };

        let mut slab = Slab {
            object_size,
            pages: Vec::with_capacity(std::cmp::max(pages_needed, 1)),
            free_list: Vec::with_capacity(initial_capacity),
            allocated_count: 0,
            total_capacity: 0,
        };

        for _ in 0..pages_needed {
            slab.add_page();
        }

        Ok(slab)
    }

    fn add_page(&mut self) {
        let page = SlabPage::new(self.object_size);
        let page_idx = self.pages.len();
        let num_slots = page.num_slots;
        for slot in 0..num_slots {
            self.free_list.push((page_idx, slot));
        }
        self.total_capacity += num_slots;
        self.pages.push(page);
    }

    pub fn allocate(&mut self) -> Result<*mut u8> {
        if self.free_list.is_empty() {
            self.add_page();
        }

        let (page_idx, slot_idx) = self.free_list.pop().unwrap();
        self.allocated_count += 1;
        Ok(self.pages[page_idx].slot_ptr(slot_idx, self.object_size))
    }

    pub fn deallocate(&mut self, ptr: *mut u8) {
        for (page_idx, page) in self.pages.iter().enumerate() {
            if page.contains(ptr) {
                let slot_idx = page.slot_index(ptr, self.object_size);
                self.free_list.push((page_idx, slot_idx));
                self.allocated_count -= 1;
                return;
            }
        }
    }

    pub fn object_size(&self) -> usize {
        self.object_size
    }

    pub fn allocated_count(&self) -> usize {
        self.allocated_count
    }

    pub fn capacity(&self) -> usize {
        self.total_capacity
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_object_size() {
        let result = Slab::new(0, 10);
        assert!(result.is_err());
        let result = Slab::new(100, 10);
        assert!(result.is_err());
        let result = Slab::new(16, 10);
        assert!(result.is_err());
    }

    #[test]
    fn allocate_returns_valid_ptr() {
        let mut slab = Slab::new(32, 1).unwrap();
        let ptr = slab.allocate().unwrap();
        assert!(!ptr.is_null());
        unsafe {
            std::ptr::write(ptr, 0xAB);
            assert_eq!(*ptr, 0xAB);
        }
    }

    #[test]
    fn allocate_deallocate_cycle() {
        let mut slab = Slab::new(64, 1).unwrap();
        let ptr = slab.allocate().unwrap();
        assert_eq!(slab.allocated_count(), 1);
        slab.deallocate(ptr);
        assert_eq!(slab.allocated_count(), 0);
    }

    #[test]
    fn slot_reuse_after_deallocate() {
        let mut slab = Slab::new(32, 1).unwrap();
        let ptr1 = slab.allocate().unwrap();
        slab.deallocate(ptr1);
        let ptr2 = slab.allocate().unwrap();
        assert_eq!(ptr1, ptr2);
    }

    #[test]
    fn capacity_grows_with_allocations() {
        let mut slab = Slab::new(32, 1).unwrap();
        let initial_cap = slab.capacity();
        let slots_per_page = SLAB_PAGE_SIZE / 32;
        let mut ptrs = Vec::new();
        for _ in 0..(slots_per_page + 1) {
            ptrs.push(slab.allocate().unwrap());
        }
        assert!(slab.capacity() > initial_cap);
        for ptr in ptrs {
            slab.deallocate(ptr);
        }
    }

    #[test]
    fn allocate_one_page_without_growth() {
        let mut slab = Slab::new(32, 1).unwrap();
        let slots_per_page = SLAB_PAGE_SIZE / 32;
        let mut ptrs = Vec::new();
        for _ in 0..slots_per_page {
            ptrs.push(slab.allocate().unwrap());
        }
        assert_eq!(slab.allocated_count(), slots_per_page);
        assert_eq!(slab.capacity(), slots_per_page);
        for ptr in ptrs {
            slab.deallocate(ptr);
        }
    }

    #[test]
    fn allocated_count_tracking() {
        let mut slab = Slab::new(64, 1).unwrap();
        assert_eq!(slab.allocated_count(), 0);
        let p1 = slab.allocate().unwrap();
        assert_eq!(slab.allocated_count(), 1);
        let p2 = slab.allocate().unwrap();
        assert_eq!(slab.allocated_count(), 2);
        slab.deallocate(p1);
        assert_eq!(slab.allocated_count(), 1);
        slab.deallocate(p2);
        assert_eq!(slab.allocated_count(), 0);
    }

    #[test]
    fn object_size_accessor() {
        let slab = Slab::new(128, 1).unwrap();
        assert_eq!(slab.object_size(), 128);
    }

    #[test]
    fn deallocate_unknown_ptr_does_nothing() {
        let mut slab = Slab::new(32, 1).unwrap();
        let dummy = Box::into_raw(Box::new(0u8));
        slab.deallocate(dummy);
        assert_eq!(slab.allocated_count(), 0);
        unsafe {
            drop(Box::from_raw(dummy));
        }
    }

    #[test]
    fn write_to_slot_and_read_back() {
        let mut slab = Slab::new(256, 1).unwrap();
        let ptr = slab.allocate().unwrap();
        unsafe {
            let val_ptr = ptr as *mut u64;
            std::ptr::write(val_ptr, 0x1234567890ABCDEF);
            assert_eq!(std::ptr::read(val_ptr), 0x1234567890ABCDEF);
        }
        slab.deallocate(ptr);
    }

    #[test]
    fn multiple_pages_allocated() {
        let mut slab = Slab::new(4096, 1).unwrap();
        let p1 = slab.allocate().unwrap();
        let p2 = slab.allocate().unwrap();
        assert_ne!(p1, p2);
        slab.deallocate(p1);
        slab.deallocate(p2);
    }
}
