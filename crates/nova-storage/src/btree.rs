use std::ops::Range;
use xxhash_rust::xxh3::xxh3_64;
use nova_core::types::*;
use nova_core::error::*;

const INTERNAL_NODE: u16 = 0;
const LEAF_NODE: u16 = 1;

const NODE_TYPE_OFF: usize = 0;
const COUNT_OFF: usize = 2;
const PARENT_OFF: usize = 4;
const NEXT_LEAF_OFF: usize = 12;
const PREV_LEAF_OFF: usize = 20;
const LEAF_ENTRIES_OFF: usize = 28;
const INT_ENTRIES_OFF: usize = 12;

const PAGE_DATA_SIZE: usize = 4096;
const LEAF_ENTRY_SIZE: usize = 23;
const INT_ENTRY_SIZE: usize = 20;

fn key_hash(key: &Key) -> u64 {
    xxh3_64(key.as_bytes())
}

fn read_u16(data: &[u8], off: usize) -> u16 {
    u16::from_le_bytes(data[off..off + 2].try_into().unwrap())
}

fn write_u16(data: &mut [u8], off: usize, val: u16) {
    data[off..off + 2].copy_from_slice(&val.to_le_bytes());
}

fn read_u64(data: &[u8], off: usize) -> u64 {
    u64::from_le_bytes(data[off..off + 8].try_into().unwrap())
}

fn write_u64(data: &mut [u8], off: usize, val: u64) {
    data[off..off + 8].copy_from_slice(&val.to_le_bytes());
}

#[derive(Clone, Debug)]
struct IntEntry {
    key_hash: u64,
    child_id: u64,
    key_off: u16,
    key_len: u16,
}

#[derive(Clone, Debug)]
struct LeafEntry {
    key_hash: u64,
    val_off: u16,
    val_len: u16,
    key_len: u16,
    flags: u8,
}

fn read_int_entry(data: &[u8], idx: usize) -> IntEntry {
    let entry_off = INT_ENTRIES_OFF + idx * INT_ENTRY_SIZE;
    IntEntry {
        key_hash: u64::from_le_bytes(data[entry_off..entry_off + 8].try_into().unwrap()),
        child_id: u64::from_le_bytes(data[entry_off + 8..entry_off + 16].try_into().unwrap()),
        key_off: u16::from_le_bytes(data[entry_off + 16..entry_off + 18].try_into().unwrap()),
        key_len: u16::from_le_bytes(data[entry_off + 18..entry_off + 20].try_into().unwrap()),
    }
}

fn read_leaf_entry(data: &[u8], idx: usize) -> LeafEntry {
    let entry_off = LEAF_ENTRIES_OFF + idx * LEAF_ENTRY_SIZE;
    LeafEntry {
        key_hash: u64::from_le_bytes(data[entry_off..entry_off + 8].try_into().unwrap()),
        val_off: u16::from_le_bytes(data[entry_off + 8..entry_off + 10].try_into().unwrap()),
        val_len: u16::from_le_bytes(data[entry_off + 10..entry_off + 12].try_into().unwrap()),
        key_len: u16::from_le_bytes(data[entry_off + 12..entry_off + 14].try_into().unwrap()),
        flags: data[entry_off + 22],
    }
}

fn read_key_from_data(data: &[u8], off: usize, len: usize) -> Key {
    Key::new(data[off..off + len].to_vec())
}

fn read_value_from_data(data: &[u8], off: usize, len: usize) -> Value {
    Value::new(data[off..off + len].to_vec())
}

fn get_int_child_index(data: &[u8], hash: u64) -> usize {
    let count = read_u16(data, COUNT_OFF) as usize;
    let mut idx = 0usize;
    for i in 0..count {
        let entry = read_int_entry(data, i);
        if entry.key_hash <= hash {
            idx = i + 1;
        } else {
            break;
        }
    }
    if idx > 0 {
        idx - 1
    } else {
        0
    }
}

pub struct BTree {
    root_id: std::cell::Cell<Option<PageId>>,
    order: usize,
}

impl BTree {
    pub fn new(order: usize) -> Self {
        BTree {
            root_id: std::cell::Cell::new(None),
            order: if order < 4 { 128 } else { order },
        }
    }

    pub fn set_root(&self, id: PageId) {
        self.root_id.set(Some(id));
    }

    pub fn get_root(&self) -> Option<PageId> {
        self.root_id.get()
    }

    pub fn get(&self, cache: &super::page_cache::PageCache, key: &Key) -> Result<Option<Value>> {
        let root_id = match self.root_id.get() {
            Some(id) => id,
            None => return Ok(None),
        };
        let page = cache.get(root_id)?.ok_or_else(|| RuntimeError::Internal("BTree root not found".into()))?;
        self.get_from_page(cache, &page.data, key)
    }

    fn get_from_page(&self, cache: &super::page_cache::PageCache, data: &[u8; 4096], key: &Key) -> Result<Option<Value>> {
        let node_type = read_u16(data, NODE_TYPE_OFF);
        if node_type == LEAF_NODE {
            let hash = key_hash(key);
            let count = read_u16(data, COUNT_OFF) as usize;
            for i in 0..count {
                let entry = read_leaf_entry(data, i);
                if entry.key_hash != hash {
                    continue;
                }
                if entry.key_len as usize != key.len() {
                    continue;
                }
                let stored_key = read_key_from_data(data, entry.val_off as usize - entry.key_len as usize, entry.key_len as usize);
                if stored_key.as_bytes() == key.as_bytes() {
                    if entry.flags & 0x01 != 0 {
                        return Ok(None);
                    }
                    return Ok(Some(read_value_from_data(data, entry.val_off as usize, entry.val_len as usize)));
                }
            }
            Ok(None)
        } else {
            let hash = key_hash(key);
            let idx = get_int_child_index(data, hash);
            let entry = read_int_entry(data, idx);
            let child_id = PageId::new(entry.child_id);
            let child_page = cache.get(child_id)?.ok_or_else(|| RuntimeError::Internal("BTree child not found".into()))?;
            self.get_from_page(cache, &child_page.data, key)
        }
    }

    pub fn insert(&self, cache: &super::page_cache::PageCache, key: Key, value: Value) -> Result<()> {
        let root_id = self.root_id.get();
        let root_id = match root_id {
            Some(id) => id,
            None => {
                let mut page = Page::new(PageId::new(1));
                write_u16(&mut page.data, NODE_TYPE_OFF, LEAF_NODE);
                write_u16(&mut page.data, COUNT_OFF, 0);
                write_u64(&mut page.data, PARENT_OFF, PageId::INVALID.value());
                write_u64(&mut page.data, NEXT_LEAF_OFF, PageId::INVALID.value());
                write_u64(&mut page.data, PREV_LEAF_OFF, PageId::INVALID.value());
                let page_id = PageId::new(1);
                page.id = page_id;
                page.mark_dirty();
                cache.insert(page)?;
                self.root_id.set(Some(page_id));
                page_id
            }
        };

        let result = self.insert_into(cache, root_id, key, value)?;
        if let Some((sep_key, new_child_id)) = result {
            let old_root_id = root_id;
            let new_root_id = PageId::new(allocate_page_id());
            let mut new_root = Page::new(new_root_id);
            write_u16(&mut new_root.data, NODE_TYPE_OFF, INTERNAL_NODE);
            write_u16(&mut new_root.data, COUNT_OFF, 1);
            write_u64(&mut new_root.data, PARENT_OFF, PageId::INVALID.value());
            let hash = key_hash(&sep_key);
            push_int_entry(&mut new_root.data, 0, hash, old_root_id.value(), &sep_key);
            push_int_entry(&mut new_root.data, 1, hash, new_child_id.value(), &sep_key);
            new_root.mark_dirty();
            cache.insert(new_root)?;

            if let Ok(Some(old_page)) = cache.get(old_root_id) {
                let mut old_page = old_page;
                write_u64(&mut old_page.data, PARENT_OFF, new_root_id.value());
                old_page.mark_dirty();
                cache.insert(old_page)?;
            }
            if let Ok(Some(new_page)) = cache.get(new_child_id) {
                let mut new_page = new_page;
                write_u64(&mut new_page.data, PARENT_OFF, new_root_id.value());
                new_page.mark_dirty();
                cache.insert(new_page)?;
            }

            self.root_id.set(Some(new_root_id));
        }
        Ok(())
    }

    fn insert_into(
        &self,
        cache: &super::page_cache::PageCache,
        page_id: PageId,
        key: Key,
        value: Value,
    ) -> Result<Option<(Key, PageId)>> {
        let page = cache.get(page_id)?.ok_or_else(|| RuntimeError::Internal("BTree page not found".into()))?;
        let is_leaf = read_u16(&page.data, NODE_TYPE_OFF) == LEAF_NODE;
        if is_leaf {
            self.insert_into_leaf(cache, page, key, value)
        } else {
            let hash = key_hash(&key);
            let idx = get_int_child_index(&page.data, hash);
            let entry = read_int_entry(&page.data, idx);
            let child_id = PageId::new(entry.child_id);
            let result = self.insert_into(cache, child_id, key, value)?;
            if let Some((sep_key, new_child_id)) = result {
                let updated = cache.get(page_id)?.ok_or_else(|| RuntimeError::Internal("BTree page gone".into()))?;
                self.insert_into_internal(cache, updated, sep_key, new_child_id)
            } else {
                Ok(None)
            }
        }
    }

    fn insert_into_leaf(
        &self,
        cache: &super::page_cache::PageCache,
        mut page: Page,
        key: Key,
        value: Value,
    ) -> Result<Option<(Key, PageId)>> {
        let count = read_u16(&page.data, COUNT_OFF) as usize;
        let order = self.order;

        if count < order * 2 {
            insert_leaf_entry(&mut page.data, count, &key, &value)?;
            write_u16(&mut page.data, COUNT_OFF, (count + 1) as u16);
            page.mark_dirty();
            cache.insert(page)?;
            return Ok(None);
        }

        let mid = order;
        let mut entries: Vec<(Key, Value, u8)> = Vec::new();
        for i in 0..count {
            let entry = read_leaf_entry(&page.data, i);
            let k = read_key_from_data(&page.data, entry.val_off as usize - entry.key_len as usize, entry.key_len as usize);
            let v = read_value_from_data(&page.data, entry.val_off as usize, entry.val_len as usize);
            entries.push((k, v, entry.flags));
        }

        let insert_hash = key_hash(&key);
        let insert_pos = entries.iter().position(|(k, _, _)| key_hash(k) >= insert_hash)
            .unwrap_or(entries.len());
        entries.insert(insert_pos, (key.clone(), value.clone(), 0));

        let split_key = entries[mid].0.clone();
        let new_page_id = PageId::new(allocate_page_id());
        let mut new_page = Page::new(new_page_id);
        write_u16(&mut new_page.data, NODE_TYPE_OFF, LEAF_NODE);
        write_u16(&mut new_page.data, COUNT_OFF, 0);
        write_u64(&mut new_page.data, PARENT_OFF, read_u64(&page.data, PARENT_OFF));
        write_u64(&mut new_page.data, NEXT_LEAF_OFF, read_u64(&page.data, NEXT_LEAF_OFF));
        write_u64(&mut new_page.data, PREV_LEAF_OFF, page.id.value());

        let mut new_count = 0usize;
        for i in mid..entries.len() {
            insert_leaf_entry(&mut new_page.data, new_count, &entries[i].0, &entries[i].1)?;
            if entries[i].2 & 0x01 != 0 {
                let entry_off = LEAF_ENTRIES_OFF + new_count * LEAF_ENTRY_SIZE;
                new_page.data[entry_off + 22] = 0x01;
            }
            new_count += 1;
        }
        write_u16(&mut new_page.data, COUNT_OFF, new_count as u16);

        write_u64(&mut page.data, NEXT_LEAF_OFF, new_page_id.value());
        write_u16(&mut page.data, COUNT_OFF, mid as u16);

        let old_next = read_u64(&page.data, NEXT_LEAF_OFF);
        if old_next == new_page_id.value() {
            write_u64(&mut page.data, NEXT_LEAF_OFF, new_page_id.value());
        }

        let new_next_leaf = read_u64(&new_page.data, NEXT_LEAF_OFF);

        page.mark_dirty();
        new_page.mark_dirty();
        cache.insert(page)?;
        cache.insert(new_page)?;

        if new_next_leaf != PageId::INVALID.value() {
            if let Ok(Some(mut next_page)) = cache.get(PageId::new(new_next_leaf)) {
                if next_page.id != PageId::INVALID {
                    write_u64(&mut next_page.data, PREV_LEAF_OFF, new_page_id.value());
                    next_page.mark_dirty();
                    cache.insert(next_page)?;
                }
            }
        }

        Ok(Some((split_key, new_page_id)))
    }

    fn insert_into_internal(
        &self,
        cache: &super::page_cache::PageCache,
        mut page: Page,
        sep_key: Key,
        new_child_id: PageId,
    ) -> Result<Option<(Key, PageId)>> {
        let count = read_u16(&page.data, COUNT_OFF) as usize;
        let order = self.order;

        if count < order * 2 {
            push_int_entry(&mut page.data, count, key_hash(&sep_key), new_child_id.value(), &sep_key);
            write_u16(&mut page.data, COUNT_OFF, (count + 1) as u16);
            page.mark_dirty();
            cache.insert(page)?;
            return Ok(None);
        }

        let mut entries: Vec<(u64, u64, Key)> = Vec::new();
        for i in 0..count {
            let entry = read_int_entry(&page.data, i);
            let k = read_key_from_data(&page.data, entry.key_off as usize, entry.key_len as usize);
            entries.push((entry.key_hash, entry.child_id, k));
        }
        entries.push((key_hash(&sep_key), new_child_id.value(), sep_key.clone()));
        entries.sort_by(|a, b| a.2.as_bytes().cmp(b.2.as_bytes()));

        let mid = order;
        let promoted = entries[mid].clone();
        let new_page_id = PageId::new(allocate_page_id());
        let mut new_page = Page::new(new_page_id);
        write_u16(&mut new_page.data, NODE_TYPE_OFF, INTERNAL_NODE);
        write_u16(&mut new_page.data, COUNT_OFF, 0);
        write_u64(&mut new_page.data, PARENT_OFF, read_u64(&page.data, PARENT_OFF));

        let mut new_count = 0usize;
        for i in mid + 1..entries.len() {
            push_int_entry(&mut new_page.data, new_count, entries[i].0, entries[i].1, &entries[i].2);
            new_count += 1;
        }
        write_u16(&mut new_page.data, COUNT_OFF, new_count as u16);

        write_u16(&mut page.data, COUNT_OFF, mid as u16);
        page.mark_dirty();
        new_page.mark_dirty();

        let child_ids: Vec<u64> = (0..new_count)
            .map(|i| read_int_entry(&new_page.data, i).child_id)
            .collect();

        cache.insert(page)?;
        cache.insert(new_page)?;

        for child_id in child_ids {
            if let Ok(Some(mut child)) = cache.get(PageId::new(child_id)) {
                write_u64(&mut child.data, PARENT_OFF, new_page_id.value());
                child.mark_dirty();
                cache.insert(child)?;
            }
        }

        Ok(Some((promoted.2, new_page_id)))
    }

    pub fn delete(&self, cache: &super::page_cache::PageCache, key: &Key) -> Result<bool> {
        let root_id = match self.root_id.get() {
            Some(id) => id,
            None => return Ok(false),
        };
        self.delete_from(cache, root_id, key)
    }

    fn delete_from(&self, cache: &super::page_cache::PageCache, page_id: PageId, key: &Key) -> Result<bool> {
        let page = cache.get(page_id)?.ok_or_else(|| RuntimeError::Internal("BTree page not found".into()))?;
        let is_leaf = read_u16(&page.data, NODE_TYPE_OFF) == LEAF_NODE;
        if is_leaf {
            let hash = key_hash(key);
            let count = read_u16(&page.data, COUNT_OFF) as usize;
            for i in 0..count {
                let entry = read_leaf_entry(&page.data, i);
                if entry.key_hash != hash {
                    continue;
                }
                let stored_key = read_key_from_data(&page.data, entry.val_off as usize - entry.key_len as usize, entry.key_len as usize);
                if stored_key.as_bytes() == key.as_bytes() {
                    let mut page = page;
                    let entry_off = LEAF_ENTRIES_OFF + i * LEAF_ENTRY_SIZE;
                    page.data[entry_off + 22] |= 0x01;
                    page.mark_dirty();
                    cache.insert(page)?;
                    return Ok(true);
                }
            }
            Ok(false)
        } else {
            let hash = key_hash(key);
            let idx = get_int_child_index(&page.data, hash);
            let entry = read_int_entry(&page.data, idx);
            let child_id = PageId::new(entry.child_id);
            self.delete_from(cache, child_id, key)
        }
    }

    pub fn scan(&self, cache: &super::page_cache::PageCache, range: Range<Key>) -> Result<Vec<(Key, Value)>> {
        let root_id = match self.root_id.get() {
            Some(id) => id,
            None => return Ok(vec![]),
        };
        let mut results = Vec::new();
        let start_leaf = self.find_start_leaf(cache, root_id, &range.start)?;
        if let Some(start_id) = start_leaf {
            let mut current_id = start_id;
            loop {
                let page = cache.get(current_id)?.ok_or_else(|| RuntimeError::Internal("leaf not found".into()))?;
                let count = read_u16(&page.data, COUNT_OFF) as usize;
                for i in 0..count {
                    let entry = read_leaf_entry(&page.data, i);
                    if entry.flags & 0x01 != 0 {
                        continue;
                    }
                    let k = read_key_from_data(&page.data, entry.val_off as usize - entry.key_len as usize, entry.key_len as usize);
                    if k.as_bytes() < range.start.as_bytes() {
                        continue;
                    }
                    if k.as_bytes() >= range.end.as_bytes() {
                        return Ok(results);
                    }
                    let v = read_value_from_data(&page.data, entry.val_off as usize, entry.val_len as usize);
                    results.push((k, v));
                }
                let next_id = read_u64(&page.data, NEXT_LEAF_OFF);
                if next_id == PageId::INVALID.value() {
                    break;
                }
                current_id = PageId::new(next_id);
            }
        }
        Ok(results)
    }

    fn find_start_leaf(&self, cache: &super::page_cache::PageCache, page_id: PageId, key: &Key) -> Result<Option<PageId>> {
        let page = cache.get(page_id)?.ok_or_else(|| RuntimeError::Internal("page not found".into()))?;
        let is_leaf = read_u16(&page.data, NODE_TYPE_OFF) == LEAF_NODE;
        if is_leaf {
            return Ok(Some(page_id));
        }
        let hash = key_hash(key);
        let idx = get_int_child_index(&page.data, hash);
        let entry = read_int_entry(&page.data, idx);
        self.find_start_leaf(cache, PageId::new(entry.child_id), key)
    }
}

fn insert_leaf_entry(data: &mut [u8], idx: usize, key: &Key, value: &Value) -> Result<()> {
    let hash = key_hash(key);
    let key_bytes = key.as_bytes();
    let val_bytes = value.as_bytes();
    let entry_off = LEAF_ENTRIES_OFF + idx * LEAF_ENTRY_SIZE;
    let entry_size = key_bytes.len() + val_bytes.len();
    let count = read_u16(data, COUNT_OFF) as usize;
    let mut cumulative: usize = entry_size;
    for i in 0..count {
        let e = read_leaf_entry(data, i);
        cumulative += e.key_len as usize + e.val_len as usize;
    }
    cumulative -= entry_size;
    let data_end = PAGE_DATA_SIZE;
    let val_off = data_end - cumulative - val_bytes.len();
    let key_off = val_off - key_bytes.len();
    data[key_off..key_off + key_bytes.len()].copy_from_slice(key_bytes);
    data[val_off..val_off + val_bytes.len()].copy_from_slice(val_bytes);
    data[entry_off..entry_off + 8].copy_from_slice(&hash.to_le_bytes());
    data[entry_off + 8..entry_off + 10].copy_from_slice(&(val_off as u16).to_le_bytes());
    data[entry_off + 10..entry_off + 12].copy_from_slice(&(val_bytes.len() as u16).to_le_bytes());
    data[entry_off + 12..entry_off + 14].copy_from_slice(&(key_bytes.len() as u16).to_le_bytes());
    data[entry_off + 14..entry_off + 22].copy_from_slice(&0u64.to_le_bytes());
    data[entry_off + 22] = 0;
    Ok(())
}

fn push_int_entry(data: &mut [u8], idx: usize, key_hash: u64, child_id: u64, key: &Key) {
    let entry_off = INT_ENTRIES_OFF + idx * INT_ENTRY_SIZE;
    let key_bytes = key.as_bytes();
    let data_off = PAGE_DATA_SIZE - 1 - idx * (key_bytes.len() + 2) - key_bytes.len();
    let key_off = data_off;
    data[key_off..key_off + key_bytes.len()].copy_from_slice(key_bytes);
    data[entry_off..entry_off + 8].copy_from_slice(&key_hash.to_le_bytes());
    data[entry_off + 8..entry_off + 16].copy_from_slice(&child_id.to_le_bytes());
    data[entry_off + 16..entry_off + 18].copy_from_slice(&(key_off as u16).to_le_bytes());
    data[entry_off + 18..entry_off + 20].copy_from_slice(&(key_bytes.len() as u16).to_le_bytes());
}

static NEXT_ALLOC_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1000);

fn allocate_page_id() -> u64 {
    NEXT_ALLOC_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::page_cache::PageCache;

    fn setup() -> (BTree, PageCache) {
        let btree = BTree::new(4);
        let cache = PageCache::new(1024);
        (btree, cache)
    }

    #[test]
    fn test_btree_new_empty() {
        let btree = BTree::new(4);
        assert!(btree.get_root().is_none());
    }

    #[test]
    fn test_btree_insert_and_get() {
        let (btree, cache) = setup();
        btree.insert(&cache, Key::from("hello"), Value::new(b"world".to_vec())).unwrap();
        let result = btree.get(&cache, &Key::from("hello")).unwrap();
        assert_eq!(result, Some(Value::new(b"world".to_vec())));
    }

    #[test]
    fn test_btree_get_nonexistent() {
        let (btree, cache) = setup();
        let result = btree.get(&cache, &Key::from("missing")).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_btree_get_empty_tree() {
        let btree = BTree::new(4);
        let cache = PageCache::new(1024);
        let result = btree.get(&cache, &Key::from("anything")).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_btree_delete_marks_tombstone() {
        let (btree, cache) = setup();
        btree.insert(&cache, Key::from("hello"), Value::new(b"world".to_vec())).unwrap();
        let deleted = btree.delete(&cache, &Key::from("hello")).unwrap();
        assert!(deleted);
        let result = btree.get(&cache, &Key::from("hello")).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_btree_delete_nonexistent() {
        let (btree, cache) = setup();
        let deleted = btree.delete(&cache, &Key::from("missing")).unwrap();
        assert!(!deleted);
    }

    #[test]
    fn test_btree_delete_empty_tree() {
        let btree = BTree::new(4);
        let cache = PageCache::new(1024);
        let deleted = btree.delete(&cache, &Key::from("anything")).unwrap();
        assert!(!deleted);
    }

    #[test]
    fn test_btree_update_overwrites() {
        let (btree, cache) = setup();
        btree.insert(&cache, Key::from("key"), Value::new(b"val1".to_vec())).unwrap();
        btree.insert(&cache, Key::from("key"), Value::new(b"val2".to_vec())).unwrap();
        let result = btree.get(&cache, &Key::from("key")).unwrap();
        assert_eq!(result, Some(Value::new(b"val2".to_vec())));
    }

    #[test]
    fn test_btree_scan_range() {
        let (btree, cache) = setup();
        for (k, v) in [
            (Key::from("a"), Value::new(b"1".to_vec())),
            (Key::from("b"), Value::new(b"2".to_vec())),
            (Key::from("c"), Value::new(b"3".to_vec())),
            (Key::from("d"), Value::new(b"4".to_vec())),
            (Key::from("e"), Value::new(b"5".to_vec())),
        ] {
            btree.insert(&cache, k, v).unwrap();
        }
        let results = btree.scan(&cache, Key::from("b")..Key::from("d")).unwrap();
        assert_eq!(results.len(), 2);
        for (k, _) in &results {
            assert!(k.as_bytes() >= b"b" && k.as_bytes() < b"d");
        }
    }

    #[test]
    fn test_btree_scan_empty_tree() {
        let (btree, cache) = setup();
        let results = btree.scan(&cache, Key::from("a")..Key::from("z")).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_btree_scan_excludes_tombstones() {
        let (btree, cache) = setup();
        btree.insert(&cache, Key::from("a"), Value::new(b"1".to_vec())).unwrap();
        btree.insert(&cache, Key::from("b"), Value::new(b"2".to_vec())).unwrap();
        btree.insert(&cache, Key::from("c"), Value::new(b"3".to_vec())).unwrap();
        btree.delete(&cache, &Key::from("b")).unwrap();
        let results = btree.scan(&cache, Key::from("a")..Key::from("d")).unwrap();
        assert_eq!(results.len(), 2);
        for (k, _) in &results {
            assert_ne!(k.as_bytes(), b"b");
        }
    }

    #[test]
    fn test_btree_multiple_keys_no_split() {
        let (btree, cache) = setup();
        for i in 0..7 {
            let key = Key::new(vec![i]);
            let value = Value::new(vec![i + 100]);
            btree.insert(&cache, key.clone(), value.clone()).unwrap();
            let result = btree.get(&cache, &key).unwrap();
            assert_eq!(result, Some(value));
        }
    }

    #[test]
    fn test_btree_insert_get_large_value() {
        let (btree, cache) = setup();
        let large_val = Value::new(vec![0xAB; 2000]);
        btree.insert(&cache, Key::from("large"), large_val.clone()).unwrap();
        let result = btree.get(&cache, &Key::from("large")).unwrap();
        assert_eq!(result, Some(large_val));
    }

    #[test]
    fn test_btree_get_after_delete_then_reinsert() {
        let (btree, cache) = setup();
        btree.insert(&cache, Key::from("x"), Value::new(b"first".to_vec())).unwrap();
        btree.delete(&cache, &Key::from("x")).unwrap();
        assert!(btree.get(&cache, &Key::from("x")).unwrap().is_none());
        btree.insert(&cache, Key::from("x"), Value::new(b"second".to_vec())).unwrap();
        let result = btree.get(&cache, &Key::from("x")).unwrap();
        assert_eq!(result, Some(Value::new(b"second".to_vec())));
    }

    #[test]
    fn test_btree_binary_key_roundtrip() {
        let (btree, cache) = setup();
        let key = Key::new(vec![0x00, 0xFF, 0xAB, 0xCD]);
        let value = Value::new(vec![0xDE, 0xAD, 0xBE, 0xEF]);
        btree.insert(&cache, key.clone(), value.clone()).unwrap();
        let result = btree.get(&cache, &key).unwrap();
        assert_eq!(result, Some(value));
    }

    #[test]
    fn test_btree_single_key_then_delete_then_get() {
        let (btree, cache) = setup();
        btree.insert(&cache, Key::from("only"), Value::new(b"one".to_vec())).unwrap();
        assert!(btree.get(&cache, &Key::from("only")).unwrap().is_some());
        btree.delete(&cache, &Key::from("only")).unwrap();
        assert!(btree.get(&cache, &Key::from("only")).unwrap().is_none());
    }

    #[test]
    fn test_btree_scan_exact_boundary() {
        let (btree, cache) = setup();
        btree.insert(&cache, Key::from("a"), Value::new(b"1".to_vec())).unwrap();
        btree.insert(&cache, Key::from("b"), Value::new(b"2".to_vec())).unwrap();
        btree.insert(&cache, Key::from("c"), Value::new(b"3".to_vec())).unwrap();
        let results = btree.scan(&cache, Key::from("a")..Key::from("b")).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, Key::from("a"));
    }

    #[test]
    fn test_btree_set_root() {
        let btree = BTree::new(4);
        assert!(btree.get_root().is_none());
        btree.set_root(PageId::new(42));
        assert_eq!(btree.get_root(), Some(PageId::new(42)));
    }
}
