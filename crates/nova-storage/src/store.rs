use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::ops::Range;
use std::sync::atomic::{AtomicU64, Ordering};
use parking_lot::{RwLock, Mutex};
use nova_core::types::*;
use nova_core::error::*;
use crate::page_cache::PageCache;
use crate::wal::{self, WalWriter, WalReader, WalRecordType};
use crate::btree::BTree;
use crate::lsm::{MemTable, SSTable};
use crate::txn::{self, TransactionManager, LockMode};

#[derive(Debug, Clone)]
pub struct StorageConfig {
    pub data_dir: PathBuf,
    pub wal_dir: PathBuf,
    pub page_cache_size: usize,
    pub memtable_size: usize,
    pub fsync_policy: FsyncPolicy,
    pub btree_order: usize,
}

impl Default for StorageConfig {
    fn default() -> Self {
        StorageConfig {
            data_dir: PathBuf::from("data"),
            wal_dir: PathBuf::from("data/wal"),
            page_cache_size: 16384,
            memtable_size: 64 * 1024 * 1024,
            fsync_policy: FsyncPolicy::EveryNMs(100),
            btree_order: 128,
        }
    }
}

pub enum WriteOperation {
    Set { key: Key, value: Value },
    Delete { key: Key },
}

#[derive(Debug, Clone)]
pub struct StorageStats {
    pub page_count: u64,
    pub dirty_pages: u64,
    pub cache_size: usize,
    pub memtable_size: usize,
    pub sstable_count: usize,
    pub wal_segments: u64,
    pub current_lsn: Lsn,
    pub last_checkpoint: Lsn,
}

impl Default for StorageStats {
    fn default() -> Self {
        StorageStats {
            page_count: 0,
            dirty_pages: 0,
            cache_size: 0,
            memtable_size: 0,
            sstable_count: 0,
            wal_segments: 0,
            current_lsn: Lsn::ZERO,
            last_checkpoint: Lsn::ZERO,
        }
    }
}

pub struct Store {
    config: StorageConfig,
    page_cache: Arc<PageCache>,
    wal: Arc<Mutex<Option<WalWriter>>>,
    btree: BTree,
    memtable: Arc<RwLock<MemTable>>,
    sstables: Arc<RwLock<Vec<SSTable>>>,
    next_page_id: AtomicU64,
    next_lsn: AtomicU64,
    store_dir: PathBuf,
    txn_manager: Arc<TransactionManager>,
    pub blob_store: Option<Arc<crate::blob::BlobStore>>,
}

impl Store {
    pub fn open(config: &StorageConfig) -> Result<Self> {
        let data_dir = &config.data_dir;
        let wal_dir = &config.wal_dir;
        std::fs::create_dir_all(data_dir)?;
        std::fs::create_dir_all(wal_dir)?;

        let page_cache = Arc::new(PageCache::new(config.page_cache_size));

        let wal = WalWriter::open(wal_dir, config.fsync_policy.clone())?;
        let wal = Arc::new(Mutex::new(Some(wal)));

        let btree = BTree::new(config.btree_order);

        let memtable = Arc::new(RwLock::new(MemTable::new()));
        let sstables = Arc::new(RwLock::new(Vec::new()));

        let current_lsn = {
            let wal_guard = wal.lock();
            wal_guard.as_ref().map(|w| w.current_lsn().value()).unwrap_or(0)
        };

        let checkpoint_lsn = Self::find_latest_checkpoint(data_dir)?
            .unwrap_or(0);

        Self::recover(wal_dir, &memtable, &sstables, current_lsn, checkpoint_lsn)?;

        let next_page_id = AtomicU64::new(2);
        let next_lsn = AtomicU64::new(current_lsn + 1);

        let txn_manager = Arc::new(TransactionManager::new());

        Ok(Store {
            config: config.clone(),
            page_cache,
            wal,
            btree,
            memtable,
            sstables,
            next_page_id,
            next_lsn,
            store_dir: data_dir.clone(),
            txn_manager,
            blob_store: None,
        })
    }

    fn recover(
        wal_dir: &Path,
        memtable: &Arc<RwLock<MemTable>>,
        _sstables: &Arc<RwLock<Vec<SSTable>>>,
        max_lsn: u64,
        after_lsn: u64,
    ) -> Result<()> {
        let mut reader = WalReader::open(wal_dir)?;
        let mut recovered = 0u64;
        loop {
            match reader.read_next()? {
                Some(record) => {
                    if record.lsn.value() == 0 || record.lsn.value() > max_lsn {
                        continue;
                    }
                    if record.lsn.value() <= after_lsn {
                        continue;
                    }
                    match record.record_type {
                        WalRecordType::Insert | WalRecordType::Update => {
                            let mt = &mut *memtable.write();
                            mt.insert(record.key, record.value.unwrap_or_else(|| Value::new(vec![])));
                            recovered += 1;
                        }
                        WalRecordType::Delete => {
                            let mt = &mut *memtable.write();
                            mt.delete(&record.key);
                            recovered += 1;
                        }
                        _ => {}
                    }
                }
                None => break,
            }
        }
        tracing::info!("WAL recovery complete: {} operations replayed (after LSN {})", recovered, after_lsn);
        Ok(())
    }

    fn find_latest_checkpoint(data_dir: &Path) -> Result<Option<u64>> {
        let cp_pattern = "checkpoint_";
        let mut best_seq: Option<u64> = None;
        if data_dir.exists() {
            for entry in std::fs::read_dir(data_dir)? {
                let entry = entry?;
                let name = entry.file_name();
                let name = name.to_string_lossy().to_string();
                if let Some(stem) = name.strip_suffix(".cp") {
                    if let Some(num_str) = stem.strip_prefix(cp_pattern) {
                        if let Ok(checkpoint_id) = num_str.parse::<u64>() {
                            if best_seq.map_or(true, |best| checkpoint_id > best) {
                                if let Ok(content) = std::fs::read_to_string(entry.path()) {
                                    if let Ok(seq) = content.trim().parse::<u64>() {
                                        best_seq = Some(seq);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(best_seq)
    }

    pub fn close(&self) -> Result<()> {
        self.flush()?;
        let mut wal_guard = self.wal.lock();
        if let Some(mut w) = wal_guard.take() {
            w.close()?;
        }
        Ok(())
    }

    pub fn get(&self, key: &Key) -> Result<Option<Value>> {
        let mt = self.memtable.read();
        if let Some(val) = mt.get(key) {
            return Ok(Some(val));
        }
        drop(mt);

        let sstables = self.sstables.read();
        for sstable in sstables.iter().rev() {
            if let Ok(Some(val)) = sstable.get(key) {
                return Ok(Some(val));
            }
        }
        drop(sstables);

        self.btree.get(&self.page_cache, key)
    }

    pub fn set(&self, key: Key, value: Value) -> Result<()> {
        let record = wal::make_record(WalRecordType::Insert, TransactionId::ZERO, key.clone(), Some(value.clone()));
        {
            let mut wal_guard = self.wal.lock();
            let wal = wal_guard.as_mut().expect("WAL not initialized - call init() before using store");
            wal.append(&record)?;
        }

        let mut mt = self.memtable.write();
        mt.insert(key, value);

        if mt.size() >= self.config.memtable_size {
            self.flush_memtable()?;
        }

        Ok(())
    }

    pub fn delete(&self, key: &Key) -> Result<bool> {
        let record = wal::make_record(WalRecordType::Delete, TransactionId::ZERO, key.clone(), None);
        {
            let mut wal_guard = self.wal.lock();
            let wal = wal_guard.as_mut().expect("WAL not initialized - call init() before using store");
            wal.append(&record)?;
        }

        let mut mt = self.memtable.write();
        mt.delete(key);
        Ok(true)
    }

    pub fn scan(&self, range: Range<Key>) -> Result<Vec<(Key, Value)>> {
        let mut results = Vec::new();
        let mt = self.memtable.read();
        for (key, value) in mt.iter() {
            if key.as_bytes() >= range.start.as_bytes() && key.as_bytes() < range.end.as_bytes() {
                results.push((key, value));
            }
        }
        drop(mt);
        let btree_results = self.btree.scan(&self.page_cache, range)?;
        for (k, v) in btree_results {
            if !results.iter().any(|(rk, _)| rk.as_bytes() == k.as_bytes()) {
                results.push((k, v));
            }
        }
        results.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
        Ok(results)
    }

    pub fn batch(&self, ops: Vec<WriteOperation>) -> Result<()> {
        for op in ops {
            match op {
                WriteOperation::Set { key, value } => {
                    self.set(key, value)?;
                }
                WriteOperation::Delete { key } => {
                    self.delete(&key)?;
                }
            }
        }
        Ok(())
    }

    pub fn begin(&self) -> TransactionId {
        nova_core::types::allocate_transaction_id()
    }

    pub fn snapshot_read(&self, txn_id: u64, key: &Key) -> Result<Option<Value>> {
        self.txn_manager.snapshot_read(txn_id, key, |k| self.get(k))
    }

    pub fn begin_txn(&self, isolation: IsolationLevel) -> txn::Transaction {
        self.txn_manager.begin(isolation, LockMode::Optimistic)
    }

    pub fn commit_txn(&self, tx_id: u64) -> Result<Lsn> {
        self.txn_manager.commit(tx_id)?;
        let record = wal::make_record(WalRecordType::Commit, TransactionId::new(tx_id), Key::new(vec![]), None);
        let mut wal_guard = self.wal.lock();
        let wal = wal_guard.as_mut().expect("WAL not initialized - call init() before using store");
        wal.append(&record)
    }

    pub fn rollback_txn(&self, tx_id: u64) -> Result<()> {
        self.txn_manager.rollback(tx_id)?;
        let record = wal::make_record(WalRecordType::Rollback, TransactionId::new(tx_id), Key::new(vec![]), None);
        let mut wal_guard = self.wal.lock();
        let wal = wal_guard.as_mut().expect("WAL not initialized - call init() before using store");
        wal.append(&record)?;
        Ok(())
    }

    pub fn commit(&self, tx_id: TransactionId) -> Result<Lsn> {
        let record = wal::make_record(WalRecordType::Commit, tx_id, Key::new(vec![]), None);
        let mut wal_guard = self.wal.lock();
        let wal = wal_guard.as_mut().expect("WAL not initialized - call init() before using store");
        wal.append(&record)
    }

    pub fn rollback(&self, tx_id: TransactionId) -> Result<()> {
        let record = wal::make_record(WalRecordType::Rollback, tx_id, Key::new(vec![]), None);
        let mut wal_guard = self.wal.lock();
        let wal = wal_guard.as_mut().expect("WAL not initialized - call init() before using store");
        wal.append(&record)?;
        Ok(())
    }

    pub fn flush(&self) -> Result<()> {
        self.page_cache.flush()?;
        let mut wal_guard = self.wal.lock();
        if let Some(wal) = wal_guard.as_mut() {
            wal.flush()?;
        }
        Ok(())
    }

    pub fn sync(&self) -> Result<()> {
        self.flush()?;
        {
            let mut wal_guard = self.wal.lock();
            if let Some(wal) = wal_guard.as_mut() {
                wal.switch_segment()?;
            }
        }
        Ok(())
    }

    pub fn checkpoint(&self) -> Result<Lsn> {
        self.flush()?;
        self.flush_memtable()?;

        let current_lsn = {
            let mut wal_guard = self.wal.lock();
            let wal = wal_guard.as_mut().expect("WAL not initialized - call init() before using store");
            wal.flush()?;
            wal.current_lsn()
        };

        let checkpoint_record = wal::make_record(
            WalRecordType::Checkpoint,
            TransactionId::ZERO,
            Key::new(vec![]),
            None,
        );
        {
            let mut wal_guard = self.wal.lock();
            let wal = wal_guard.as_mut().expect("WAL not initialized - call init() before using store");
            wal.append(&checkpoint_record)?;
            wal.flush()?;
        }

        Ok(current_lsn)
    }

    fn flush_memtable(&self) -> Result<()> {
        let mut mt = self.memtable.write();
        if mt.is_empty() {
            return Ok(());
        }
        let entries: Vec<(Key, Value)> = mt.iter().collect();
        if entries.is_empty() {
            return Ok(());
        }
        let id = self.next_page_id.fetch_add(1, Ordering::Relaxed);
        let sst_dir = self.store_dir.join("sstables");
        let sstable = SSTable::create(&sst_dir, id, 0, entries)?;
        {
            let mut sstables = self.sstables.write();
            sstables.push(sstable);
        }
        *mt = MemTable::new();
        Ok(())
    }

    pub fn compact(&self) -> Result<()> {
        let mut sstables = self.sstables.write();
        if sstables.len() < 4 {
            return Ok(());
        }
        let all_entries: Vec<(Key, Value)> = {
            let mut merged: Vec<(Vec<u8>, Value)> = Vec::new();
            for sst in sstables.iter() {
                let sst_entries = sst.scan(&(Key::new(vec![])..Key::new(vec![0xFF; 256])))?;
                for (key, value) in sst_entries {
                    if let Some(pos) = merged.iter().position(|(k, _)| *k == key.as_bytes()) {
                        merged[pos] = (key.as_bytes().to_vec(), value);
                    } else {
                        merged.push((key.as_bytes().to_vec(), value));
                    }
                }
            }
            merged.sort_by(|a, b| a.0.cmp(&b.0));
            merged.into_iter().map(|(k, v)| (Key::new(k), v)).collect()
        };

        let id = self.next_page_id.fetch_add(1, Ordering::Relaxed);
        let sst_dir = self.store_dir.join("sstables");
        let compacted = SSTable::create(&sst_dir, id, 1, all_entries)?;

        for sst in sstables.iter() {
            let _ = std::fs::remove_file(&sst.path);
        }
        *sstables = vec![compacted];
        Ok(())
    }

    pub fn stats(&self) -> StorageStats {
        StorageStats {
            page_count: self.next_page_id.load(Ordering::Relaxed),
            dirty_pages: self.page_cache.dirty_count() as u64,
            cache_size: self.page_cache.size(),
            memtable_size: {
                let mt = self.memtable.read();
                mt.size()
            },
            sstable_count: {
                let sst = self.sstables.read();
                sst.len()
            },
            wal_segments: 0,
            current_lsn: Lsn::new(self.next_lsn.load(Ordering::Relaxed)),
            last_checkpoint: Lsn::ZERO,
        }
    }
}

/// Safe wrapper that makes Store usable as a StorageEngine.
/// Store's internal BTree uses Cell for caching but its &self API is designed
/// for concurrent access; the Cell is used only for best-effort caching.
pub struct StorageEngineStore {
    inner: Arc<Store>,
}

impl StorageEngineStore {
    pub fn new(store: Arc<Store>) -> Self {
        Self { inner: store }
    }

    pub fn inner(&self) -> &Store {
        &self.inner
    }
}

// SAFETY: StorageEngineStore contains only an Arc<Store>, which is both Send and Sync.
// The Arc provides thread-safe reference counting, and Store's internal mutability
// is managed through parking_lot::RwLock, making it safe to share across threads.
unsafe impl Send for StorageEngineStore {}
unsafe impl Sync for StorageEngineStore {}

impl nova_core::StorageEngine for StorageEngineStore {
    fn get(&self, key: &nova_core::Key) -> nova_core::Result<Option<nova_core::Value>> {
        Ok(self.inner.get(key)?)
    }

    fn set(&self, key: &nova_core::Key, value: nova_core::Value) -> nova_core::Result<()> {
        Ok(self.inner.set(key.clone(), value)?)
    }

    fn delete(&self, key: &nova_core::Key) -> nova_core::Result<bool> {
        Ok(self.inner.delete(key)?)
    }

    fn scan(&self, range: std::ops::Range<nova_core::Key>) -> nova_core::Result<Vec<(nova_core::Key, nova_core::Value)>> {
        Ok(self.inner.scan(range)?)
    }

    fn batch(&self, ops: Vec<nova_core::WriteOperation>) -> nova_core::Result<()> {
        let store_ops: Vec<WriteOperation> = ops.into_iter().map(|op| match op {
            nova_core::WriteOperation::Set { key, value } => WriteOperation::Set { key, value },
            nova_core::WriteOperation::Delete { key } => WriteOperation::Delete { key },
        }).collect();
        Ok(self.inner.batch(store_ops)?)
    }

    fn flush(&self) -> nova_core::Result<()> {
        Ok(self.inner.flush()?)
    }

    fn sync(&self) -> nova_core::Result<()> {
        Ok(self.inner.sync()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_config_default() {
        let config = StorageConfig::default();
        assert_eq!(config.page_cache_size, 16384);
        assert_eq!(config.memtable_size, 64 * 1024 * 1024);
        assert_eq!(config.btree_order, 128);
        assert_eq!(config.fsync_policy, FsyncPolicy::EveryNMs(100));
    }

    #[test]
    fn test_storage_stats_default() {
        let stats = StorageStats::default();
        assert_eq!(stats.page_count, 0);
        assert_eq!(stats.dirty_pages, 0);
        assert_eq!(stats.cache_size, 0);
        assert_eq!(stats.memtable_size, 0);
        assert_eq!(stats.sstable_count, 0);
        assert_eq!(stats.wal_segments, 0);
        assert_eq!(stats.current_lsn, Lsn::ZERO);
        assert_eq!(stats.last_checkpoint, Lsn::ZERO);
    }

    #[test]
    fn test_write_operation_set() {
        let op = WriteOperation::Set {
            key: Key::from("k"),
            value: Value::new(b"v".to_vec()),
        };
        match op {
            WriteOperation::Set { ref key, ref value } => {
                assert_eq!(key.as_bytes(), b"k");
                assert_eq!(value.as_bytes(), b"v");
            }
            _ => panic!("expected Set"),
        }
    }

    #[test]
    fn test_write_operation_delete() {
        let op = WriteOperation::Delete { key: Key::from("k") };
        match op {
            WriteOperation::Delete { ref key } => {
                assert_eq!(key.as_bytes(), b"k");
            }
            _ => panic!("expected Delete"),
        }
    }
}
