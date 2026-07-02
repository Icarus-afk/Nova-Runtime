use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};
use crate::EventError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EventId(pub [u8; 16]);

pub(crate) static EVENT_COUNTER: AtomicU64 = AtomicU64::new(0);

impl EventId {
    pub fn new() -> Self {
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let mut bytes = [0u8; 16];
        bytes[0..6].copy_from_slice(&timestamp_ms.to_be_bytes()[2..8]);
        bytes[6] = (bytes[6] & 0x0f) | 0x70;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        let counter = EVENT_COUNTER.fetch_add(1, Ordering::Relaxed);
        bytes[7] = ((counter >> 8) & 0xff) as u8;
        bytes[9..16].copy_from_slice(&counter.to_le_bytes()[0..7]);
        EventId(bytes)
    }

    pub fn timestamp(&self) -> u64 {
        let mut buf = [0u8; 8];
        buf[2..8].copy_from_slice(&self.0[0..6]);
        u64::from_be_bytes(buf)
    }

    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        EventId(bytes)
    }

    pub fn to_bytes(&self) -> [u8; 16] {
        self.0
    }
}

impl Default for EventId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EventType {
    pub segments: Vec<String>,
    pub canonical: String,
}

impl EventType {
    pub fn new(canonical: &str) -> Result<Self, EventError> {
        if canonical.is_empty() {
            return Err(EventError::InvalidEventType("empty event type".into()));
        }
        let segments: Vec<String> = canonical
            .split('.')
            .map(|s| s.to_string())
            .collect();
        if segments.iter().any(|s| s.is_empty()) {
            return Err(EventError::InvalidEventType(
                format!("empty segment in event type: {}", canonical),
            ));
        }
        if segments.iter().any(|s| s == "+" || s == "*") {
            return Err(EventError::InvalidEventType(
                format!("wildcard in event type: {}", canonical),
            ));
        }
        Ok(EventType {
            canonical: canonical.to_string(),
            segments,
        })
    }

    pub fn segment(&self, n: usize) -> Option<&str> {
        self.segments.get(n).map(|s| s.as_str())
    }

    pub fn depth(&self) -> usize {
        self.segments.len()
    }

    pub fn matches(&self, pattern: &crate::TopicPattern) -> bool {
        pattern.matches(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Subsystem {
    Storage,
    Execution,
    Auth,
    Queue,
    Scheduler,
    Search,
    Blob,
    Api,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventSource {
    pub subsystem: Subsystem,
    pub component: String,
    pub node_id: String,
    pub instance_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceContext {
    pub trace_id: [u8; 16],
    pub span_id: [u8; 8],
    pub parent_span_id: Option<[u8; 8]>,
    pub sampled: bool,
    pub baggage: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMetadata {
    pub event_id: EventId,
    pub event_type: EventType,
    pub source: EventSource,
    pub timestamp: u64,
    pub ordering_key: Option<String>,
    pub content_type: String,
    pub payload_size: u32,
    pub ttl_ms: u64,
    pub priority: EventPriority,
    pub persistent: bool,
    pub schema_version: u32,
    pub trace_context: Option<TraceContext>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub metadata: EventMetadata,
    pub payload: Vec<u8>,
}

pub struct EventBuilder {
    event_type: EventType,
    source: Option<EventSource>,
    ordering_key: Option<String>,
    ttl_ms: u64,
    priority: EventPriority,
    persistent: bool,
    content_type: String,
    schema_version: u32,
}

impl EventBuilder {
    pub fn new(event_type: &str) -> Result<Self, EventError> {
        Ok(EventBuilder {
            event_type: EventType::new(event_type)?,
            source: None,
            ordering_key: None,
            ttl_ms: 0,
            priority: EventPriority::Normal,
            persistent: false,
            content_type: "application/x-msgpack".to_string(),
            schema_version: 1,
        })
    }

    pub fn source(mut self, subsystem: Subsystem, component: &str, node_id: &str, instance_id: &str) -> Self {
        self.source = Some(EventSource {
            subsystem,
            component: component.to_string(),
            node_id: node_id.to_string(),
            instance_id: instance_id.to_string(),
        });
        self
    }

    pub fn ordering_key(mut self, key: &str) -> Self {
        self.ordering_key = Some(key.to_string());
        self
    }

    pub fn ttl(mut self, ttl_ms: u64) -> Self {
        self.ttl_ms = ttl_ms;
        self
    }

    pub fn priority(mut self, priority: EventPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn persistent(mut self, persistent: bool) -> Self {
        self.persistent = persistent;
        self
    }

    pub fn build(self, payload: Vec<u8>) -> Event {
        let source = self.source.unwrap_or(EventSource {
            subsystem: Subsystem::System,
            component: "unknown".to_string(),
            node_id: "local".to_string(),
            instance_id: "default".to_string(),
        });
        let event_id = EventId::new();
        let payload_size = payload.len() as u32;
        Event {
            metadata: EventMetadata {
                event_id,
                event_type: self.event_type,
                source,
                trace_context: None,
                timestamp: event_id.timestamp(),
                ordering_key: self.ordering_key,
                content_type: self.content_type,
                payload_size,
                ttl_ms: self.ttl_ms,
                priority: self.priority,
                persistent: self.persistent,
                schema_version: self.schema_version,
            },
            payload,
        }
    }
}
