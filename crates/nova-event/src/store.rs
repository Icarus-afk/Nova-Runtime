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
