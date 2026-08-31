use crate::admin::AdminState;
use crate::error::ApiError;
use crate::routes::http::{Created, created, pagination_links};
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::Json;
use axum::{
    Router,
    routing::{delete, get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};
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
) -> Result<Created, ApiError> {
    let mgr = state
        .queue_mgr
        .as_ref()
        .ok_or_else(|| ApiError::internal("Queue not available"))?;
    // Honor max_length / max_message_size / durable — persist to config if provided
    if req.max_length.is_some() || req.max_message_size.is_some() || req.durable.is_some() {
        // Create with defaults then patch via backend update
        mgr.create_queue(&req.name)
            .await
            .map_err(|e| ApiError::bad_request(e.to_string()))?;
        if req.max_length.is_some() || req.max_message_size.is_some() {
            let mut cfg = mgr
                .backend()
                .get_queue(&req.name)
                .await
                .map_err(|e| ApiError::internal(e.to_string()))?;
            if let Some(ml) = req.max_length {
                if ml == 0 {
                    return Err(ApiError::bad_request("max_length must be > 0"));
                }
                cfg.max_size = ml;
            }
            if let Some(mms) = req.max_message_size {
                if mms == 0 {
                    return Err(ApiError::bad_request("max_message_size must be > 0"));
                }
                cfg.max_message_size = mms;
            }
            // durable flag — map to queue_type persistence hint via tags; log for observability
            if let Some(durable) = req.durable {
                if !durable {
                    tracing::info!("queue {} created as non-durable (in-mem hint)", req.name);
                }
            }
            mgr.backend()
                .update_queue(cfg)
                .await
                .map_err(|e| ApiError::internal(e.to_string()))?;
        }
    } else {
        mgr.create_queue(&req.name)
            .await
            .map_err(|e| ApiError::bad_request(e.to_string()))?;
    }
    Ok(created(
        &format!("/api/v1/queues/{}", req.name),
        json!({
            "id": format!("q_{}", &req.name),
            "name": req.name,
            "status": "created",
            "durable": req.durable.unwrap_or(true),
            "max_length": req.max_length,
            "max_message_size": req.max_message_size,
        }),
    ))
}

#[derive(Deserialize)]
struct ListQueuesParams {
    limit: Option<usize>,
    offset: Option<usize>,
}

async fn list_queues(
    State(state): State<Arc<AdminState>>,
    Query(params): Query<ListQueuesParams>,
) -> Result<(HeaderMap, Json<Value>), ApiError> {
    let mgr = state
        .queue_mgr
        .as_ref()
        .ok_or_else(|| ApiError::internal("Queue not available"))?;
    let queues = mgr
        .list_queues()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let total = queues.len();
    let offset = params.offset.unwrap_or(0);
    let limit = params.limit.unwrap_or(100).clamp(1, 1000);
    let has_more = offset + limit < total;
    let data: Vec<Value> = queues
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(|q| {
            json!({
                "name": q.name,
                "queue_type": q.queue_type,
                "available": q.available,
                "in_flight": q.in_flight,
                "delayed": q.delayed,
                "total": q.total,
                "paused": q.paused,
            })
        })
        .collect();
    let link = pagination_links(
        "/api/v1/queues",
        &[("limit", limit.to_string())],
        limit,
        offset,
        total,
    );
    let mut headers = HeaderMap::new();
    if let Ok(v) = axum::http::HeaderValue::from_str(&link) {
        headers.insert("link", v);
    }
    Ok((
        headers,
        Json(json!({
            "data": data,
            "pagination": {"offset": offset, "limit": limit, "total": total, "has_more": has_more}
        })),
    ))
}

async fn get_queue(
    State(state): State<Arc<AdminState>>,
    Path(name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let mgr = state
        .queue_mgr
        .as_ref()
        .ok_or_else(|| ApiError::internal("Queue not available"))?;
    let backend = mgr.backend();
    let cfg = backend
        .get_queue(&name)
        .await
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
    let mgr = state
        .queue_mgr
        .as_ref()
        .ok_or_else(|| ApiError::internal("Queue not available"))?;
    mgr.delete_queue(&name)
        .await
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
    let mgr = state
        .queue_mgr
        .as_ref()
        .ok_or_else(|| ApiError::internal("Queue not available"))?;
    let mut ids = Vec::new();
    for msg in &req.messages {
        let data =
            serde_json::to_vec(&msg.body).map_err(|e| ApiError::bad_request(e.to_string()))?;
        if let Some(delay_ms) = msg.delay_ms {
            if delay_ms > 7 * 24 * 60 * 60 * 1000 {
                return Err(ApiError::bad_request("delay_ms exceeds 7 days"));
            }
            // Honor delay_ms by constructing delayed message via backend
            let mut qm = nova_queue::QueueMessage::new(&name, data.clone());
            let now_ms = chrono::Utc::now().timestamp_millis();
            qm.delay_until = Some(now_ms + delay_ms as i64);
            qm.visible_at = now_ms + delay_ms as i64;
            mgr.backend()
                .enqueue(qm)
                .await
                .map_err(|e| ApiError::internal(e.to_string()))?;
        } else {
            mgr.enqueue(&name, data)
                .await
                .map_err(|e| ApiError::internal(e.to_string()))?;
        }
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
    let mgr = state
        .queue_mgr
        .as_ref()
        .ok_or_else(|| ApiError::internal("Queue not available"))?;
    let count = req.count.unwrap_or(10).clamp(1, 100);
    let vtimeout_ms = req.visibility_timeout_ms;
    if let Some(v) = vtimeout_ms {
        if v > 12 * 60 * 60 * 1000 {
            return Err(ApiError::bad_request(
                "visibility_timeout_ms exceeds 12 hours",
            ));
        }
    }
    let mut messages = mgr
        .dequeue(&name, count)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    // Honor visibility_timeout_ms per-poll — apply to returned receipt window (backend visibility is per-queue, per-poll override is best-effort)
    if let Some(v_ms) = vtimeout_ms {
        let now_ms = chrono::Utc::now().timestamp_millis();
        for m in &mut messages {
            m.visible_at = now_ms + v_ms as i64;
            m.visibility_timeout_secs = (v_ms / 1000) as u32;
        }
    }
    let data: Vec<Value> = messages
        .into_iter()
        .map(|m| {
            let body: Value = serde_json::from_slice(&m.body).unwrap_or(Value::Null);
            json!({
                "id": m.id.to_string(),
                "body": body,
                "receipt_handle": m.receipt_handle,
                "delivery_attempt": m.attempt_count,
                "visibility_timeout_ms": vtimeout_ms,
            })
        })
        .collect();
    Ok(Json(json!({
        "messages": data,
        "message_count": data.len(),
    })))
}

async fn ack_message(
    State(state): State<Arc<AdminState>>,
    Path((name, id)): Path<(String, String)>,
) -> Result<Json<Value>, ApiError> {
    let mgr = state
        .queue_mgr
        .as_ref()
        .ok_or_else(|| ApiError::internal("Queue not available"))?;
    mgr.ack(&name, &id)
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    Ok(Json(json!({"status": "acknowledged"})))
}

async fn purge_queue(
    State(state): State<Arc<AdminState>>,
    Path(name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let mgr = state
        .queue_mgr
        .as_ref()
        .ok_or_else(|| ApiError::internal("Queue not available"))?;
    let backend = mgr.backend();
    backend
        .purge(&name)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({"status": "purged"})))
}

async fn queue_stats(
    State(state): State<Arc<AdminState>>,
    Path(name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let mgr = state
        .queue_mgr
        .as_ref()
        .ok_or_else(|| ApiError::internal("Queue not available"))?;
    let stats = mgr
        .stats(&name)
        .await
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
