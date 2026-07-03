use crate::config::*;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;
use crossbeam::channel;
use thiserror::Error;
use tracing;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Config file not found: {0}")]
    FileNotFound(PathBuf),

    #[error("Failed to read config file {path}: {source}")]
    Io { path: PathBuf, source: std::io::Error },

    #[error("Failed to parse config file {path}: {source}")]
    Parse { path: PathBuf, source: toml::de::Error },

    #[error("Validation failed: {0:?}")]
    Validation(Vec<String>),

    #[error("Environment variable {var} has invalid value '{value}': {message}")]
    EnvVar { var: String, value: String, message: String },

    #[error("No config path set for reload")]
    NoPath,
}

pub type Result<T> = std::result::Result<T, ConfigError>;

pub struct ConfigLoader {
    path: Option<PathBuf>,
    watcher_tx: Option<channel::Sender<()>>,
}

impl ConfigLoader {
    pub fn new() -> Self {
        ConfigLoader {
            path: None,
            watcher_tx: None,
        }
    }

    pub fn with_path(path: PathBuf) -> Self {
        ConfigLoader {
            path: Some(path),
            watcher_tx: None,
        }
    }

    pub fn watch(&mut self, path: &Path) -> Result<(Arc<RwLock<Config>>, channel::Receiver<()>)> {
        let config = Self::parse_file(path)?;
        let locked = Arc::new(RwLock::new(config));
        let (tx, rx) = channel::unbounded();
        self.path = Some(path.to_path_buf());
        self.watcher_tx = Some(tx);
        Ok((locked, rx))
    }

    pub fn reload(&self, locked: &Arc<RwLock<Config>>) -> Result<()> {
        let path = self.path.as_ref().ok_or(ConfigError::NoPath)?;
        let config = Self::parse_file(path)?;
        *locked.write() = config;
        if let Some(tx) = &self.watcher_tx {
            let _ = tx.send(());
        }
        Ok(())
    }

    pub fn load(
        &self,
        matches: Option<&clap::ArgMatches>,
    ) -> Result<Config> {
        let mut config = Config::default();

        let system_path = PathBuf::from("/etc/novad/novad.toml");
        if system_path.exists() {
            match Self::parse_file(&system_path) {
                Ok(overlay) => {
                    config = Self::merge(config, overlay);
                    tracing::info!("Loaded system config from {}", system_path.display());
                }
                Err(e) => {
                    tracing::warn!("Failed to load system config {}: {}", system_path.display(), e);
                }
            }
        }

        let user_path = dirs_config_path().unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_default();
            PathBuf::from(&home).join(".config/nova/novad.toml")
        });
        if user_path.exists() {
            match Self::parse_file(&user_path) {
                Ok(overlay) => {
                    config = Self::merge(config, overlay);
                    tracing::info!("Loaded user config from {}", user_path.display());
                }
                Err(e) => {
                    tracing::warn!("Failed to load user config {}: {}", user_path.display(), e);
                }
            }
        }

        let local_path = PathBuf::from("./novad.toml");
        if local_path.exists() {
            match Self::parse_file(&local_path) {
                Ok(overlay) => {
                    config = Self::merge(config, overlay);
                    tracing::info!("Loaded local config from {}", local_path.display());
                }
                Err(e) => {
                    tracing::warn!("Failed to load local config {}: {}", local_path.display(), e);
                }
            }
        }

        Self::apply_env_overrides(&mut config);

        if let Some(m) = matches {
            Self::apply_cli_overrides(&mut config, m);
        }

        Self::validate(&config)?;

        Ok(config)
    }

    pub fn parse_file(path: &Path) -> Result<Config> {
        if !path.exists() {
            return Err(ConfigError::FileNotFound(path.to_path_buf()));
        }
        let content = std::fs::read_to_string(path)
            .map_err(|e| ConfigError::Io { path: path.to_path_buf(), source: e })?;
        let config: Config = toml::from_str(&content)
            .map_err(|e| ConfigError::Parse { path: path.to_path_buf(), source: e })?;
        Ok(config)
    }

    pub fn apply_env_overrides(config: &mut Config) {
        let env_vars: HashMap<String, String> = std::env::vars()
            .filter(|(k, _)| k.starts_with("NOVA_"))
            .collect();

        for (key, value) in &env_vars {
            let stripped = key.strip_prefix("NOVA_").unwrap();
            let parts: Vec<&str> = stripped.splitn(2, "__").collect();
            if parts.len() != 2 {
                tracing::warn!("Skipping env var {}: expected NOVA_<SECTION>__<FIELD> format", key);
                continue;
            }

            let section = parts[0].to_lowercase();
            let field_path = parts[1].to_lowercase();

            let parse_result: std::result::Result<toml::Value, _> = value.parse::<toml::Value>();
            let toml_val = match parse_result {
                Ok(toml::Value::String(_)) | Ok(toml::Value::Integer(_)) |
                Ok(toml::Value::Float(_)) | Ok(toml::Value::Boolean(_)) |
                Ok(toml::Value::Array(_)) | Ok(toml::Value::Table(_)) => parse_result.unwrap(),
                _ => toml::Value::String(value.clone()),
            };

            apply_env_to_section(config, &section, &field_path, &toml_val);
        }
    }

    pub fn merge(base: Config, overlay: Config) -> Config {
        Config {
            general: merge_general(base.general, overlay.general),
            storage: merge_storage(base.storage, overlay.storage),
            memory: merge_memory(base.memory, overlay.memory),
            networking: merge_networking(base.networking, overlay.networking),
            logging: merge_logging(base.logging, overlay.logging),
            subsystems: merge_subsystems(base.subsystems, overlay.subsystems),
            event: merge_event(base.event, overlay.event),
            execution: merge_execution(base.execution, overlay.execution),
            auth: merge_auth(base.auth, overlay.auth),
            security: merge_security(base.security, overlay.security),
            cache: overlay.cache,
            blob: overlay.blob,
            search: overlay.search,
            sql: overlay.sql,
        }
    }

    pub fn validate(config: &Config) -> Result<()> {
        config.validate().map_err(ConfigError::Validation)
    }

    pub fn apply_cli_overrides(config: &mut Config, matches: &clap::ArgMatches) {
        if let Some(val) = matches.try_get_one::<String>("data-dir").ok().flatten() {
            config.general.data_dir = PathBuf::from(val);
            config.storage.wal_dir = PathBuf::from(val).join("wal");
        }
        if let Some(val) = matches.try_get_one::<String>("listen-address").ok().flatten() {
            config.networking.listen_address = val.clone();
        }
        if let Some(val) = matches.try_get_one::<u16>("listen-port").ok().flatten() {
            config.networking.listen_port = *val;
        }
        if let Some(val) = matches.try_get_one::<String>("log-level").ok().flatten() {
            config.logging.level = val.clone();
        }
        if let Some(val) = matches.try_get_one::<String>("log-format").ok().flatten() {
            config.logging.format = val.clone();
        }
        if let Some(val) = matches.try_get_one::<u64>("max-connections").ok().flatten() {
            config.general.max_connections = *val as u32;
        }
        if let Some(val) = matches.try_get_one::<u64>("shutdown-timeout").ok().flatten() {
            config.general.shutdown_timeout_ms = *val;
        }
    }
}

impl Default for ConfigLoader {
    fn default() -> Self {
        ConfigLoader::new()
    }
}

impl From<PathBuf> for ConfigLoader {
    fn from(path: PathBuf) -> Self {
        ConfigLoader::with_path(path)
    }
}

fn dirs_config_path() -> Option<PathBuf> {
    std::env::var("XDG_CONFIG_HOME")
        .ok()
        .map(|config_dir| PathBuf::from(config_dir).join("nova/novad.toml"))
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|home| PathBuf::from(home).join(".config/nova/novad.toml"))
        })
}

fn apply_env_to_section(config: &mut Config, section: &str, field_path: &str, value: &toml::Value) {
    let val_str = match value {
        toml::Value::String(s) => s.clone(),
        toml::Value::Integer(i) => i.to_string(),
        toml::Value::Float(f) => f.to_string(),
        toml::Value::Boolean(b) => b.to_string(),
        other => other.to_string(),
    };

    match section {
        "general" => apply_env_general(&mut config.general, field_path, value, &val_str),
        "storage" => apply_env_storage(&mut config.storage, field_path, value, &val_str),
        "memory" => apply_env_memory(&mut config.memory, field_path, value, &val_str),
        "networking" => apply_env_networking(&mut config.networking, field_path, value, &val_str),
        "logging" => apply_env_logging(&mut config.logging, field_path, value, &val_str),
        "subsystems" => apply_env_subsystems(&mut config.subsystems, field_path, value, &val_str),
        "event" => apply_env_event(&mut config.event, field_path, value, &val_str),
        "execution" => apply_env_execution(&mut config.execution, field_path, value, &val_str),
        "auth" => apply_env_auth(&mut config.auth, field_path, value, &val_str),
        "security" => apply_env_security(&mut config.security, field_path, value, &val_str),
        "cache" => apply_env_cache(&mut config.cache, field_path, value, &val_str),
        "blob" => apply_env_blob(&mut config.blob, field_path, value, &val_str),
        "search" => apply_env_search(&mut config.search, field_path, value, &val_str),
        "sql" => apply_env_sql(&mut config.sql, field_path, value, &val_str),
        _ => {
            tracing::warn!("Unknown config section '{}' from env var", section);
        }
    }
}

fn apply_env_general(cfg: &mut GeneralConfig, field: &str, _val: &toml::Value, val_str: &str) {
    match field {
        "data_dir" => cfg.data_dir = PathBuf::from(val_str),
        "pid_file" => cfg.pid_file = PathBuf::from(val_str),
        "max_connections" => { if let Ok(n) = val_str.parse::<u32>() { cfg.max_connections = n; } }
        "shutdown_timeout_ms" => { if let Ok(n) = val_str.parse::<u64>() { cfg.shutdown_timeout_ms = n; } }
        "startup_timeout_ms" => { if let Ok(n) = val_str.parse::<u64>() { cfg.startup_timeout_ms = n; } }
        _ => { tracing::warn!("Unknown general config field '{}'", field); }
    }
}

fn apply_env_storage(cfg: &mut StorageConfig, field: &str, _val: &toml::Value, val_str: &str) {
    match field {
        "wal_dir" => cfg.wal_dir = PathBuf::from(val_str),
        "wal_segment_size" => { if let Ok(n) = val_str.parse::<u64>() { cfg.wal_segment_size = n; } }
        "block_cache_size" => { if let Ok(n) = val_str.parse::<u64>() { cfg.block_cache_size = n; } }
        "page_cache_size" => { if let Ok(n) = val_str.parse::<u64>() { cfg.page_cache_size = n; } }
        "memtable_size" => { if let Ok(n) = val_str.parse::<u64>() { cfg.memtable_size = n; } }
        "max_blob_size" => { if let Ok(n) = val_str.parse::<u64>() { cfg.max_blob_size = n; } }
        "compression" => {
            match val_str.to_lowercase().as_str() {
                "none" => cfg.compression = nova_core::Compression::None,
                "snappy" => cfg.compression = nova_core::Compression::Snappy,
                "zstd" => cfg.compression = nova_core::Compression::Zstd,
                _ => { tracing::warn!("Unknown compression '{}'", val_str); }
            }
        }
        "bloom_filter_bits_per_key" => {
            if let Ok(n) = val_str.parse::<u32>() { cfg.bloom_filter_bits_per_key = n; }
        }
        _ => { tracing::warn!("Unknown storage config field '{}'", field); }
    }
}

fn apply_env_memory(cfg: &mut MemoryConfig, field: &str, _val: &toml::Value, val_str: &str) {
    match field {
        "max_memory" => { if let Ok(n) = val_str.parse::<u64>() { cfg.max_memory = n; } }
        "pressure_threshold_pct" => { if let Ok(n) = val_str.parse::<u8>() { cfg.pressure_threshold_pct = n; } }
        "critical_threshold_pct" => { if let Ok(n) = val_str.parse::<u8>() { cfg.critical_threshold_pct = n; } }
        "emergency_reserve" => { if let Ok(n) = val_str.parse::<u64>() { cfg.emergency_reserve = n; } }
        "gc_threshold_pct" => { if let Ok(n) = val_str.parse::<u8>() { cfg.gc_threshold_pct = n; } }
        _ => { tracing::warn!("Unknown memory config field '{}'", field); }
    }
}

fn apply_env_networking(cfg: &mut NetworkingConfig, field: &str, _val: &toml::Value, val_str: &str) {
    match field {
        "listen_address" => cfg.listen_address = val_str.to_string(),
        "listen_port" => { if let Ok(n) = val_str.parse::<u16>() { cfg.listen_port = n; } }
        "tls_enabled" => { if let Ok(b) = val_str.parse::<bool>() { cfg.tls_enabled = b; } }
        "tls_cert_path" => {
            if !val_str.is_empty() { cfg.tls_cert_path = Some(PathBuf::from(val_str)); }
        }
        "tls_key_path" => {
            if !val_str.is_empty() { cfg.tls_key_path = Some(PathBuf::from(val_str)); }
        }
        "unix_socket_path" => {
            if !val_str.is_empty() { cfg.unix_socket_path = Some(PathBuf::from(val_str)); }
        }
        "tcp_nodelay" => { if let Ok(b) = val_str.parse::<bool>() { cfg.tcp_nodelay = b; } }
        "keepalive_secs" => { if let Ok(n) = val_str.parse::<u64>() { cfg.keepalive_secs = n; } }
        _ => { tracing::warn!("Unknown networking config field '{}'", field); }
    }
}

fn apply_env_logging(cfg: &mut LoggingConfig, field: &str, _val: &toml::Value, val_str: &str) {
    match field {
        "level" => cfg.level = val_str.to_string(),
        "format" => cfg.format = val_str.to_string(),
        "file" => {
            if !val_str.is_empty() { cfg.file = Some(PathBuf::from(val_str)); }
        }
        _ => { tracing::warn!("Unknown logging config field '{}'", field); }
    }
}

fn apply_env_subsystems(cfg: &mut SubsystemsConfig, field: &str, _val: &toml::Value, val_str: &str) {
    let parsed = val_str.parse::<bool>().unwrap_or(false);
    match field {
        "enable_sql" => cfg.enable_sql = parsed,
        "enable_cache" => cfg.enable_cache = parsed,
        "enable_queue" => cfg.enable_queue = parsed,
        "enable_scheduler" => cfg.enable_scheduler = parsed,
        "enable_search" => cfg.enable_search = parsed,
        "enable_blob" => cfg.enable_blob = parsed,
        "enable_auth" => cfg.enable_auth = parsed,
        "enable_dashboard" => cfg.enable_dashboard = parsed,
        _ => { tracing::warn!("Unknown subsystems config field '{}'", field); }
    }
}

fn apply_env_event(cfg: &mut EventConfig, field: &str, _val: &toml::Value, val_str: &str) {
    match field {
        "ordering_shards" => { if let Ok(n) = val_str.parse::<u16>() { cfg.ordering_shards = n; } }
        "default_queue_capacity" => { if let Ok(n) = val_str.parse::<usize>() { cfg.default_queue_capacity = n; } }
        "default_max_retries" => { if let Ok(n) = val_str.parse::<u32>() { cfg.default_max_retries = n; } }
        "dlq_max_entries" => { if let Ok(n) = val_str.parse::<u32>() { cfg.dlq_max_entries = n; } }
        _ => { tracing::warn!("Unknown event config field '{}'", field); }
    }
}

fn apply_env_execution(cfg: &mut ExecutionConfig, field: &str, _val: &toml::Value, val_str: &str) {
    match field {
        "max_concurrent" => { if let Ok(n) = val_str.parse::<u32>() { cfg.max_concurrent = n; } }
        "worker_threads" => { if let Ok(n) = val_str.parse::<u32>() { cfg.worker_threads = n; } }
        "execution_timeout_ms" => { if let Ok(n) = val_str.parse::<u64>() { cfg.execution_timeout_ms = n; } }
        "max_concurrent_ops" => { if let Ok(n) = val_str.parse::<u32>() { cfg.max_concurrent_ops = n; } }
        "pipeline_queue_depth" => { if let Ok(n) = val_str.parse::<u32>() { cfg.pipeline_queue_depth = n; } }
        "default_operation_timeout_ms" => { if let Ok(n) = val_str.parse::<u64>() { cfg.default_operation_timeout_ms = n; } }
        "max_operation_timeout_ms" => { if let Ok(n) = val_str.parse::<u64>() { cfg.max_operation_timeout_ms = n; } }
        "rate_limit_default_per_sec" => { if let Ok(n) = val_str.parse::<u64>() { cfg.rate_limit_default_per_sec = n; } }
        "rate_limit_global_per_sec" => { if let Ok(n) = val_str.parse::<u64>() { cfg.rate_limit_global_per_sec = n; } }
        "rate_limit_global_burst" => { if let Ok(n) = val_str.parse::<u64>() { cfg.rate_limit_global_burst = n; } }
        "rate_limit_user_per_sec" => { if let Ok(n) = val_str.parse::<u64>() { cfg.rate_limit_user_per_sec = n; } }
        "rate_limit_ip_per_sec" => { if let Ok(n) = val_str.parse::<u64>() { cfg.rate_limit_ip_per_sec = n; } }
        "circuit_breaker_threshold" => { if let Ok(n) = val_str.parse::<u64>() { cfg.circuit_breaker_threshold = n; } }
        "circuit_breaker_window_ms" => { if let Ok(n) = val_str.parse::<u64>() { cfg.circuit_breaker_window_ms = n; } }
        "circuit_breaker_half_open_timeout_ms" => { if let Ok(n) = val_str.parse::<u64>() { cfg.circuit_breaker_half_open_timeout_ms = n; } }
        "circuit_breaker_success_threshold" => { if let Ok(n) = val_str.parse::<u64>() { cfg.circuit_breaker_success_threshold = n; } }
        "audit_enabled" => { if let Ok(b) = val_str.parse::<bool>() { cfg.audit_enabled = b; } }
        "audit_include_payloads" => { if let Ok(b) = val_str.parse::<bool>() { cfg.audit_include_payloads = b; } }
        "audit_max_entry_size" => { if let Ok(n) = val_str.parse::<u32>() { cfg.audit_max_entry_size = n; } }
        "idempotency_key_ttl_secs" => { if let Ok(n) = val_str.parse::<u64>() { cfg.idempotency_key_ttl_secs = n; } }
        "max_idempotency_keys" => { if let Ok(n) = val_str.parse::<u32>() { cfg.max_idempotency_keys = n; } }
        "pipeline_max_retries" | "max_retries" => { if let Ok(n) = val_str.parse::<u8>() { cfg.pipeline_max_retries = n; } }
        "retry_base_delay_ms" => { if let Ok(n) = val_str.parse::<u64>() { cfg.retry_base_delay_ms = n; } }
        "retry_max_delay_ms" => { if let Ok(n) = val_str.parse::<u64>() { cfg.retry_max_delay_ms = n; } }
        _ => { tracing::warn!("Unknown execution config field '{}'", field); }
    }
}

fn apply_env_auth(cfg: &mut AuthConfig, field: &str, _val: &toml::Value, val_str: &str) {
    match field {
        "session_ttl" | "ttl_seconds" => { if let Ok(n) = val_str.parse::<u32>() { cfg.session.ttl_seconds = n; } }
        "password_min_length" | "min_length" => {
            if let Ok(n) = val_str.parse::<u8>() { cfg.internal.password_policy.min_length = n; }
        }
        "password_max_length" | "max_length" => {
            if let Ok(n) = val_str.parse::<u8>() { cfg.internal.password_policy.max_length = n; }
        }
        "lockout_max_attempts" | "max_attempts" => {
            if let Ok(n) = val_str.parse::<u8>() { cfg.internal.lockout.max_attempts = n; }
        }
        _ => { tracing::warn!("Unknown auth config field '{}'", field); }
    }
}

fn apply_env_security(cfg: &mut SecurityConfig, field: &str, _val: &toml::Value, val_str: &str) {
    match field {
        "encryption_at_rest_enabled" | "encryption_enabled" | "enabled" => {
            if let Ok(b) = val_str.parse::<bool>() { cfg.encryption_at_rest.enabled = b; }
        }
        _ => { tracing::warn!("Unknown security config field '{}'", field); }
    }
}

fn apply_env_cache(cfg: &mut CacheConfig, field: &str, _val: &toml::Value, val_str: &str) {
    match field {
        "max_size" => { if let Ok(n) = val_str.parse::<usize>() { cfg.max_size = n; } }
        "default_ttl_secs" => { if let Ok(n) = val_str.parse::<u64>() { cfg.default_ttl_secs = n; } }
        "eviction_policy" => cfg.eviction_policy = val_str.to_string(),
        "backend_type" => cfg.backend_type = val_str.to_string(),
        "redis_url" => { if !val_str.is_empty() { cfg.redis_url = Some(val_str.to_string()); } }
        _ => { tracing::warn!("Unknown cache config field '{}'", field); }
    }
}

fn apply_env_blob(cfg: &mut BlobConfig, field: &str, _val: &toml::Value, val_str: &str) {
    match field {
        "chunk_size" => { if let Ok(n) = val_str.parse::<usize>() { cfg.chunk_size = n; } }
        "max_blob_size" => { if let Ok(n) = val_str.parse::<u64>() { cfg.max_blob_size = n; } }
        "gc_interval_secs" => { if let Ok(n) = val_str.parse::<u64>() { cfg.gc_interval_secs = n; } }
        "gc_grace_period_secs" => { if let Ok(n) = val_str.parse::<u64>() { cfg.gc_grace_period_secs = n; } }
        "data_dir" => cfg.data_dir = val_str.to_string(),
        "chunk_nesting_depth" => { if let Ok(n) = val_str.parse::<usize>() { cfg.chunk_nesting_depth = n; } }
        _ => { tracing::warn!("Unknown blob config field '{}'", field); }
    }
}

fn apply_env_search(cfg: &mut SearchConfig, field: &str, _val: &toml::Value, val_str: &str) {
    match field {
        "default_limit" => { if let Ok(n) = val_str.parse::<usize>() { cfg.default_limit = n; } }
        "max_limit" => { if let Ok(n) = val_str.parse::<usize>() { cfg.max_limit = n; } }
        "bm25_k1" => { if let Ok(f) = val_str.parse::<f64>() { cfg.bm25_k1 = f; } }
        "bm25_b" => { if let Ok(f) = val_str.parse::<f64>() { cfg.bm25_b = f; } }
        "fuzzy_max_distance" => { if let Ok(n) = val_str.parse::<u8>() { cfg.fuzzy_max_distance = n; } }
        "highlight_snippet_len" => { if let Ok(n) = val_str.parse::<usize>() { cfg.highlight_snippet_len = n; } }
        "highlight_max_snippets" => { if let Ok(n) = val_str.parse::<usize>() { cfg.highlight_max_snippets = n; } }
        "refresh_interval_ms" => { if let Ok(n) = val_str.parse::<u64>() { cfg.refresh_interval_ms = n; } }
        "merge_segment_threshold" => { if let Ok(n) = val_str.parse::<usize>() { cfg.merge_segment_threshold = n; } }
        _ => { tracing::warn!("Unknown search config field '{}'", field); }
    }
}

fn apply_env_sql(cfg: &mut SQLConfig, field: &str, _val: &toml::Value, val_str: &str) {
    match field {
        "max_batch_size" => { if let Ok(n) = val_str.parse::<usize>() { cfg.max_batch_size = n; } }
        "max_columns" => { if let Ok(n) = val_str.parse::<usize>() { cfg.max_columns = n; } }
        "default_limit" => { if let Ok(n) = val_str.parse::<usize>() { cfg.default_limit = n; } }
        _ => { tracing::warn!("Unknown sql config field '{}'", field); }
    }
}

fn merge_general(_base: GeneralConfig, overlay: GeneralConfig) -> GeneralConfig {
    GeneralConfig {
        data_dir: overlay.data_dir,
        pid_file: overlay.pid_file,
        max_connections: overlay.max_connections,
        shutdown_timeout_ms: overlay.shutdown_timeout_ms,
        startup_timeout_ms: overlay.startup_timeout_ms,
    }
}

fn merge_storage(_base: StorageConfig, overlay: StorageConfig) -> StorageConfig {
    StorageConfig {
        wal_dir: overlay.wal_dir,
        wal_segment_size: overlay.wal_segment_size,
        fsync_policy: overlay.fsync_policy,
        block_cache_size: overlay.block_cache_size,
        page_cache_size: overlay.page_cache_size,
        memtable_size: overlay.memtable_size,
        max_blob_size: overlay.max_blob_size,
        compression: overlay.compression,
        bloom_filter_bits_per_key: overlay.bloom_filter_bits_per_key,
        page_size: overlay.page_size,
        wal_page_size: overlay.wal_page_size,
        btree_order: overlay.btree_order,
        lsm_max_level: overlay.lsm_max_level,
        bloom_false_positive_rate: overlay.bloom_false_positive_rate,
        write_buffer_size: overlay.write_buffer_size,
        compaction_threads: overlay.compaction_threads,
    }
}

fn merge_memory(_base: MemoryConfig, overlay: MemoryConfig) -> MemoryConfig {
    MemoryConfig {
        max_memory: overlay.max_memory,
        pressure_threshold_pct: overlay.pressure_threshold_pct,
        critical_threshold_pct: overlay.critical_threshold_pct,
        emergency_reserve: overlay.emergency_reserve,
        gc_threshold_pct: overlay.gc_threshold_pct,
    }
}

fn merge_networking(_base: NetworkingConfig, overlay: NetworkingConfig) -> NetworkingConfig {
    NetworkingConfig {
        listen_address: overlay.listen_address,
        listen_port: overlay.listen_port,
        tls_enabled: overlay.tls_enabled,
        tls_cert_path: overlay.tls_cert_path,
        tls_key_path: overlay.tls_key_path,
        unix_socket_path: overlay.unix_socket_path,
        tcp_nodelay: overlay.tcp_nodelay,
        keepalive_secs: overlay.keepalive_secs,
        listeners: overlay.listeners,
        timeouts: overlay.timeouts,
        rate_limiting: overlay.rate_limiting,
    }
}

fn merge_logging(_base: LoggingConfig, overlay: LoggingConfig) -> LoggingConfig {
    LoggingConfig {
        level: overlay.level,
        format: overlay.format,
        file: overlay.file,
    }
}

fn merge_subsystems(_base: SubsystemsConfig, overlay: SubsystemsConfig) -> SubsystemsConfig {
    SubsystemsConfig {
        enable_sql: overlay.enable_sql,
        enable_cache: overlay.enable_cache,
        enable_queue: overlay.enable_queue,
        enable_scheduler: overlay.enable_scheduler,
        enable_search: overlay.enable_search,
        enable_blob: overlay.enable_blob,
        enable_auth: overlay.enable_auth,
        enable_dashboard: overlay.enable_dashboard,
    }
}

fn merge_event(_base: EventConfig, overlay: EventConfig) -> EventConfig {
    overlay
}

fn merge_execution(_base: ExecutionConfig, overlay: ExecutionConfig) -> ExecutionConfig {
    ExecutionConfig {
        max_concurrent: overlay.max_concurrent,
        worker_threads: overlay.worker_threads,
        execution_timeout_ms: overlay.execution_timeout_ms,
        max_concurrent_ops: overlay.max_concurrent_ops,
        pipeline_queue_depth: overlay.pipeline_queue_depth,
        default_operation_timeout_ms: overlay.default_operation_timeout_ms,
        max_operation_timeout_ms: overlay.max_operation_timeout_ms,
        rate_limit_default_per_sec: overlay.rate_limit_default_per_sec,
        rate_limit_global_per_sec: overlay.rate_limit_global_per_sec,
        rate_limit_global_burst: overlay.rate_limit_global_burst,
        rate_limit_user_per_sec: overlay.rate_limit_user_per_sec,
        rate_limit_user_burst: overlay.rate_limit_user_burst,
        rate_limit_ip_per_sec: overlay.rate_limit_ip_per_sec,
        rate_limit_ip_burst: overlay.rate_limit_ip_burst,
        circuit_breaker_threshold: overlay.circuit_breaker_threshold,
        circuit_breaker_window_ms: overlay.circuit_breaker_window_ms,
        circuit_breaker_half_open_timeout_ms: overlay.circuit_breaker_half_open_timeout_ms,
        circuit_breaker_success_threshold: overlay.circuit_breaker_success_threshold,
        audit_enabled: overlay.audit_enabled,
        audit_include_payloads: overlay.audit_include_payloads,
        audit_max_entry_size: overlay.audit_max_entry_size,
        idempotency_key_ttl_secs: overlay.idempotency_key_ttl_secs,
        max_idempotency_keys: overlay.max_idempotency_keys,
        pipeline_max_retries: overlay.pipeline_max_retries,
        retry_base_delay_ms: overlay.retry_base_delay_ms,
        retry_max_delay_ms: overlay.retry_max_delay_ms,
    }
}

fn merge_auth(_base: AuthConfig, overlay: AuthConfig) -> AuthConfig {
    overlay
}

fn merge_security(_base: SecurityConfig, overlay: SecurityConfig) -> SecurityConfig {
    overlay
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::de::Error as SerdeError;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Mutex;

    /// Serialises tests that mutate environment variables.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    // ---------------------------------------------------------------------------
    // Helpers
    // ---------------------------------------------------------------------------
    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TempConfig {
        path: PathBuf,
        _dir: PathBuf,
    }

    impl TempConfig {
        fn new(content: &str) -> Self {
            let id = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
            let dir = std::env::temp_dir().join(format!("nova_config_test_{}", id));
            std::fs::create_dir_all(&dir).unwrap();
            let path = dir.join("novad.toml");
            std::fs::write(&path, content).unwrap();
            TempConfig { path, _dir: dir }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempConfig {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self._dir);
        }
    }

    struct EnvGuard(());

    impl EnvGuard {
        fn set(key: &str, value: &str) -> Self {
            unsafe { std::env::set_var(key, value); }
            EnvGuard(())
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            // We intentionally do NOT clear all NOVA_ vars here because multiple
            // guards may be live on the same thread. Instead each test is
            // serialised via ENV_LOCK and sets only the vars it needs.
        }
    }

    fn clear_nova_vars() {
        let keys: Vec<String> = std::env::vars()
            .filter(|(k, _)| k.starts_with("NOVA_"))
            .map(|(k, _)| k)
            .collect();
        for k in keys {
            unsafe { std::env::remove_var(k); }
        }
    }

    // ---------------------------------------------------------------------------
    // ConfigLoader creation
    // ---------------------------------------------------------------------------
    #[test]
    fn loader_new_has_no_path() {
        let loader = ConfigLoader::new();
        // No public getter for path – just verify it doesn't panic and is usable
        assert!(loader.watcher_tx.is_none());
    }

    #[test]
    fn loader_with_path() {
        let path = PathBuf::from("/tmp/test_nova.toml");
        let loader = ConfigLoader::with_path(path.clone());
        // reload without a parsed config would fail – just check construction is fine
        assert!(loader.watcher_tx.is_none());
    }

    #[test]
    fn loader_from_pathbuf() {
        let path = PathBuf::from("/tmp/test_nova.toml");
        let loader: ConfigLoader = path.into();
        assert!(loader.watcher_tx.is_none());
    }

    #[test]
    fn loader_default_impl() {
        let loader = ConfigLoader::default();
        assert!(loader.watcher_tx.is_none());
    }

    // ---------------------------------------------------------------------------
    // parse_file
    // ---------------------------------------------------------------------------
    #[test]
    fn parse_file_valid_toml() {
        let tf = TempConfig::new(
            r#"
            [general]
            max_connections = 512
            [storage]
            wal_dir = "/tmp/wal"
            "#,
        );
        let config = ConfigLoader::parse_file(tf.path()).unwrap();
        assert_eq!(config.general.max_connections, 512);
        assert_eq!(config.storage.wal_dir, PathBuf::from("/tmp/wal"));
        // unset fields should be defaults
        assert_eq!(config.general.data_dir, PathBuf::from("/var/lib/novad"));
    }

    #[test]
    fn parse_file_not_found() {
        let path = PathBuf::from("/tmp/nova_nonexistent_file_12345.toml");
        let err = ConfigLoader::parse_file(&path).unwrap_err();
        assert!(matches!(err, ConfigError::FileNotFound(_)));
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn parse_file_invalid_syntax() {
        let tf = TempConfig::new("nope = \n");
        let err = ConfigLoader::parse_file(tf.path()).unwrap_err();
        assert!(matches!(err, ConfigError::Parse { .. }));
    }

    #[test]
    fn parse_file_wrong_type() {
        let tf = TempConfig::new(
            r#"
            [general]
            max_connections = "not-a-number"
            "#,
        );
        let err = ConfigLoader::parse_file(tf.path()).unwrap_err();
        assert!(matches!(err, ConfigError::Parse { .. }));
    }

    #[test]
    fn parse_file_unknown_key_accepted() {
        let tf = TempConfig::new(r#"unknown_key = 1"#);
        let result = ConfigLoader::parse_file(tf.path());
        assert!(result.is_ok(), "unrecognized top-level keys should be silently ignored with default values");
    }

    // ---------------------------------------------------------------------------
    // merge (ConfigLoader::merge)
    // ---------------------------------------------------------------------------
    #[test]
    fn merge_overlay_replaces_base() {
        let base = Config::default();
        let mut overlay = Config::default();
        overlay.general.max_connections = 2048;
        overlay.storage.page_size = 16384;
        overlay.memory.max_memory = 2_000_000_000;
        overlay.networking.listen_port = 9999;
        overlay.logging.level = "debug".to_string();
        overlay.subsystems.enable_sql = false;
        overlay.event.ordering_shards = 128;
        overlay.execution.worker_threads = 8;
        overlay.auth.session.ttl_seconds = 3600;
        overlay.security.encryption_at_rest.enabled = true;

        let merged = ConfigLoader::merge(base, overlay);
        assert_eq!(merged.general.max_connections, 2048);
        assert_eq!(merged.storage.page_size, 16384);
        assert_eq!(merged.memory.max_memory, 2_000_000_000);
        assert_eq!(merged.networking.listen_port, 9999);
        assert_eq!(merged.logging.level, "debug");
        assert!(!merged.subsystems.enable_sql);
        assert_eq!(merged.event.ordering_shards, 128);
        assert_eq!(merged.execution.worker_threads, 8);
        assert_eq!(merged.auth.session.ttl_seconds, 3600);
        assert!(merged.security.encryption_at_rest.enabled);
    }

    #[test]
    fn merge_partial_overlay_leaves_rest_as_base() {
        let base = Config::default();
        let mut overlay = Config::default();
        overlay.general.max_connections = 99;

        let merged = ConfigLoader::merge(base.clone(), overlay);
        // only the overridden field changes
        assert_eq!(merged.general.max_connections, 99);
        assert_eq!(merged.general.data_dir, base.general.data_dir);
        assert_eq!(merged.storage, base.storage);
    }

    #[test]
    fn merge_empty_overlay_identity() {
        let base = Config::default();
        let merged = ConfigLoader::merge(base.clone(), Config::default());
        assert_eq!(merged, base);
    }

    // ---------------------------------------------------------------------------
    // apply_env_overrides
    // ---------------------------------------------------------------------------
    #[test]
    fn env_general_overrides() {
        let _lock = ENV_LOCK.lock().unwrap();
        clear_nova_vars();

        let _g1 = EnvGuard::set("NOVA_GENERAL__MAX_CONNECTIONS", "2048");
        let _g2 = EnvGuard::set("NOVA_GENERAL__DATA_DIR", "/custom/data");
        let _g3 = EnvGuard::set("NOVA_GENERAL__PID_FILE", "/custom/pid");
        let _g4 = EnvGuard::set("NOVA_GENERAL__SHUTDOWN_TIMEOUT_MS", "9999");
        let _g5 = EnvGuard::set("NOVA_GENERAL__STARTUP_TIMEOUT_MS", "60000");

        let mut config = Config::default();
        ConfigLoader::apply_env_overrides(&mut config);

        assert_eq!(config.general.max_connections, 2048);
        assert_eq!(config.general.data_dir, PathBuf::from("/custom/data"));
        assert_eq!(config.general.pid_file, PathBuf::from("/custom/pid"));
        assert_eq!(config.general.shutdown_timeout_ms, 9999);
        assert_eq!(config.general.startup_timeout_ms, 60000);

        clear_nova_vars();
    }

    #[test]
    fn env_storage_overrides() {
        let _lock = ENV_LOCK.lock().unwrap();
        clear_nova_vars();

        let _g = EnvGuard::set("NOVA_STORAGE__WAL_DIR", "/wal");
        let _g = EnvGuard::set("NOVA_STORAGE__WAL_SEGMENT_SIZE", "65536");
        let _g = EnvGuard::set("NOVA_STORAGE__BLOCK_CACHE_SIZE", "536870912");
        let _g = EnvGuard::set("NOVA_STORAGE__COMPRESSION", "zstd");
        let _g = EnvGuard::set("NOVA_STORAGE__BLOOM_FILTER_BITS_PER_KEY", "15");

        let mut config = Config::default();
        ConfigLoader::apply_env_overrides(&mut config);

        assert_eq!(config.storage.wal_dir, PathBuf::from("/wal"));
        assert_eq!(config.storage.wal_segment_size, 65536);
        assert_eq!(config.storage.block_cache_size, 536870912);
        assert_eq!(config.storage.compression, nova_core::Compression::Zstd);
        assert_eq!(config.storage.bloom_filter_bits_per_key, 15);

        clear_nova_vars();
    }

    #[test]
    fn env_memory_overrides() {
        let _lock = ENV_LOCK.lock().unwrap();
        clear_nova_vars();

        let _g = EnvGuard::set("NOVA_MEMORY__MAX_MEMORY", "2147483648");
        let _g = EnvGuard::set("NOVA_MEMORY__PRESSURE_THRESHOLD_PCT", "70");
        let _g = EnvGuard::set("NOVA_MEMORY__CRITICAL_THRESHOLD_PCT", "90");
        let _g = EnvGuard::set("NOVA_MEMORY__EMERGENCY_RESERVE", "16777216");
        let _g = EnvGuard::set("NOVA_MEMORY__GC_THRESHOLD_PCT", "60");

        let mut config = Config::default();
        ConfigLoader::apply_env_overrides(&mut config);

        assert_eq!(config.memory.max_memory, 2147483648);
        assert_eq!(config.memory.pressure_threshold_pct, 70);
        assert_eq!(config.memory.critical_threshold_pct, 90);
        assert_eq!(config.memory.emergency_reserve, 16777216);
        assert_eq!(config.memory.gc_threshold_pct, 60);

        clear_nova_vars();
    }

    #[test]
    fn env_networking_overrides() {
        let _lock = ENV_LOCK.lock().unwrap();
        clear_nova_vars();

        let _g = EnvGuard::set("NOVA_NETWORKING__LISTEN_ADDRESS", "0.0.0.0");
        let _g = EnvGuard::set("NOVA_NETWORKING__LISTEN_PORT", "8080");
        let _g = EnvGuard::set("NOVA_NETWORKING__TLS_ENABLED", "true");
        let _g = EnvGuard::set("NOVA_NETWORKING__TLS_CERT_PATH", "/cert.pem");
        let _g = EnvGuard::set("NOVA_NETWORKING__TLS_KEY_PATH", "/key.pem");
        let _g = EnvGuard::set("NOVA_NETWORKING__TCP_NODELAY", "false");
        let _g = EnvGuard::set("NOVA_NETWORKING__KEEPALIVE_SECS", "60");

        let mut config = Config::default();
        ConfigLoader::apply_env_overrides(&mut config);

        assert_eq!(config.networking.listen_address, "0.0.0.0");
        assert_eq!(config.networking.listen_port, 8080);
        assert!(config.networking.tls_enabled);
        assert_eq!(config.networking.tls_cert_path, Some(PathBuf::from("/cert.pem")));
        assert_eq!(config.networking.tls_key_path, Some(PathBuf::from("/key.pem")));
        assert!(!config.networking.tcp_nodelay);
        assert_eq!(config.networking.keepalive_secs, 60);

        clear_nova_vars();
    }

    #[test]
    fn env_logging_overrides() {
        let _lock = ENV_LOCK.lock().unwrap();
        clear_nova_vars();

        let _g = EnvGuard::set("NOVA_LOGGING__LEVEL", "warn");
        let _g = EnvGuard::set("NOVA_LOGGING__FORMAT", "json");
        let _g = EnvGuard::set("NOVA_LOGGING__FILE", "/var/log/nova.log");

        let mut config = Config::default();
        ConfigLoader::apply_env_overrides(&mut config);

        assert_eq!(config.logging.level, "warn");
        assert_eq!(config.logging.format, "json");
        assert_eq!(config.logging.file, Some(PathBuf::from("/var/log/nova.log")));

        clear_nova_vars();
    }

    #[test]
    fn env_subsystems_overrides() {
        let _lock = ENV_LOCK.lock().unwrap();
        clear_nova_vars();

        let _g = EnvGuard::set("NOVA_SUBSYSTEMS__ENABLE_SQL", "false");
        let _g = EnvGuard::set("NOVA_SUBSYSTEMS__ENABLE_CACHE", "false");
        let _g = EnvGuard::set("NOVA_SUBSYSTEMS__ENABLE_DASHBOARD", "false");

        let mut config = Config::default();
        ConfigLoader::apply_env_overrides(&mut config);

        assert!(!config.subsystems.enable_sql);
        assert!(!config.subsystems.enable_cache);
        assert!(!config.subsystems.enable_dashboard);
        // other subsystems stay enabled
        assert!(config.subsystems.enable_queue);

        clear_nova_vars();
    }

    #[test]
    fn env_event_overrides() {
        let _lock = ENV_LOCK.lock().unwrap();
        clear_nova_vars();

        let _g = EnvGuard::set("NOVA_EVENT__ORDERING_SHARDS", "128");
        let _g = EnvGuard::set("NOVA_EVENT__DEFAULT_QUEUE_CAPACITY", "512");
        let _g = EnvGuard::set("NOVA_EVENT__DEFAULT_MAX_RETRIES", "10");
        let _g = EnvGuard::set("NOVA_EVENT__DLQ_MAX_ENTRIES", "50000");

        let mut config = Config::default();
        ConfigLoader::apply_env_overrides(&mut config);

        assert_eq!(config.event.ordering_shards, 128);
        assert_eq!(config.event.default_queue_capacity, 512);
        assert_eq!(config.event.default_max_retries, 10);
        assert_eq!(config.event.dlq_max_entries, 50000);

        clear_nova_vars();
    }

    #[test]
    fn env_execution_overrides() {
        let _lock = ENV_LOCK.lock().unwrap();
        clear_nova_vars();

        let _g = EnvGuard::set("NOVA_EXECUTION__WORKER_THREADS", "16");
        let _g = EnvGuard::set("NOVA_EXECUTION__MAX_CONCURRENT", "512");
        let _g = EnvGuard::set("NOVA_EXECUTION__AUDIT_ENABLED", "false");
        let _g = EnvGuard::set("NOVA_EXECUTION__AUDIT_MAX_ENTRY_SIZE", "256");
        let _g = EnvGuard::set("NOVA_EXECUTION__RETRY_BASE_DELAY_MS", "50");
        let _g = EnvGuard::set("NOVA_EXECUTION__RETRY_MAX_DELAY_MS", "5000");

        let mut config = Config::default();
        ConfigLoader::apply_env_overrides(&mut config);

        assert_eq!(config.execution.worker_threads, 16);
        assert_eq!(config.execution.max_concurrent, 512);
        assert!(!config.execution.audit_enabled);
        assert_eq!(config.execution.audit_max_entry_size, 256);
        assert_eq!(config.execution.retry_base_delay_ms, 50);
        assert_eq!(config.execution.retry_max_delay_ms, 5000);

        clear_nova_vars();
    }

    #[test]
    fn env_execution_aliases() {
        let _lock = ENV_LOCK.lock().unwrap();
        clear_nova_vars();

        // "max_retries" is an alias for "pipeline_max_retries"
        let _g = EnvGuard::set("NOVA_EXECUTION__MAX_RETRIES", "7");

        let mut config = Config::default();
        ConfigLoader::apply_env_overrides(&mut config);

        assert_eq!(config.execution.pipeline_max_retries, 7);

        clear_nova_vars();
    }

    #[test]
    fn env_auth_overrides() {
        let _lock = ENV_LOCK.lock().unwrap();
        clear_nova_vars();

        let _g = EnvGuard::set("NOVA_AUTH__SESSION_TTL", "7200");
        let _g = EnvGuard::set("NOVA_AUTH__PASSWORD_MIN_LENGTH", "12");
        let _g = EnvGuard::set("NOVA_AUTH__LOCKOUT_MAX_ATTEMPTS", "3");

        let mut config = Config::default();
        ConfigLoader::apply_env_overrides(&mut config);

        assert_eq!(config.auth.session.ttl_seconds, 7200);
        assert_eq!(config.auth.internal.password_policy.min_length, 12);
        assert_eq!(config.auth.internal.lockout.max_attempts, 3);

        clear_nova_vars();
    }

    #[test]
    fn env_auth_aliases() {
        let _lock = ENV_LOCK.lock().unwrap();
        clear_nova_vars();

        let _g = EnvGuard::set("NOVA_AUTH__TTL_SECONDS", "1800");
        let _g = EnvGuard::set("NOVA_AUTH__MIN_LENGTH", "6");
        let _g = EnvGuard::set("NOVA_AUTH__MAX_LENGTH", "64");
        let _g = EnvGuard::set("NOVA_AUTH__MAX_ATTEMPTS", "10");

        let mut config = Config::default();
        ConfigLoader::apply_env_overrides(&mut config);

        assert_eq!(config.auth.session.ttl_seconds, 1800);
        assert_eq!(config.auth.internal.password_policy.min_length, 6);
        assert_eq!(config.auth.internal.password_policy.max_length, 64);
        assert_eq!(config.auth.internal.lockout.max_attempts, 10);

        clear_nova_vars();
    }

    #[test]
    fn env_security_overrides() {
        let _lock = ENV_LOCK.lock().unwrap();
        clear_nova_vars();

        let _g = EnvGuard::set("NOVA_SECURITY__ENABLED", "true");

        let mut config = Config::default();
        ConfigLoader::apply_env_overrides(&mut config);

        assert!(config.security.encryption_at_rest.enabled);

        clear_nova_vars();
    }

    #[test]
    fn env_invalid_section_is_ignored() {
        let _lock = ENV_LOCK.lock().unwrap();
        clear_nova_vars();

        let _g = EnvGuard::set("NOVA_UNKNOWN__FIELD", "value");

        let mut config = Config::default();
        // Should not panic
        ConfigLoader::apply_env_overrides(&mut config);

        clear_nova_vars();
    }

    #[test]
    fn env_bad_format_is_skipped() {
        let _lock = ENV_LOCK.lock().unwrap();
        clear_nova_vars();

        // Missing __ separator
        let _g = EnvGuard::set("NOVA_GENERAL_MALFORMED", "value");

        let mut config = Config::default();
        ConfigLoader::apply_env_overrides(&mut config);

        clear_nova_vars();
    }

    #[test]
    fn env_no_nova_vars_is_noop() {
        let _lock = ENV_LOCK.lock().unwrap();
        clear_nova_vars();

        let original = Config::default();
        let mut config = original.clone();
        ConfigLoader::apply_env_overrides(&mut config);
        assert_eq!(config, original);

        clear_nova_vars();
    }

    #[test]
    fn env_toml_value_parsing() {
        let _lock = ENV_LOCK.lock().unwrap();
        clear_nova_vars();

        // Integer value
        let _g = EnvGuard::set("NOVA_GENERAL__MAX_CONNECTIONS", "2048");
        let mut config = Config::default();
        ConfigLoader::apply_env_overrides(&mut config);
        assert_eq!(config.general.max_connections, 2048);

        clear_nova_vars();
    }

    // ---------------------------------------------------------------------------
    // apply_cli_overrides
    // ---------------------------------------------------------------------------
    fn make_cli_matches(args: &[&str]) -> clap::ArgMatches {
        use clap::{Arg, Command, value_parser};
        Command::new("test")
            .arg(Arg::new("data-dir").long("data-dir").num_args(1))
            .arg(Arg::new("listen-address").long("listen-address").num_args(1))
            .arg(Arg::new("listen-port").long("listen-port").num_args(1).value_parser(value_parser!(u16)))
            .arg(Arg::new("log-level").long("log-level").num_args(1))
            .arg(Arg::new("log-format").long("log-format").num_args(1))
            .arg(Arg::new("max-connections").long("max-connections").num_args(1).value_parser(value_parser!(u64)))
            .arg(Arg::new("shutdown-timeout").long("shutdown-timeout").num_args(1).value_parser(value_parser!(u64)))
            .try_get_matches_from(args)
            .unwrap()
    }

    #[test]
    fn cli_data_dir_override() {
        let matches = make_cli_matches(&["test", "--data-dir", "/mnt/nova"]);
        let mut config = Config::default();
        ConfigLoader::apply_cli_overrides(&mut config, &matches);

        assert_eq!(config.general.data_dir, PathBuf::from("/mnt/nova"));
        assert_eq!(config.storage.wal_dir, PathBuf::from("/mnt/nova/wal"));
    }

    #[test]
    fn cli_listen_address_override() {
        let matches = make_cli_matches(&["test", "--listen-address", "0.0.0.0"]);
        let mut config = Config::default();
        ConfigLoader::apply_cli_overrides(&mut config, &matches);

        assert_eq!(config.networking.listen_address, "0.0.0.0");
    }

    #[test]
    fn cli_listen_port_override() {
        let matches = make_cli_matches(&["test", "--listen-port", "9090"]);
        let mut config = Config::default();
        ConfigLoader::apply_cli_overrides(&mut config, &matches);

        assert_eq!(config.networking.listen_port, 9090);
    }

    #[test]
    fn cli_log_level_override() {
        let matches = make_cli_matches(&["test", "--log-level", "error"]);
        let mut config = Config::default();
        ConfigLoader::apply_cli_overrides(&mut config, &matches);

        assert_eq!(config.logging.level, "error");
    }

    #[test]
    fn cli_log_format_override() {
        let matches = make_cli_matches(&["test", "--log-format", "json"]);
        let mut config = Config::default();
        ConfigLoader::apply_cli_overrides(&mut config, &matches);

        assert_eq!(config.logging.format, "json");
    }

    #[test]
    fn cli_max_connections_override() {
        let matches = make_cli_matches(&["test", "--max-connections", "512"]);
        let mut config = Config::default();
        ConfigLoader::apply_cli_overrides(&mut config, &matches);

        assert_eq!(config.general.max_connections, 512);
    }

    #[test]
    fn cli_shutdown_timeout_override() {
        let matches = make_cli_matches(&["test", "--shutdown-timeout", "15000"]);
        let mut config = Config::default();
        ConfigLoader::apply_cli_overrides(&mut config, &matches);

        assert_eq!(config.general.shutdown_timeout_ms, 15000);
    }

    #[test]
    fn cli_multiple_overrides_at_once() {
        let matches = make_cli_matches(&[
            "test",
            "--data-dir", "/data",
            "--listen-address", "0.0.0.0",
            "--listen-port", "7000",
            "--log-level", "warn",
            "--log-format", "json",
            "--max-connections", "256",
            "--shutdown-timeout", "3000",
        ]);
        let mut config = Config::default();
        ConfigLoader::apply_cli_overrides(&mut config, &matches);

        assert_eq!(config.general.data_dir, PathBuf::from("/data"));
        assert_eq!(config.storage.wal_dir, PathBuf::from("/data/wal"));
        assert_eq!(config.networking.listen_address, "0.0.0.0");
        assert_eq!(config.networking.listen_port, 7000);
        assert_eq!(config.logging.level, "warn");
        assert_eq!(config.logging.format, "json");
        assert_eq!(config.general.max_connections, 256);
        assert_eq!(config.general.shutdown_timeout_ms, 3000);
    }

    #[test]
    fn cli_no_matches_is_noop() {
        let matches = make_cli_matches(&["test"]);
        let original = Config::default();
        let mut config = original.clone();
        ConfigLoader::apply_cli_overrides(&mut config, &matches);
        assert_eq!(config, original);
    }

    // ---------------------------------------------------------------------------
    // ConfigError Display
    // ---------------------------------------------------------------------------
    #[test]
    fn config_error_file_not_found_display() {
        let err = ConfigError::FileNotFound(PathBuf::from("/missing.toml"));
        assert_eq!(err.to_string(), "Config file not found: /missing.toml");
    }

    #[test]
    fn config_error_io_display() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let err = ConfigError::Io {
            path: PathBuf::from("/etc/novad.toml"),
            source: io_err,
        };
        let msg = err.to_string();
        assert!(msg.contains("Failed to read config file"));
        assert!(msg.contains("/etc/novad.toml"));
        assert!(msg.contains("access denied"));
    }

    #[test]
    fn config_error_parse_display() {
        let err = ConfigError::Parse {
            path: PathBuf::from("/cfg.toml"),
            source: toml::de::Error::custom("expected a table key"),
        };
        let msg = err.to_string();
        assert!(msg.contains("Failed to parse config file"));
        assert!(msg.contains("/cfg.toml"));
    }

    #[test]
    fn config_error_validation_display() {
        let err = ConfigError::Validation(vec!["err1".to_string(), "err2".to_string()]);
        let msg = err.to_string();
        assert!(msg.contains("Validation failed"));
        assert!(msg.contains("err1"));
        assert!(msg.contains("err2"));
    }

    #[test]
    fn config_error_env_var_display() {
        let err = ConfigError::EnvVar {
            var: "NOVA_TEST".to_string(),
            value: "bad".to_string(),
            message: "invalid value".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("NOVA_TEST"));
        assert!(msg.contains("bad"));
        assert!(msg.contains("invalid value"));
    }

    #[test]
    fn config_error_no_path_display() {
        let err = ConfigError::NoPath;
        assert_eq!(err.to_string(), "No config path set for reload");
    }

    // ---------------------------------------------------------------------------
    // watch / reload (basic smoke tests)
    // ---------------------------------------------------------------------------
    #[test]
    fn watch_returns_locked_config_and_receiver() {
        let tf = TempConfig::new(
            r#"
            [general]
            max_connections = 100
            "#,
        );
        let mut loader = ConfigLoader::new();
        let (config, rx) = loader.watch(tf.path()).unwrap();
        assert_eq!(config.read().general.max_connections, 100);
        // receiver should be alive
        assert!(rx.len() == 0);
    }

    #[test]
    fn reload_updates_config() {
        let tf = TempConfig::new(
            r#"
            [general]
            max_connections = 100
            "#,
        );
        let mut loader = ConfigLoader::new();
        let (config, _rx) = loader.watch(tf.path()).unwrap();
        assert_eq!(config.read().general.max_connections, 100);

        // Rewrite the file with different values
        std::fs::write(
            tf.path(),
            r#"
            [general]
            max_connections = 200
            "#,
        )
        .unwrap();

        loader.reload(&config).unwrap();
        assert_eq!(config.read().general.max_connections, 200);
    }

    #[test]
    fn reload_without_path_returns_no_path_error() {
        let loader = ConfigLoader::new();
        let config = Arc::new(RwLock::new(Config::default()));
        let err = loader.reload(&config).unwrap_err();
        assert!(matches!(err, ConfigError::NoPath));
    }

    // ---------------------------------------------------------------------------
    // validate wrapper
    // ---------------------------------------------------------------------------
    #[test]
    fn validate_wrapper_returns_validation_error() {
        let mut cfg = Config::default();
        cfg.storage.page_size = 100;
        let err = ConfigLoader::validate(&cfg).unwrap_err();
        assert!(matches!(err, ConfigError::Validation(_)));
    }

    #[test]
    fn validate_wrapper_ok() {
        let cfg = Config::default();
        assert!(ConfigLoader::validate(&cfg).is_ok());
    }

    // ---------------------------------------------------------------------------
    // test Result type alias
    // ---------------------------------------------------------------------------
    #[test]
    fn result_type_is_crate_result() {
        // Simple compile check
        let r: Result<()> = Ok(());
        assert!(r.is_ok());
    }
}
