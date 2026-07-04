use crate::admin::AdminState;
use crate::error::ApiError;
use axum::extract::{Path, State};
use axum::response::Json;
use axum::{routing::{get, post, delete}, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

pub fn routes(state: Arc<AdminState>) -> Router {
    Router::new()
        .route("/", post(create_queue))
        .route("/", get(list_queues))
        .route("/:name", get(get_queue))
        .route("/:name", delete(delete_queue))
        .route("/:name/messages", post(publish_message))
        .route("/:name/messages/poll", post(poll_messages))
        .route("/:name/messages/:id/ack", post(ack_message))
        .route("/:name/purge", post(purge_queue))
        .route("/:name/stats", get(queue_stats))
        .with_state(state)
}

#[derive(Deserialize)]
struct CreateQueueRequest {
    name: String,
    durable: Option<bool>,
    max_length: Option<usize>,
    max_message_size: Option<usize>,
}

async fn create_queue(
    State(state): State<Arc<AdminState>>,
    Json(req): Json<CreateQueueRequest>,
) -> Result<Json<Value>, ApiError> {
    let mgr = state.queue_mgr.as_ref()
        .ok_or_else(|| ApiError::internal("Queue not available"))?;
    mgr.create_queue(&req.name).await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    Ok(Json(json!({
        "id": format!("q_{}", &req.name),
        "name": req.name,
        "status": "created",
    })))
}

async fn list_queues(
    State(state): State<Arc<AdminState>>,
) -> Result<Json<Value>, ApiError> {
    let mgr = state.queue_mgr.as_ref()
        .ok_or_else(|| ApiError::internal("Queue not available"))?;
    let queues = mgr.list_queues().await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let data: Vec<Value> = queues.into_iter().map(|q| json!({
        "name": q.name,
        "queue_type": q.queue_type,
        "available": q.available,
        "in_flight": q.in_flight,
        "delayed": q.delayed,
        "total": q.total,
        "paused": q.paused,
    })).collect();
    Ok(Json(json!({"data": data, "pagination": {"cursor": null, "limit": 100, "has_more": false}})))
}

async fn get_queue(
    State(state): State<Arc<AdminState>>,
    Path(name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let mgr = state.queue_mgr.as_ref()
        .ok_or_else(|| ApiError::internal("Queue not available"))?;
    let backend = mgr.backend();
    let cfg = backend.get_queue(&name).await
        .map_err(|e| ApiError::not_found(e.to_string()))?;
    Ok(Json(json!({
        "name": cfg.name,
        "queue_type": cfg.queue_type,
        "max_size": cfg.max_size,
        "paused": cfg.paused,
    })))
}

async fn delete_queue(
    State(state): State<Arc<AdminState>>,
    Path(name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let mgr = state.queue_mgr.as_ref()
        .ok_or_else(|| ApiError::internal("Queue not available"))?;
    mgr.delete_queue(&name).await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    Ok(Json(json!({"status": "deleted"})))
}

#[derive(Deserialize)]
struct PublishRequest {
    messages: Vec<MessageBody>,
}

#[derive(Deserialize)]
struct MessageBody {
    body: Value,
    delay_ms: Option<u64>,
}

async fn publish_message(
    State(state): State<Arc<AdminState>>,
    Path(name): Path<String>,
    Json(req): Json<PublishRequest>,
) -> Result<Json<Value>, ApiError> {
    let mgr = state.queue_mgr.as_ref()
        .ok_or_else(|| ApiError::internal("Queue not available"))?;
    let mut ids = Vec::new();
    for msg in &req.messages {
        let data = serde_json::to_vec(&msg.body)
            .map_err(|e| ApiError::bad_request(e.to_string()))?;
        mgr.enqueue(&name, data).await
            .map_err(|e| ApiError::internal(e.to_string()))?;
        ids.push(format!("msg_{}", uuid::Uuid::new_v4()));
    }
    Ok(Json(json!({
        "published_count": req.messages.len(),
        "message_ids": ids,
    })))
}

#[derive(Deserialize)]
struct PollRequest {
    count: Option<u32>,
    visibility_timeout_ms: Option<u64>,
}

async fn poll_messages(
    State(state): State<Arc<AdminState>>,
    Path(name): Path<String>,
    Json(req): Json<PollRequest>,
) -> Result<Json<Value>, ApiError> {
    let mgr = state.queue_mgr.as_ref()
        .ok_or_else(|| ApiError::internal("Queue not available"))?;
    let count = req.count.unwrap_or(10);
    let messages = mgr.dequeue(&name, count).await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let data: Vec<Value> = messages.into_iter().map(|m| {
        let body: Value = serde_json::from_slice(&m.body).unwrap_or(Value::Null);
        json!({
            "id": m.id.to_string(),
            "body": body,
            "receipt_handle": m.receipt_handle,
            "delivery_attempt": m.attempt_count,
        })
    }).collect();
    Ok(Json(json!({
        "messages": data,
        "message_count": data.len(),
    })))
}

async fn ack_message(
    State(state): State<Arc<AdminState>>,
    Path((name, id)): Path<(String, String)>,
) -> Result<Json<Value>, ApiError> {
    let mgr = state.queue_mgr.as_ref()
        .ok_or_else(|| ApiError::internal("Queue not available"))?;
    mgr.ack(&name, &id).await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    Ok(Json(json!({"status": "acknowledged"})))
}

async fn purge_queue(
    State(state): State<Arc<AdminState>>,
    Path(name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let mgr = state.queue_mgr.as_ref()
        .ok_or_else(|| ApiError::internal("Queue not available"))?;
    let backend = mgr.backend();
    backend.purge(&name).await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({"status": "purged"})))
}

async fn queue_stats(
    State(state): State<Arc<AdminState>>,
    Path(name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let mgr = state.queue_mgr.as_ref()
        .ok_or_else(|| ApiError::internal("Queue not available"))?;
    let stats = mgr.stats(&name).await
        .map_err(|e| ApiError::not_found(e.to_string()))?;
    Ok(Json(json!({
        "available_messages": stats.available_messages,
        "in_flight_messages": stats.in_flight_messages,
        "delayed_messages": stats.delayed_messages,
        "total_messages": stats.total_messages,
        "dlq_messages": stats.dlq_messages,
        "messages_enqueued": stats.messages_enqueued,
        "messages_dequeued": stats.messages_dequeued,
    })))
}
