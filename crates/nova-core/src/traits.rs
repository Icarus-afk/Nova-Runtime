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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // --- Mock StorageEngine ---

    struct MockStorage {
        data: Mutex<Vec<(Key, Value)>>,
    }

    impl MockStorage {
        fn new() -> Self {
            MockStorage {
                data: Mutex::new(Vec::new()),
            }
        }
    }

    impl StorageEngine for MockStorage {
        fn get(&self, key: &Key) -> Result<Option<Value>> {
            let data = self.data.lock().unwrap();
            Ok(data.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone()))
        }

        fn set(&self, key: &Key, value: Value) -> Result<()> {
            let mut data = self.data.lock().unwrap();
            if let Some(pos) = data.iter().position(|(k, _)| k == key) {
                data[pos] = (key.clone(), value);
            } else {
                data.push((key.clone(), value));
            }
            Ok(())
        }

        fn delete(&self, key: &Key) -> Result<bool> {
            let mut data = self.data.lock().unwrap();
            let len = data.len();
            data.retain(|(k, _)| k != key);
            Ok(data.len() < len)
        }

        fn scan(&self, _range: Range<Key>) -> Result<Vec<(Key, Value)>> {
            let data = self.data.lock().unwrap();
            Ok(data.clone())
        }

        fn batch(&self, ops: Vec<WriteOperation>) -> Result<()> {
            for op in ops {
                match op {
                    WriteOperation::Set { key, value } => self.set(&key, value)?,
                    WriteOperation::Delete { key } => { self.delete(&key)?; }
                }
            }
            Ok(())
        }

        fn flush(&self) -> Result<()> {
            Ok(())
        }

        fn sync(&self) -> Result<()> {
            Ok(())
        }
    }

    // --- Mock TransactionalStorage ---

    struct MockTransactionalStorage {
        inner: MockStorage,
    }

    impl MockTransactionalStorage {
        fn new() -> Self {
            MockTransactionalStorage {
                inner: MockStorage::new(),
            }
        }
    }

    impl StorageEngine for MockTransactionalStorage {
        fn get(&self, key: &Key) -> Result<Option<Value>> {
            self.inner.get(key)
        }
        fn set(&self, key: &Key, value: Value) -> Result<()> {
            self.inner.set(key, value)
        }
        fn delete(&self, key: &Key) -> Result<bool> {
            self.inner.delete(key)
        }
        fn scan(&self, range: Range<Key>) -> Result<Vec<(Key, Value)>> {
            self.inner.scan(range)
        }
        fn batch(&self, ops: Vec<WriteOperation>) -> Result<()> {
            self.inner.batch(ops)
        }
        fn flush(&self) -> Result<()> {
            self.inner.flush()
        }
        fn sync(&self) -> Result<()> {
            self.inner.sync()
        }
    }

    impl TransactionalStorage for MockTransactionalStorage {
        fn begin(&self) -> Result<TransactionId> {
            Ok(TransactionId::new(1))
        }
        fn commit(&self, _tx: TransactionId) -> Result<Lsn> {
            Ok(Lsn::new(100))
        }
        fn rollback(&self, _tx: TransactionId) -> Result<()> {
            Ok(())
        }
        fn get_tx(&self, _tx: TransactionId, key: &Key) -> Result<Option<Value>> {
            self.inner.get(key)
        }
        fn set_tx(&self, _tx: TransactionId, key: Key, value: Value) -> Result<()> {
            self.inner.set(&key, value)
        }
        fn delete_tx(&self, _tx: TransactionId, key: &Key) -> Result<bool> {
            self.inner.delete(key)
        }
    }

    // --- Mock EventPublisher ---

    struct MockEventPublisher {
        events: Mutex<Vec<(String, Vec<u8>)>>,
    }

    impl MockEventPublisher {
        fn new() -> Self {
            MockEventPublisher {
                events: Mutex::new(Vec::new()),
            }
        }
    }

    impl EventPublisher for MockEventPublisher {
        fn publish(&self, topic: &str, payload: &[u8]) -> Result<()> {
            self.events
                .lock()
                .unwrap()
                .push((topic.to_string(), payload.to_vec()));
            Ok(())
        }

        fn publish_with_key(&self, topic: &str, key: &str, payload: &[u8]) -> Result<()> {
            let mut events = self.events.lock().unwrap();
            let mut full = Vec::new();
            full.extend_from_slice(key.as_bytes());
            full.extend_from_slice(b":");
            full.extend_from_slice(payload);
            events.push((topic.to_string(), full));
            Ok(())
        }
    }

    // --- Mock Executor ---

    struct MockExecutor;

    impl Executor for MockExecutor {
        fn execute(&self, op: Operation) -> Result<OperationResult> {
            match op {
                Operation::Read { .. } => Ok(OperationResult::Read(None)),
                Operation::Write { .. } => Ok(OperationResult::Write(None)),
                Operation::Delete { .. } => Ok(OperationResult::Delete(true)),
                Operation::Query { .. } => Ok(OperationResult::Query(Vec::new())),
            }
        }

        fn execute_batch(&self, ops: Vec<Operation>) -> Result<Vec<OperationResult>> {
            ops.into_iter().map(|op| self.execute(op)).collect()
        }
    }

    // --- StorageEngine tests ---

    #[test]
    fn test_mock_storage_get_set() {
        let store = MockStorage::new();
        let key = Key::from("foo");
        let value = Value::new(b"bar".to_vec());

        assert_eq!(store.get(&key).unwrap(), None);

        store.set(&key, value.clone()).unwrap();
        assert_eq!(store.get(&key).unwrap(), Some(value));
    }

    #[test]
    fn test_mock_storage_set_overwrite() {
        let store = MockStorage::new();
        let key = Key::from("k");
        store.set(&key, Value::new(b"v1".to_vec())).unwrap();
        store.set(&key, Value::new(b"v2".to_vec())).unwrap();

        assert_eq!(
            store.get(&key).unwrap(),
            Some(Value::new(b"v2".to_vec()))
        );
    }

    #[test]
    fn test_mock_storage_delete() {
        let store = MockStorage::new();
        let key = Key::from("k");
        store.set(&key, Value::new(b"v".to_vec())).unwrap();
        assert!(store.delete(&key).unwrap());
        assert!(!store.delete(&key).unwrap());
    }

    #[test]
    fn test_mock_storage_delete_nonexistent() {
        let store = MockStorage::new();
        assert!(!store.delete(&Key::from("missing")).unwrap());
    }

    #[test]
    fn test_mock_storage_flush_sync() {
        let store = MockStorage::new();
        assert!(store.flush().is_ok());
        assert!(store.sync().is_ok());
    }

    #[test]
    fn test_mock_storage_scan() {
        let store = MockStorage::new();
        store
            .set(&Key::from("a"), Value::new(b"1".to_vec()))
            .unwrap();
        store
            .set(&Key::from("b"), Value::new(b"2".to_vec()))
            .unwrap();
        let results = store
            .scan(Key::from("")..Key::from("z"))
            .unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_mock_storage_batch_set() {
        let store = MockStorage::new();
        let ops = vec![
            WriteOperation::Set {
                key: Key::from("a"),
                value: Value::new(b"1".to_vec()),
            },
            WriteOperation::Set {
                key: Key::from("b"),
                value: Value::new(b"2".to_vec()),
            },
        ];
        store.batch(ops).unwrap();
        assert!(store.get(&Key::from("a")).unwrap().is_some());
        assert!(store.get(&Key::from("b")).unwrap().is_some());
    }

    #[test]
    fn test_mock_storage_batch_delete() {
        let store = MockStorage::new();
        store
            .set(&Key::from("x"), Value::new(b"y".to_vec()))
            .unwrap();
        let ops = vec![WriteOperation::Delete { key: Key::from("x") }];
        store.batch(ops).unwrap();
        assert!(store.get(&Key::from("x")).unwrap().is_none());
    }

    // --- TransactionalStorage tests ---

    #[test]
    fn test_mock_transactional_begin_commit_rollback() {
        let store = MockTransactionalStorage::new();
        let tx = store.begin().unwrap();
        assert_eq!(tx, TransactionId::new(1));

        let lsn = store.commit(tx).unwrap();
        assert_eq!(lsn, Lsn::new(100));

        assert!(store.rollback(tx).is_ok());
    }

    #[test]
    fn test_mock_transactional_get_set_tx() {
        let store = MockTransactionalStorage::new();
        let tx = store.begin().unwrap();
        let key = Key::from("tx_key");

        assert!(store.get_tx(tx, &key).unwrap().is_none());

        store
            .set_tx(tx, key.clone(), Value::new(b"tx_val".to_vec()))
            .unwrap();
        assert_eq!(
            store.get_tx(tx, &key).unwrap(),
            Some(Value::new(b"tx_val".to_vec()))
        );
    }

    #[test]
    fn test_mock_transactional_delete_tx() {
        let store = MockTransactionalStorage::new();
        let tx = store.begin().unwrap();
        let key = Key::from("del_key");
        store
            .set_tx(tx, key.clone(), Value::new(b"val".to_vec()))
            .unwrap();
        assert!(store.delete_tx(tx, &key).unwrap());
        assert!(!store.delete_tx(tx, &key).unwrap());
    }

    // --- WriteOperation tests ---

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
        let op = WriteOperation::Delete {
            key: Key::from("k"),
        };
        match op {
            WriteOperation::Delete { ref key } => {
                assert_eq!(key.as_bytes(), b"k");
            }
            _ => panic!("expected Delete"),
        }
    }

    #[test]
    fn test_write_operation_debug() {
        let set = WriteOperation::Set {
            key: Key::from("k"),
            value: Value::new(b"v".to_vec()),
        };
        let debug = format!("{:?}", set);
        assert!(debug.contains("Set"));
        assert!(debug.contains("[107]")); // "k" as bytes

        let del = WriteOperation::Delete {
            key: Key::from("d"),
        };
        let debug = format!("{:?}", del);
        assert!(debug.contains("Delete"));
        assert!(debug.contains("[100]")); // "d" as bytes
    }

    // --- Event tests ---

    #[test]
    fn test_event_construction() {
        let event = Event {
            id: uuid::Uuid::nil(),
            event_type: "test.event".into(),
            timestamp: 1234567890,
            source: "test".into(),
            key: Some("mykey".into()),
            payload: b"payload".to_vec(),
            lsn: Some(Lsn::new(42)),
        };
        assert_eq!(event.event_type, "test.event");
        assert_eq!(event.timestamp, 1234567890);
        assert_eq!(event.source, "test");
        assert_eq!(event.key, Some("mykey".into()));
        assert_eq!(event.payload, b"payload");
        assert_eq!(event.lsn, Some(Lsn::new(42)));
    }

    #[test]
    fn test_event_without_key_lsn() {
        let event = Event {
            id: uuid::Uuid::new_v4(),
            event_type: "simple".into(),
            timestamp: 0,
            source: "s".into(),
            key: None,
            payload: vec![],
            lsn: None,
        };
        assert!(event.key.is_none());
        assert!(event.lsn.is_none());
    }

    #[test]
    fn test_event_debug() {
        let event = Event {
            id: uuid::Uuid::nil(),
            event_type: "e".into(),
            timestamp: 1,
            source: "s".into(),
            key: None,
            payload: vec![],
            lsn: None,
        };
        assert!(format!("{:?}", event).contains("Event"));
    }

    // --- EventPublisher tests ---

    #[test]
    fn test_mock_event_publisher_publish() {
        let pubsub = MockEventPublisher::new();
        pubsub
            .publish("topic1", b"hello")
            .unwrap();
        let events = pubsub.events.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].0, "topic1");
        assert_eq!(events[0].1, b"hello");
    }

    #[test]
    fn test_mock_event_publisher_publish_with_key() {
        let pubsub = MockEventPublisher::new();
        pubsub
            .publish_with_key("t", "mykey", b"data")
            .unwrap();
        let events = pubsub.events.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].0, "t");
        assert_eq!(events[0].1, b"mykey:data");
    }

    // --- Operation tests ---

    #[test]
    fn test_operation_read_construction() {
        let op = Operation::Read {
            key: Key::from("k"),
            collection: "col".into(),
        };
        match op {
            Operation::Read { ref key, ref collection } => {
                assert_eq!(key.as_bytes(), b"k");
                assert_eq!(collection, "col");
            }
            _ => panic!("expected Read"),
        }
    }

    #[test]
    fn test_operation_write_construction() {
        let op = Operation::Write {
            key: Key::from("k"),
            value: Value::new(b"v".to_vec()),
            collection: "col".into(),
        };
        match op {
            Operation::Write { ref key, ref value, ref collection } => {
                assert_eq!(key.as_bytes(), b"k");
                assert_eq!(value.as_bytes(), b"v");
                assert_eq!(collection, "col");
            }
            _ => panic!("expected Write"),
        }
    }

    #[test]
    fn test_operation_delete_construction() {
        let op = Operation::Delete {
            key: Key::from("k"),
            collection: "col".into(),
        };
        match op {
            Operation::Delete { ref key, ref collection } => {
                assert_eq!(key.as_bytes(), b"k");
                assert_eq!(collection, "col");
            }
            _ => panic!("expected Delete"),
        }
    }

    #[test]
    fn test_operation_query_construction() {
        let op = Operation::Query {
            collection: "col".into(),
            query: b"{}".to_vec(),
        };
        match op {
            Operation::Query { ref collection, ref query } => {
                assert_eq!(collection, "col");
                assert_eq!(query, b"{}");
            }
            _ => panic!("expected Query"),
        }
    }

    #[test]
    fn test_operation_display_read() {
        let op = Operation::Read {
            key: Key::from("mykey"),
            collection: "mycol".into(),
        };
        assert_eq!(format!("{}", op), "Read(mycol/mykey)");
    }

    #[test]
    fn test_operation_display_write() {
        let op = Operation::Write {
            key: Key::from("k"),
            value: Value::new(b"v".to_vec()),
            collection: "c".into(),
        };
        assert_eq!(format!("{}", op), "Write(c/k)");
    }

    #[test]
    fn test_operation_display_delete() {
        let op = Operation::Delete {
            key: Key::from("k"),
            collection: "c".into(),
        };
        assert_eq!(format!("{}", op), "Delete(c/k)");
    }

    #[test]
    fn test_operation_display_query() {
        let op = Operation::Query {
            collection: "users".into(),
            query: vec![],
        };
        assert_eq!(format!("{}", op), "Query(users)");
    }

    #[test]
    fn test_operation_debug() {
        let op = Operation::Read {
            key: Key::from("k"),
            collection: "c".into(),
        };
        assert!(format!("{:?}", op).contains("Read"));
    }

    // --- OperationResult tests ---

    #[test]
    fn test_operation_result_read() {
        let r = OperationResult::Read(Some(Value::new(b"data".to_vec())));
        match r {
            OperationResult::Read(Some(v)) => assert_eq!(v.as_bytes(), b"data"),
            _ => panic!("expected Read"),
        }
    }

    #[test]
    fn test_operation_result_read_none() {
        let r = OperationResult::Read(None);
        match r {
            OperationResult::Read(None) => {}
            _ => panic!("expected Read(None)"),
        }
    }

    #[test]
    fn test_operation_result_write() {
        let r = OperationResult::Write(Some(Value::new(b"old".to_vec())));
        match r {
            OperationResult::Write(v) => assert_eq!(v, Some(Value::new(b"old".to_vec()))),
            _ => panic!("expected Write"),
        }
    }

    #[test]
    fn test_operation_result_delete() {
        let r = OperationResult::Delete(true);
        match r {
            OperationResult::Delete(b) => assert!(b),
            _ => panic!("expected Delete"),
        }
    }

    #[test]
    fn test_operation_result_query() {
        let r = OperationResult::Query(vec![(
            Key::from("k"),
            Value::new(b"v".to_vec()),
        )]);
        match r {
            OperationResult::Query(results) => assert_eq!(results.len(), 1),
            _ => panic!("expected Query"),
        }
    }

    #[test]
    fn test_operation_result_batch() {
        let r = OperationResult::Batch(vec![
            OperationResult::Delete(true),
            OperationResult::Read(None),
        ]);
        match r {
            OperationResult::Batch(ref results) => assert_eq!(results.len(), 2),
            _ => panic!("expected Batch"),
        }
    }

    #[test]
    fn test_operation_result_debug() {
        assert!(format!("{:?}", OperationResult::Delete(false)).contains("Delete"));
        assert!(format!("{:?}", OperationResult::Read(None)).contains("Read"));
    }

    // --- Executor tests ---

    #[test]
    fn test_mock_executor_execute() {
        let exec = MockExecutor;
        let read = exec
            .execute(Operation::Read {
                key: Key::from("k"),
                collection: "c".into(),
            })
            .unwrap();
        assert!(matches!(read, OperationResult::Read(None)));

        let write = exec
            .execute(Operation::Write {
                key: Key::from("k"),
                value: Value::new(vec![]),
                collection: "c".into(),
            })
            .unwrap();
        assert!(matches!(write, OperationResult::Write(_)));

        let del = exec
            .execute(Operation::Delete {
                key: Key::from("k"),
                collection: "c".into(),
            })
            .unwrap();
        assert!(matches!(del, OperationResult::Delete(true)));

        let query = exec
            .execute(Operation::Query {
                collection: "c".into(),
                query: vec![],
            })
            .unwrap();
        assert!(matches!(query, OperationResult::Query(_)));
    }

    #[test]
    fn test_mock_executor_execute_batch() {
        let exec = MockExecutor;
        let ops = vec![
            Operation::Read {
                key: Key::from("k"),
                collection: "c".into(),
            },
            Operation::Delete {
                key: Key::from("k"),
                collection: "c".into(),
            },
        ];
        let results = exec.execute_batch(ops).unwrap();
        assert_eq!(results.len(), 2);
        assert!(matches!(results[0], OperationResult::Read(_)));
        assert!(matches!(results[1], OperationResult::Delete(_)));
    }
}
