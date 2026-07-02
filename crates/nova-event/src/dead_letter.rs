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
