use async_graphql::*;

// ============================================================
// Common Inputs
// ============================================================

#[derive(InputObject)]
pub struct PaginationInput {
    pub first: Option<i32>,
    pub after: Option<String>,
    pub last: Option<i32>,
    pub before: Option<String>,
}

#[derive(InputObject)]
pub struct SortInput {
    pub field: String,
    pub direction: Option<SortDirection>,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum SortDirection {
    Asc,
    Desc,
}

// ============================================================
// Runtime Inputs
// ============================================================

#[derive(InputObject)]
pub struct MetricsInput {
    pub since: Option<String>,
    pub resolution: Option<MetricsResolutionInput>,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum MetricsResolutionInput {
    OneSecond,
    OneMinute,
    FiveMinutes,
    FifteenMinutes,
    OneHour,
}

#[derive(InputObject)]
pub struct ConfigurationInput {
    pub log_level: Option<String>,
    pub query_timeout_ms: Option<i32>,
    pub max_connections: Option<i32>,
}

// ============================================================
// Cache Inputs
// ============================================================

#[derive(InputObject)]
pub struct CacheSetInput {
    pub key: String,
    pub value: serde_json::Value,
    pub ttl_ms: Option<i64>,
}

// ============================================================
// Queue Inputs
// ============================================================

#[derive(InputObject)]
pub struct CreateQueueInput {
    pub name: String,
    pub description: Option<String>,
}

#[derive(InputObject)]
pub struct QueueSendInput {
    pub body: serde_json::Value,
    pub content_type: Option<String>,
    pub delay_ms: Option<i32>,
    pub priority: Option<MessagePriorityInput>,
    pub deduplication_id: Option<String>,
    pub group_id: Option<String>,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum MessagePriorityInput {
    Low,
    Normal,
    High,
    Critical,
}

// ============================================================
// Scheduler Inputs
// ============================================================

#[derive(InputObject)]
pub struct CreateJobInput {
    pub name: String,
    pub description: Option<String>,
    pub job_type: JobTypeInput,
    pub schedule: Option<String>,
    pub max_retries: Option<i32>,
    pub timeout_ms: Option<i32>,
    pub tags: Option<Vec<String>>,
    pub input: Option<serde_json::Value>,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum JobTypeInput {
    Cron,
    ScheduledOnce,
    EventDriven,
}

#[derive(InputObject)]
pub struct UpdateJobInput {
    pub name: Option<String>,
    pub description: Option<String>,
    pub schedule: Option<String>,
    pub max_retries: Option<i32>,
    pub timeout_ms: Option<i32>,
    pub tags: Option<Vec<String>>,
    pub input: Option<serde_json::Value>,
}

// ============================================================
// Search Inputs
// ============================================================

#[derive(InputObject)]
pub struct SearchOptions {
    pub fields: Option<Vec<String>>,
    pub min_score: Option<f64>,
}

#[derive(InputObject)]
pub struct CreateSearchIndexInput {
    pub name: String,
    pub fields: Vec<IndexFieldInput>,
    pub analyzer: Option<String>,
}

#[derive(InputObject)]
pub struct IndexFieldInput {
    pub name: String,
    pub field_type: IndexFieldTypeInput,
    pub searchable: Option<bool>,
    pub sortable: Option<bool>,
    pub facetable: Option<bool>,
    pub stored: Option<bool>,
    pub boost: Option<f64>,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum IndexFieldTypeInput {
    Text,
    Keyword,
    Integer,
    Float,
    Boolean,
    Date,
    Object,
    Array,
    GeoPoint,
    IpAddress,
}

// ============================================================
// Blob Inputs
// ============================================================

#[derive(InputObject)]
pub struct BlobUploadInput {
    pub key: String,
    pub content: String,
    pub content_type: Option<String>,
    pub content_encoding: Option<String>,
    pub storage_tier: Option<StorageTierInput>,
    pub expires_at: Option<String>,
    pub metadata: Option<BlobMetadataInput>,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum StorageTierInput {
    Hot,
    Warm,
    Cold,
}

#[derive(InputObject)]
pub struct BlobMetadataInput {
    pub filename: Option<String>,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
    pub custom: Option<serde_json::Value>,
}

// ============================================================
// Auth Inputs
// ============================================================

#[derive(InputObject)]
pub struct LoginInput {
    pub username: Option<String>,
    pub email: Option<String>,
    pub password: String,
}

#[derive(InputObject)]
pub struct RegisterInput {
    pub username: String,
    pub email: String,
    pub password: String,
    pub display_name: String,
}

#[derive(InputObject)]
pub struct CreateApiKeyInput {
    pub name: String,
    pub permissions: Vec<String>,
    pub roles: Option<Vec<String>>,
    pub expires_at: Option<String>,
}
