use crate::admin::AdminState;
use crate::error::ApiError;
use crate::routes::http::{Created, created, pagination_links};
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::{Json, Response};
use axum::{
    Router,
    routing::{delete, get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;

pub fn routes(state: Arc<AdminState>) -> Router {
    Router::new()
        .route("/", post(upload_blob))
        .route("/", get(list_blobs))
        .route("/:id", get(download_blob))
        .route("/:id", delete(delete_blob))
        .route("/:id/info", get(blob_info))
        .route("/stats", get(blob_stats))
        .with_state(state)
}

#[derive(Deserialize)]
struct UploadQuery {
    namespace: Option<String>,
    bucket: Option<String>,
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn extract_multipart_file(content_type: &str, body: &[u8]) -> Option<(Vec<u8>, String)> {
    let boundary = content_type
        .split("boundary=")
        .nth(1)?
        .trim()
        .trim_matches('"')
        .trim()
        .to_string();
    if boundary.is_empty() {
        return None;
    }
    let boundary_bytes = format!("--{}", boundary).into_bytes();
    let mut cursor = 0usize;
    while cursor < body.len() {
        let start = find_subslice(&body[cursor..], &boundary_bytes)? + cursor;
        let mut part_start = start + boundary_bytes.len();
        // Skip optional \r\n after boundary
        if body.get(part_start..part_start + 2) == Some(b"\r\n") {
            part_start += 2;
        } else if body.get(part_start..part_start + 1) == Some(b"\n") {
            part_start += 1;
        } else if body.get(part_start..part_start + 2) == Some(b"--") {
            break; // final boundary
        }
        // Find next boundary to delimit this part
        let next = find_subslice(&body[part_start..], &boundary_bytes).map(|p| p + part_start);
        let part_end = next.unwrap_or(body.len());
        let part = &body[part_start..part_end];
        // Header/body split is \r\n\r\n
        let header_end = find_subslice(part, b"\r\n\r\n")?;
        let header = &part[..header_end];
        let header_str = String::from_utf8_lossy(header).to_ascii_lowercase();
        if !header_str.contains("name=\"file\"") && !header_str.contains("name='file'") {
            cursor = part_end;
            continue;
        }
        let mut ct = "application/octet-stream".to_string();
        for line in String::from_utf8_lossy(header).lines() {
            if line.to_ascii_lowercase().starts_with("content-type:") {
                let v = line
                    .split_once(':')
                    .map(|x| x.1)
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if !v.is_empty() {
                    ct = v;
                }
            }
        }
        let data_start = header_end + 4;
        let mut data_end = part.len();
        // Trim trailing \r\n that precedes boundary
        if data_end >= 2 && &part[data_end - 2..] == b"\r\n" {
            data_end -= 2;
        }
        if data_start <= data_end {
            return Some((part[data_start..data_end].to_vec(), ct));
        }
        cursor = part_end;
    }
    None
}

async fn upload_blob(
    State(state): State<Arc<AdminState>>,
    Query(q): Query<UploadQuery>,
    headers: axum::http::HeaderMap,
    body: Bytes,
) -> Result<Created, ApiError> {
    let mgr = state
        .blob_mgr
        .as_ref()
        .ok_or_else(|| ApiError::internal("Blob storage not available"))?;
    let namespace = q
        .namespace
        .or(q.bucket)
        .unwrap_or_else(|| "default".to_string());
    // Validate namespace early for nicer error
    if namespace.contains("..") || namespace.contains('/') || namespace.contains('\\') {
        return Err(ApiError::bad_request(format!(
            "invalid namespace: '{}'",
            namespace
        )));
    }
    let raw_ct = headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();

    let (data, content_type) = if raw_ct.starts_with("multipart/form-data") {
        if let Some((file_bytes, file_ct)) = extract_multipart_file(&raw_ct, &body) {
            (file_bytes, file_ct)
        } else {
            // Fallback: if parsing fails but body likely contains file, try to use raw body minus multipart framing
            // If we cannot parse, treat as error rather than storing corrupted data
            return Err(ApiError::bad_request(
                "failed to parse multipart file upload: expected field 'file'",
            ));
        }
    } else {
        (body.to_vec(), raw_ct)
    };

    if data.is_empty() {
        return Err(ApiError::bad_request("empty file upload"));
    }

    let meta = mgr
        .create_blob(&namespace, &data, &content_type, HashMap::new())
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let id = meta.id.clone();
    Ok(created(
        &format!("/api/v1/blobs/{id}"),
        json!({
            "id": meta.id,
            "size_bytes": meta.size,
            "content_type": meta.content_type,
            "checksum_sha256": hex_encode(meta.sha256.as_bytes()),
            "created_at": meta.created_at,
        }),
    ))
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

async fn download_blob(
    State(state): State<Arc<AdminState>>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let mgr = state
        .blob_mgr
        .as_ref()
        .ok_or_else(|| ApiError::internal("Blob storage not available"))?;
    let data = mgr
        .get_blob(&id)
        .await
        .map_err(|e| ApiError::not_found(e.to_string()))?;
    let meta = mgr.get_metadata(&id).await.ok();
    let body = axum::body::Body::from(data);
    let mut response = Response::new(body);
    if let Some(m) = meta {
        if let Ok(v) = m.size.to_string().parse() {
            response.headers_mut().insert("X-Blob-Size", v);
        }
        if let Ok(v) = hex_encode(m.sha256.as_bytes()).parse() {
            response.headers_mut().insert("X-Blob-Checksum-SHA256", v);
        }
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            m.content_type
                .parse()
                .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
        );
    }
    Ok(response)
}

async fn delete_blob(
    State(state): State<Arc<AdminState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let mgr = state
        .blob_mgr
        .as_ref()
        .ok_or_else(|| ApiError::internal("Blob storage not available"))?;
    mgr.delete_blob(&id)
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    Ok(Json(json!({"status": "deleted"})))
}

async fn blob_info(
    State(state): State<Arc<AdminState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let mgr = state
        .blob_mgr
        .as_ref()
        .ok_or_else(|| ApiError::internal("Blob storage not available"))?;
    let meta = mgr
        .get_metadata(&id)
        .await
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
    offset: Option<usize>,
    namespace: Option<String>,
}

async fn list_blobs(
    State(state): State<Arc<AdminState>>,
    Query(params): Query<ListBlobsParams>,
) -> Result<(HeaderMap, Json<Value>), ApiError> {
    let mgr = state
        .blob_mgr
        .as_ref()
        .ok_or_else(|| ApiError::internal("Blob storage not available"))?;
    let ns = params.namespace.as_deref().unwrap_or("default");
    let mut blob_ids = mgr
        .list_blobs(ns)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    // Honor prefix filter
    if let Some(prefix) = &params.prefix
        && !prefix.is_empty()
    {
        blob_ids.retain(|id| id.starts_with(prefix.as_str()));
    }
    let total = blob_ids.len();
    let offset = params.offset.unwrap_or(0);
    let limit = params.limit.unwrap_or(50).clamp(1, 1000);
    let has_more = offset + limit < total;
    blob_ids = blob_ids.into_iter().skip(offset).take(limit).collect();
    let mut data = Vec::new();
    for id in blob_ids {
        if let Ok(meta) = mgr.get_metadata(&id).await {
            data.push(json!({
                "id": meta.id,
                "filename": &meta.id,
                "size_bytes": meta.size,
                "content_type": meta.content_type,
                "created_at": meta.created_at,
            }));
        } else {
            data.push(json!({
                "id": id,
                "filename": &id,
                "size_bytes": 0,
                "content_type": "application/octet-stream",
            }));
        }
    }
    let link = pagination_links(
        "/api/v1/blobs",
        &[("limit", limit.to_string())],
        limit,
        offset,
        total,
    );
    let mut headers = HeaderMap::new();
    if let Ok(v) = HeaderValue::from_str(&link) {
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

async fn blob_stats(State(state): State<Arc<AdminState>>) -> Result<Json<Value>, ApiError> {
    let mgr = state
        .blob_mgr
        .as_ref()
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
