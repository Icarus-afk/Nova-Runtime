use async_graphql::*;

// ============================================================
// Common Types
// ============================================================

#[derive(SimpleObject)]
pub struct PageInfo {
    pub has_next_page: bool,
    pub has_previous_page: bool,
    pub start_cursor: Option<String>,
    pub end_cursor: Option<String>,
}

// ============================================================
// Runtime Types
// ============================================================

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum HealthState {
    Healthy,
    Degraded,
    Unhealthy,
}

#[derive(SimpleObject)]
pub struct HealthStatus {
    pub status: HealthState,
    pub uptime_seconds: i64,
    pub version: String,
    pub subsystems: Vec<SubsystemHealth>,
    pub last_startup: String,
}

#[derive(SimpleObject)]
pub struct SubsystemHealth {
    pub name: String,
    pub status: HealthState,
    pub latency_ms: f64,
    pub last_error: Option<String>,
    pub last_checked: String,
}

#[derive(SimpleObject)]
pub struct ServerConfiguration {
    pub version: String,
    pub build_mode: String,
    pub log_level: String,
    pub max_connections: i32,
    pub query_timeout_ms: i32,
    pub subsystems: SubsystemConfigs,
}

#[derive(SimpleObject)]
pub struct SubsystemConfigs {
    pub database: DatabaseConfig,
    pub cache: CacheConfig,
    pub queue: QueueConfig,
    pub scheduler: SchedulerConfig,
    pub search: SearchConfig,
    pub blob: BlobConfig,
    pub auth: AuthConfig,
}

#[derive(SimpleObject)]
pub struct DatabaseConfig {
    pub max_connections: i32,
    pub statement_cache_size: i32,
    pub default_fetch_size: i32,
    pub transaction_timeout_ms: i32,
}

#[derive(SimpleObject)]
pub struct CacheConfig {
    pub max_memory_mb: i32,
    pub default_ttl_ms: i64,
    pub eviction_policy: String,
    pub max_item_size_bytes: i32,
}

#[derive(SimpleObject)]
pub struct QueueConfig {
    pub max_queues: i32,
    pub default_visibility_timeout_ms: i32,
    pub max_message_size_bytes: i32,
    pub message_retention_ms: i64,
    pub dead_letter_max_receives: i32,
}

#[derive(SimpleObject)]
pub struct SchedulerConfig {
    pub max_jobs: i32,
    pub scheduler_interval_ms: i32,
    pub max_retries: i32,
    pub default_timeout_ms: i32,
}

#[derive(SimpleObject)]
pub struct SearchConfig {
    pub max_indexes: i32,
    pub default_analyzer: String,
    pub max_result_window: i32,
}

#[derive(SimpleObject)]
pub struct BlobConfig {
    pub max_blob_size_mb: i32,
    pub storage_path: String,
    pub default_tier: String,
}

#[derive(SimpleObject)]
pub struct AuthConfig {
    pub token_expiry_ms: i64,
    pub refresh_token_expiry_ms: i64,
    pub max_api_keys_per_user: i32,
    pub session_timeout_ms: i64,
    pub password_min_length: i32,
    pub bcrypt_cost: i32,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum MetricsResolution {
    OneSecond,
    OneMinute,
    FiveMinutes,
    FifteenMinutes,
    OneHour,
}

#[derive(SimpleObject)]
pub struct MetricsSnapshot {
    pub collected_at: String,
    pub time_range: MetricsTimeRange,
    pub system: SystemMetrics,
    pub subsystems: SubsystemMetrics,
}

#[derive(SimpleObject)]
pub struct MetricsTimeRange {
    pub start: String,
    pub end: String,
}

#[derive(SimpleObject)]
pub struct SystemMetrics {
    pub cpu_usage_percent: f64,
    pub memory_usage_bytes: i64,
    pub memory_total_bytes: i64,
    pub disk_usage_bytes: i64,
    pub disk_total_bytes: i64,
    pub network_bytes_in: i64,
    pub network_bytes_out: i64,
    pub open_file_descriptors: i32,
    pub goroutines: i32,
}

#[derive(SimpleObject)]
pub struct SubsystemMetrics {
    pub database: Option<DatabaseMetrics>,
    pub cache: Option<CacheMetrics>,
    pub queue: Option<QueueMetrics>,
    pub scheduler: Option<SchedulerMetrics>,
    pub search: Option<SearchMetrics>,
    pub blob: Option<BlobMetrics>,
}

#[derive(SimpleObject)]
pub struct DatabaseMetrics {
    pub queries_total: i64,
    pub queries_per_second: f64,
    pub avg_latency_ms: f64,
    pub p50_latency_ms: f64,
    pub p95_latency_ms: f64,
    pub p99_latency_ms: f64,
    pub active_connections: i32,
    pub cache_hit_rate: f64,
    pub transactions_committed: i64,
    pub transactions_rolled_back: i64,
}

#[derive(SimpleObject)]
pub struct CacheMetrics {
    pub hits: i64,
    pub misses: i64,
    pub hit_rate: f64,
    pub entries: i32,
    pub memory_used_bytes: i64,
    pub evictions: i64,
    pub avg_ttl_remaining_ms: f64,
}

#[derive(SimpleObject)]
pub struct QueueMetrics {
    pub messages_sent: i64,
    pub messages_received: i64,
    pub messages_deleted: i64,
    pub messages_dead_lettered: i64,
    pub queues_count: i32,
    pub total_messages: i64,
    pub avg_latency_ms: f64,
    pub dead_letter_count: i64,
}

#[derive(SimpleObject)]
pub struct SchedulerMetrics {
    pub jobs_executed: i64,
    pub jobs_failed: i64,
    pub jobs_skipped: i64,
    pub active_jobs: i32,
    pub avg_execution_time_ms: f64,
    pub p95_execution_time_ms: f64,
    pub triggers_fired: i64,
}

#[derive(SimpleObject)]
pub struct SearchMetrics {
    pub queries_total: i64,
    pub indexing_total: i64,
    pub avg_query_latency_ms: f64,
    pub p95_query_latency_ms: f64,
    pub indexes_count: i32,
    pub documents_indexed: i64,
    pub avg_index_time_ms: f64,
}

#[derive(SimpleObject)]
pub struct BlobMetrics {
    pub uploads_total: i64,
    pub downloads_total: i64,
    pub deletes_total: i64,
    pub total_blobs: i64,
    pub total_storage_bytes: i64,
    pub avg_upload_size_bytes: f64,
    pub avg_download_latency_ms: f64,
}

#[derive(SimpleObject)]
pub struct VersionInfo {
    pub version: String,
    pub build_commit: String,
    pub build_date: String,
    pub rust_version: String,
}

// ============================================================
// Database Types
// ============================================================

#[derive(SimpleObject)]
pub struct SqlQueryResult {
    pub columns: Vec<ColumnInfo>,
    pub rows: Vec<serde_json::Value>,
    pub row_count: i32,
    pub execution_time_ms: f64,
    pub warnings: Vec<String>,
}

#[derive(SimpleObject)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub primary_key: bool,
    pub default_value: Option<serde_json::Value>,
    pub comment: Option<String>,
}

#[derive(SimpleObject)]
pub struct TableInfo {
    pub name: String,
    pub schema: String,
    pub columns: Vec<ColumnInfo>,
    pub primary_key: Vec<String>,
    pub row_count: i64,
    pub size_bytes: i64,
    pub created_at: String,
    pub updated_at: String,
    pub comment: Option<String>,
}

#[derive(SimpleObject)]
pub struct SchemaInfo {
    pub version: i32,
    pub tables: i32,
    pub size_bytes: i64,
    pub last_migration: String,
}

#[derive(SimpleObject)]
pub struct DatabaseStats {
    pub query_count: i64,
    pub avg_query_time_ms: f64,
    pub p95_query_time_ms: f64,
    pub cache_hit_rate: f64,
    pub active_connections: i32,
    pub deadlocks_detected: i32,
    pub transactions: TransactionStats,
}

#[derive(SimpleObject)]
pub struct TransactionStats {
    pub committed: i64,
    pub rolled_back: i64,
    pub active: i32,
    pub avg_duration_ms: f64,
}

// ============================================================
// Cache Types
// ============================================================

#[derive(SimpleObject)]
pub struct CacheEntry {
    pub key: String,
    pub value: serde_json::Value,
    pub data_type: String,
    pub ttl_ms: Option<i64>,
    pub expires_at: Option<String>,
    pub size_bytes: i32,
    pub created_at: String,
    pub last_accessed_at: String,
    pub access_count: i64,
}

#[derive(SimpleObject)]
pub struct CacheStats {
    pub hit_count: i64,
    pub miss_count: i64,
    pub hit_rate: f64,
    pub entry_count: i32,
    pub memory_used_bytes: i64,
    pub max_memory_bytes: i64,
    pub eviction_count: i64,
    pub avg_ttl_ms: f64,
    pub keyspace_hits: i64,
    pub keyspace_misses: i64,
}

// ============================================================
// Queue Types
// ============================================================

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum MessagePriority {
    Low,
    Normal,
    High,
    Critical,
}

#[derive(SimpleObject)]
pub struct Queue {
    pub name: String,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub message_count: i64,
    pub messages_sent: i64,
    pub messages_received: i64,
    pub messages_deleted: i64,
    pub messages_dead_lettered: i64,
    pub oldest_message_age_ms: i64,
    pub config: QueueConfigStats,
}

#[derive(SimpleObject)]
pub struct QueueConfigStats {
    pub visibility_timeout_ms: i32,
    pub max_message_size_bytes: i32,
    pub message_retention_ms: i64,
    pub dead_letter_max_receives: i32,
    pub dead_letter_queue: bool,
    pub delivery_delay_ms: i32,
}

#[derive(SimpleObject)]
pub struct QueueOverallStats {
    pub total_queues: i32,
    pub total_messages: i64,
    pub total_messages_sent: i64,
    pub total_messages_received: i64,
    pub total_messages_dead_lettered: i64,
    pub avg_queue_depth: f64,
    pub avg_processing_time_ms: f64,
}

#[derive(SimpleObject)]
pub struct QueueMessage {
    pub id: uuid::Uuid,
    pub body: serde_json::Value,
    pub content_type: String,
    pub sent_at: String,
    pub first_received_at: Option<String>,
    pub receive_count: i32,
    pub visibility_timeout_expires_at: Option<String>,
    pub delay_until: Option<String>,
    pub attributes: MessageAttributes,
}

#[derive(SimpleObject)]
pub struct MessageAttributes {
    pub priority: MessagePriority,
    pub deduplication_id: Option<String>,
    pub group_id: Option<String>,
    pub sender: Option<String>,
    pub custom: Option<serde_json::Value>,
}

#[derive(SimpleObject)]
pub struct DeadLetterStats {
    pub total_dead_lettered: i64,
    pub total_dead_letter_queues: i32,
    pub top_reasons: Vec<DeadLetterReason>,
}

#[derive(SimpleObject)]
pub struct DeadLetterReason {
    pub reason: String,
    pub count: i64,
    pub last_occurrence: String,
}

// ============================================================
// Scheduler Types
// ============================================================

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum JobType {
    Cron,
    ScheduledOnce,
    EventDriven,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum JobStateEnum {
    Active,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum ExecutionStatus {
    Pending,
    Running,
    Success,
    Failed,
    Skipped,
    Timeout,
    Cancelled,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum ExecutionTrigger {
    Scheduled,
    Manual,
    Event,
    Retry,
}

#[derive(SimpleObject)]
pub struct Job {
    pub id: uuid::Uuid,
    pub name: String,
    pub description: Option<String>,
    pub job_type: JobType,
    pub state: JobStateEnum,
    pub schedule: Option<CronExpression>,
    pub max_retries: i32,
    pub retry_count: i32,
    pub timeout_ms: i32,
    pub created_at: String,
    pub updated_at: String,
    pub last_executed_at: Option<String>,
    pub last_error: Option<String>,
    pub next_execution_at: Option<String>,
    pub tags: Vec<String>,
    pub input: Option<serde_json::Value>,
    pub metadata: JobMetadata,
}

#[derive(SimpleObject)]
pub struct CronExpression {
    pub expression: String,
    pub description: String,
    pub timezone: String,
    pub next_fire_times: Vec<String>,
}

#[derive(SimpleObject)]
pub struct JobMetadata {
    pub total_executions: i64,
    pub successful_executions: i64,
    pub failed_executions: i64,
    pub avg_duration_ms: f64,
    pub total_duration_ms: i64,
    pub last_execution_id: Option<uuid::Uuid>,
}

#[derive(SimpleObject)]
pub struct JobExecution {
    pub id: uuid::Uuid,
    pub job_id: uuid::Uuid,
    pub job_name: String,
    pub status: ExecutionStatus,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub duration_ms: f64,
    pub retry_attempt: i32,
    pub trigger: ExecutionTrigger,
    pub input: Option<serde_json::Value>,
    pub output: Option<serde_json::Value>,
    pub error: Option<ExecutionError>,
    pub logs: Vec<ExecutionLogEntry>,
}

#[derive(SimpleObject)]
pub struct ExecutionError {
    pub message: String,
    pub code: String,
    pub stack_trace: Option<String>,
    pub subsystem: Option<String>,
}

#[derive(SimpleObject)]
pub struct ExecutionLogEntry {
    pub timestamp: String,
    pub level: String,
    pub message: String,
    pub metadata: Option<serde_json::Value>,
}

#[derive(SimpleObject)]
pub struct SchedulerStats {
    pub total_jobs: i32,
    pub active_jobs: i32,
    pub paused_jobs: i32,
    pub failed_jobs: i32,
    pub completed_jobs: i32,
    pub executions_total: i64,
    pub executions_today: i64,
    pub avg_execution_time_ms: f64,
    pub p95_execution_time_ms: f64,
    pub p99_execution_time_ms: f64,
    pub success_rate: f64,
    pub triggers_fired_total: i64,
}

// ============================================================
// Search Types
// ============================================================

#[derive(SimpleObject)]
pub struct SearchResultConnection {
    pub edges: Vec<SearchResultEdge>,
    pub page_info: PageInfo,
    pub total_count: i32,
    pub max_score: f64,
    pub took_ms: f64,
}

#[derive(SimpleObject)]
pub struct SearchResultEdge {
    pub node: SearchResult,
    pub cursor: String,
    pub score: f64,
}

#[derive(SimpleObject)]
pub struct SearchResult {
    pub id: uuid::Uuid,
    pub index: String,
    pub document: serde_json::Value,
    pub score: f64,
}

#[derive(SimpleObject)]
pub struct SearchIndex {
    pub name: String,
    pub document_count: i64,
    pub size_bytes: i64,
    pub field_count: i32,
    pub analyzer: String,
    pub created_at: String,
    pub updated_at: String,
    pub fields: Vec<IndexField>,
}

#[derive(SimpleObject)]
pub struct IndexField {
    pub name: String,
    pub field_type: String,
    pub searchable: bool,
    pub sortable: bool,
    pub facetable: bool,
    pub stored: bool,
    pub analyzer: Option<String>,
    pub boost: f64,
}

#[derive(SimpleObject)]
pub struct Suggestion {
    pub text: String,
    pub score: f64,
    pub frequency: i32,
    pub payload: Option<serde_json::Value>,
}

#[derive(SimpleObject)]
pub struct SearchStats {
    pub total_indexes: i32,
    pub total_documents: i64,
    pub total_size_bytes: i64,
    pub avg_index_time_ms: f64,
    pub avg_query_time_ms: f64,
    pub p95_query_time_ms: f64,
    pub queries_total: i64,
    pub indexing_total: i64,
}

#[derive(SimpleObject)]
pub struct SearchDocument {
    pub id: uuid::Uuid,
    pub index: String,
    pub document: serde_json::Value,
    pub indexed_at: String,
    pub last_updated_at: String,
    pub version: i32,
}

// ============================================================
// Blob Types
// ============================================================

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum StorageTier {
    Hot,
    Warm,
    Cold,
}

#[derive(SimpleObject)]
pub struct Blob {
    pub key: String,
    pub size_bytes: i64,
    pub content_type: String,
    pub content_encoding: Option<String>,
    pub etag: String,
    pub md5: String,
    pub sha256: String,
    pub storage_tier: StorageTier,
    pub created_at: String,
    pub updated_at: String,
    pub expires_at: Option<String>,
    pub metadata: BlobMetadata,
    pub url: String,
}

#[derive(SimpleObject)]
pub struct BlobMetadata {
    pub filename: Option<String>,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub custom: Option<serde_json::Value>,
}

#[derive(SimpleObject)]
pub struct BlobStats {
    pub total_blobs: i64,
    pub total_size_bytes: i64,
    pub total_hot_bytes: i64,
    pub total_warm_bytes: i64,
    pub total_cold_bytes: i64,
    pub avg_blob_size_bytes: f64,
    pub largest_blob_bytes: i64,
    pub uploads_total: i64,
    pub downloads_total: i64,
    pub deletes_total: i64,
}

// ============================================================
// Auth Types
// ============================================================

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum UserStatus {
    Active,
    Inactive,
    Suspended,
    PendingVerification,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum PermissionAction {
    Create,
    Read,
    Update,
    Delete,
    Admin,
    Execute,
}

#[derive(SimpleObject)]
pub struct User {
    pub id: uuid::Uuid,
    pub username: String,
    pub email: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub roles: Vec<Role>,
    pub status: UserStatus,
    pub email_verified: bool,
    pub created_at: String,
    pub updated_at: String,
    pub last_login_at: Option<String>,
    pub metadata: UserMetadata,
}

#[derive(SimpleObject)]
pub struct UserMetadata {
    pub department: Option<String>,
    pub title: Option<String>,
    pub phone: Option<String>,
    pub custom: Option<serde_json::Value>,
}

#[derive(SimpleObject)]
pub struct Role {
    pub name: String,
    pub description: String,
    pub permissions: Vec<Permission>,
    pub is_system: bool,
    pub created_at: String,
    pub updated_at: String,
    pub user_count: i32,
}

#[derive(SimpleObject)]
pub struct Permission {
    pub name: String,
    pub resource: String,
    pub action: PermissionAction,
    pub description: String,
}

#[derive(SimpleObject)]
pub struct ApiKey {
    pub id: uuid::Uuid,
    pub name: String,
    pub key_prefix: String,
    pub permissions: Vec<String>,
    pub roles: Vec<String>,
    pub expires_at: Option<String>,
    pub last_used_at: Option<String>,
    pub created_at: String,
    pub is_active: bool,
}

#[derive(SimpleObject)]
pub struct AuthResult {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
    pub token_type: String,
    pub user: User,
}

#[derive(SimpleObject)]
pub struct ApiKeyFull {
    pub api_key: ApiKey,
    pub raw_key: String,
}

// ============================================================
// Metric Alert Types
// ============================================================

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

#[derive(SimpleObject)]
pub struct MetricAlert {
    pub metric: String,
    pub current_value: f64,
    pub threshold: f64,
    pub severity: AlertSeverity,
    pub timestamp: String,
    pub message: String,
}
