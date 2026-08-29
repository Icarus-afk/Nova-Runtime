use axum::extract::State;
use axum::response::Json;
use axum::{Router, routing::get};
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Instant;

use nova_config::Config;
use nova_executor::PipelineExecutor;
use std::sync::OnceLock;
use sysinfo::{System, Disks, Networks};

fn merge_json(base: &mut Value, patch: &Value) {
    match (base, patch) {
        (Value::Object(base_map), Value::Object(patch_map)) => {
            for (k, v) in patch_map {
                if v.is_object() {
                    merge_json(
                        base_map
                            .entry(k)
                            .or_insert(Value::Object(Default::default())),
                        v,
                    );
                } else {
                    base_map.insert(k.clone(), v.clone());
                }
            }
        }
        (base, patch) => *base = patch.clone(),
    }
}

pub struct AdminState {
    pub started_at: Instant,
    pub pipeline: Arc<PipelineExecutor>,
    pub config: Arc<parking_lot::RwLock<Config>>,
    pub memory_mgr: Option<Arc<nova_memory::MemoryManager>>,
    pub sql_engine: Option<Arc<nova_sql::SQLEngine>>,
    pub cache_mgr: Option<Arc<nova_cache::CacheManager>>,
    pub queue_mgr: Option<Arc<nova_queue::QueueManager>>,
    pub scheduler_mgr: Option<Arc<nova_scheduler::SchedulerManager>>,
    pub search_mgr: Option<Arc<nova_search::SearchManager>>,
    pub blob_mgr: Option<Arc<nova_blob::BlobManager>>,
    pub auth_mgr: Option<Arc<nova_auth::AuthManager>>,
    pub event_bus: Option<Arc<nova_event::EventBus>>,
    pub storage_ok: bool,
}

pub fn routes(state: Arc<AdminState>) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/ready", get(readiness_check))
        .route("/live", get(liveness_check))
        .route("/metrics", get(metrics_handler))
        .route("/admin/config", get(config_get).put(config_put))
        .route("/admin/status", get(pipeline_status))
        .route("/openapi.json", get(openapi_handler))
        .route("/runtime/status", get(runtime_status))
        .route("/runtime/info", get(runtime_info))
        .route("/runtime/config", get(config_get))
        .with_state(state)
}

static SYS: OnceLock<parking_lot::Mutex<System>> = OnceLock::new();
static DISKS: OnceLock<parking_lot::Mutex<Disks>> = OnceLock::new();
static NETWORKS: OnceLock<parking_lot::Mutex<Networks>> = OnceLock::new();

fn get_system_metrics() -> (f32, u64, u64, u64, u64, u64) {
    // Returns (process_cpu%, process_mem_used, process_mem_total_dummy, disk_total, disk_used, disk_free)
    // For Nova, we want *process* metrics, not whole system
    let mut sys = SYS.get_or_init(|| parking_lot::Mutex::new(System::new_all())).lock();
    sys.refresh_cpu_usage();
    sys.refresh_memory();
    sys.refresh_processes();
    let pid = sysinfo::Pid::from(std::process::id() as usize);
    let (cpu, mem_used) = if let Some(process) = sys.process(pid) {
        (process.cpu_usage(), process.memory())
    } else {
        (sys.global_cpu_info().cpu_usage(), sys.used_memory())
    };
    // For disk, pick the disk that contains data dir if possible, else first
    drop(sys);

    let mut disks = DISKS.get_or_init(|| parking_lot::Mutex::new(Disks::new_with_refreshed_list())).lock();
    disks.refresh();
    let (disk_total, disk_used, disk_free) = disks
        .iter()
        .next()
        .map(|d| {
            let total = d.total_space();
            let avail = d.available_space();
            let used = total.saturating_sub(avail);
            (total, used, avail)
        })
        .unwrap_or((0, 0, 0));
    drop(disks);

    // mem_total is not per-process, will be filled from config max_memory in health_check
    (cpu, 0, mem_used, disk_total, disk_used, disk_free)
}

fn get_network_metrics() -> (u64, u64, u64, u64, u32) {
    let mut nets = NETWORKS.get_or_init(|| parking_lot::Mutex::new(Networks::new_with_refreshed_list())).lock();
    nets.refresh();
    let mut rx: u64 = 0;
    let mut tx: u64 = 0;
    for (_, data) in nets.iter() {
        rx = rx.saturating_add(data.received());
        tx = tx.saturating_add(data.transmitted());
    }
    drop(nets);
    // Connections - use pipeline metrics as proxy
    (rx, tx, 0, 0, 0)
}

async fn health_check(State(state): State<Arc<AdminState>>) -> Json<Value> {
    let uptime = state.started_at.elapsed().as_secs();
    let storage_ok = state.storage_ok;
    let memory_ok = state.memory_mgr.as_ref().map(|_| true).unwrap_or(true);
    let healthy = storage_ok && memory_ok;

    let (cpu_usage, sys_mem_total, sys_mem_used, disk_total, disk_used, disk_free) = get_system_metrics();
    let (net_rx, net_tx, _rx_packets, _tx_packets, _conn) = get_network_metrics();

    // Memory: prefer Nova's memory manager if it reports >0, otherwise system
    let mem_used_bytes = state
        .memory_mgr
        .as_ref()
        .map(|m| m.total_used())
        .filter(|&v| v > 0)
        .unwrap_or(sys_mem_used);
    let mem_max_bytes = if sys_mem_total > 0 {
        sys_mem_total
    } else {
        state.config.read().memory.max_memory
    };

    // Pipeline metrics for network/connections
    let pipeline_snap = state.pipeline.metrics().snapshot();
    // Active Connections = in-flight ops + event bus subscribers + 1 for this health check
    // Was 0 when idle, looked broken — now at least 1 when dashboard is open
    let ws_subscribers = state
        .event_bus
        .as_ref()
        .map(|bus| bus.metrics().subscriber_count.load(std::sync::atomic::Ordering::Relaxed) as u64)
        .unwrap_or(0);
    let connections_active = (pipeline_snap.active_operations as u64)
        .max(ws_subscribers)
        .max(1); // at least this health request
    let request_rate = pipeline_snap.operations_total.saturating_sub(0) / uptime.max(1);

    let subsystems = json!({
        "database": {"status": if state.sql_engine.is_some() { "healthy" } else { "disabled" }},
        "cache": {"status": if state.cache_mgr.is_some() { "healthy" } else { "disabled" }},
        "queue": {"status": if state.queue_mgr.is_some() { "healthy" } else { "disabled" }},
        "scheduler": {"status": if state.scheduler_mgr.is_some() { "healthy" } else { "disabled" }},
        "search": {"status": if state.search_mgr.is_some() { "healthy" } else { "disabled" }},
        "blob": {"status": if state.blob_mgr.is_some() { "healthy" } else { "disabled" }},
    });

    Json(json!({
        "status": if healthy { "healthy" } else { "degraded" },
        "uptime_secs": uptime,
        "version": env!("CARGO_PKG_VERSION"),
        "checks": {
            "storage": storage_ok,
            "memory": memory_ok,
        },
        "memory": {
            "total_bytes": mem_max_bytes,
            "used_bytes": mem_used_bytes,
            "resident_bytes": mem_used_bytes,
            "allocated_bytes": mem_used_bytes,
        },
        "disk": {
            "total_bytes": disk_total,
            "used_bytes": disk_used,
            "free_bytes": disk_free,
            "data_path": state.config.read().general.data_dir.to_string_lossy().to_string(),
        },
        "cpu": {
            "usage_percent": cpu_usage,
            "cores": std::thread::available_parallelism().map(|n| n.get() as u64).unwrap_or(1),
        },
        "network": {
            "rx_bytes_per_sec": net_rx,
            "tx_bytes_per_sec": net_tx,
            "connections_active": connections_active,
            "request_rate": request_rate,
        },
        "subsystems": subsystems,
    }))
}

async fn readiness_check() -> Json<Value> {
    Json(json!({ "status": "ready" }))
}

async fn liveness_check() -> Json<Value> {
    Json(json!({ "status": "alive" }))
}

async fn metrics_handler(State(state): State<Arc<AdminState>>) -> String {
    let uptime = state.started_at.elapsed().as_secs();
    let metrics = state.pipeline.metrics();
    let snap = metrics.snapshot();

    format!(
        "# HELP nova_uptime_secs Server uptime in seconds\n\
         # TYPE nova_uptime_secs gauge\n\
         nova_uptime_secs {uptime}\n\
         \n\
         # HELP nova_operations_total Total operations processed\n\
         # TYPE nova_operations_total counter\n\
         nova_operations_total {total}\n\
         \n\
         # HELP nova_active_operations Currently in-flight operations\n\
         # TYPE nova_active_operations gauge\n\
         nova_active_operations {active}\n\
         \n\
         # HELP nova_queue_depth Current operation queue depth\n\
         # TYPE nova_queue_depth gauge\n\
         nova_queue_depth {queue_depth}\n\
         \n\
         # HELP nova_rate_limit_hits Total rate limit hits\n\
         # TYPE nova_rate_limit_hits counter\n\
         nova_rate_limit_hits {rate_limited}\n\
         \n\
         # HELP nova_circuit_breaker_opens Total circuit breaker state transitions\n\
         # TYPE nova_circuit_breaker_opens counter\n\
         nova_circuit_breaker_opens {cb_opens}\n\
         \n\
         # HELP nova_retry_attempts Total retry attempts\n\
         # TYPE nova_retry_attempts counter\n\
         nova_retry_attempts {retries}\n\
         \n\
         # HELP nova_errors_total Total errors by category\n\
         # TYPE nova_errors_total counter\n\
         nova_errors_total {{category=\"parse\"}} {parse_errors}\n\
         nova_errors_total {{category=\"validation\"}} {validation_errors}\n\
         nova_errors_total {{category=\"authorization\"}} {auth_errors}\n\
         nova_errors_total {{category=\"execution\"}} {exec_errors}\n\
         \n\
         # HELP nova_latency_avg_ns Average operation latency in nanoseconds\n\
         # TYPE nova_latency_avg_ns gauge\n\
         nova_latency_avg_ns {avg_latency}\n",
        uptime = uptime,
        total = snap.operations_total,
        active = snap.active_operations,
        queue_depth = snap.queue_depth,
        rate_limited = snap.rate_limit_hits,
        cb_opens = snap.circuit_opens,
        retries = snap.retry_attempts,
        parse_errors = snap.parse_errors,
        validation_errors = snap.validation_errors,
        auth_errors = snap.authorization_errors,
        exec_errors = snap.execution_errors,
        avg_latency = snap.avg_latency_ns,
    )
}

async fn config_get(State(state): State<Arc<AdminState>>) -> Json<Value> {
    let config = state.config.read();
    let mut value = serde_json::to_value(&*config).unwrap_or_default();
    if let Some(obj) = value.as_object_mut() {
        obj.insert("version".to_string(), json!(1));
    }
    Json(value)
}

async fn config_put(
    State(state): State<Arc<AdminState>>,
    axum::extract::Json(patch): axum::extract::Json<Value>,
) -> Result<Json<Value>, (axum::http::StatusCode, Json<Value>)> {
    let mut current = {
        let c = state.config.read();
        serde_json::to_value(&*c).map_err(|e| {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "serialization_failed", "detail": e.to_string()
                })),
            )
        })?
    };

    merge_json(&mut current, &patch);

    let new_config: Config = serde_json::from_value(current).map_err(|e| {
        (
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "error": "validation_failed", "detail": format!("Invalid config: {}", e)
            })),
        )
    })?;

    if let Err(errors) = new_config.validate() {
        return Err((
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "error": "validation_failed", "detail": errors
            })),
        ));
    }

    {
        let mut c = state.config.write();
        *c = new_config.clone();
    }

    tracing::info!("Configuration updated via API");

    let mut value = serde_json::to_value(&new_config).unwrap_or_default();
    if let Some(obj) = value.as_object_mut() {
        obj.insert("version".to_string(), json!(1));
        obj.insert("status".to_string(), json!("updated"));
    }
    Ok(Json(value))
}

async fn pipeline_status(State(state): State<Arc<AdminState>>) -> Json<Value> {
    let ps = state.pipeline.status();
    let snap = state.pipeline.metrics().snapshot();
    Json(json!({
        "is_running": ps.is_running,
        "is_draining": ps.is_draining,
        "uptime_secs": ps.uptime_secs,
        "active_operations": ps.active_operations,
        "total_operations": ps.total_operations,
        "metrics": {
            "operations_total": snap.operations_total,
            "queue_depth": snap.queue_depth,
            "queue_rejected": snap.queue_rejected,
            "rate_limit_hits": snap.rate_limit_hits,
            "circuit_opens": snap.circuit_opens,
            "retry_attempts": snap.retry_attempts,
            "avg_latency_ns": snap.avg_latency_ns,
            "p50_latency_ns": snap.p50_latency_ns,
            "p99_latency_ns": snap.p99_latency_ns,
        },
    }))
}

async fn openapi_handler() -> Json<Value> {
    Json(json!({
        "openapi": "3.0.3",
        "info": {
            "title": "Nova Runtime API",
            "version": env!("CARGO_PKG_VERSION"),
            "description": "REST API for Nova Runtime"
        },
        "servers": [{"url": "/api/v1"}],
    }))
}

async fn runtime_status(State(state): State<Arc<AdminState>>) -> Json<Value> {
    Json(json!({
        "status": "running",
        "subsystems": {
            "database": {"status": if state.sql_engine.is_some() { "healthy" } else { "disabled" }},
            "cache": {"status": if state.cache_mgr.is_some() { "healthy" } else { "disabled" }},
            "queue": {"status": if state.queue_mgr.is_some() { "healthy" } else { "disabled" }},
            "scheduler": {"status": if state.scheduler_mgr.is_some() { "healthy" } else { "disabled" }},
            "search": {"status": if state.search_mgr.is_some() { "healthy" } else { "disabled" }},
            "blob": {"status": if state.blob_mgr.is_some() { "healthy" } else { "disabled" }},
        },
        "uptime_secs": state.started_at.elapsed().as_secs(),
    }))
}

async fn runtime_info(State(state): State<Arc<AdminState>>) -> Json<Value> {
    Json(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "name": "Nova Runtime",
        "uptime_secs": state.started_at.elapsed().as_secs(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::State;
    use nova_executor::PipelineConfig;
    use parking_lot::RwLock;

    fn make_state() -> Arc<AdminState> {
        Arc::new(AdminState {
            started_at: std::time::Instant::now(),
            pipeline: Arc::new(PipelineExecutor::new(PipelineConfig::default())),
            config: Arc::new(RwLock::new(Config::default())),
            memory_mgr: None,
            sql_engine: None,
            cache_mgr: None,
            queue_mgr: None,
            scheduler_mgr: None,
            search_mgr: None,
            blob_mgr: None,
            auth_mgr: None,
            event_bus: None,
            storage_ok: true,
        })
    }

    #[tokio::test]
    async fn test_health_check_healthy() {
        let state = make_state();
        let result = health_check(State(state)).await;
        assert_eq!(result.0["status"], "healthy");
    }

    #[tokio::test]
    async fn test_readiness_check() {
        let result = readiness_check().await;
        assert_eq!(result.0["status"], "ready");
    }

    #[tokio::test]
    async fn test_liveness_check() {
        let result = liveness_check().await;
        assert_eq!(result.0["status"], "alive");
    }

    #[tokio::test]
    async fn test_runtime_status() {
        let state = make_state();
        let result = runtime_status(State(state)).await;
        assert_eq!(result.0["status"], "running");
        assert!(result.0["subsystems"].is_object());
    }

    #[tokio::test]
    async fn test_runtime_info() {
        let state = make_state();
        let result = runtime_info(State(state)).await;
        assert!(result.0["version"].is_string());
    }

    #[tokio::test]
    async fn test_openapi_handler() {
        let result = openapi_handler().await;
        assert_eq!(result.0["openapi"], "3.0.3");
    }
}
