use std::sync::atomic::{AtomicU64, Ordering};
use parking_lot::RwLock;
use crate::{Event, EventId, EventType, TraceContext};

#[derive(Debug, Clone)]
pub struct StoredEvent {
    pub offset: u64,
    pub event_id: EventId,
    pub event_type: EventType,
    pub timestamp: u64,
    pub ordering_key: Option<String>,
    pub payload: Vec<u8>,
    pub trace_context: Option<TraceContext>,
}

#[derive(Debug, Clone)]
pub struct ReplayCursor {
    pub subscriber_id: String,
    pub last_processed_offset: u64,
    pub last_processed_timestamp: u64,
    pub target_timestamp: Option<u64>,
}

pub struct EventStore {
    events: RwLock<Vec<StoredEvent>>,
    next_offset: AtomicU64,
    max_entries: usize,
}

impl EventStore {
    pub fn new(max_entries: usize) -> Self {
        EventStore {
            events: RwLock::new(Vec::with_capacity(max_entries.min(1024))),
            next_offset: AtomicU64::new(0),
            max_entries,
        }
    }

    pub fn append(&self, event: &Event) -> u64 {
        let offset = self.next_offset.fetch_add(1, Ordering::Relaxed);
        let stored = StoredEvent {
            offset,
            event_id: event.metadata.event_id,
            event_type: event.metadata.event_type.clone(),
            timestamp: event.metadata.timestamp,
            ordering_key: event.metadata.ordering_key.clone(),
            payload: event.payload.clone(),
            trace_context: event.metadata.trace_context.clone(),
        };
        let mut events = self.events.write();
        events.push(stored);
        if events.len() > self.max_entries {
            let excess = events.len() - self.max_entries;
            events.drain(0..excess);
        }
        offset
    }

    pub fn scan_from(&self, offset: u64, limit: usize) -> Vec<StoredEvent> {
        let events = self.events.read();
        let start = offset as usize;
        if start >= events.len() {
            return vec![];
        }
        events[start..].iter().take(limit).cloned().collect()
    }

    pub fn len(&self) -> usize {
        self.events.read().len()
    }

    pub fn latest_offset(&self) -> u64 {
        self.next_offset.load(Ordering::Relaxed).saturating_sub(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EventBuilder, Subsystem};

    fn make_event(payload: Vec<u8>) -> Event {
        EventBuilder::new("test.event.occurred")
            .unwrap()
            .source(Subsystem::System, "test", "node1", "inst1")
            .build(payload)
    }

    #[test]
    fn test_new_store_is_empty() {
        let store = EventStore::new(100);
        assert_eq!(store.len(), 0);
        assert_eq!(store.scan_from(0, 10).len(), 0);
    }

    #[test]
    fn test_append_returns_offset() {
        let store = EventStore::new(100);
        let event = make_event(vec![1, 2, 3]);
        let offset = store.append(&event);
        assert_eq!(offset, 0);
    }

    #[test]
    fn test_append_increments_offset() {
        let store = EventStore::new(100);
        let e1 = make_event(vec![1]);
        let e2 = make_event(vec![2]);
        assert_eq!(store.append(&e1), 0);
        assert_eq!(store.append(&e2), 1);
    }

    #[test]
    fn test_scan_from_returns_events() {
        let store = EventStore::new(100);
        let event = make_event(vec![42]);
        store.append(&event);
        let results = store.scan_from(0, 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].offset, 0);
        assert_eq!(results[0].payload, vec![42]);
    }

    #[test]
    fn test_scan_from_offset() {
        let store = EventStore::new(100);
        store.append(&make_event(vec![1]));
        store.append(&make_event(vec![2]));
        store.append(&make_event(vec![3]));
        let results = store.scan_from(1, 10);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].offset, 1);
        assert_eq!(results[1].offset, 2);
    }

    #[test]
    fn test_scan_from_beyond_store() {
        let store = EventStore::new(100);
        store.append(&make_event(vec![1]));
        let results = store.scan_from(100, 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_scan_from_with_limit() {
        let store = EventStore::new(100);
        for i in 0..10 {
            store.append(&make_event(vec![i]));
        }
        let results = store.scan_from(0, 3);
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].offset, 0);
        assert_eq!(results[1].offset, 1);
        assert_eq!(results[2].offset, 2);
    }

    #[test]
    fn test_len_grows_with_appends() {
        let store = EventStore::new(100);
        assert_eq!(store.len(), 0);
        store.append(&make_event(vec![1]));
        assert_eq!(store.len(), 1);
        store.append(&make_event(vec![2]));
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn test_latest_offset() {
        let store = EventStore::new(100);
        assert_eq!(store.latest_offset(), 0);
        store.append(&make_event(vec![1]));
        assert_eq!(store.latest_offset(), 0);
        store.append(&make_event(vec![2]));
        assert_eq!(store.latest_offset(), 1);
    }

    #[test]
    fn test_store_capacity_trims_oldest() {
        let store = EventStore::new(3);
        store.append(&make_event(vec![1]));
        store.append(&make_event(vec![2]));
        store.append(&make_event(vec![3]));
        assert_eq!(store.len(), 3);
        store.append(&make_event(vec![4]));
        assert_eq!(store.len(), 3);
        let results = store.scan_from(0, 10);
        assert_eq!(results[0].offset, 1);
        assert_eq!(results[0].payload, vec![2]);
    }

    #[test]
    fn test_store_capacity_trims_multiple() {
        let store = EventStore::new(2);
        store.append(&make_event(vec![1]));
        store.append(&make_event(vec![2]));
        store.append(&make_event(vec![3]));
        store.append(&make_event(vec![4]));
        assert_eq!(store.len(), 2);
        let results = store.scan_from(0, 10);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].offset, 2);
        assert_eq!(results[1].offset, 3);
    }

    #[test]
    fn test_stored_event_fields() {
        let store = EventStore::new(100);
        let event = make_event(vec![10, 20]);
        let offset = store.append(&event);
        let results = store.scan_from(offset, 1);
        assert_eq!(results[0].event_id, event.metadata.event_id);
        assert_eq!(results[0].event_type.canonical, "test.event.occurred");
        assert_eq!(results[0].timestamp, event.metadata.timestamp);
        assert_eq!(results[0].ordering_key, event.metadata.ordering_key);
    }

    #[test]
    fn test_replay_cursor_defaults() {
        let cursor = ReplayCursor {
            subscriber_id: "sub-1".into(),
            last_processed_offset: 0,
            last_processed_timestamp: 0,
            target_timestamp: None,
        };
        assert_eq!(cursor.subscriber_id, "sub-1");
        assert_eq!(cursor.last_processed_offset, 0);
        assert!(cursor.target_timestamp.is_none());
    }
}
