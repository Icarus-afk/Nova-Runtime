use clap::Parser;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "novad", version, about = "Nova Runtime Daemon")]
struct DaemonArgs {
    #[arg(short, long)]
    config: Option<String>,
    #[arg(short, long)]
    data_dir: Option<String>,
    #[arg(short = 'l', long)]
    listen: Option<String>,
    #[arg(long, default_value = "info")]
    log_level: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = DaemonArgs::parse();

    let _subscriber = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::builder().parse_lossy(&args.log_level))
        .init();

    tracing::info!("Nova Runtime v{} starting...", env!("CARGO_PKG_VERSION"));

    // Resolve config file path for hot-reload
    let config_path = resolve_config_path(&args.config);
    if let Some(ref path) = config_path {
        tracing::info!("Config file: {}", path.display());
    }

    // Create loader with the resolved path so reload() knows which file to re-read
    let loader = config_path.as_ref()
        .map(|p| nova_config::ConfigLoader::with_path(p.clone()))
        .unwrap_or_else(nova_config::ConfigLoader::new);

    let mut config = match &args.config {
        Some(path) => nova_config::ConfigLoader::parse_file(Path::new(path))?,
        None => loader.load(None)?,
    };

    if let Some(dir) = &args.data_dir {
        config.general.data_dir = std::path::PathBuf::from(dir);
    }
    if let Some(listen) = &args.listen {
        if let Some((addr, port)) = listen.split_once(':') {
            config.networking.listen_address = addr.to_string();
            if let Ok(p) = port.parse() {
                config.networking.listen_port = p;
            }
        }
    }

    tracing::info!("Configuration loaded");
    tracing::info!("Data directory: {}", config.general.data_dir.display());
    tracing::info!("Listen: {}:{}", config.networking.listen_address, config.networking.listen_port);

    let config = Arc::new(parking_lot::RwLock::new(config));

    // Initialize memory manager
    let mem_config = nova_memory::MemoryConfig {
        max_memory: config.read().memory.max_memory,
        pressure_threshold_pct: config.read().memory.pressure_threshold_pct,
        critical_threshold_pct: config.read().memory.critical_threshold_pct,
        emergency_reserve: config.read().memory.emergency_reserve,
    };
    let memory_mgr = Arc::new(nova_memory::MemoryManager::new(&mem_config));
    tracing::info!("Memory manager initialized (max: {} MB)", config.read().memory.max_memory / 1024 / 1024);

    // Initialize storage engine
    let storage_config = nova_storage::StorageConfig {
        data_dir: config.read().general.data_dir.clone(),
        wal_dir: config.read().storage.wal_dir.clone(),
        page_cache_size: config.read().storage.page_cache_size as usize,
        memtable_size: config.read().storage.memtable_size as usize,
        fsync_policy: match &config.read().storage.fsync_policy {
            nova_config::FsyncPolicy::EveryWrite => nova_core::FsyncPolicy::EveryWrite,
            nova_config::FsyncPolicy::EveryNMs(ms) => nova_core::FsyncPolicy::EveryNMs(*ms),
            nova_config::FsyncPolicy::Async => nova_core::FsyncPolicy::Async,
        },
        btree_order: 128,
    };
    let store = Arc::new(nova_storage::Store::open(&storage_config)?);
    let stats = store.stats();
    tracing::info!("Storage engine opened");
    tracing::info!("  Page cache: {} / {} pages", stats.cache_size, storage_config.page_cache_size);
    tracing::info!("  WAL segments: {}", stats.wal_segments);
    tracing::info!("  Current LSN: {}", stats.current_lsn);

    // Setup TLS
    if config.read().networking.tls_enabled {
        let cfg = config.read();
        let cert_path = Path::new(cfg.networking.tls_cert_path.as_ref()
            .ok_or_else(|| anyhow::anyhow!("TLS cert path required"))?);
        let key_path = Path::new(cfg.networking.tls_key_path.as_ref()
            .ok_or_else(|| anyhow::anyhow!("TLS key path required"))?);
        if !cert_path.exists() {
            anyhow::bail!("TLS certificate not found: {}", cert_path.display());
        }
        if !key_path.exists() {
            anyhow::bail!("TLS key not found: {}", key_path.display());
        }
        tracing::info!("TLS configured: cert={}, key={}", cert_path.display(), key_path.display());
    } else {
        tracing::info!("TLS is disabled");
    }

    // Initialize pipeline executor
    let exec_config = nova_executor::PipelineConfig {
        max_concurrent_ops: config.read().execution.max_concurrent_ops,
        pipeline_queue_depth: config.read().execution.pipeline_queue_depth,
        worker_threads: config.read().execution.worker_threads as u32,
        default_operation_timeout_ms: config.read().execution.default_operation_timeout_ms,
        max_operation_timeout_ms: config.read().execution.max_operation_timeout_ms,
        rate_limit_global_per_sec: config.read().execution.rate_limit_global_per_sec as f64,
        rate_limit_global_burst: config.read().execution.rate_limit_global_burst as f64,
        rate_limit_user_per_sec: config.read().execution.rate_limit_user_per_sec as f64,
        rate_limit_user_burst: config.read().execution.rate_limit_user_burst as f64,
        rate_limit_ip_per_sec: config.read().execution.rate_limit_ip_per_sec as f64,
        rate_limit_ip_burst: config.read().execution.rate_limit_ip_burst as f64,
        circuit_breaker_threshold: config.read().execution.circuit_breaker_threshold,
        circuit_breaker_window_ms: config.read().execution.circuit_breaker_window_ms,
        circuit_breaker_half_open_timeout_ms: config.read().execution.circuit_breaker_half_open_timeout_ms,
        circuit_breaker_success_threshold: config.read().execution.circuit_breaker_success_threshold,
        audit_enabled: config.read().execution.audit_enabled,
        audit_include_payloads: config.read().execution.audit_include_payloads,
        audit_max_entry_size: config.read().execution.audit_max_entry_size,
        idempotency_key_ttl_secs: config.read().execution.idempotency_key_ttl_secs,
        max_idempotency_keys: config.read().execution.max_idempotency_keys,
        max_retries: config.read().execution.pipeline_max_retries,
        retry_base_delay_ms: config.read().execution.retry_base_delay_ms,
        retry_max_delay_ms: config.read().execution.retry_max_delay_ms,
    };
    let pipeline = Arc::new(nova_executor::PipelineExecutor::new(exec_config));
    tracing::info!("Execution engine initialized");

    // Initialize cache manager
    {
        let cfg = config.read().cache.clone();
        let eviction_policy = match cfg.eviction_policy.as_str() {
            "Lfu" => nova_cache::EvictionPolicy::Lfu,
            "Ttl" => nova_cache::EvictionPolicy::Ttl,
            "LruWithTtl" => nova_cache::EvictionPolicy::LruWithTtl,
            "NoEviction" => nova_cache::EvictionPolicy::NoEviction,
            _ => nova_cache::EvictionPolicy::Lru,
        };
        let backend_type = match cfg.backend_type.as_str() {
            "Redis" => nova_cache::BackendType::Redis,
            _ => nova_cache::BackendType::HashMap,
        };
        let cache_cfg = nova_cache::CacheConfig {
            max_size: cfg.max_size,
            default_ttl_secs: cfg.default_ttl_secs,
            eviction_policy,
            backend_type,
            redis_url: cfg.redis_url.clone(),
        };
        let backend = Arc::new(nova_cache::HashMapBackend::new(
            cache_cfg.max_size,
            Arc::new(nova_cache::CacheMetrics::default()),
        ));
        let _cache_mgr = Arc::new(nova_cache::CacheManager::new(backend, cache_cfg));
    }
    tracing::info!("Cache manager initialized");

    // Initialize blob manager
    let blob_cfg = {
        let cfg = config.read().blob.clone();
        nova_blob::BlobConfig {
            chunk_size: cfg.chunk_size,
            max_blob_size: cfg.max_blob_size,
            gc_interval_secs: cfg.gc_interval_secs,
            gc_grace_period_secs: cfg.gc_grace_period_secs,
            data_dir: cfg.data_dir,
        }
    };
    let _blob_mgr = Arc::new(nova_blob::BlobManager::new(blob_cfg).await?);
    tracing::info!("Blob manager initialized");

    // Initialize search manager
    let search_cfg = {
        let cfg = config.read().search.clone();
        nova_search::SearchConfig {
            default_limit: cfg.default_limit,
            max_limit: cfg.max_limit,
            bm25_k1: cfg.bm25_k1,
            bm25_b: cfg.bm25_b,
            fuzzy_max_distance: cfg.fuzzy_max_distance,
            highlight_snippet_len: cfg.highlight_snippet_len,
            highlight_max_snippets: cfg.highlight_max_snippets,
            refresh_interval_ms: cfg.refresh_interval_ms,
            merge_segment_threshold: cfg.merge_segment_threshold,
        }
    };
    let _search_mgr = Arc::new(parking_lot::RwLock::new(
        nova_search::SearchManager::with_config(search_cfg),
    ));
    tracing::info!("Search manager initialized");

    // Initialize SQL engine
    let sql_cfg = {
        let cfg = config.read().sql.clone();
        nova_sql::SQLConfig {
            max_batch_size: cfg.max_batch_size,
            max_columns: cfg.max_columns,
            default_limit: cfg.default_limit,
        }
    };
    let _sql_engine = Arc::new(nova_sql::SQLEngine::new(sql_cfg));
    tracing::info!("SQL engine initialized");

    // Build admin state
    let listen_addr = format!("{}:{}", config.read().networking.listen_address, config.read().networking.listen_port);
    let admin_state = Arc::new(nova_api::admin::AdminState {
        started_at: std::time::Instant::now(),
        pipeline: pipeline.clone(),
        config: config.clone(),
        memory_mgr: Some(memory_mgr),
        storage_ok: true,
    });

    // Shutdown channel
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    // Handle SIGINT / SIGTERM
    let shutdown_tx_clone = shutdown_tx.clone();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        tracing::info!("Shutdown signal received");
        let _ = shutdown_tx_clone.send(true);
    });

    // Handle SIGHUP for config hot-reload
    if let Ok(mut sighup) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup()) {
        let config_for_reload = config.clone();
        tokio::spawn(async move {
            loop {
                sighup.recv().await;
                tracing::info!("SIGHUP received, reloading configuration...");
                match loader.reload(&config_for_reload) {
                    Ok(()) => tracing::info!("Configuration reloaded successfully"),
                    Err(e) => tracing::error!("Failed to reload configuration: {}", e),
                }
            }
        });
    } else {
        tracing::warn!("SIGHUP handler not available on this platform");
    }

    println!();
    println!("  ╔══════════════════════════════════════╗");
    println!("  ║         Nova Runtime v{:<14} ║", env!("CARGO_PKG_VERSION"));
    println!("  ║     Status: RUNNING                   ║");
    println!("  ║     Listen: {:18} ║", listen_addr);
    println!("  ║     PID:    {:<27} ║", std::process::id());
    println!("  ╚══════════════════════════════════════╝");
    println!();

    // Start HTTP server
    let server_result = nova_api::server::start_server(
        &listen_addr,
        admin_state,
        shutdown_rx,
    ).await;

    match server_result {
        Ok(()) => tracing::info!("HTTP server shut down gracefully"),
        Err(e) => tracing::error!("HTTP server error: {}", e),
    }

    // Graceful shutdown
    tracing::info!("Shutting down...");
    tracing::info!("Draining pipeline...");
    let drain_result = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        pipeline.drain(std::time::Duration::from_secs(30)),
    ).await;
    match drain_result {
        Ok(Ok(())) => tracing::info!("Pipeline drained"),
        _ => tracing::warn!("Pipeline drain incomplete"),
    }

    tracing::info!("Closing storage...");
    if let Err(e) = store.close() {
        tracing::error!("Storage close error: {}", e);
    } else {
        tracing::info!("Storage engine closed");
    }

    tracing::info!("Goodbye.");
    Ok(())
}

/// Resolve the config file path to watch for hot-reload.
/// Priority: CLI `--config` argument > local ./novad.toml > user config > system config.
fn resolve_config_path(cli_path: &Option<String>) -> Option<PathBuf> {
    if let Some(path) = cli_path {
        return Some(PathBuf::from(path));
    }
    let local = PathBuf::from("./novad.toml");
    if local.exists() {
        return Some(local);
    }
    let user_path = if let Some(config_dir) = std::env::var("XDG_CONFIG_HOME").ok() {
        PathBuf::from(config_dir).join("nova/novad.toml")
    } else if let Some(home) = std::env::var("HOME").ok() {
        PathBuf::from(home).join(".config/nova/novad.toml")
    } else {
        PathBuf::from("/etc/novad/novad.toml")
    };
    if user_path.exists() {
        return Some(user_path);
    }
    let system_path = PathBuf::from("/etc/novad/novad.toml");
    if system_path.exists() {
        return Some(system_path);
    }
    None
}
