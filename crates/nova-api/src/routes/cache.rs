use crate::admin::AdminState;
use crate::error::ApiError;
use crate::routes::http::pagination_links;
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
use std::time::Duration;

pub fn routes(state: Arc<AdminState>) -> Router {
    Router::new()
        .route("/:key", get(cache_get))
        .route("/:key", post(cache_set))
        .route("/:key", delete(cache_delete))
        .route("/batch", post(cache_batch_set))
        .route("/keys", get(cache_list_keys))
        .route("/stats", get(cache_stats))
        .with_state(state)
}

async fn cache_get(
    State(state): State<Arc<AdminState>>,
    Path(key): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let mgr = state
        .cache_mgr
        .as_ref()
        .ok_or_else(|| ApiError::internal("Cache not available"))?;
    let value = mgr
        .get(&key)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    match value {
        Some(data) => {
            let parsed: Value = serde_json::from_slice(&data).unwrap_or_else(|_| {
                json!(base64::Engine::encode(
                    &base64::engine::general_purpose::STANDARD,
                    &data
                ))
            });
            Ok(Json(json!({
                "key": key,
                "value": parsed,
                "ttl_remaining_ms": null,
            })))
        }
        None => Err(ApiError::not_found(format!("Key not found: {}", key))),
    }
}

#[derive(Deserialize)]
struct CacheSetRequest {
    value: Value,
    ttl_ms: Option<u64>,
}

async fn cache_set(
    State(state): State<Arc<AdminState>>,
    Path(key): Path<String>,
    Json(req): Json<CacheSetRequest>,
) -> Result<Json<Value>, ApiError> {
    let mgr = state
        .cache_mgr
        .as_ref()
        .ok_or_else(|| ApiError::internal("Cache not available"))?;
    let data = serde_json::to_vec(&req.value).map_err(|e| ApiError::bad_request(e.to_string()))?;
    let ttl = req.ttl_ms.map(Duration::from_millis);
    mgr.set(key, data, ttl)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({"status": "set"})))
}

async fn cache_delete(
    State(state): State<Arc<AdminState>>,
    Path(key): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let mgr = state
        .cache_mgr
        .as_ref()
        .ok_or_else(|| ApiError::internal("Cache not available"))?;
    mgr.delete(&key)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({"status": "deleted"})))
}

#[derive(Deserialize)]
struct BatchSetItem {
    key: String,
    value: Value,
    ttl_ms: Option<u64>,
}

async fn cache_batch_set(
    State(state): State<Arc<AdminState>>,
    Json(items): Json<Vec<BatchSetItem>>,
) -> Result<Json<Value>, ApiError> {
    let mgr = state
        .cache_mgr
        .as_ref()
        .ok_or_else(|| ApiError::internal("Cache not available"))?;
    let count = items.len();
    let entries: Vec<(
        nova_cache::CacheKey,
        nova_cache::CacheValue,
        Option<Duration>,
    )> = items
        .into_iter()
        .map(|item| {
            let data = serde_json::to_vec(&item.value).unwrap_or_default();
            let key: nova_cache::CacheKey = item.key;
            let ttl = item.ttl_ms.map(Duration::from_millis);
            (key, data, ttl)
        })
        .collect();
    mgr.set_many(entries)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({"status": "set", "count": count})))
}

#[derive(Deserialize)]
struct ListKeysParams {
    pattern: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
}

async fn cache_list_keys(
    State(state): State<Arc<AdminState>>,
    Query(params): Query<ListKeysParams>,
) -> Result<(HeaderMap, Json<Value>), ApiError> {
    let mgr = state
        .cache_mgr
        .as_ref()
        .ok_or_else(|| ApiError::internal("Cache not available"))?;
    let mut keys = mgr
        .keys()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    // Honor pattern filtering (glob-like * and ?)
    if let Some(pat) = &params.pattern
        && !pat.is_empty()
        && pat != "*"
    {
        let regex_str = format!(
            "^{}$",
            regex::escape(pat).replace("\\*", ".*").replace("\\?", ".")
        );
        if let Ok(re) = regex::Regex::new(&regex_str) {
            keys.retain(|k| re.is_match(k));
        }
    }
    keys.sort();
    let total = keys.len();
    let offset = params.offset.unwrap_or(0);
    let limit = params.limit.unwrap_or(50).clamp(1, 1000);
    let has_more = offset + limit < total;
    let data: Vec<String> = keys.into_iter().skip(offset).take(limit).collect();
    let link = pagination_links(
        "/api/v1/cache/keys",
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
            "pagination": {"offset": offset, "limit": limit, "total": total, "has_more": has_more},
            "pattern": params.pattern,
        })),
    ))
}

async fn cache_stats(State(state): State<Arc<AdminState>>) -> Result<Json<Value>, ApiError> {
    let mgr = state
        .cache_mgr
        .as_ref()
        .ok_or_else(|| ApiError::internal("Cache not available"))?;
    let metrics = mgr.metrics();
    let count = mgr.len().await.unwrap_or(0);
    Ok(Json(json!({
        "keys": count,
        "hits": metrics.hits(),
        "misses": metrics.misses(),
        "hit_rate": metrics.hit_rate(),
        "memory_bytes": 0,
        "evictions": metrics.evictions(),
    })))
}
