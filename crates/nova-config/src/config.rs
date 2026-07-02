use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const DEFAULT_TOML: &str = r##"
[general]
data_dir = "/var/lib/novad"
pid_file = "/var/run/novad.pid"
max_connections = 1024
shutdown_timeout_ms = 5000
startup_timeout_ms = 30000

[storage]
wal_dir = ""
wal_segment_size = 67108864
fsync_policy = { every_n_ms = 100 }
block_cache_size = 268435456
page_cache_size = 67108864
memtable_size = 67108864
max_blob_size = 10737418240
compression = "snappy"
bloom_filter_bits_per_key = 10

[memory]
max_memory = 1073741824
pressure_threshold_pct = 80
critical_threshold_pct = 95
emergency_reserve = 33554432
gc_threshold_pct = 70

[networking]
listen_address = "127.0.0.1"
listen_port = 8642
tls_enabled = false
tcp_nodelay = true
keepalive_secs = 30

[logging]
level = "info"
format = "text"

[subsystems]
enable_sql = true
enable_cache = true
enable_queue = true
enable_scheduler = true
enable_search = true
enable_blob = true
enable_auth = true
enable_dashboard = true
"##;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FsyncPolicy {
    EveryWrite,
    #[serde(rename = "every_n_ms")]
    EveryNMs(u64),
    Async,
}

impl Default for FsyncPolicy {
    fn default() -> Self {
        FsyncPolicy::EveryNMs(100)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GeneralConfig {
    #[serde(default = "default_data_dir")]
    pub data_dir: PathBuf,
    #[serde(default = "default_pid_file")]
    pub pid_file: PathBuf,
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
    #[serde(default = "default_shutdown_timeout_ms")]
    pub shutdown_timeout_ms: u64,
    #[serde(default = "default_startup_timeout_ms")]
    pub startup_timeout_ms: u64,
}

fn default_data_dir() -> PathBuf { PathBuf::from("/var/lib/novad") }
fn default_pid_file() -> PathBuf { PathBuf::from("/var/run/novad.pid") }
fn default_max_connections() -> u32 { 1024 }
fn default_shutdown_timeout_ms() -> u64 { 5000 }
fn default_startup_timeout_ms() -> u64 { 30000 }

impl Default for GeneralConfig {
    fn default() -> Self {
        GeneralConfig {
            data_dir: default_data_dir(),
            pid_file: default_pid_file(),
            max_connections: default_max_connections(),
            shutdown_timeout_ms: default_shutdown_timeout_ms(),
            startup_timeout_ms: default_startup_timeout_ms(),
        }
    }
}

// ---- Storage ----

fn default_wal_dir() -> PathBuf { PathBuf::from("/var/lib/novad/wal") }
fn default_wal_segment_size() -> u64 { 67_108_864 }
fn default_block_cache_size() -> u64 { 268_435_456 }
fn default_page_cache_size() -> u64 { 67_108_864 }
fn default_memtable_size() -> u64 { 67_108_864 }
fn default_max_blob_size() -> u64 { 10_737_418_240 }
fn default_compression() -> nova_core::Compression { nova_core::Compression::Snappy }
fn default_bloom_filter_bits_per_key() -> u32 { 10 }
fn default_page_size() -> u16 { 8192 }
fn default_wal_page_size() -> u16 { 4096 }
fn default_btree_order() -> u8 { 4 }
fn default_lsm_max_level() -> u8 { 7 }
fn default_bloom_false_positive_rate() -> f64 { 0.01 }
fn default_write_buffer_size() -> u64 { 67_108_864 }
fn default_compaction_threads() -> u8 { 2 }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StorageConfig {
    #[serde(default = "default_wal_dir")]
    pub wal_dir: PathBuf,
    #[serde(default = "default_wal_segment_size")]
    pub wal_segment_size: u64,
    #[serde(default)]
    pub fsync_policy: FsyncPolicy,
    #[serde(default = "default_block_cache_size")]
    pub block_cache_size: u64,
    #[serde(default = "default_page_cache_size")]
    pub page_cache_size: u64,
    #[serde(default = "default_memtable_size")]
    pub memtable_size: u64,
    #[serde(default = "default_max_blob_size")]
    pub max_blob_size: u64,
    #[serde(default = "default_compression")]
    pub compression: nova_core::Compression,
    #[serde(default = "default_bloom_filter_bits_per_key")]
    pub bloom_filter_bits_per_key: u32,
    #[serde(default = "default_page_size")]
    pub page_size: u16,
    #[serde(default = "default_wal_page_size")]
    pub wal_page_size: u16,
    #[serde(default = "default_btree_order")]
    pub btree_order: u8,
    #[serde(default = "default_lsm_max_level")]
    pub lsm_max_level: u8,
    #[serde(default = "default_bloom_false_positive_rate")]
    pub bloom_false_positive_rate: f64,
    #[serde(default = "default_write_buffer_size")]
    pub write_buffer_size: u64,
    #[serde(default = "default_compaction_threads")]
    pub compaction_threads: u8,
}

impl Default for StorageConfig {
    fn default() -> Self {
        StorageConfig {
            wal_dir: default_wal_dir(),
            wal_segment_size: default_wal_segment_size(),
            fsync_policy: FsyncPolicy::default(),
            block_cache_size: default_block_cache_size(),
            page_cache_size: default_page_cache_size(),
            memtable_size: default_memtable_size(),
            max_blob_size: default_max_blob_size(),
            compression: nova_core::Compression::Snappy,
            bloom_filter_bits_per_key: default_bloom_filter_bits_per_key(),
            page_size: default_page_size(),
            wal_page_size: default_wal_page_size(),
            btree_order: default_btree_order(),
            lsm_max_level: default_lsm_max_level(),
            bloom_false_positive_rate: default_bloom_false_positive_rate(),
            write_buffer_size: default_write_buffer_size(),
            compaction_threads: default_compaction_threads(),
        }
    }
}

// ---- Memory ----

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MemoryConfig {
    #[serde(default = "default_max_memory")]
    pub max_memory: u64,
    #[serde(default = "default_pressure_threshold_pct")]
    pub pressure_threshold_pct: u8,
    #[serde(default = "default_critical_threshold_pct")]
    pub critical_threshold_pct: u8,
    #[serde(default = "default_emergency_reserve")]
    pub emergency_reserve: u64,
    #[serde(default = "default_gc_threshold_pct")]
    pub gc_threshold_pct: u8,
}

fn default_max_memory() -> u64 { 1_073_741_824 }
fn default_pressure_threshold_pct() -> u8 { 80 }
fn default_critical_threshold_pct() -> u8 { 95 }
fn default_emergency_reserve() -> u64 { 33_554_432 }
fn default_gc_threshold_pct() -> u8 { 70 }

impl Default for MemoryConfig {
    fn default() -> Self {
        MemoryConfig {
            max_memory: default_max_memory(),
            pressure_threshold_pct: default_pressure_threshold_pct(),
            critical_threshold_pct: default_critical_threshold_pct(),
            emergency_reserve: default_emergency_reserve(),
            gc_threshold_pct: default_gc_threshold_pct(),
        }
    }
}

// ---- Networking ----

fn default_listen_address() -> String { "127.0.0.1".to_string() }
fn default_listen_port() -> u16 { 8642 }
fn default_tcp_nodelay() -> bool { true }
fn default_keepalive_secs() -> u64 { 30 }
fn default_read_timeout_ms() -> u64 { 30_000 }
fn default_write_timeout_ms() -> u64 { 60_000 }
fn default_tokens_per_second() -> u32 { 1000 }
fn default_burst_size() -> u32 { 2000 }
fn default_listener_enabled() -> bool { true }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ListenerConfig {
    #[serde(default)]
    pub address: String,
    #[serde(default = "default_listener_enabled")]
    pub enabled: bool,
}

impl Default for ListenerConfig {
    fn default() -> Self {
        ListenerConfig {
            address: ":443".to_string(),
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TimeoutConfig {
    #[serde(default = "default_read_timeout_ms")]
    pub read_timeout_ms: u64,
    #[serde(default = "default_write_timeout_ms")]
    pub write_timeout_ms: u64,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        TimeoutConfig {
            read_timeout_ms: default_read_timeout_ms(),
            write_timeout_ms: default_write_timeout_ms(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RateLimitingConfig {
    #[serde(default = "default_tokens_per_second")]
    pub default_tokens_per_second: u32,
    #[serde(default = "default_burst_size")]
    pub default_burst_size: u32,
}

impl Default for RateLimitingConfig {
    fn default() -> Self {
        RateLimitingConfig {
            default_tokens_per_second: default_tokens_per_second(),
            default_burst_size: default_burst_size(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NetworkingConfig {
    #[serde(default = "default_listen_address")]
    pub listen_address: String,
    #[serde(default = "default_listen_port")]
    pub listen_port: u16,
    #[serde(default)]
    pub tls_enabled: bool,
    #[serde(default)]
    pub tls_cert_path: Option<PathBuf>,
    #[serde(default)]
    pub tls_key_path: Option<PathBuf>,
    #[serde(default)]
    pub unix_socket_path: Option<PathBuf>,
    #[serde(default = "default_tcp_nodelay")]
    pub tcp_nodelay: bool,
    #[serde(default = "default_keepalive_secs")]
    pub keepalive_secs: u64,
    #[serde(default)]
    pub listeners: Vec<ListenerConfig>,
    #[serde(default)]
    pub timeouts: TimeoutConfig,
    #[serde(default)]
    pub rate_limiting: RateLimitingConfig,
}

impl Default for NetworkingConfig {
    fn default() -> Self {
        NetworkingConfig {
            listen_address: default_listen_address(),
            listen_port: default_listen_port(),
            tls_enabled: false,
            tls_cert_path: None,
            tls_key_path: None,
            unix_socket_path: None,
            tcp_nodelay: default_tcp_nodelay(),
            keepalive_secs: default_keepalive_secs(),
            listeners: vec![ListenerConfig::default()],
            timeouts: TimeoutConfig::default(),
            rate_limiting: RateLimitingConfig::default(),
        }
    }
}

// ---- Logging ----

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
    #[serde(default = "default_log_format")]
    pub format: String,
    #[serde(default)]
    pub file: Option<PathBuf>,
}

fn default_log_level() -> String { "info".to_string() }
fn default_log_format() -> String { "text".to_string() }

impl Default for LoggingConfig {
    fn default() -> Self {
        LoggingConfig {
            level: default_log_level(),
            format: default_log_format(),
            file: None,
        }
    }
}

// ---- Subsystems ----

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SubsystemsConfig {
    #[serde(default = "default_enable_sql")]
    pub enable_sql: bool,
    #[serde(default = "default_enable_cache")]
    pub enable_cache: bool,
    #[serde(default = "default_enable_queue")]
    pub enable_queue: bool,
    #[serde(default = "default_enable_scheduler")]
    pub enable_scheduler: bool,
    #[serde(default = "default_enable_search")]
    pub enable_search: bool,
    #[serde(default = "default_enable_blob")]
    pub enable_blob: bool,
    #[serde(default = "default_enable_auth")]
    pub enable_auth: bool,
    #[serde(default = "default_enable_dashboard")]
    pub enable_dashboard: bool,
}

fn default_enable_sql() -> bool { true }
fn default_enable_cache() -> bool { true }
fn default_enable_queue() -> bool { true }
fn default_enable_scheduler() -> bool { true }
fn default_enable_search() -> bool { true }
fn default_enable_blob() -> bool { true }
fn default_enable_auth() -> bool { true }
fn default_enable_dashboard() -> bool { true }

impl Default for SubsystemsConfig {
    fn default() -> Self {
        SubsystemsConfig {
            enable_sql: default_enable_sql(),
            enable_cache: default_enable_cache(),
            enable_queue: default_enable_queue(),
            enable_scheduler: default_enable_scheduler(),
            enable_search: default_enable_search(),
            enable_blob: default_enable_blob(),
            enable_auth: default_enable_auth(),
            enable_dashboard: default_enable_dashboard(),
        }
    }
}

// ---- Event ----

fn default_ordering_shards() -> u16 { 64 }
fn default_queue_capacity() -> usize { 1024 }
fn default_max_retries() -> u32 { 3 }
fn default_dlq_max_entries() -> u32 { 100_000 }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EventConfig {
    #[serde(default = "default_ordering_shards")]
    pub ordering_shards: u16,
    #[serde(default = "default_queue_capacity")]
    pub default_queue_capacity: usize,
    #[serde(default = "default_max_retries")]
    pub default_max_retries: u32,
    #[serde(default = "default_dlq_max_entries")]
    pub dlq_max_entries: u32,
}

impl Default for EventConfig {
    fn default() -> Self {
        EventConfig {
            ordering_shards: default_ordering_shards(),
            default_queue_capacity: default_queue_capacity(),
            default_max_retries: default_max_retries(),
            dlq_max_entries: default_dlq_max_entries(),
        }
    }
}

// ---- Execution ----

fn default_max_concurrent() -> u32 { 1024 }
fn default_worker_threads() -> u32 { 4 }
fn default_execution_timeout_ms() -> u64 { 30_000 }
fn default_max_concurrent_ops() -> u32 { 256 }
fn default_pipeline_queue_depth() -> u32 { 1024 }
fn default_default_operation_timeout_ms() -> u64 { 5000 }
fn default_max_operation_timeout_ms() -> u64 { 60_000 }
fn default_rate_limit_default_per_sec() -> u64 { 1000 }
fn default_rate_limit_global_per_sec() -> u64 { 10_000 }
fn default_rate_limit_global_burst() -> u64 { 20_000 }
fn default_rate_limit_user_per_sec() -> u64 { 100 }
fn default_rate_limit_ip_per_sec() -> u64 { 1000 }
fn default_circuit_breaker_threshold() -> u64 { 50 }
fn default_circuit_breaker_window_ms() -> u64 { 10_000 }
fn default_circuit_breaker_half_open_timeout_ms() -> u64 { 10_000 }
fn default_circuit_breaker_success_threshold() -> u64 { 10 }
fn default_audit_enabled() -> bool { true }
fn default_audit_include_payloads() -> bool { false }
fn default_audit_max_entry_size() -> u32 { 4096 }
fn default_idempotency_key_ttl_secs() -> u64 { 86_400 }
fn default_max_idempotency_keys() -> u32 { 100_000 }
fn default_pipeline_max_retries() -> u8 { 3 }
fn default_retry_base_delay_ms() -> u64 { 10 }
fn default_retry_max_delay_ms() -> u64 { 1000 }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExecutionConfig {
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent: u32,
    #[serde(default = "default_worker_threads")]
    pub worker_threads: u32,
    #[serde(default = "default_execution_timeout_ms")]
    pub execution_timeout_ms: u64,

    // PipelineConfig fields
    #[serde(default = "default_max_concurrent_ops")]
    pub max_concurrent_ops: u32,
    #[serde(default = "default_pipeline_queue_depth")]
    pub pipeline_queue_depth: u32,
    #[serde(default = "default_default_operation_timeout_ms")]
    pub default_operation_timeout_ms: u64,
    #[serde(default = "default_max_operation_timeout_ms")]
    pub max_operation_timeout_ms: u64,
    #[serde(default = "default_rate_limit_default_per_sec")]
    pub rate_limit_default_per_sec: u64,
    #[serde(default = "default_rate_limit_global_per_sec")]
    pub rate_limit_global_per_sec: u64,
    #[serde(default = "default_rate_limit_global_burst")]
    pub rate_limit_global_burst: u64,
    #[serde(default = "default_rate_limit_user_per_sec")]
    pub rate_limit_user_per_sec: u64,
    #[serde(default = "default_rate_limit_ip_per_sec")]
    pub rate_limit_ip_per_sec: u64,
    #[serde(default = "default_circuit_breaker_threshold")]
    pub circuit_breaker_threshold: u64,
    #[serde(default = "default_circuit_breaker_window_ms")]
    pub circuit_breaker_window_ms: u64,
    #[serde(default = "default_circuit_breaker_half_open_timeout_ms")]
    pub circuit_breaker_half_open_timeout_ms: u64,
    #[serde(default = "default_circuit_breaker_success_threshold")]
    pub circuit_breaker_success_threshold: u64,
    #[serde(default = "default_audit_enabled")]
    pub audit_enabled: bool,
    #[serde(default = "default_audit_include_payloads")]
    pub audit_include_payloads: bool,
    #[serde(default = "default_audit_max_entry_size")]
    pub audit_max_entry_size: u32,
    #[serde(default = "default_idempotency_key_ttl_secs")]
    pub idempotency_key_ttl_secs: u64,
    #[serde(default = "default_max_idempotency_keys")]
    pub max_idempotency_keys: u32,
    #[serde(default = "default_pipeline_max_retries")]
    pub pipeline_max_retries: u8,
    #[serde(default = "default_retry_base_delay_ms")]
    pub retry_base_delay_ms: u64,
    #[serde(default = "default_retry_max_delay_ms")]
    pub retry_max_delay_ms: u64,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        ExecutionConfig {
            max_concurrent: default_max_concurrent(),
            worker_threads: default_worker_threads(),
            execution_timeout_ms: default_execution_timeout_ms(),
            max_concurrent_ops: default_max_concurrent_ops(),
            pipeline_queue_depth: default_pipeline_queue_depth(),
            default_operation_timeout_ms: default_default_operation_timeout_ms(),
            max_operation_timeout_ms: default_max_operation_timeout_ms(),
            rate_limit_default_per_sec: default_rate_limit_default_per_sec(),
            rate_limit_global_per_sec: default_rate_limit_global_per_sec(),
            rate_limit_global_burst: default_rate_limit_global_burst(),
            rate_limit_user_per_sec: default_rate_limit_user_per_sec(),
            rate_limit_ip_per_sec: default_rate_limit_ip_per_sec(),
            circuit_breaker_threshold: default_circuit_breaker_threshold(),
            circuit_breaker_window_ms: default_circuit_breaker_window_ms(),
            circuit_breaker_half_open_timeout_ms: default_circuit_breaker_half_open_timeout_ms(),
            circuit_breaker_success_threshold: default_circuit_breaker_success_threshold(),
            audit_enabled: default_audit_enabled(),
            audit_include_payloads: default_audit_include_payloads(),
            audit_max_entry_size: default_audit_max_entry_size(),
            idempotency_key_ttl_secs: default_idempotency_key_ttl_secs(),
            max_idempotency_keys: default_max_idempotency_keys(),
            pipeline_max_retries: default_pipeline_max_retries(),
            retry_base_delay_ms: default_retry_base_delay_ms(),
            retry_max_delay_ms: default_retry_max_delay_ms(),
        }
    }
}

// ---- Auth ----

fn default_password_min_length() -> u8 { 8 }
fn default_password_max_length() -> u8 { 128 }
fn default_lockout_max_attempts() -> u8 { 5 }
fn default_session_ttl() -> u32 { 86_400 }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PasswordPolicy {
    #[serde(default = "default_password_min_length")]
    pub min_length: u8,
    #[serde(default = "default_password_max_length")]
    pub max_length: u8,
}

impl Default for PasswordPolicy {
    fn default() -> Self {
        PasswordPolicy {
            min_length: default_password_min_length(),
            max_length: default_password_max_length(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LockoutConfig {
    #[serde(default = "default_lockout_max_attempts")]
    pub max_attempts: u8,
}

impl Default for LockoutConfig {
    fn default() -> Self {
        LockoutConfig {
            max_attempts: default_lockout_max_attempts(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InternalAuthConfig {
    #[serde(default)]
    pub password_policy: PasswordPolicy,
    #[serde(default)]
    pub lockout: LockoutConfig,
}

impl Default for InternalAuthConfig {
    fn default() -> Self {
        InternalAuthConfig {
            password_policy: PasswordPolicy::default(),
            lockout: LockoutConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionConfig {
    #[serde(default = "default_session_ttl")]
    pub ttl_seconds: u32,
}

impl Default for SessionConfig {
    fn default() -> Self {
        SessionConfig {
            ttl_seconds: default_session_ttl(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuthConfig {
    #[serde(default)]
    pub internal: InternalAuthConfig,
    #[serde(default)]
    pub session: SessionConfig,
}

impl Default for AuthConfig {
    fn default() -> Self {
        AuthConfig {
            internal: InternalAuthConfig::default(),
            session: SessionConfig::default(),
        }
    }
}

// ---- Security ----

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EncryptionAtRestConfig {
    #[serde(default)]
    pub enabled: bool,
}

impl Default for EncryptionAtRestConfig {
    fn default() -> Self {
        EncryptionAtRestConfig { enabled: false }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SecurityConfig {
    #[serde(default)]
    pub encryption_at_rest: EncryptionAtRestConfig,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        SecurityConfig {
            encryption_at_rest: EncryptionAtRestConfig::default(),
        }
    }
}

// ---- Root Config ----

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Config {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub storage: StorageConfig,
    #[serde(default)]
    pub memory: MemoryConfig,
    #[serde(default)]
    pub networking: NetworkingConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub subsystems: SubsystemsConfig,
    #[serde(default)]
    pub event: EventConfig,
    #[serde(default)]
    pub execution: ExecutionConfig,
    #[serde(default)]
    pub auth: AuthConfig,
    #[serde(default)]
    pub security: SecurityConfig,
}

impl Config {
    pub fn default() -> Self {
        toml::from_str(DEFAULT_TOML).expect("built-in default TOML is valid")
    }

    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        // 2a: page_size must be power of 2 (4096, 8192, 16384, 32768)
        let valid_page_sizes = [4096u16, 8192, 16384, 32768];
        if !valid_page_sizes.contains(&self.storage.page_size) {
            errors.push(format!("storage.page_size must be power of 2 (4096, 8192, 16384, 32768), got {}", self.storage.page_size));
        }

        // 2b: wal_segment_size >= wal_page_size * 64
        if self.storage.wal_segment_size < self.storage.wal_page_size as u64 * 64 {
            errors.push("storage.wal_segment_size must be >= wal_page_size * 64".into());
        }

        // 2c: btree_order must be >= 2 and <= 32
        if self.storage.btree_order < 2 || self.storage.btree_order > 32 {
            errors.push("storage.btree_order must be between 2 and 32".into());
        }

        // 2d: lsm_max_level must be >= 1 and <= 10
        if self.storage.lsm_max_level < 1 || self.storage.lsm_max_level > 10 {
            errors.push("storage.lsm_max_level must be between 1 and 10".into());
        }

        // 2e: bloom_false_positive_rate must be > 0.0 and <= 0.1
        if self.storage.bloom_false_positive_rate <= 0.0 || self.storage.bloom_false_positive_rate > 0.1 {
            errors.push("storage.bloom_false_positive_rate must be > 0.0 and <= 0.1".into());
        }

        // 2f: write_buffer_size >= page_size * 64
        if self.storage.write_buffer_size < self.storage.page_size as u64 * 64 {
            errors.push("storage.write_buffer_size must be >= page_size * 64".into());
        }

        // 2g: compaction_threads >= 1
        if self.storage.compaction_threads < 1 {
            errors.push("storage.compaction_threads must be >= 1".into());
        }

        // 3a: each listener address must not be empty
        for (i, listener) in self.networking.listeners.iter().enumerate() {
            if listener.address.is_empty() {
                errors.push(format!("networking.listeners[{}].address must not be empty", i));
            }
        }

        // 3e: read_timeout_ms >= write_timeout_ms / 2
        if self.networking.timeouts.read_timeout_ms < self.networking.timeouts.write_timeout_ms / 2 {
            errors.push("networking.timeouts.read_timeout_ms must be >= write_timeout_ms / 2".into());
        }

        // 3g: rate limit tokens_per_second >= burst_size / 10
        if self.networking.rate_limiting.default_tokens_per_second < self.networking.rate_limiting.default_burst_size / 10 {
            errors.push("networking.rate_limiting.default_tokens_per_second must be >= burst_size / 10".into());
        }

        // 4a: ordering_shards must be power of 2
        let shards = self.event.ordering_shards;
        if shards == 0 || (shards & (shards - 1)) != 0 {
            errors.push("event.ordering_shards must be a power of 2".into());
        }

        // 4b: default_queue_capacity >= 64
        if self.event.default_queue_capacity < 64 {
            errors.push("event.default_queue_capacity must be >= 64".into());
        }

        // 4c: default_max_retries <= 100
        if self.event.default_max_retries > 100 {
            errors.push("event.default_max_retries must be <= 100".into());
        }

        // 4d: dlq_max_entries <= 1000000
        if self.event.dlq_max_entries > 1_000_000 {
            errors.push("event.dlq_max_entries must be <= 1,000,000".into());
        }

        // 5a: max_concurrent >= 1
        if self.execution.max_concurrent < 1 {
            errors.push("execution.max_concurrent must be >= 1".into());
        }

        // 5b: worker_threads >= 1
        if self.execution.worker_threads < 1 {
            errors.push("execution.worker_threads must be >= 1".into());
        }

        // 5c: execution_timeout_ms between 100 and 3,600,000
        if self.execution.execution_timeout_ms < 100 || self.execution.execution_timeout_ms > 3_600_000 {
            errors.push("execution.execution_timeout_ms must be between 100 and 3,600,000".into());
        }

        // 5d: max_concurrent_ops >= 1
        if self.execution.max_concurrent_ops < 1 {
            errors.push("execution.max_concurrent_ops must be >= 1".into());
        }

        // 5e: pipeline_queue_depth >= 16
        if self.execution.pipeline_queue_depth < 16 {
            errors.push("execution.pipeline_queue_depth must be >= 16".into());
        }

        // 5f: default_operation_timeout_ms between 100 and max_operation_timeout_ms
        if self.execution.default_operation_timeout_ms < 100 {
            errors.push("execution.default_operation_timeout_ms must be >= 100".into());
        }
        if self.execution.default_operation_timeout_ms > self.execution.max_operation_timeout_ms {
            errors.push("execution.default_operation_timeout_ms must be <= max_operation_timeout_ms".into());
        }

        // 5g: max_operation_timeout_ms <= 3,600,000
        if self.execution.max_operation_timeout_ms > 3_600_000 {
            errors.push("execution.max_operation_timeout_ms must be <= 3,600,000".into());
        }

        // 5h: rate_limit_global_burst >= rate_limit_global_per_sec
        if self.execution.rate_limit_global_burst < self.execution.rate_limit_global_per_sec {
            errors.push("execution.rate_limit_global_burst must be >= rate_limit_global_per_sec".into());
        }

        // 5i: circuit_breaker_threshold >= 1
        if self.execution.circuit_breaker_threshold < 1 {
            errors.push("execution.circuit_breaker_threshold must be >= 1".into());
        }

        // 5j: circuit_breaker_window_ms >= 1000
        if self.execution.circuit_breaker_window_ms < 1000 {
            errors.push("execution.circuit_breaker_window_ms must be >= 1000".into());
        }

        // 5k: circuit_breaker_success_threshold >= 1
        if self.execution.circuit_breaker_success_threshold < 1 {
            errors.push("execution.circuit_breaker_success_threshold must be >= 1".into());
        }

        // 5l: max_idempotency_keys <= 1_000_000
        if self.execution.max_idempotency_keys > 1_000_000 {
            errors.push("execution.max_idempotency_keys must be <= 1,000,000".into());
        }

        // 5m: pipeline_max_retries <= 10
        if self.execution.pipeline_max_retries > 10 {
            errors.push("execution.pipeline_max_retries must be <= 10".into());
        }

        // 5n: retry_base_delay_ms >= 1
        if self.execution.retry_base_delay_ms < 1 {
            errors.push("execution.retry_base_delay_ms must be >= 1".into());
        }

        // 5o: retry_max_delay_ms >= retry_base_delay_ms
        if self.execution.retry_max_delay_ms < self.execution.retry_base_delay_ms {
            errors.push("execution.retry_max_delay_ms must be >= retry_base_delay_ms".into());
        }

        // 5p: audit_max_entry_size >= 64
        if self.execution.audit_max_entry_size < 64 {
            errors.push("execution.audit_max_entry_size must be >= 64".into());
        }

        // 6b: password_policy.min_length <= password_policy.max_length
        if self.auth.internal.password_policy.min_length > self.auth.internal.password_policy.max_length {
            errors.push("auth.internal.password_policy.min_length must be <= max_length".into());
        }

        // 6c: lockout.max_attempts >= 1
        if self.auth.internal.lockout.max_attempts < 1 {
            errors.push("auth.internal.lockout.max_attempts must be >= 1".into());
        }

        // 6d: session TTL >= 60
        if self.auth.session.ttl_seconds < 60 {
            errors.push("auth.session.ttl_seconds must be >= 60".into());
        }

        // --- Legacy checks from loader ---

        if self.storage.block_cache_size == 0 {
            errors.push("storage.block_cache_size must be > 0".into());
        }
        if self.memory.max_memory == 0 {
            errors.push("memory.max_memory must be > 0".into());
        }
        if self.memory.pressure_threshold_pct >= self.memory.critical_threshold_pct {
            errors.push(format!(
                "memory.pressure_threshold_pct ({}) must be < memory.critical_threshold_pct ({})",
                self.memory.pressure_threshold_pct, self.memory.critical_threshold_pct
            ));
        }
        if self.memory.critical_threshold_pct >= 100 {
            errors.push(format!(
                "memory.critical_threshold_pct ({}) must be < 100",
                self.memory.critical_threshold_pct
            ));
        }
        if self.storage.wal_segment_size < 4096 {
            errors.push(format!("storage.wal_segment_size ({}) must be >= 4096", self.storage.wal_segment_size));
        }
        if self.networking.listen_port == 0 {
            errors.push("networking.listen_port must be > 0".into());
        }
        if self.networking.tls_enabled {
            if self.networking.tls_cert_path.is_none() {
                errors.push("networking.tls_cert_path must be set when tls_enabled is true".into());
            }
            if self.networking.tls_key_path.is_none() {
                errors.push("networking.tls_key_path must be set when tls_enabled is true".into());
            }
        }
        if self.general.max_connections == 0 {
            errors.push("general.max_connections must be > 0".into());
        }
        if self.general.shutdown_timeout_ms == 0 {
            errors.push("general.shutdown_timeout_ms must be > 0".into());
        }
        if self.memory.gc_threshold_pct > 100 {
            errors.push(format!("memory.gc_threshold_pct ({}) must be <= 100", self.memory.gc_threshold_pct));
        }
        if self.storage.bloom_filter_bits_per_key == 0 {
            errors.push("storage.bloom_filter_bits_per_key must be > 0".into());
        }

        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }
}

impl Default for Config {
    fn default() -> Self {
        Config::default()
    }
}
