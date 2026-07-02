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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_clean_page(id: u64) -> Page {
        Page::new(PageId::new(id))
    }

    fn make_dirty_page(id: u64) -> Page {
        let mut page = Page::new(PageId::new(id));
        page.mark_dirty();
        page
    }

    #[test]
    fn test_new_zero_capacity_defaults() {
        let cache = PageCache::new(0);
        assert!(cache.capacity() > 0);
    }

    #[test]
    fn test_get_empty_returns_none() {
        let cache = PageCache::new(16);
        let result = cache.get(PageId::new(1)).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_insert_clean_and_get() {
        let cache = PageCache::new(16);
        let page = make_clean_page(1);
        cache.insert(page).unwrap();
        let result = cache.get(PageId::new(1)).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().id, PageId::new(1));
    }

    #[test]
    fn test_insert_dirty_and_get() {
        let cache = PageCache::new(16);
        let page = make_dirty_page(1);
        cache.insert(page).unwrap();
        let result = cache.get(PageId::new(1)).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().id, PageId::new(1));
    }

    #[test]
    fn test_dirty_overrides_clean() {
        let cache = PageCache::new(16);
        let clean = make_clean_page(1);
        cache.insert(clean).unwrap();
        let dirty = make_dirty_page(1);
        cache.insert(dirty).unwrap();
        let result = cache.get(PageId::new(1)).unwrap().unwrap();
        assert!(result.is_dirty());
    }

    #[test]
    fn test_remove_clean() {
        let cache = PageCache::new(16);
        cache.insert(make_clean_page(1)).unwrap();
        let removed = cache.remove(PageId::new(1)).unwrap();
        assert!(removed.is_some());
        assert!(cache.get(PageId::new(1)).unwrap().is_none());
    }

    #[test]
    fn test_remove_dirty() {
        let cache = PageCache::new(16);
        cache.insert(make_dirty_page(1)).unwrap();
        let removed = cache.remove(PageId::new(1)).unwrap();
        assert!(removed.is_some());
        assert_eq!(cache.dirty_count(), 0);
    }

    #[test]
    fn test_remove_nonexistent() {
        let cache = PageCache::new(16);
        let removed = cache.remove(PageId::new(999)).unwrap();
        assert!(removed.is_none());
    }

    #[test]
    fn test_flush_dirty_pages() {
        let cache = PageCache::new(16);
        cache.insert(make_dirty_page(1)).unwrap();
        cache.insert(make_dirty_page(2)).unwrap();
        assert_eq!(cache.dirty_count(), 2);
        let flushed = cache.flush().unwrap();
        assert_eq!(flushed, 2);
        assert_eq!(cache.dirty_count(), 0);
    }

    #[test]
    fn test_flush_no_dirty() {
        let cache = PageCache::new(16);
        cache.insert(make_clean_page(1)).unwrap();
        let flushed = cache.flush().unwrap();
        assert_eq!(flushed, 0);
    }

    #[test]
    fn test_size_tracks_total_pages() {
        let cache = PageCache::new(16);
        assert_eq!(cache.size(), 0);
        cache.insert(make_clean_page(1)).unwrap();
        assert_eq!(cache.size(), 1);
        cache.insert(make_dirty_page(2)).unwrap();
        assert_eq!(cache.size(), 2);
        cache.remove(PageId::new(1)).unwrap();
        assert_eq!(cache.size(), 1);
    }

    #[test]
    fn test_capacity() {
        let cache = PageCache::new(100);
        assert_eq!(cache.capacity(), 100);
    }

    #[test]
    fn test_dirty_count() {
        let cache = PageCache::new(16);
        assert_eq!(cache.dirty_count(), 0);
        cache.insert(make_dirty_page(1)).unwrap();
        assert_eq!(cache.dirty_count(), 1);
        cache.flush().unwrap();
        assert_eq!(cache.dirty_count(), 0);
    }

    #[test]
    fn test_lru_eviction_of_clean_pages() {
        let cache = PageCache::new(2);
        cache.insert(make_clean_page(0)).unwrap();
        cache.insert(make_clean_page(1)).unwrap();
        // Insert third clean page — LRU should evict page 0 (oldest)
        cache.insert(make_clean_page(2)).unwrap();
        assert!(cache.get(PageId::new(0)).unwrap().is_none());
        assert!(cache.get(PageId::new(1)).unwrap().is_some());
        assert!(cache.get(PageId::new(2)).unwrap().is_some());
    }

    #[test]
    fn test_dirty_pages_not_evicted() {
        let cache = PageCache::new(2);
        cache.insert(make_dirty_page(0)).unwrap();
        cache.insert(make_dirty_page(1)).unwrap();
        // Dirty pages live in HashMap — no eviction even with small capacity
        cache.insert(make_dirty_page(2)).unwrap();
        assert!(cache.get(PageId::new(0)).unwrap().is_some());
        assert!(cache.get(PageId::new(1)).unwrap().is_some());
        assert!(cache.get(PageId::new(2)).unwrap().is_some());
    }

    #[test]
    fn test_writeback_called_on_flush() {
        let mut cache = PageCache::new(16);
        let flushed_id = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let fid = flushed_id.clone();
        cache.set_writeback(move |p: Page| {
            fid.store(p.id.value(), std::sync::atomic::Ordering::SeqCst);
            Ok(())
        });
        cache.insert(make_dirty_page(42)).unwrap();
        cache.flush().unwrap();
        assert_eq!(flushed_id.load(std::sync::atomic::Ordering::SeqCst), 42);
    }
}
