use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::Arc;
use lru::LruCache;
use parking_lot::RwLock;
use nova_core::types::*;
use nova_core::error::*;

struct PageCacheInner {
    clean: LruCache<PageId, Page>,
    dirty: HashMap<PageId, Page>,
    capacity: usize,
}

pub struct PageCache {
    inner: RwLock<PageCacheInner>,
    writeback: Option<Arc<dyn Fn(Page) -> Result<()> + Send + Sync>>,
}

impl PageCache {
    pub fn new(capacity: usize) -> Self {
        let cap = if capacity == 0 { 1024 } else { capacity };
        PageCache {
            inner: RwLock::new(PageCacheInner {
                clean: LruCache::new(NonZeroUsize::new(cap).unwrap_or(NonZeroUsize::new(1024).unwrap())),
                dirty: HashMap::new(),
                capacity: cap,
            }),
            writeback: None,
        }
    }

    pub fn set_writeback<F>(&mut self, cb: F)
    where
        F: Fn(Page) -> Result<()> + Send + Sync + 'static,
    {
        self.writeback = Some(Arc::new(cb));
    }

    pub fn get(&self, id: PageId) -> Result<Option<Page>> {
        let inner = self.inner.read();
        if let Some(page) = inner.dirty.get(&id) {
            return Ok(Some(page.clone()));
        }
        match inner.clean.peek(&id) {
            Some(page) => Ok(Some(page.clone())),
            None => Ok(None),
        }
    }

    pub fn insert(&self, page: Page) -> Result<()> {
        let mut inner = self.inner.write();
        let id = page.id;
        if page.is_dirty() {
            inner.dirty.insert(id, page);
        } else {
            inner.clean.put(id, page);
        }
        Ok(())
    }

    pub fn remove(&self, id: PageId) -> Result<Option<Page>> {
        let mut inner = self.inner.write();
        if let Some(page) = inner.dirty.remove(&id) {
            return Ok(Some(page));
        }
        Ok(inner.clean.pop(&id))
    }

    pub fn flush(&self) -> Result<usize> {
        let mut inner = self.inner.write();
        let dirty_pages: Vec<Page> = inner.dirty.drain().map(|(_, p)| p).collect();
        let count = dirty_pages.len();
        if let Some(ref wb) = self.writeback {
            for page in dirty_pages {
                let mut p = page;
                p.clear_dirty();
                wb(p)?;
            }
        }
        Ok(count)
    }

    pub fn size(&self) -> usize {
        let inner = self.inner.read();
        inner.clean.len() + inner.dirty.len()
    }

    pub fn capacity(&self) -> usize {
        let inner = self.inner.read();
        inner.capacity
    }

    pub fn dirty_count(&self) -> usize {
        let inner = self.inner.read();
        inner.dirty.len()
    }
}
