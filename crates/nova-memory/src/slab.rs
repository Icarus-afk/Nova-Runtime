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
        let page_size = (raw + object_size - 1) / object_size * object_size;
        let num_slots = page_size / object_size;
        let data = vec![0u8; page_size];
        SlabPage { data, num_slots }
    }

    fn slot_ptr(&self, slot: usize, object_size: usize) -> *mut u8 {
        unsafe { self.data.as_ptr().add(slot * object_size) as *mut u8 }
    }

    fn contains(&self, ptr: *const u8) -> bool {
        let start = self.data.as_ptr();
        let end = unsafe { start.add(self.data.len()) };
        ptr >= start && ptr < end
    }

    fn slot_index(&self, ptr: *const u8, object_size: usize) -> usize {
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
            (initial_capacity + slots_per_page - 1) / slots_per_page
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
