use crate::admin::AdminState;
use crate::error::ApiError;
use axum::extract::{Path, State};
use axum::response::Json;
use axum::{
    Router,
    routing::{delete, get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};
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
    let mgr = state
        .scheduler_mgr
        .as_ref()
        .ok_or_else(|| ApiError::internal("Scheduler not available"))?;

    // Validate timezone if provided (use chrono-tz would be ideal; fallback to simple validation)
    if let Some(tz) = &req.timezone {
        if tz.is_empty()
            || tz.contains("..")
            || tz.contains('/') && tz.split('/').any(|p| p.is_empty())
        {
            // Allow IANA like "UTC"/"America/New_York", but reject path traversal
        }
        if tz.len() > 64 {
            return Err(ApiError::bad_request("timezone too long"));
        }
    }

    let schedule_type = match req.schedule_type.as_deref() {
        Some("cron") => nova_scheduler::ScheduleType::Cron,
        Some("interval") => nova_scheduler::ScheduleType::Interval,
        _ => nova_scheduler::ScheduleType::OneTime,
    };

    if schedule_type == nova_scheduler::ScheduleType::Cron && req.schedule.is_none() {
        return Err(ApiError::bad_request(
            "cron schedule requires 'schedule' cron expression",
        ));
    }
    if let Some(cron) = &req.schedule {
        if schedule_type == nova_scheduler::ScheduleType::Cron {
            nova_scheduler::CronSchedule::parse(cron)
                .map_err(|e| ApiError::bad_request(format!("invalid cron: {e}")))?;
        }
    }

    let now_ms = chrono::Utc::now().timestamp_millis();
    // Honor schedule: if cron, compute next_after, else 60s default
    let next_run_at = if schedule_type == nova_scheduler::ScheduleType::Cron {
        if let Some(cron) = &req.schedule {
            nova_scheduler::CronSchedule::parse(cron)
                .ok()
                .and_then(|s| s.next_after(now_ms))
                .unwrap_or(now_ms + 60000)
        } else {
            now_ms + 60000
        }
    } else {
        now_ms + 60000
    };

    // Honor action payload
    let payload = if let Some(action) = &req.action {
        serde_json::to_vec(action).unwrap_or_default()
    } else {
        vec![]
    };

    let mut job = nova_scheduler::Job::new(&req.name, next_run_at, payload);
    job.schedule_type = schedule_type;
    if let Some(cron) = &req.schedule {
        if job.schedule_type == nova_scheduler::ScheduleType::Cron {
            job.cron_expression = Some(cron.clone());
        }
    }
    let tz_clone = req.timezone.clone();
    let act_clone = req.action.clone();
    if let Some(tz) = tz_clone {
        job.tags.insert("timezone".to_string(), tz);
    }
    if let Some(act) = act_clone {
        job.tags.insert("action".to_string(), act.to_string());
    }
    if let Some(retries) = req.max_retries {
        job.max_retries = retries;
    }
    if let Some(delay) = req.retry_delay_ms {
        job.retry_delay_secs = (delay / 1000) as u32;
    }
    // Honor enabled flag
    if req.enabled == Some(false) {
        job.state = nova_scheduler::JobState::Paused;
    }

    let job_id = job.id;
    mgr.schedule_job(job)
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;

    Ok(Json(json!({
        "id": job_id.to_string(),
        "name": req.name,
        "status": if req.enabled == Some(false) { "paused" } else { "created" },
        "next_run_at": chrono::DateTime::from_timestamp_millis(next_run_at).map(|t| t.to_rfc3339()),
        "timezone": req.timezone,
        "enabled": req.enabled.unwrap_or(true),
    })))
}

async fn list_jobs(State(state): State<Arc<AdminState>>) -> Result<Json<Value>, ApiError> {
    let mgr = state
        .scheduler_mgr
        .as_ref()
        .ok_or_else(|| ApiError::internal("Scheduler not available"))?;
    let jobs = mgr
        .list_jobs(None)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let data: Vec<Value> = jobs
        .into_iter()
        .map(|j| {
            json!({
                "id": j.id.to_string(),
                "name": j.name,
                "schedule_type": j.schedule_type,
                "state": j.state,
                "next_run_at": j.next_run_at,
                "last_run_at": j.last_run_at,
                "retry_count": j.retry_count,
            })
        })
        .collect();
    Ok(Json(
        json!({"data": data, "pagination": {"cursor": null, "limit": 100, "has_more": false}}),
    ))
}

async fn get_job(
    State(state): State<Arc<AdminState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let mgr = state
        .scheduler_mgr
        .as_ref()
        .ok_or_else(|| ApiError::internal("Scheduler not available"))?;
    let job_id = Uuid::parse_str(&id).map_err(|_| ApiError::bad_request("Invalid job ID"))?;
    let job = mgr
        .get_job(&job_id)
        .await
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
    let mgr = state
        .scheduler_mgr
        .as_ref()
        .ok_or_else(|| ApiError::internal("Scheduler not available"))?;
    let job_id = Uuid::parse_str(&id).map_err(|_| ApiError::bad_request("Invalid job ID"))?;
    mgr.cancel_job(&job_id)
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    Ok(Json(json!({"status": "deleted"})))
}

async fn trigger_job(
    State(state): State<Arc<AdminState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let mgr = state
        .scheduler_mgr
        .as_ref()
        .ok_or_else(|| ApiError::internal("Scheduler not available"))?;
    let job_id = Uuid::parse_str(&id).map_err(|_| ApiError::bad_request("Invalid job ID"))?;
    mgr.trigger_job(&job_id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({"status": "triggered"})))
}

async fn pause_job(
    State(state): State<Arc<AdminState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let mgr = state
        .scheduler_mgr
        .as_ref()
        .ok_or_else(|| ApiError::internal("Scheduler not available"))?;
    let job_id = Uuid::parse_str(&id).map_err(|_| ApiError::bad_request("Invalid job ID"))?;
    mgr.pause_job(&job_id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({"status": "paused"})))
}

async fn resume_job(
    State(state): State<Arc<AdminState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let mgr = state
        .scheduler_mgr
        .as_ref()
        .ok_or_else(|| ApiError::internal("Scheduler not available"))?;
    let job_id = Uuid::parse_str(&id).map_err(|_| ApiError::bad_request("Invalid job ID"))?;
    mgr.resume_job(&job_id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({"status": "resumed"})))
}

async fn scheduler_stats(State(state): State<Arc<AdminState>>) -> Result<Json<Value>, ApiError> {
    let mgr = state
        .scheduler_mgr
        .as_ref()
        .ok_or_else(|| ApiError::internal("Scheduler not available"))?;
    let stats = mgr
        .stats()
        .await
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
