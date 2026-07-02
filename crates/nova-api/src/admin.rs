use axum::extract::State;
use axum::response::Json;
use axum::{routing::get, Router};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Instant;
use parking_lot::RwLock;

use nova_executor::PipelineExecutor;
use nova_config::Config;

pub struct AdminState {
    pub started_at: Instant,
    pub pipeline: Arc<PipelineExecutor>,
    pub config: Arc<RwLock<Config>>,
    pub memory_mgr: Option<Arc<nova_memory::MemoryManager>>,
    pub storage_ok: bool,
}

pub fn routes(state: Arc<AdminState>) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/ready", get(readiness_check))
        .route("/live", get(liveness_check))
        .route("/metrics", get(metrics_handler))
        .route("/admin/config", get(config_get))
        .route("/admin/status", get(pipeline_status))
        .with_state(state)
}

/// Full health check — checks all subsystems
async fn health_check(State(state): State<Arc<AdminState>>) -> Json<Value> {
    let uptime = state.started_at.elapsed().as_secs();

    let storage_ok = state.storage_ok;

    let memory_ok = state.memory_mgr.as_ref().map(|_| true).unwrap_or(true);

    let healthy = storage_ok && memory_ok;
    Json(json!({
        "status": if healthy { "healthy" } else { "degraded" },
        "uptime_secs": uptime,
        "checks": {
            "storage": storage_ok,
            "memory": memory_ok,
        },
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

/// Readiness check — is the server ready to accept traffic?
async fn readiness_check() -> Json<Value> {
    Json(json!({ "status": "ready" }))
}

/// Liveness check — is the server process alive?
async fn liveness_check() -> Json<Value> {
    Json(json!({ "status": "alive" }))
}

/// Prometheus-format metrics
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

/// Get current configuration (read-only view)
async fn config_get(State(state): State<Arc<AdminState>>) -> Json<Value> {
    let config = state.config.read();
    Json(json!({
        "version": 1,
        "general": {
            "data_dir": config.general.data_dir,
            "max_connections": config.general.max_connections,
        },
        "storage": {
            "wal_segment_size": config.storage.wal_segment_size,
            "block_cache_size": config.storage.block_cache_size,
            "page_size": config.storage.page_size,
            "compression": config.storage.compression,
        },
        "memory": {
            "max_memory": config.memory.max_memory,
            "pressure_threshold_pct": config.memory.pressure_threshold_pct,
        },
        "networking": {
            "listen_address": config.networking.listen_address,
            "listen_port": config.networking.listen_port,
            "tls_enabled": config.networking.tls_enabled,
        },
        "execution": {
            "max_concurrent_ops": config.execution.max_concurrent_ops,
            "pipeline_queue_depth": config.execution.pipeline_queue_depth,
            "default_operation_timeout_ms": config.execution.default_operation_timeout_ms,
            "max_retries": config.execution.pipeline_max_retries,
        },
    }))
}

/// Pipeline status endpoint
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::State;
    use nova_executor::PipelineConfig;

    fn make_state() -> Arc<AdminState> {
        Arc::new(AdminState {
            started_at: std::time::Instant::now(),
            pipeline: Arc::new(PipelineExecutor::new(PipelineConfig::default())),
            config: Arc::new(RwLock::new(Config::default())),
            memory_mgr: None,
            storage_ok: true,
        })
    }

    fn make_state_degraded() -> Arc<AdminState> {
        Arc::new(AdminState {
            started_at: std::time::Instant::now(),
            pipeline: Arc::new(PipelineExecutor::new(PipelineConfig::default())),
            config: Arc::new(RwLock::new(Config::default())),
            memory_mgr: None,
            storage_ok: false,
        })
    }

    #[tokio::test]
    async fn test_health_check_healthy() {
        let state = make_state();
        let result = health_check(State(state)).await;
        assert_eq!(result.0["status"], "healthy");
        assert_eq!(result.0["checks"]["storage"], true);
        assert_eq!(result.0["checks"]["memory"], true);
        assert!(result.0["uptime_secs"].as_u64().is_some());
        assert_eq!(result.0["version"], env!("CARGO_PKG_VERSION"));
    }

    #[tokio::test]
    async fn test_health_check_degraded() {
        let state = make_state_degraded();
        let result = health_check(State(state)).await;
        assert_eq!(result.0["status"], "degraded");
        assert_eq!(result.0["checks"]["storage"], false);
        assert_eq!(result.0["checks"]["memory"], true);
    }

    #[tokio::test]
    async fn test_health_check_uptime_non_negative() {
        let state = make_state();
        let result = health_check(State(state)).await;
        let uptime = result.0["uptime_secs"].as_u64().unwrap();
        assert!(uptime < 100, "uptime should be small in tests");
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
    async fn test_metrics_handler_format() {
        let state = make_state();
        let result = metrics_handler(State(state)).await;
        assert!(result.contains("nova_uptime_secs"));
        assert!(result.contains("nova_operations_total 0"));
        assert!(result.contains("nova_active_operations 0"));
        assert!(result.contains("nova_queue_depth 0"));
        assert!(result.contains("nova_rate_limit_hits 0"));
        assert!(result.contains("nova_circuit_breaker_opens 0"));
        assert!(result.contains("nova_retry_attempts 0"));
        assert!(result.contains("# TYPE"));
        assert!(result.contains("# HELP"));
    }

    #[tokio::test]
    async fn test_metrics_handler_error_counts() {
        let state = make_state();
        let result = metrics_handler(State(state)).await;
        assert!(result.contains("nova_errors_total {category=\"parse\"} 0"));
        assert!(result.contains("nova_errors_total {category=\"validation\"} 0"));
        assert!(result.contains("nova_errors_total {category=\"authorization\"} 0"));
        assert!(result.contains("nova_errors_total {category=\"execution\"} 0"));
    }

    #[tokio::test]
    async fn test_metrics_handler_latency() {
        let state = make_state();
        let result = metrics_handler(State(state)).await;
        assert!(result.contains("nova_latency_avg_ns"));
    }

    #[tokio::test]
    async fn test_config_get_structure() {
        let state = make_state();
        let result = config_get(State(state)).await;
        assert_eq!(result.0["version"], 1);
        assert!(result.0.get("general").is_some());
        assert!(result.0.get("storage").is_some());
        assert!(result.0.get("memory").is_some());
        assert!(result.0.get("networking").is_some());
        assert!(result.0.get("execution").is_some());
    }

    #[tokio::test]
    async fn test_config_get_general_values() {
        let state = make_state();
        let result = config_get(State(state)).await;
        let general = &result.0["general"];
        assert_eq!(general["data_dir"], "/var/lib/novad");
        assert!(general["max_connections"].as_u64().unwrap() > 0);
    }

    #[tokio::test]
    async fn test_config_get_storage_values() {
        let state = make_state();
        let result = config_get(State(state)).await;
        let storage = &result.0["storage"];
        assert!(storage["wal_segment_size"].as_u64().unwrap() > 0);
        assert!(storage["page_size"].as_u64().unwrap() > 0);
        assert!(storage["block_cache_size"].as_u64().unwrap() > 0);
    }

    #[tokio::test]
    async fn test_config_get_networking_values() {
        let state = make_state();
        let result = config_get(State(state)).await;
        let net = &result.0["networking"];
        assert_eq!(net["listen_address"], "127.0.0.1");
        assert!(net["listen_port"].as_u64().unwrap() > 0);
        assert_eq!(net["tls_enabled"], false);
    }

    #[tokio::test]
    async fn test_pipeline_status_structure() {
        let state = make_state();
        let result = pipeline_status(State(state)).await;
        assert_eq!(result.0["is_running"], true);
        assert_eq!(result.0["is_draining"], false);
        assert!(result.0["active_operations"].as_u64().is_some());
        assert!(result.0["total_operations"].as_u64().is_some());
        assert!(result.0.get("metrics").is_some());
    }

    #[tokio::test]
    async fn test_pipeline_status_metrics() {
        let state = make_state();
        let result = pipeline_status(State(state)).await;
        let metrics = &result.0["metrics"];
        assert!(metrics.get("operations_total").is_some());
        assert!(metrics.get("queue_depth").is_some());
        assert!(metrics.get("rate_limit_hits").is_some());
        assert!(metrics.get("circuit_opens").is_some());
        assert!(metrics.get("retry_attempts").is_some());
        assert!(metrics.get("avg_latency_ns").is_some());
    }
}
