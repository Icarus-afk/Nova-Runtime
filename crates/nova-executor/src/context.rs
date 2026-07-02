use crate::types::*;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use uuid::Uuid;

pub struct OperationContextBuilder {
    trace_id: Option<u128>,
    span_id: Option<u64>,
    parent_span_id: Option<u64>,
    user_session: Option<UserSession>,
    source_addr: SocketAddr,
    deadline: Option<Instant>,
    operation_priority: Priority,
    protocol: Protocol,
    subsystem: SubsystemId,
    operation_type: OperationType,
    metadata: HashMap<String, String>,
    kv_store: HashMap<String, serde_json::Value>,
}

impl OperationContextBuilder {
    pub fn new(source_addr: SocketAddr) -> Self {
        Self {
            trace_id: None,
            span_id: None,
            parent_span_id: None,
            user_session: None,
            source_addr,
            deadline: None,
            operation_priority: Priority::Normal,
            protocol: Protocol::Internal,
            subsystem: SubsystemId::Pipeline,
            operation_type: OperationType::Query,
            metadata: HashMap::new(),
            kv_store: HashMap::new(),
        }
    }

    pub fn trace_id(mut self, id: u128) -> Self {
        self.trace_id = Some(id);
        self
    }

    pub fn span_id(mut self, id: u64) -> Self {
        self.span_id = Some(id);
        self
    }

    pub fn parent_span_id(mut self, id: u64) -> Self {
        self.parent_span_id = Some(id);
        self
    }

    pub fn user_session(mut self, session: UserSession) -> Self {
        self.user_session = Some(session);
        self
    }

    pub fn deadline(mut self, deadline: Instant) -> Self {
        self.deadline = Some(deadline);
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.deadline = Some(Instant::now() + timeout);
        self
    }

    pub fn priority(mut self, priority: Priority) -> Self {
        self.operation_priority = priority;
        self
    }

    pub fn protocol(mut self, protocol: Protocol) -> Self {
        self.protocol = protocol;
        self
    }

    pub fn subsystem(mut self, subsystem: SubsystemId) -> Self {
        self.subsystem = subsystem;
        self
    }

    pub fn operation_type(mut self, op_type: OperationType) -> Self {
        self.operation_type = op_type;
        self
    }

    pub fn metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }

    pub fn kv_store(mut self, key: &str, value: serde_json::Value) -> Self {
        self.kv_store.insert(key.to_string(), value);
        self
    }

    pub fn build(self) -> OperationContext {
        let trace_id = self.trace_id.unwrap_or_else(|| generate_trace_id());
        let span_id = self.span_id.unwrap_or_else(generate_span_id);
        let deadline = self.deadline.unwrap_or_else(|| Instant::now() + Duration::from_secs(5));

        OperationContext {
            trace_id,
            span_id,
            parent_span_id: self.parent_span_id,
            user_session: self.user_session,
            source_addr: self.source_addr,
            deadline,
            cancellation_token: crate::CancellationToken::new(),
            operation_priority: self.operation_priority,
            protocol: self.protocol,
            subsystem: self.subsystem,
            operation_type: self.operation_type,
            metadata: self.metadata,
            stage: PipelineStage::Parse,
            stage_elapsed: Duration::ZERO,
            total_elapsed: Duration::ZERO,
            retry_count: 0,
            kv_store: self.kv_store,
        }
    }
}

static NEXT_SPAN_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

pub fn generate_trace_id() -> u128 {
    Uuid::now_v7().as_u128()
}

pub fn generate_span_id() -> u64 {
    NEXT_SPAN_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;
    use std::time::{Duration, Instant};

    fn test_addr() -> SocketAddr {
        "127.0.0.1:8080".parse().unwrap()
    }

    #[test]
    fn test_builder_creates_context_with_defaults() {
        let ctx = OperationContextBuilder::new(test_addr()).build();
        assert_eq!(ctx.source_addr, test_addr());
        assert_eq!(ctx.operation_priority, Priority::Normal);
        assert_eq!(ctx.protocol, Protocol::Internal);
        assert_eq!(ctx.subsystem, SubsystemId::Pipeline);
        assert_eq!(ctx.operation_type, OperationType::Query);
        assert_eq!(ctx.stage, PipelineStage::Parse);
        assert_eq!(ctx.stage_elapsed, Duration::ZERO);
        assert_eq!(ctx.total_elapsed, Duration::ZERO);
        assert_eq!(ctx.retry_count, 0);
        assert!(ctx.metadata.is_empty());
        assert!(ctx.kv_store.is_empty());
        assert!(ctx.user_session.is_none());
        assert!(ctx.parent_span_id.is_none());
    }

    #[test]
    fn test_builder_sets_trace_id() {
        let ctx = OperationContextBuilder::new(test_addr())
            .trace_id(42)
            .build();
        assert_eq!(ctx.trace_id, 42);
    }

    #[test]
    fn test_builder_sets_span_id() {
        let ctx = OperationContextBuilder::new(test_addr())
            .span_id(100)
            .build();
        assert_eq!(ctx.span_id, 100);
    }

    #[test]
    fn test_builder_sets_parent_span_id() {
        let ctx = OperationContextBuilder::new(test_addr())
            .parent_span_id(99)
            .build();
        assert_eq!(ctx.parent_span_id, Some(99));
    }

    #[test]
    fn test_builder_sets_user_session() {
        let session = UserSession {
            user_id: 1,
            username: "test".into(),
            roles: vec!["admin".into()],
            permissions: vec!["write".into()],
            session_id: 42,
            metadata: HashMap::new(),
        };
        let ctx = OperationContextBuilder::new(test_addr())
            .user_session(session.clone())
            .build();
        assert!(ctx.user_session.is_some());
        let s = ctx.user_session.unwrap();
        assert_eq!(s.user_id, 1);
        assert_eq!(s.username, "test");
        assert_eq!(s.roles, vec!["admin"]);
    }

    #[test]
    fn test_builder_sets_deadline_via_deadline_method() {
        let future = Instant::now() + Duration::from_secs(60);
        let ctx = OperationContextBuilder::new(test_addr())
            .deadline(future)
            .build();
        assert!(ctx.deadline >= future - Duration::from_millis(1));
    }

    #[test]
    fn test_builder_sets_deadline_via_timeout_method() {
        let ctx = OperationContextBuilder::new(test_addr())
            .timeout(Duration::from_secs(30))
            .build();
        let expected = Instant::now() + Duration::from_secs(30);
        let diff = if expected > ctx.deadline { expected - ctx.deadline } else { ctx.deadline - expected };
        assert!(diff < Duration::from_millis(10));
    }

    #[test]
    fn test_builder_sets_priority() {
        let ctx = OperationContextBuilder::new(test_addr())
            .priority(Priority::Critical)
            .build();
        assert_eq!(ctx.operation_priority, Priority::Critical);
    }

    #[test]
    fn test_builder_sets_protocol() {
        let ctx = OperationContextBuilder::new(test_addr())
            .protocol(Protocol::Http)
            .build();
        assert_eq!(ctx.protocol, Protocol::Http);
    }

    #[test]
    fn test_builder_sets_subsystem() {
        let ctx = OperationContextBuilder::new(test_addr())
            .subsystem(SubsystemId::Storage)
            .build();
        assert_eq!(ctx.subsystem, SubsystemId::Storage);
    }

    #[test]
    fn test_builder_sets_operation_type() {
        let ctx = OperationContextBuilder::new(test_addr())
            .operation_type(OperationType::Create)
            .build();
        assert_eq!(ctx.operation_type, OperationType::Create);
    }

    #[test]
    fn test_builder_metadata() {
        let ctx = OperationContextBuilder::new(test_addr())
            .metadata("key1", "value1")
            .metadata("key2", "value2")
            .build();
        assert_eq!(ctx.metadata.get("key1").unwrap(), "value1");
        assert_eq!(ctx.metadata.get("key2").unwrap(), "value2");
        assert_eq!(ctx.metadata.len(), 2);
    }

    #[test]
    fn test_builder_kv_store() {
        let ctx = OperationContextBuilder::new(test_addr())
            .kv_store("key", serde_json::json!("value"))
            .build();
        assert_eq!(ctx.kv_store.get("key").unwrap(), &serde_json::json!("value"));
    }

    #[test]
    fn test_builder_default_deadline() {
        let ctx = OperationContextBuilder::new(test_addr()).build();
        let expected = Instant::now() + Duration::from_secs(5);
        let diff = if expected > ctx.deadline { expected - ctx.deadline } else { ctx.deadline - expected };
        assert!(diff < Duration::from_millis(10), "default deadline should be ~5s from now");
    }

    #[test]
    fn test_context_is_expired_returns_false_when_not_expired() {
        let ctx = OperationContextBuilder::new(test_addr())
            .deadline(Instant::now() + Duration::from_secs(60))
            .build();
        assert!(!ctx.is_expired());
    }

    #[test]
    fn test_context_is_expired_returns_true_when_expired() {
        let ctx = OperationContextBuilder::new(test_addr())
            .deadline(Instant::now() - Duration::from_secs(1))
            .build();
        assert!(ctx.is_expired());
    }

    #[test]
    fn test_context_remaining_deadline() {
        let deadline = Instant::now() + Duration::from_secs(10);
        let ctx = OperationContextBuilder::new(test_addr())
            .deadline(deadline)
            .build();
        let remaining = ctx.remaining_deadline();
        assert!(remaining > Duration::from_secs(9));
        assert!(remaining <= Duration::from_secs(10));
    }

    #[test]
    fn test_context_remaining_deadline_expired() {
        let past = Instant::now() - Duration::from_secs(5);
        let ctx = OperationContextBuilder::new(test_addr())
            .deadline(past)
            .build();
        assert_eq!(ctx.remaining_deadline(), Duration::ZERO);
    }

    #[test]
    fn test_context_trace_id_generated_if_not_set() {
        let ctx1 = OperationContextBuilder::new(test_addr()).build();
        let ctx2 = OperationContextBuilder::new(test_addr()).build();
        // Generated trace IDs should be unique
        assert_ne!(ctx1.trace_id, ctx2.trace_id);
    }

    #[test]
    fn test_context_span_id_generated_if_not_set() {
        let ctx1 = OperationContextBuilder::new(test_addr()).build();
        let ctx2 = OperationContextBuilder::new(test_addr()).build();
        // Generated span IDs should be unique
        assert_ne!(ctx1.span_id, ctx2.span_id);
    }

    #[test]
    fn test_context_has_cancellation_token() {
        let ctx = OperationContextBuilder::new(test_addr()).build();
        assert!(!ctx.cancellation_token.is_cancelled());
    }

    #[test]
    fn test_multiple_metadata_entries() {
        let ctx = OperationContextBuilder::new(test_addr())
            .metadata("trace", "abc123")
            .metadata("source", "test")
            .build();

        assert_eq!(ctx.metadata.len(), 2);
        assert_eq!(ctx.metadata.get("trace").unwrap(), "abc123");
        assert_eq!(ctx.metadata.get("source").unwrap(), "test");
    }
}
