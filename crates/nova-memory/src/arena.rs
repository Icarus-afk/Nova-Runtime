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
        let end = unsafe { start.add(self.capacity) };
        ptr >= start && ptr < end
    }
}
