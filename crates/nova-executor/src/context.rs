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
