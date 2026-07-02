use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OperationType {
    Get, List, Create, Update, Delete, Patch,
    Query, Search,
    Enqueue, Dequeue, Peek, Ack,
    Schedule, Cancel,
    BlobPut, BlobGet, BlobDelete,
    Authenticate, Authorize, CreateToken, RevokeToken,
    Health, Metrics, Config, Profile, AdminAction,
}

impl OperationType {
    pub fn is_mutation(&self) -> bool {
        matches!(self, OperationType::Create | OperationType::Update
            | OperationType::Delete | OperationType::Patch
            | OperationType::Enqueue | OperationType::Dequeue
            | OperationType::Ack | OperationType::Schedule
            | OperationType::Cancel | OperationType::BlobPut
            | OperationType::BlobDelete | OperationType::CreateToken
            | OperationType::RevokeToken | OperationType::AdminAction)
    }

    pub fn is_read(&self) -> bool {
        matches!(self, OperationType::Get | OperationType::List
            | OperationType::Peek | OperationType::BlobGet
            | OperationType::Health | OperationType::Metrics
            | OperationType::Config | OperationType::Profile
            | OperationType::Query | OperationType::Search)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Priority {
    #[default]
    Critical = 0,
    High = 1,
    Normal = 2,
    Background = 3,
}

impl Priority {
    pub fn max_wait(&self) -> Duration {
        match self {
            Priority::Critical => Duration::from_millis(100),
            Priority::High => Duration::from_millis(500),
            Priority::Normal => Duration::from_secs(2),
            Priority::Background => Duration::from_secs(10),
        }
    }

    pub fn age_up(&self) -> Option<Self> {
        match self {
            Priority::Background => Some(Priority::Normal),
            Priority::Normal => Some(Priority::High),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Protocol {
    Http,
    WebSocket,
    Sql,
    Admin,
    Internal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PipelineStage {
    Parse,
    Validate,
    Authorize,
    Execute,
    Log,
    Notify,
}

impl PipelineStage {
    pub fn all() -> &'static [PipelineStage] {
        &[
            PipelineStage::Parse,
            PipelineStage::Validate,
            PipelineStage::Authorize,
            PipelineStage::Execute,
            PipelineStage::Log,
            PipelineStage::Notify,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StageStatus {
    Success,
    Skipped,
    ShortCircuit,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Consistency {
    Eventual,
    Strong,
}

impl Default for Consistency {
    fn default() -> Self { Consistency::Strong }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Durability {
    Async,
    Sync,
    Durable,
}

impl Default for Durability {
    fn default() -> Self { Durability::Sync }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StatusCode {
    Ok = 200,
    Created = 201,
    Accepted = 202,
    NoContent = 204,
    BadRequest = 400,
    Unauthorized = 401,
    Forbidden = 403,
    NotFound = 404,
    MethodNotAllowed = 405,
    Conflict = 409,
    Gone = 410,
    TooManyRequests = 429,
    RequestTooLarge = 413,
    UnprocessableEntity = 422,
    InternalError = 500,
    NotImplemented = 501,
    ServiceUnavailable = 503,
    GatewayTimeout = 504,
    InsufficientStorage = 507,
    CircuitBreakerOpen,
    DeadlineExceeded,
    Cancelled = 499,
}

impl StatusCode {
    pub fn is_success(&self) -> bool {
        matches!(self, StatusCode::Ok | StatusCode::Created
            | StatusCode::Accepted | StatusCode::NoContent)
    }

    pub fn is_client_error(&self) -> bool {
        matches!(self, StatusCode::BadRequest | StatusCode::Unauthorized
            | StatusCode::Forbidden | StatusCode::NotFound
            | StatusCode::MethodNotAllowed | StatusCode::Conflict
            | StatusCode::Gone | StatusCode::TooManyRequests
            | StatusCode::RequestTooLarge | StatusCode::UnprocessableEntity
            | StatusCode::Cancelled)
    }

    pub fn is_server_error(&self) -> bool {
        matches!(self, StatusCode::InternalError | StatusCode::NotImplemented
            | StatusCode::ServiceUnavailable | StatusCode::GatewayTimeout
            | StatusCode::InsufficientStorage | StatusCode::CircuitBreakerOpen
            | StatusCode::DeadlineExceeded)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OperationTarget {
    Object { type_name: String, id: Option<u128> },
    Collection { type_name: String },
    Queue { name: String },
    Schedule { task_id: Option<u128> },
    Blob { blob_id: Option<u128> },
    Auth { realm: String },
    Admin { endpoint: String },
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationOptions {
    #[serde(default)]
    pub consistency: Consistency,
    #[serde(default)]
    pub durability: Durability,
    pub ttl: Option<Duration>,
    #[serde(default)]
    pub priority: Priority,
    pub idempotency_key: Option<u128>,
    pub timeout: Option<Duration>,
    #[serde(default = "default_max_retries")]
    pub max_retries: u8,
    #[serde(default)]
    pub tracing: bool,
}

fn default_max_retries() -> u8 { 3 }

impl Default for OperationOptions {
    fn default() -> Self {
        OperationOptions {
            consistency: Consistency::Strong,
            durability: Durability::Sync,
            ttl: None,
            priority: Priority::Normal,
            idempotency_key: None,
            timeout: None,
            max_retries: 3,
            tracing: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct OperationRequest {
    pub operation_type: OperationType,
    pub target: OperationTarget,
    pub params: HashMap<String, serde_json::Value>,
    pub payload: Option<Vec<u8>>,
    pub payload_size: u64,
    pub options: OperationOptions,
    pub sequence: u64,
    pub submitted_at: Instant,
}

impl OperationRequest {
    pub fn new(operation_type: OperationType, target: OperationTarget) -> Self {
        OperationRequest {
            operation_type,
            target,
            params: HashMap::new(),
            payload: None,
            payload_size: 0,
            options: OperationOptions::default(),
            sequence: 0,
            submitted_at: Instant::now(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ErrorInfo {
    pub code: ErrorCode,
    pub message: String,
    pub details: serde_json::Value,
    pub retryable: bool,
    pub retry_after_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ErrorCode {
    ParseError,
    ValidationError,
    AuthorizationError,
    AuthenticationError,
    NotFound,
    Conflict,
    RateLimited,
    CircuitBreakerOpen,
    DeadlineExceeded,
    Cancelled,
    PayloadTooLarge,
    Unprocessable,
    InternalError,
    ServiceUnavailable,
    NotImplemented,
    InsufficientStorage,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StageTiming {
    pub stage: PipelineStage,
    pub duration_ns: u64,
    pub status: StageStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OperationResponse {
    pub status: StatusCode,
    pub success: bool,
    pub data: Option<serde_json::Value>,
    pub data_size: u64,
    pub trace_id: u128,
    pub duration_ns: u64,
    pub error: Option<ErrorInfo>,
    pub warnings: Vec<String>,
    pub stage_timings: Vec<StageTiming>,
}

impl OperationResponse {
    pub fn ok(data: serde_json::Value) -> Self {
        OperationResponse {
            status: StatusCode::Ok,
            success: true,
            data: Some(data),
            data_size: 0,
            trace_id: 0,
            duration_ns: 0,
            error: None,
            warnings: Vec::new(),
            stage_timings: Vec::new(),
        }
    }

    pub fn error(code: ErrorCode, message: impl Into<String>) -> Self {
        let msg = message.into();
        let status = match code {
            ErrorCode::ParseError => StatusCode::BadRequest,
            ErrorCode::ValidationError => StatusCode::UnprocessableEntity,
            ErrorCode::AuthorizationError => StatusCode::Forbidden,
            ErrorCode::AuthenticationError => StatusCode::Unauthorized,
            ErrorCode::NotFound => StatusCode::NotFound,
            ErrorCode::Conflict => StatusCode::Conflict,
            ErrorCode::RateLimited => StatusCode::TooManyRequests,
            ErrorCode::CircuitBreakerOpen => StatusCode::ServiceUnavailable,
            ErrorCode::DeadlineExceeded => StatusCode::DeadlineExceeded,
            ErrorCode::Cancelled => StatusCode::Cancelled,
            ErrorCode::PayloadTooLarge => StatusCode::RequestTooLarge,
            ErrorCode::Unprocessable => StatusCode::UnprocessableEntity,
            ErrorCode::InternalError => StatusCode::InternalError,
            ErrorCode::ServiceUnavailable => StatusCode::ServiceUnavailable,
            ErrorCode::NotImplemented => StatusCode::NotImplemented,
            ErrorCode::InsufficientStorage => StatusCode::InsufficientStorage,
        };
        let retryable = matches!(code, ErrorCode::RateLimited
            | ErrorCode::CircuitBreakerOpen | ErrorCode::DeadlineExceeded
            | ErrorCode::ServiceUnavailable | ErrorCode::InternalError);
        OperationResponse {
            status,
            success: false,
            data: None,
            data_size: 0,
            trace_id: 0,
            duration_ns: 0,
            error: Some(ErrorInfo {
                code,
                message: msg,
                details: serde_json::Value::Null,
                retryable,
                retry_after_ms: None,
            }),
            warnings: Vec::new(),
            stage_timings: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SubsystemId {
    Storage,
    Cache,
    Queue,
    Scheduler,
    Search,
    Blob,
    Auth,
    Pipeline,
    Admin,
}

#[derive(Debug, Clone)]
pub struct OperationContext {
    pub trace_id: u128,
    pub span_id: u64,
    pub parent_span_id: Option<u64>,
    pub user_session: Option<UserSession>,
    pub source_addr: SocketAddr,
    pub deadline: Instant,
    pub cancellation_token: crate::CancellationToken,
    pub operation_priority: Priority,
    pub protocol: Protocol,
    pub subsystem: SubsystemId,
    pub operation_type: OperationType,
    pub metadata: HashMap<String, String>,
    pub stage: PipelineStage,
    pub stage_elapsed: Duration,
    pub total_elapsed: Duration,
    pub retry_count: u8,
    pub kv_store: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct UserSession {
    pub user_id: u128,
    pub username: String,
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
    pub session_id: u128,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PipelineResult {
    Continue,
    ShortCircuit(OperationResponse),
    Error(PipelineError),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PipelineError {
    pub code: ErrorCode,
    pub message: String,
    pub details: serde_json::Value,
    pub stage: PipelineStage,
    pub retryable: bool,
}

impl PipelineError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        let msg = message.into();
        PipelineError {
            code,
            message: msg,
            details: serde_json::Value::Null,
            stage: PipelineStage::Parse,
            retryable: matches!(code, ErrorCode::RateLimited
                | ErrorCode::CircuitBreakerOpen | ErrorCode::DeadlineExceeded
                | ErrorCode::ServiceUnavailable),
        }
    }

    pub fn with_stage(mut self, stage: PipelineStage) -> Self {
        self.stage = stage;
        self
    }
}

impl From<PipelineError> for PipelineResult {
    fn from(err: PipelineError) -> Self {
        PipelineResult::Error(err)
    }
}

impl OperationContext {
    pub fn remaining_deadline(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }

    pub fn is_expired(&self) -> bool {
        Instant::now() >= self.deadline
    }
}

pub struct AuditEntry {
    pub id: u128,
    pub timestamp: u64,
    pub trace_id: u128,
    pub user_id: Option<u128>,
    pub session_id: Option<u128>,
    pub source_addr: std::net::IpAddr,
    pub source_port: u16,
    pub protocol: Protocol,
    pub operation: OperationType,
    pub target: OperationTarget,
    pub status: StatusCode,
    pub error: Option<String>,
    pub duration_ns: u64,
    pub payload_size: u64,
    pub idempotency_key: Option<u128>,
    pub metadata: HashMap<String, String>,
}

pub struct IdempotencyRecord {
    pub key: u128,
    pub trace_id: u128,
    pub response: Option<OperationResponse>,
    pub created_at: Instant,
    pub ttl: Duration,
    pub expires_at: Instant,
}
