use std::collections::VecDeque;
use parking_lot::RwLock;
use crate::{Event, SubscriberId, EventBus, Result, EventError};

#[derive(Debug, Clone)]
pub struct DeadLetterEntry {
    pub event: Event,
    pub failed_subscriber: SubscriberId,
    pub failure_reason: String,
    pub failure_timestamp: u64,
    pub retry_count: u32,
    pub last_error: String,
}

pub struct DeadLetterQueue {
    entries: RwLock<VecDeque<DeadLetterEntry>>,
    max_size: usize,
}

impl DeadLetterQueue {
    pub fn new(max_size: usize) -> Self {
        DeadLetterQueue {
            entries: RwLock::new(VecDeque::with_capacity(max_size.min(1024))),
            max_size,
        }
    }

    pub fn push(&self, entry: DeadLetterEntry) -> Result<()> {
        let mut entries = self.entries.write();
        if entries.len() >= self.max_size {
            entries.pop_front();
        }
        entries.push_back(entry);
        Ok(())
    }

    pub fn pop(&self) -> Option<DeadLetterEntry> {
        self.entries.write().pop_front()
    }

    pub fn requeue(&self, bus: &EventBus, index: usize) -> Result<()> {
        let entry = {
            let entries = self.entries.read();
            entries.get(index).cloned()
        };
        match entry {
            Some(entry) => {
                let _event_id = bus.publish(entry.event)?;
                let mut entries = self.entries.write();
                if index < entries.len() {
                    entries.remove(index);
                }
                Ok(())
            }
            None => Err(EventError::SubscriberNotFound),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.read().len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.read().is_empty()
    }

    pub fn entries(&self) -> Vec<DeadLetterEntry> {
        self.entries.read().iter().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EventBuilder, Subsystem};

    fn make_entry(event_num: u8) -> DeadLetterEntry {
        let event = EventBuilder::new("test.event")
            .unwrap()
            .source(Subsystem::System, "test", "n1", "i1")
            .build(vec![event_num]);
        DeadLetterEntry {
            event,
            failed_subscriber: SubscriberId {
                id: "sub-1".into(),
                subsystem: Subsystem::Execution,
                name: "worker".into(),
            },
            failure_reason: "timeout".into(),
            failure_timestamp: 1000,
            retry_count: 0,
            last_error: "connection timeout".into(),
        }
    }

    #[test]
    fn test_new_dlq_is_empty() {
        let dlq = DeadLetterQueue::new(100);
        assert!(dlq.is_empty());
        assert_eq!(dlq.len(), 0);
    }

    #[test]
    fn test_push_and_len() {
        let dlq = DeadLetterQueue::new(100);
        dlq.push(make_entry(1)).unwrap();
        assert_eq!(dlq.len(), 1);
        assert!(!dlq.is_empty());
    }

    #[test]
    fn test_push_multiple() {
        let dlq = DeadLetterQueue::new(100);
        dlq.push(make_entry(1)).unwrap();
        dlq.push(make_entry(2)).unwrap();
        dlq.push(make_entry(3)).unwrap();
        assert_eq!(dlq.len(), 3);
    }

    #[test]
    fn test_pop_returns_entries_fifo() {
        let dlq = DeadLetterQueue::new(100);
        dlq.push(make_entry(10)).unwrap();
        dlq.push(make_entry(20)).unwrap();
        let entry1 = dlq.pop().unwrap();
        assert_eq!(entry1.event.payload, vec![10]);
        let entry2 = dlq.pop().unwrap();
        assert_eq!(entry2.event.payload, vec![20]);
        assert!(dlq.is_empty());
    }

    #[test]
    fn test_pop_empty_returns_none() {
        let dlq = DeadLetterQueue::new(100);
        assert!(dlq.pop().is_none());
    }

    #[test]
    fn test_entries_returns_all() {
        let dlq = DeadLetterQueue::new(100);
        dlq.push(make_entry(1)).unwrap();
        dlq.push(make_entry(2)).unwrap();
        let entries = dlq.entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].event.payload, vec![1]);
        assert_eq!(entries[1].event.payload, vec![2]);
    }

    #[test]
    fn test_entries_does_not_consume() {
        let dlq = DeadLetterQueue::new(100);
        dlq.push(make_entry(1)).unwrap();
        assert_eq!(dlq.entries().len(), 1);
        assert_eq!(dlq.entries().len(), 1);
    }

    #[test]
    fn test_capacity_drops_oldest() {
        let dlq = DeadLetterQueue::new(2);
        dlq.push(make_entry(1)).unwrap();
        dlq.push(make_entry(2)).unwrap();
        dlq.push(make_entry(3)).unwrap();
        assert_eq!(dlq.len(), 2);
        let entries = dlq.entries();
        assert_eq!(entries[0].event.payload, vec![2]);
        assert_eq!(entries[1].event.payload, vec![3]);
    }

    #[test]
    fn test_capacity_drops_multiple() {
        let dlq = DeadLetterQueue::new(1);
        dlq.push(make_entry(1)).unwrap();
        dlq.push(make_entry(2)).unwrap();
        dlq.push(make_entry(3)).unwrap();
        assert_eq!(dlq.len(), 1);
        assert_eq!(dlq.entries()[0].event.payload, vec![3]);
    }

    #[test]
    fn test_entry_fields() {
        let entry = make_entry(42);
        assert_eq!(entry.failed_subscriber.id, "sub-1");
        assert_eq!(entry.failure_reason, "timeout");
        assert_eq!(entry.failure_timestamp, 1000);
        assert_eq!(entry.retry_count, 0);
        assert_eq!(entry.last_error, "connection timeout");
    }

    #[test]
    fn test_len_after_pop() {
        let dlq = DeadLetterQueue::new(100);
        dlq.push(make_entry(1)).unwrap();
        dlq.push(make_entry(2)).unwrap();
        dlq.pop();
        assert_eq!(dlq.len(), 1);
        dlq.pop();
        assert_eq!(dlq.len(), 0);
    }
}
