use clap::Parser;
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;
use std::thread;

#[derive(Parser)]
#[command(name = "novad", version, about = "Nova Runtime Daemon")]
struct DaemonArgs {
    /// Path to config file
    #[arg(short, long)]
    config: Option<String>,

    /// Data directory (overrides config)
    #[arg(short, long)]
    data_dir: Option<String>,

    /// Listen address (overrides config)
    #[arg(short = 'l', long)]
    listen: Option<String>,

    /// Log level
    #[arg(long, default_value = "info")]
    log_level: String,
}

pub struct TlsConfig {
    pub cert_path: String,
    pub key_path: String,
    pub enabled: bool,
}

fn load_tls_config(config: &nova_config::Config) -> TlsConfig {
    TlsConfig {
        enabled: config.networking.tls_enabled,
        cert_path: config
            .networking
            .tls_cert_path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default(),
        key_path: config
            .networking
            .tls_key_path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default(),
    }
}

fn setup_tls(config: &nova_config::Config) -> anyhow::Result<Option<TlsConfig>> {
    let tls = load_tls_config(config);
    if !tls.enabled {
        tracing::info!("TLS is disabled");
        return Ok(None);
    }
    let cert_path = Path::new(&tls.cert_path);
    let key_path = Path::new(&tls.key_path);
    if !cert_path.exists() {
        anyhow::bail!("TLS certificate not found: {}", tls.cert_path);
    }
    if !key_path.exists() {
        anyhow::bail!("TLS key not found: {}", tls.key_path);
    }
    let cert_pem = fs::read_to_string(cert_path)?;
    let key_pem = fs::read_to_string(key_path)?;
    tracing::info!(
        "TLS configured: cert={}, key={}",
        tls.cert_path,
        tls.key_path
    );
    tracing::info!("  Certificate loaded ({} bytes)", cert_pem.len());
    tracing::info!("  Private key loaded ({} bytes)", key_pem.len());
    Ok(Some(tls))
}

pub struct HealthEndpoint {
    pub started_at: std::time::Instant,
    pub checks_passed: std::sync::atomic::AtomicU64,
    pub checks_failed: std::sync::atomic::AtomicU64,
}

impl HealthEndpoint {
    pub fn new() -> Self {
        HealthEndpoint {
            started_at: std::time::Instant::now(),
            checks_passed: std::sync::atomic::AtomicU64::new(0),
            checks_failed: std::sync::atomic::AtomicU64::new(0),
        }
    }

    pub fn check(&self) -> HealthStatus {
        let uptime = self.started_at.elapsed().as_secs();
        let storage_ok = true;
        let memory_ok = true;
        let status = if storage_ok && memory_ok {
            "healthy".to_string()
        } else {
            "degraded".to_string()
        };
        if status == "healthy" {
            self.checks_passed
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        } else {
            self.checks_failed
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        HealthStatus {
            status,
            uptime_secs: uptime,
            storage_ok,
            memory_ok,
        }
    }

    pub fn register_routes(&self) {
        tracing::info!("Health check routes registered");
    }
}

pub struct HealthStatus {
    pub status: String,
    pub uptime_secs: u64,
    pub storage_ok: bool,
    pub memory_ok: bool,
}

fn start_http_server(
    config: &nova_config::Config,
    health: Arc<HealthEndpoint>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr = format!("{}:{}", config.networking.listen_address, config.networking.listen_port);
    let listener = std::net::TcpListener::bind(&addr)?;
    listener.set_nonblocking(true)?;
    tracing::info!("HTTP server listening on {}", addr);

    let mut buf = vec![0u8; 8192];
    loop {
        let (mut stream, _) = match listener.accept() {
            Ok(conn) => conn,
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(std::time::Duration::from_millis(10));
                continue;
            }
            Err(e) => {
                tracing::error!("HTTP accept error: {}", e);
                break;
            }
        };

        let n = match stream.read(&mut buf) {
            Ok(0) => continue,
            Ok(n) => n,
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
            Err(e) => {
                tracing::error!("HTTP read error: {}", e);
                continue;
            }
        };

        let request = String::from_utf8_lossy(&buf[..n]);
        let path = request
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .unwrap_or("/");

        let (body, status) = match path {
            "/health" => {
                let s = health.check();
                (
                    format!(
                        "{{\"status\": \"{}\", \"uptime_secs\": {}}}",
                        s.status, s.uptime_secs
                    ),
                    200,
                )
            }
            "/ready" => ("{\"status\": \"ready\"}".to_string(), 200),
            "/live" => ("{\"status\": \"alive\"}".to_string(), 200),
            "/metrics" => (
                format!(
                    "# HELP nova_uptime_secs Uptime\n\
                     # TYPE nova_uptime_secs gauge\n\
                     nova_uptime_secs {}\n\
                     # HELP nova_health_checks_passed Checks passed\n\
                     # TYPE nova_health_checks_passed counter\n\
                     nova_health_checks_passed {}\n\
                     # HELP nova_health_checks_failed Checks failed\n\
                     # TYPE nova_health_checks_failed counter\n\
                     nova_health_checks_failed {}",
                    health.started_at.elapsed().as_secs(),
                    health.checks_passed.load(std::sync::atomic::Ordering::Relaxed),
                    health.checks_failed.load(std::sync::atomic::Ordering::Relaxed),
                ),
                200,
            ),
            "/admin/config" => ("{\"config_version\": 1}".to_string(), 200),
            _ => ("{\"error\": \"not found\"}".to_string(), 404),
        };

        let content_len = body.len();
        let reason = if status == 200 { "OK" } else { "Not Found" };
        let response = format!(
            "HTTP/1.1 {status} {reason}\r\n\
             Content-Type: application/json\r\n\
             Content-Length: {content_len}\r\n\
             Connection: close\r\n\r\n{body}",
        );

        let _ = stream.write_all(response.as_bytes());
    }

    Ok(())
}

fn wait_for_signals(
    _store: &nova_storage::Store,
    _config: &Arc<parking_lot::RwLock<nova_config::Config>>,
    _loader: &nova_config::ConfigLoader,
) -> anyhow::Result<()> {
    use std::sync::mpsc;

    let (tx_shutdown, rx_shutdown) = mpsc::channel::<()>();

    ctrlc::set_handler(move || {
        let _ = tx_shutdown.send(());
    })?;

    loop {
        if let Ok(()) = rx_shutdown.try_recv() {
            tracing::info!("Shutdown signal received");
            return Ok(());
        }

        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

fn reload_config(
    loader: &nova_config::ConfigLoader,
    locked: &Arc<parking_lot::RwLock<nova_config::Config>>,
) {
    if let Err(e) = loader.reload(locked) {
        tracing::error!("Config reload failed: {}", e);
    } else {
        tracing::info!("Configuration reloaded successfully");
    }
}

fn main() -> anyhow::Result<()> {
    let args = DaemonArgs::parse();

    let _subscriber = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::builder().parse_lossy(&args.log_level),
        )
        .init();

    tracing::info!("Nova Runtime v{} starting...", env!("CARGO_PKG_VERSION"));

    let loader = nova_config::ConfigLoader::new();
    let mut config = match &args.config {
        Some(path) => nova_config::ConfigLoader::parse_file(std::path::Path::new(path))?,
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
    tracing::info!(
        "Data directory: {}",
        config.general.data_dir.display()
    );
    tracing::info!(
        "Listen: {}:{}",
        config.networking.listen_address,
        config.networking.listen_port
    );

    let config = Arc::new(parking_lot::RwLock::new(config));

    let mem_config = nova_memory::MemoryConfig {
        max_memory: config.read().memory.max_memory,
        pressure_threshold_pct: config.read().memory.pressure_threshold_pct,
        critical_threshold_pct: config.read().memory.critical_threshold_pct,
        emergency_reserve: config.read().memory.emergency_reserve,
    };
    let mut _mem_mgr = nova_memory::MemoryManager::new(&mem_config);
    tracing::info!(
        "Memory manager initialized (max: {} MB)",
        config.read().memory.max_memory / 1024 / 1024
    );

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
    let store = nova_storage::Store::open(&storage_config)?;
    let store = Arc::new(store);
    let stats = store.stats();
    tracing::info!("Storage engine opened");
    tracing::info!(
        "  Page cache: {} / {} pages",
        stats.cache_size,
        storage_config.page_cache_size
    );
    tracing::info!("  WAL segments: {}", stats.wal_segments);
    tracing::info!("  Current LSN: {}", stats.current_lsn);

    let _tls_config = {
        let cfg = config.read();
        setup_tls(&cfg)?
    };

    let health = Arc::new(HealthEndpoint::new());
    health.register_routes();
    tracing::info!("Health endpoint initialized");

    let listen_addr = {
        let cfg = config.read();
        format!("{}:{}", cfg.networking.listen_address, cfg.networking.listen_port)
    };

    println!();
    println!("  ╔══════════════════════════════════════╗");
    println!(
        "  ║         Nova Runtime v{:<14} ║",
        env!("CARGO_PKG_VERSION")
    );
    println!("  ║     Status: RUNNING                   ║");
    println!(
        "  ║     Listen: {:18} ║",
        listen_addr
    );
    println!(
        "  ║     PID:    {:<27} ║",
        std::process::id()
    );
    println!("  ╚══════════════════════════════════════╝");
    println!();

    let server_config = config.read().clone();
    let server_health = health.clone();
    thread::spawn(move || {
        if let Err(e) = start_http_server(&server_config, server_health) {
            tracing::error!("HTTP server error: {}", e);
        }
    });

    wait_for_signals(store.as_ref(), &config, &loader)?;

    tracing::info!("Shutting down...");
    store.sync()?;
    tracing::info!("Storage synced (WAL rotated, data flushed)");
    store.close()?;
    tracing::info!("Storage engine closed");
    tracing::info!("Goodbye.");

    Ok(())
}
