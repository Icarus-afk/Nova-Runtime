use crate::Result;
use crate::types::*;
use std::ops::Range;

pub trait StorageEngine: Send + Sync {
    fn get(&self, key: &Key) -> Result<Option<Value>>;
    fn set(&self, key: &Key, value: Value) -> Result<()>;
    fn delete(&self, key: &Key) -> Result<bool>;
    fn scan(&self, range: Range<Key>) -> Result<Vec<(Key, Value)>>;
    fn batch(&self, ops: Vec<WriteOperation>) -> Result<()>;
    fn flush(&self) -> Result<()>;
    fn sync(&self) -> Result<()>;
}

#[derive(Debug, Clone)]
pub enum WriteOperation {
    Set { key: Key, value: Value },
    Delete { key: Key },
}

pub trait TransactionalStorage: StorageEngine {
    fn begin(&self) -> Result<TransactionId>;
    fn commit(&self, tx: TransactionId) -> Result<Lsn>;
    fn rollback(&self, tx: TransactionId) -> Result<()>;
    fn get_tx(&self, tx: TransactionId, key: &Key) -> Result<Option<Value>>;
    fn set_tx(&self, tx: TransactionId, key: Key, value: Value) -> Result<()>;
    fn delete_tx(&self, tx: TransactionId, key: &Key) -> Result<bool>;
}

pub trait EventPublisher: Send + Sync {
    fn publish(&self, topic: &str, payload: &[u8]) -> Result<()>;
    fn publish_with_key(&self, topic: &str, key: &str, payload: &[u8]) -> Result<()>;
}

pub trait EventSubscriber: Send + Sync {
    type Stream: Iterator<Item = Result<Event>>;

    fn subscribe(&self, topic: &str) -> Result<Self::Stream>;
    fn subscribe_filtered(&self, topic: &str, filter: &str) -> Result<Self::Stream>;
}

#[derive(Debug, Clone)]
pub struct Event {
    pub id: uuid::Uuid,
    pub event_type: String,
    pub timestamp: i64,
    pub source: String,
    pub key: Option<String>,
    pub payload: Vec<u8>,
    pub lsn: Option<Lsn>,
}

pub trait Executor: Send + Sync {
    fn execute(&self, op: Operation) -> Result<OperationResult>;
    fn execute_batch(&self, ops: Vec<Operation>) -> Result<Vec<OperationResult>>;
}

#[derive(Debug, Clone)]
pub enum Operation {
    Read {
        key: Key,
        collection: String,
    },
    Write {
        key: Key,
        value: Value,
        collection: String,
    },
    Delete {
        key: Key,
        collection: String,
    },
    Query {
        collection: String,
        query: Vec<u8>,
    },
}

#[derive(Debug, Clone)]
pub enum OperationResult {
    Read(Option<Value>),
    Write(Option<Value>),
    Delete(bool),
    Query(Vec<(Key, Value)>),
    Batch(Vec<OperationResult>),
}

pub trait Allocator: Send + Sync {
    fn allocate(&self, size: usize) -> Result<*mut u8>;
    fn deallocate(&self, ptr: *mut u8, size: usize);
    fn allocate_zeroed(&self, size: usize) -> Result<*mut u8>;
}

impl std::fmt::Display for Operation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Operation::Read { key, collection } => {
                write!(f, "Read({}/{})", collection, key)
            }
            Operation::Write { key, collection, .. } => {
                write!(f, "Write({}/{})", collection, key)
            }
            Operation::Delete { key, collection } => {
                write!(f, "Delete({}/{})", collection, key)
            }
            Operation::Query { collection, .. } => {
                write!(f, "Query({})", collection)
            }
        }
    }
}
