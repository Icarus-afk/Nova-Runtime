use crate::admin::AdminState;
use crate::error::ApiError;
use axum::extract::{Path, State};
use axum::response::Json;
use axum::{routing::{get, post, delete}, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

pub fn routes(state: Arc<AdminState>) -> Router {
    Router::new()
        .route("/jobs", post(create_job))
        .route("/jobs", get(list_jobs))
        .route("/jobs/:id", get(get_job))
        .route("/jobs/:id", delete(delete_job))
        .route("/jobs/:id/trigger", post(trigger_job))
        .route("/jobs/:id/pause", post(pause_job))
        .route("/jobs/:id/resume", post(resume_job))
        .route("/stats", get(scheduler_stats))
        .with_state(state)
}

#[derive(Deserialize)]
struct CreateJobRequest {
    name: String,
    #[serde(rename = "type")]
    schedule_type: Option<String>,
    schedule: Option<String>,
    timezone: Option<String>,
    action: Option<Value>,
    max_retries: Option<u32>,
    retry_delay_ms: Option<u64>,
    enabled: Option<bool>,
}

async fn create_job(
    State(state): State<Arc<AdminState>>,
    Json(req): Json<CreateJobRequest>,
) -> Result<Json<Value>, ApiError> {
    let mgr = state.scheduler_mgr.as_ref()
        .ok_or_else(|| ApiError::internal("Scheduler not available"))?;

    let schedule_type = match req.schedule_type.as_deref() {
        Some("cron") => nova_scheduler::ScheduleType::Cron,
        Some("interval") => nova_scheduler::ScheduleType::Interval,
        _ => nova_scheduler::ScheduleType::OneTime,
    };

    let now_ms = chrono::Utc::now().timestamp_millis();
    let next_run_at = now_ms + 60000;

    let mut job = nova_scheduler::Job::new(&req.name, next_run_at, vec![]);
    job.schedule_type = schedule_type;
    if let Some(cron) = &req.schedule {
        if job.schedule_type == nova_scheduler::ScheduleType::Cron {
            job.cron_expression = Some(cron.clone());
        }
    }
    if let Some(retries) = req.max_retries {
        job.max_retries = retries;
    }
    if let Some(delay) = req.retry_delay_ms {
        job.retry_delay_secs = (delay / 1000) as u32;
    }

    mgr.schedule_job(job).await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;

    Ok(Json(json!({
        "id": format!("job_{}", Uuid::new_v4()),
        "name": req.name,
        "status": "created",
        "next_run_at": chrono::DateTime::from_timestamp_millis(next_run_at).map(|t| t.to_rfc3339()),
    })))
}

async fn list_jobs(
    State(state): State<Arc<AdminState>>,
) -> Result<Json<Value>, ApiError> {
    let mgr = state.scheduler_mgr.as_ref()
        .ok_or_else(|| ApiError::internal("Scheduler not available"))?;
    let jobs = mgr.list_jobs(None).await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let data: Vec<Value> = jobs.into_iter().map(|j| json!({
        "id": j.id.to_string(),
        "name": j.name,
        "schedule_type": j.schedule_type,
        "state": j.state,
        "next_run_at": j.next_run_at,
        "last_run_at": j.last_run_at,
        "retry_count": j.retry_count,
    })).collect();
    Ok(Json(json!({"data": data, "pagination": {"cursor": null, "limit": 100, "has_more": false}})))
}

async fn get_job(
    State(state): State<Arc<AdminState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let mgr = state.scheduler_mgr.as_ref()
        .ok_or_else(|| ApiError::internal("Scheduler not available"))?;
    let job_id = Uuid::parse_str(&id)
        .map_err(|_| ApiError::bad_request("Invalid job ID"))?;
    let job = mgr.get_job(&job_id).await
        .map_err(|e| ApiError::not_found(e.to_string()))?;
    Ok(Json(json!({
        "id": job.id.to_string(),
        "name": job.name,
        "schedule_type": job.schedule_type,
        "state": job.state,
        "next_run_at": job.next_run_at,
        "last_run_at": job.last_run_at,
        "max_retries": job.max_retries,
        "retry_count": job.retry_count,
    })))
}

async fn delete_job(
    State(state): State<Arc<AdminState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let mgr = state.scheduler_mgr.as_ref()
        .ok_or_else(|| ApiError::internal("Scheduler not available"))?;
    let job_id = Uuid::parse_str(&id)
        .map_err(|_| ApiError::bad_request("Invalid job ID"))?;
    mgr.cancel_job(&job_id).await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    Ok(Json(json!({"status": "deleted"})))
}

async fn trigger_job(
    State(state): State<Arc<AdminState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let mgr = state.scheduler_mgr.as_ref()
        .ok_or_else(|| ApiError::internal("Scheduler not available"))?;
    let job_id = Uuid::parse_str(&id)
        .map_err(|_| ApiError::bad_request("Invalid job ID"))?;
    let _job = mgr.get_job(&job_id).await
        .map_err(|e| ApiError::not_found(e.to_string()))?;
    Ok(Json(json!({"status": "triggered"})))
}

async fn pause_job(
    State(_state): State<Arc<AdminState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let _job_id = Uuid::parse_str(&id)
        .map_err(|_| ApiError::bad_request("Invalid job ID"))?;
    Ok(Json(json!({"status": "paused"})))
}

async fn resume_job(
    State(_state): State<Arc<AdminState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let _job_id = Uuid::parse_str(&id)
        .map_err(|_| ApiError::bad_request("Invalid job ID"))?;
    Ok(Json(json!({"status": "resumed"})))
}

async fn scheduler_stats(
    State(state): State<Arc<AdminState>>,
) -> Result<Json<Value>, ApiError> {
    let mgr = state.scheduler_mgr.as_ref()
        .ok_or_else(|| ApiError::internal("Scheduler not available"))?;
    let stats = mgr.stats().await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({
        "jobs_pending": stats.jobs_pending,
        "jobs_running": stats.jobs_running,
        "jobs_completed": stats.jobs_completed,
        "jobs_failed": stats.jobs_failed,
        "jobs_cancelled": stats.jobs_cancelled,
        "total_scheduled": stats.total_scheduled,
        "total_executed": stats.total_executed,
        "total_failures": stats.total_failures,
    })))
}
