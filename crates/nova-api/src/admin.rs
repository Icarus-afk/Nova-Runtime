use axum::extract::State;
use axum::response::Json;
use axum::{routing::get, Router};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Instant;

use nova_executor::PipelineExecutor;
use nova_config::Config;

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
        .route("/openapi.json", get(openapi_handler))
        .route("/runtime/status", get(runtime_status))
        .route("/runtime/info", get(runtime_info))
        .route("/runtime/config", get(config_get))
        .with_state(state)
}

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
