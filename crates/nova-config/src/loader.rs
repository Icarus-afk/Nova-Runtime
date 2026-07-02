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
    if let Some(config_dir) = std::env::var("XDG_CONFIG_HOME").ok() {
        Some(PathBuf::from(config_dir).join("nova/novad.toml"))
    } else if let Some(home) = std::env::var("HOME").ok() {
        Some(PathBuf::from(home).join(".config/nova/novad.toml"))
    } else {
        None
    }
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
        "worker_threads" => { if let Ok(n) = val_str.parse::<u16>() { cfg.worker_threads = n; } }
        "execution_timeout_ms" => { if let Ok(n) = val_str.parse::<u64>() { cfg.execution_timeout_ms = n; } }
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
    overlay
}

fn merge_auth(_base: AuthConfig, overlay: AuthConfig) -> AuthConfig {
    overlay
}

fn merge_security(_base: SecurityConfig, overlay: SecurityConfig) -> SecurityConfig {
    overlay
}
