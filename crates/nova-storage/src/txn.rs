use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use parking_lot::RwLock;
use nova_core::types::*;
use nova_core::error::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxnStatus {
    Active,
    Committing,
    Committed,
    Aborted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockMode {
    Conservative,
    Optimistic,
}

#[derive(Debug, Clone)]
pub struct Snapshot {
    pub transaction_id: u64,
    pub sequence: u64,
}

#[derive(Debug, Clone)]
pub struct Transaction {
    pub id: u64,
    pub snapshot: Snapshot,
    pub status: TxnStatus,
    pub isolation: IsolationLevel,
    pub lock_mode: LockMode,
    pub write_set: Vec<(Key, Option<Value>)>,
}

pub struct TransactionManager {
    next_txn_id: AtomicU64,
    active_txns: RwLock<HashMap<u64, Transaction>>,
}

impl TransactionManager {
    pub fn new() -> Self {
        TransactionManager {
            next_txn_id: AtomicU64::new(1),
            active_txns: RwLock::new(HashMap::new()),
        }
    }

    pub fn begin(&self, isolation: IsolationLevel, lock_mode: LockMode) -> Transaction {
        let id = self.next_txn_id.fetch_add(1, Ordering::Relaxed);
        let txn = Transaction {
            id,
            snapshot: Snapshot {
                transaction_id: id,
                sequence: 0,
            },
            status: TxnStatus::Active,
            isolation,
            lock_mode,
            write_set: Vec::new(),
        };
        self.active_txns.write().insert(id, txn.clone());
        txn
    }

    pub fn commit(&self, txn_id: u64) -> Result<()> {
        let mut txns = self.active_txns.write();
        if let Some(txn) = txns.get_mut(&txn_id) {
            txn.status = TxnStatus::Committed;
        }
        txns.remove(&txn_id);
        Ok(())
    }

    pub fn rollback(&self, txn_id: u64) -> Result<()> {
        self.active_txns.write().remove(&txn_id);
        Ok(())
    }

    pub fn get(&self, txn_id: u64) -> Option<Transaction> {
        self.active_txns.read().get(&txn_id).cloned()
    }

    pub fn register_write(&self, txn_id: u64, key: Key, value: Option<Value>) -> Result<()> {
        let txns = self.active_txns.read();
        let txn = txns.get(&txn_id).ok_or_else(|| {
            RuntimeError::TransactionError(format!("transaction {} not found", txn_id))
        })?;
        if txn.status != TxnStatus::Active {
            return Err(RuntimeError::TransactionError(
                "cannot write to inactive transaction".into(),
            ));
        }
        drop(txns);

        let mut txns = self.active_txns.write();
        if let Some(txn) = txns.get_mut(&txn_id) {
            txn.write_set.push((key, value));
        }
        Ok(())
    }

    pub fn snapshot_read<F>(
        &self,
        txn_id: u64,
        key: &Key,
        store_read: F,
    ) -> Result<Option<Value>>
    where
        F: Fn(&Key) -> Result<Option<Value>>,
    {
        let txns = self.active_txns.read();
        let txn = txns.get(&txn_id).ok_or_else(|| {
            RuntimeError::TransactionError(format!("transaction {} not found", txn_id))
        })?;

        for (wk, wv) in &txn.write_set {
            if wk.as_bytes() == key.as_bytes() {
                return Ok(wv.clone());
            }
        }

        match txn.isolation {
            IsolationLevel::ReadCommitted => store_read(key),
            IsolationLevel::RepeatableRead | IsolationLevel::Snapshot | IsolationLevel::Serializable => {
                store_read(key)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> TransactionManager {
        TransactionManager::new()
    }

    #[test]
    fn test_begin_creates_active_txn() {
        let tm = setup();
        let txn = tm.begin(IsolationLevel::ReadCommitted, LockMode::Optimistic);
        assert_eq!(txn.status, TxnStatus::Active);
        assert_eq!(txn.id, 1);
        assert!(txn.write_set.is_empty());
    }

    #[test]
    fn test_commit_changes_status_and_removes() {
        let tm = setup();
        let txn = tm.begin(IsolationLevel::ReadCommitted, LockMode::Optimistic);
        tm.commit(txn.id).unwrap();
        assert!(tm.get(txn.id).is_none());
    }

    #[test]
    fn test_rollback_removes_txn() {
        let tm = setup();
        let txn = tm.begin(IsolationLevel::ReadCommitted, LockMode::Optimistic);
        tm.rollback(txn.id).unwrap();
        assert!(tm.get(txn.id).is_none());
    }

    #[test]
    fn test_begin_generates_incrementing_ids() {
        let tm = setup();
        let t1 = tm.begin(IsolationLevel::ReadCommitted, LockMode::Optimistic);
        let t2 = tm.begin(IsolationLevel::ReadCommitted, LockMode::Optimistic);
        assert_eq!(t2.id, t1.id + 1);
    }

    #[test]
    fn test_get_returns_none_for_unknown() {
        let tm = setup();
        assert!(tm.get(999).is_none());
    }

    #[test]
    fn test_register_write_adds_to_write_set() {
        let tm = setup();
        let txn = tm.begin(IsolationLevel::ReadCommitted, LockMode::Optimistic);
        tm.register_write(txn.id, Key::from("key1"), Some(Value::new(b"val1".to_vec()))).unwrap();
        tm.register_write(txn.id, Key::from("key2"), None).unwrap();
        let stored = tm.get(txn.id).unwrap();
        assert_eq!(stored.write_set.len(), 2);
        assert_eq!(stored.write_set[0].0, Key::from("key1"));
        assert_eq!(stored.write_set[0].1, Some(Value::new(b"val1".to_vec())));
        assert_eq!(stored.write_set[1].0, Key::from("key2"));
        assert_eq!(stored.write_set[1].1, None);
    }

    #[test]
    fn test_register_write_fails_for_unknown_txn() {
        let tm = setup();
        let result = tm.register_write(999, Key::from("x"), None);
        assert!(result.is_err());
        match result {
            Err(RuntimeError::TransactionError(_)) => {}
            _ => panic!("expected TransactionError"),
        }
    }

    #[test]
    fn test_snapshot_read_sees_own_writes() {
        let tm = setup();
        let txn = tm.begin(IsolationLevel::ReadCommitted, LockMode::Optimistic);
        tm.register_write(txn.id, Key::from("key"), Some(Value::new(b"own".to_vec()))).unwrap();
        let result = tm.snapshot_read(txn.id, &Key::from("key"), |_| Ok(None)).unwrap();
        assert_eq!(result, Some(Value::new(b"own".to_vec())));
    }

    #[test]
    fn test_snapshot_read_delegates_to_store_for_unknown_key() {
        let tm = setup();
        let txn = tm.begin(IsolationLevel::ReadCommitted, LockMode::Optimistic);
        let result = tm.snapshot_read(txn.id, &Key::from("store_key"), |k| {
            assert_eq!(k.as_bytes(), b"store_key");
            Ok(Some(Value::new(b"from_store".to_vec())))
        }).unwrap();
        assert_eq!(result, Some(Value::new(b"from_store".to_vec())));
    }

    #[test]
    fn test_snapshot_read_own_write_overrides_store() {
        let tm = setup();
        let txn = tm.begin(IsolationLevel::ReadCommitted, LockMode::Optimistic);
        tm.register_write(txn.id, Key::from("key"), Some(Value::new(b"own".to_vec()))).unwrap();
        let result = tm.snapshot_read(txn.id, &Key::from("key"), |_| {
            Ok(Some(Value::new(b"store".to_vec())))
        }).unwrap();
        assert_eq!(result, Some(Value::new(b"own".to_vec())));
    }

    #[test]
    fn test_snapshot_read_fails_for_unknown_txn() {
        let tm = setup();
        let result = tm.snapshot_read(999, &Key::from("x"), |_| Ok(None));
        assert!(result.is_err());
    }

    #[test]
    fn test_rollback_unknown_txn_is_noop() {
        let tm = setup();
        tm.rollback(999).unwrap();
    }

    #[test]
    fn test_begin_sets_correct_isolation() {
        let tm = setup();
        let txn = tm.begin(IsolationLevel::Serializable, LockMode::Optimistic);
        assert_eq!(txn.isolation, IsolationLevel::Serializable);
        assert_eq!(txn.lock_mode, LockMode::Optimistic);
    }

    #[test]
    fn test_txn_status_derives() {
        assert_ne!(TxnStatus::Active, TxnStatus::Committed);
        assert_ne!(TxnStatus::Committed, TxnStatus::Aborted);
    }

    #[test]
    fn test_lock_mode_derives() {
        assert_ne!(LockMode::Conservative, LockMode::Optimistic);
    }
}
