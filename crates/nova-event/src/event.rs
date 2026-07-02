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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Serialize, Deserialize)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TopicPattern;

    #[test]
    fn test_event_id_new_is_unique() {
        let id1 = EventId::new();
        let id2 = EventId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_event_id_default_is_unique() {
        let id1 = EventId::default();
        let id2 = EventId::default();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_event_id_timestamp() {
        let id = EventId::new();
        let before = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let ts = id.timestamp();
        let after = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        assert!(ts >= before / 1000 * 1000 || ts <= after + 1000);
    }

    #[test]
    fn test_event_id_roundtrip() {
        let id = EventId::new();
        let bytes = id.to_bytes();
        let id2 = EventId::from_bytes(bytes);
        assert_eq!(id, id2);
    }

    #[test]
    fn test_event_id_bytes_length() {
        let id = EventId::new();
        assert_eq!(id.to_bytes().len(), 16);
    }

    #[test]
    fn test_event_type_new_valid() {
        let et = EventType::new("test.event.created").unwrap();
        assert_eq!(et.canonical, "test.event.created");
        assert_eq!(et.segments, vec!["test", "event", "created"]);
    }

    #[test]
    fn test_event_type_new_empty_rejected() {
        let err = EventType::new("").unwrap_err();
        assert!(matches!(err, EventError::InvalidEventType(_)));
    }

    #[test]
    fn test_event_type_new_empty_segment_rejected() {
        let err = EventType::new("test..event").unwrap_err();
        assert!(matches!(err, EventError::InvalidEventType(_)));
    }

    #[test]
    fn test_event_type_new_wildcard_rejected() {
        let err = EventType::new("test.*.event").unwrap_err();
        assert!(matches!(err, EventError::InvalidEventType(_)));
    }

    #[test]
    fn test_event_type_new_plus_wildcard_rejected() {
        let err = EventType::new("test.+.event").unwrap_err();
        assert!(matches!(err, EventError::InvalidEventType(_)));
    }

    #[test]
    fn test_event_type_segment() {
        let et = EventType::new("a.b.c").unwrap();
        assert_eq!(et.segment(0), Some("a"));
        assert_eq!(et.segment(1), Some("b"));
        assert_eq!(et.segment(2), Some("c"));
        assert_eq!(et.segment(3), None);
    }

    #[test]
    fn test_event_type_depth() {
        let et = EventType::new("a.b.c.d").unwrap();
        assert_eq!(et.depth(), 4);
    }

    #[test]
    fn test_event_type_single_segment() {
        let et = EventType::new("single").unwrap();
        assert_eq!(et.depth(), 1);
        assert_eq!(et.segment(0), Some("single"));
    }

    #[test]
    fn test_event_type_matches_pattern() {
        let et = EventType::new("test.event.created").unwrap();
        let pattern = TopicPattern::new("test.event.created").unwrap();
        assert!(et.matches(&pattern));
    }

    #[test]
    fn test_event_type_not_matches_pattern() {
        let et = EventType::new("test.event.created").unwrap();
        let pattern = TopicPattern::new("test.event.deleted").unwrap();
        assert!(!et.matches(&pattern));
    }

    #[test]
    fn test_event_type_matches_wildcard() {
        let et = EventType::new("test.event.created").unwrap();
        let pattern = TopicPattern::new("test.+.+").unwrap();
        assert!(et.matches(&pattern));
    }

    #[test]
    fn test_event_type_matches_multi_wildcard() {
        let et = EventType::new("test.event.created.extra").unwrap();
        let pattern = TopicPattern::new("test.*").unwrap();
        assert!(et.matches(&pattern));
    }

    #[test]
    fn test_event_builder_defaults() {
        let event = EventBuilder::new("test.event")
            .unwrap()
            .build(vec![1, 2, 3]);
        assert_eq!(event.metadata.event_type.canonical, "test.event");
        assert_eq!(event.payload, vec![1, 2, 3]);
        assert_eq!(event.metadata.content_type, "application/x-msgpack");
        assert_eq!(event.metadata.priority, EventPriority::Normal);
        assert_eq!(event.metadata.schema_version, 1);
        assert!(!event.metadata.persistent);
        assert_eq!(event.metadata.payload_size, 3);
        assert_eq!(event.metadata.source.subsystem, Subsystem::System);
        assert_eq!(event.metadata.source.component, "unknown");
        assert_eq!(event.metadata.source.node_id, "local");
        assert_eq!(event.metadata.source.instance_id, "default");
    }

    #[test]
    fn test_event_builder_with_source() {
        let event = EventBuilder::new("test.event")
            .unwrap()
            .source(Subsystem::Storage, "blob-store", "node-1", "inst-a")
            .build(vec![]);
        assert_eq!(event.metadata.source.subsystem, Subsystem::Storage);
        assert_eq!(event.metadata.source.component, "blob-store");
        assert_eq!(event.metadata.source.node_id, "node-1");
        assert_eq!(event.metadata.source.instance_id, "inst-a");
    }

    #[test]
    fn test_event_builder_with_ordering_key() {
        let event = EventBuilder::new("test.event")
            .unwrap()
            .ordering_key("my-key")
            .build(vec![]);
        assert_eq!(event.metadata.ordering_key, Some("my-key".to_string()));
    }

    #[test]
    fn test_event_builder_with_ttl() {
        let event = EventBuilder::new("test.event")
            .unwrap()
            .ttl(5000)
            .build(vec![]);
        assert_eq!(event.metadata.ttl_ms, 5000);
    }

    #[test]
    fn test_event_builder_with_priority() {
        let event = EventBuilder::new("test.event")
            .unwrap()
            .priority(EventPriority::High)
            .build(vec![]);
        assert_eq!(event.metadata.priority, EventPriority::High);
    }

    #[test]
    fn test_event_builder_persistent() {
        let event = EventBuilder::new("test.event")
            .unwrap()
            .persistent(true)
            .build(vec![]);
        assert!(event.metadata.persistent);
    }

    #[test]
    fn test_event_builder_large_payload() {
        let payload = vec![0u8; 1024 * 1024];
        let event = EventBuilder::new("test.event").unwrap().build(payload.clone());
        assert_eq!(event.metadata.payload_size, payload.len() as u32);
        assert_eq!(event.payload.len(), 1024 * 1024);
    }

    #[test]
    fn test_event_builder_rejects_empty_type() {
        let result = EventBuilder::new("");
        assert!(result.is_err());
    }

    #[test]
    fn test_event_metadata_has_event_id() {
        let event = EventBuilder::new("test.event").unwrap().build(vec![]);
        let id = event.metadata.event_id;
        let ts = id.timestamp();
        assert!(ts > 0);
    }

    #[test]
    fn test_event_metadata_timestamp_matches_event_id() {
        let event = EventBuilder::new("test.event").unwrap().build(vec![]);
        assert_eq!(event.metadata.timestamp, event.metadata.event_id.timestamp());
    }

    #[test]
    fn test_subsystem_variants() {
        assert_eq!(Subsystem::Storage as u8, 0);
        assert_eq!(Subsystem::Execution as u8, 1);
        assert_eq!(Subsystem::Auth as u8, 2);
        assert_eq!(Subsystem::Queue as u8, 3);
        assert_eq!(Subsystem::Scheduler as u8, 4);
        assert_eq!(Subsystem::Search as u8, 5);
        assert_eq!(Subsystem::Blob as u8, 6);
        assert_eq!(Subsystem::Api as u8, 7);
        assert_eq!(Subsystem::System as u8, 8);
    }

    #[test]
    fn test_event_priority_ordering() {
        assert!(EventPriority::Low < EventPriority::Normal);
        assert!(EventPriority::Normal < EventPriority::High);
        assert!(EventPriority::High < EventPriority::Critical);
    }

    #[test]
    fn test_event_clone() {
        let event = EventBuilder::new("test.event")
            .unwrap()
            .priority(EventPriority::Critical)
            .build(vec![10, 20, 30]);
        let cloned = event.clone();
        assert_eq!(event.metadata.event_id, cloned.metadata.event_id);
        assert_eq!(event.payload, cloned.payload);
    }

    #[test]
    fn test_trace_context_default() {
        let event = EventBuilder::new("test.event").unwrap().build(vec![]);
        assert!(event.metadata.trace_context.is_none());
    }

    #[test]
    fn test_event_source_debug() {
        let source = EventSource {
            subsystem: Subsystem::Api,
            component: "http".into(),
            node_id: "n1".into(),
            instance_id: "i1".into(),
        };
        let debug = format!("{:?}", source);
        assert!(debug.contains("Api"));
        assert!(debug.contains("http"));
    }
}
