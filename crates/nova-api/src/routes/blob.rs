use crate::admin::AdminState;
use crate::error::ApiError;
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::response::{Json, Response};
use axum::http::header;
use axum::{routing::{get, post, delete}, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

pub fn routes(state: Arc<AdminState>) -> Router {
    Router::new()
        .route("/", post(upload_blob))
        .route("/", get(list_blobs))
        .route("/{id}", get(download_blob))
        .route("/{id}", delete(delete_blob))
        .route("/{id}/info", get(blob_info))
        .route("/stats", get(blob_stats))
        .with_state(state)
}

async fn upload_blob(
    State(state): State<Arc<AdminState>>,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    let mgr = state.blob_mgr.as_ref()
        .ok_or_else(|| ApiError::internal("Blob storage not available"))?;
    let meta = mgr.create_blob("default", &body, "application/octet-stream", HashMap::new()).await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({
        "id": meta.id,
        "size_bytes": meta.size,
        "content_type": meta.content_type,
        "checksum_sha256": hex_encode(meta.sha256.as_bytes()),
        "created_at": meta.created_at,
    })))
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

async fn download_blob(
    State(state): State<Arc<AdminState>>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let mgr = state.blob_mgr.as_ref()
        .ok_or_else(|| ApiError::internal("Blob storage not available"))?;
    let data = mgr.get_blob(&id).await
        .map_err(|e| ApiError::not_found(e.to_string()))?;
    let meta = mgr.get_metadata(&id).await.ok();
    let body = axum::body::Body::from(data);
    let mut response = Response::new(body);
    if let Some(m) = meta {
        response.headers_mut().insert("X-Blob-Size", m.size.to_string().parse().unwrap());
        response.headers_mut().insert("X-Blob-Checksum-SHA256", hex_encode(m.sha256.as_bytes()).parse().unwrap());
        response.headers_mut().insert(header::CONTENT_TYPE, m.content_type.parse().unwrap());
    }
    Ok(response)
}

async fn delete_blob(
    State(state): State<Arc<AdminState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let mgr = state.blob_mgr.as_ref()
        .ok_or_else(|| ApiError::internal("Blob storage not available"))?;
    mgr.delete_blob(&id).await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    Ok(Json(json!({"status": "deleted"})))
}

async fn blob_info(
    State(state): State<Arc<AdminState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let mgr = state.blob_mgr.as_ref()
        .ok_or_else(|| ApiError::internal("Blob storage not available"))?;
    let meta = mgr.get_metadata(&id).await
        .map_err(|e| ApiError::not_found(e.to_string()))?;
    Ok(Json(json!({
        "id": meta.id,
        "size_bytes": meta.size,
        "content_type": meta.content_type,
        "checksum_sha256": hex_encode(meta.sha256.as_bytes()),
        "created_at": meta.created_at,
        "metadata": meta.metadata,
    })))
}

#[derive(Deserialize)]
struct ListBlobsParams {
    prefix: Option<String>,
    limit: Option<usize>,
}

async fn list_blobs(
    State(state): State<Arc<AdminState>>,
    Query(params): Query<ListBlobsParams>,
) -> Result<Json<Value>, ApiError> {
    let mgr = state.blob_mgr.as_ref()
        .ok_or_else(|| ApiError::internal("Blob storage not available"))?;
    let blob_ids = mgr.list_blobs("default").await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let data: Vec<Value> = blob_ids.into_iter().map(|id| {
        json!({"id": id, "filename": &id, "size_bytes": 0, "content_type": "application/octet-stream"})
    }).collect();
    Ok(Json(json!({
        "data": data,
        "pagination": {"cursor": null, "limit": params.limit.unwrap_or(50), "has_more": false}
    })))
}

async fn blob_stats(
    State(state): State<Arc<AdminState>>,
) -> Result<Json<Value>, ApiError> {
    let mgr = state.blob_mgr.as_ref()
        .ok_or_else(|| ApiError::internal("Blob storage not available"))?;
    let stats = mgr.stats();
    Ok(Json(json!({
        "total_blobs": stats.total_blobs,
        "total_bytes": stats.total_bytes,
        "total_chunks": stats.total_chunks,
        "unique_chunks": stats.unique_chunks,
        "active_uploads": stats.active_uploads,
        "namespaces": stats.namespaces,
    })))
}
