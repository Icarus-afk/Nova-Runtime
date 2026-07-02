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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acquire_returns_default_when_empty() {
        let mut pool = ObjectPool::<i32>::new(0);
        let val = pool.acquire();
        assert_eq!(*val, 0);
    }

    #[test]
    fn release_adds_to_pool() {
        let mut pool = ObjectPool::<i32>::new(0);
        pool.release(42);
        assert_eq!(pool.size(), 1);
    }

    #[test]
    fn acquire_reuses_released_object() {
        let mut pool = ObjectPool::<i32>::new(0);
        pool.release(42);
        let val = pool.acquire();
        assert_eq!(*val, 42);
    }

    #[test]
    fn pool_guard_auto_release_on_drop() {
        let mut pool = ObjectPool::<i32>::new(0);
        {
            let _guard = pool.acquire();
        }
        assert_eq!(pool.size(), 1);
    }

    #[test]
    fn pool_guard_deref() {
        let mut pool = ObjectPool::<i32>::new(0);
        let guard = pool.acquire();
        assert_eq!(*guard, 0);
    }

    #[test]
    fn pool_guard_deref_mut() {
        let mut pool = ObjectPool::<i32>::new(0);
        {
            let mut guard = pool.acquire();
            *guard = 100;
        }
        let guard = pool.acquire();
        assert_eq!(*guard, 100);
    }

    #[test]
    fn pool_size_tracking() {
        let mut pool = ObjectPool::<i32>::new(0);
        assert_eq!(pool.size(), 0);
        pool.release(1);
        assert_eq!(pool.size(), 1);
        pool.release(2);
        assert_eq!(pool.size(), 2);
        pool.acquire();
        assert_eq!(pool.size(), 1);
        pool.acquire();
        assert_eq!(pool.size(), 0);
    }

    #[test]
    fn pool_initial_capacity() {
        let pool = ObjectPool::<i32>::new(10);
        assert_eq!(pool.size(), 0);
    }

    #[test]
    fn pool_growth_on_empty_acquire() {
        let mut pool = ObjectPool::<String>::new(0);
        let guard = pool.acquire();
        assert_eq!(*guard, "");
    }

    #[test]
    fn acquire_and_release_many() {
        let mut pool = ObjectPool::<i32>::new(0);
        for i in 0..100 {
            pool.release(i);
        }
        assert_eq!(pool.size(), 100);
        for i in (0..100).rev() {
            let val = pool.acquire();
            assert_eq!(*val, i);
        }
        assert_eq!(pool.size(), 0);
    }
}
