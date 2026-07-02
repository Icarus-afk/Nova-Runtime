use std::ops::{Deref, DerefMut};

pub struct ObjectPool<T> {
    pool: Vec<T>,
}

impl<T> ObjectPool<T> {
    pub fn new(initial_capacity: usize) -> Self {
        ObjectPool {
            pool: Vec::with_capacity(initial_capacity),
        }
    }

    pub fn acquire(&mut self) -> PoolGuard<'_, T>
    where
        T: Default,
    {
        let value = self.pool.pop().unwrap_or_default();
        PoolGuard {
            pool: self,
            value: Some(value),
        }
    }

    pub fn release(&mut self, obj: T) {
        self.pool.push(obj);
    }

    pub fn size(&self) -> usize {
        self.pool.len()
    }
}

pub struct PoolGuard<'a, T> {
    pool: &'a mut ObjectPool<T>,
    value: Option<T>,
}

impl<'a, T> Drop for PoolGuard<'a, T> {
    fn drop(&mut self) {
        if let Some(value) = self.value.take() {
            self.pool.release(value);
        }
    }
}

impl<'a, T> Deref for PoolGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.value.as_ref().expect("PoolGuard value already taken")
    }
}

impl<'a, T> DerefMut for PoolGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.value.as_mut().expect("PoolGuard value already taken")
    }
}
