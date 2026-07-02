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
