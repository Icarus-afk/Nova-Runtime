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
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

pub fn routes(state: Arc<AdminState>) -> Router {
    Router::new()
        .route("/indexes", post(create_index))
        .route("/indexes", get(list_indexes))
        .route("/indexes/:name", get(get_index))
        .route("/indexes/:name", delete(delete_index))
        .route("/indexes/:name/documents", post(index_documents))
        .route("/indexes/:name/query", post(search_query))
        .route("/indexes/:name/stats", get(index_stats))
        .with_state(state)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IndexMeta {
    name: String,
    fields: Vec<IndexFieldDef>,
    created_at: i64,
}

static REGISTRY: OnceLock<RwLock<HashMap<String, IndexMeta>>> = OnceLock::new();
fn registry() -> &'static RwLock<HashMap<String, IndexMeta>> {
    REGISTRY.get_or_init(|| RwLock::new(HashMap::new()))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CreateIndexRequest {
    name: String,
    fields: Option<Vec<IndexFieldDef>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IndexFieldDef {
    name: String,
    #[serde(rename = "type")]
    field_type: String,
    analyzer: Option<String>,
    boost: Option<f64>,
}

async fn create_index(
    State(state): State<Arc<AdminState>>,
    Json(req): Json<CreateIndexRequest>,
) -> Result<Created, ApiError> {
    let _mgr = state
        .search_mgr
        .as_ref()
        .ok_or_else(|| ApiError::internal("Search not available"))?;
    if req.name.trim().is_empty() {
        return Err(ApiError::bad_request("index name required"));
    }
    if req.name.contains("..") || req.name.contains('/') {
        return Err(ApiError::bad_request("invalid index name"));
    }
    let fields = req.fields.clone().unwrap_or_default();
    // Validate fields
    for f in &fields {
        if f.name.trim().is_empty() {
            return Err(ApiError::bad_request("field name required"));
        }
        let allowed = ["text", "keyword", "integer", "float", "boolean"];
        if !allowed.contains(&f.field_type.as_str()) {
            return Err(ApiError::bad_request(format!(
                "unsupported field type: {}",
                f.field_type
            )));
        }
    }
    let meta = IndexMeta {
        name: req.name.clone(),
        fields: fields.clone(),
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    {
        let mut reg = registry().write();
        if reg.contains_key(&req.name) {
            return Err(ApiError::bad_request(format!(
                "index {} already exists",
                req.name
            )));
        }
        reg.insert(req.name.clone(), meta);
    }
    tracing::info!(
        "search index {} created with {} fields",
        req.name,
        fields.len()
    );
    let name = req.name.clone();
    Ok(created(
        &format!("/api/v1/search/indexes/{name}"),
        json!({
            "id": format!("idx_{name}"),
            "name": req.name,
            "fields": fields,
            "status": "created",
        }),
    ))
}

#[derive(Deserialize)]
struct ListIndexesParams {
    limit: Option<usize>,
    offset: Option<usize>,
}

async fn list_indexes(
    State(state): State<Arc<AdminState>>,
    Query(params): Query<ListIndexesParams>,
) -> Result<(HeaderMap, Json<Value>), ApiError> {
    let _mgr = state
        .search_mgr
        .as_ref()
        .ok_or_else(|| ApiError::internal("Search not available"))?;
    let reg = registry().read();
    let mut names: Vec<String> = reg.keys().cloned().collect();
    names.sort();
    let total = names.len();
    let offset = params.offset.unwrap_or(0);
    let limit = params.limit.unwrap_or(50).clamp(1, 1000);
    let has_more = offset + limit < total;
    let data: Vec<Value> = names
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(|n| {
            let stats = _mgr.stats();
            let m = &reg[&n];
            json!({
                "name": m.name,
                "fields": m.fields,
                "doc_count": stats.num_docs,
                "field_count": m.fields.len(),
                "created_at": m.created_at,
            })
        })
        .collect();
    let link = pagination_links(
        "/api/v1/search/indexes",
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

async fn get_index(
    State(state): State<Arc<AdminState>>,
    Path(name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let mgr = state
        .search_mgr
        .as_ref()
        .ok_or_else(|| ApiError::internal("Search not available"))?;
    let reg = registry().read();
    let meta = reg.get(&name).cloned();
    let stats = mgr.stats();
    if let Some(m) = meta {
        Ok(Json(json!({
            "name": name,
            "fields": m.fields,
            "num_docs": stats.num_docs,
            "num_terms": stats.num_terms,
            "field_count": m.fields.len(),
            "created_at": m.created_at,
        })))
    } else {
        Ok(Json(json!({
            "name": name,
            "num_docs": stats.num_docs,
            "num_terms": stats.num_terms,
            "field_count": stats.field_count,
        })))
    }
}

async fn delete_index(
    State(state): State<Arc<AdminState>>,
    Path(name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let _mgr = state
        .search_mgr
        .as_ref()
        .ok_or_else(|| ApiError::internal("Search not available"))?;
    let mut reg = registry().write();
    if reg.remove(&name).is_none() {
        return Err(ApiError::not_found(format!("index {name} not found")));
    }
    Ok(Json(json!({"status": "deleted", "name": name})))
}

#[derive(Deserialize)]
struct IndexDocumentsRequest {
    documents: Vec<Value>,
}

async fn index_documents(
    State(state): State<Arc<AdminState>>,
    Path(name): Path<String>,
    Json(req): Json<IndexDocumentsRequest>,
) -> Result<Json<Value>, ApiError> {
    let mgr = state
        .search_mgr
        .as_ref()
        .ok_or_else(|| ApiError::internal("Search not available"))?;
    if !registry().read().contains_key(&name) {
        return Err(ApiError::not_found(format!(
            "index {name} not found — create it first"
        )));
    }
    for doc_val in &req.documents {
        let doc_id = doc_val
            .get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let mut doc = nova_search::IndexedDocument::new(&doc_id);
        if let Some(obj) = doc_val.as_object() {
            for (field_name, field_val) in obj {
                if field_name == "id" {
                    continue;
                }
                match field_val {
                    Value::String(s) => doc = doc.add_text(field_name, s.clone()),
                    Value::Number(n) => {
                        if let Some(i) = n.as_i64() {
                            doc = doc.add_integer(field_name, i);
                        } else if let Some(f) = n.as_f64() {
                            doc = doc.add_float(field_name, f);
                        }
                    }
                    Value::Bool(b) => doc = doc.add_text(field_name, b.to_string()),
                    Value::Array(arr) => {
                        // array of strings/numbers — join as text
                        let txt = arr
                            .iter()
                            .map(|v| v.to_string())
                            .collect::<Vec<_>>()
                            .join(" ");
                        doc = doc.add_text(field_name, txt);
                    }
                    _ => continue,
                }
            }
        }
        mgr.index_document(doc)
            .map_err(|e| ApiError::internal(e.to_string()))?;
    }
    Ok(Json(json!({
        "status": "indexed",
        "index": name,
        "count": req.documents.len(),
    })))
}

#[derive(Deserialize)]
struct SearchQueryRequest {
    query: String,
    limit: Option<usize>,
    offset: Option<usize>,
}

async fn search_query(
    State(state): State<Arc<AdminState>>,
    Path(name): Path<String>,
    Json(req): Json<SearchQueryRequest>,
) -> Result<(HeaderMap, Json<Value>), ApiError> {
    let mgr = state
        .search_mgr
        .as_ref()
        .ok_or_else(|| ApiError::internal("Search not available"))?;
    if !registry().read().contains_key(&name) {
        return Err(ApiError::not_found(format!("index {name} not found")));
    }
    if req.query.trim().is_empty() {
        return Err(ApiError::bad_request("query must not be empty"));
    }
    let limit = req.limit.unwrap_or(10).clamp(1, 100);
    let offset = req.offset.unwrap_or(0);
    // Use pagination-aware search when offset is requested
    let (hits_raw, total_hits) = if offset > 0 {
        let resp = mgr
            .search_with_pagination(&req.query, limit + offset, None)
            .map_err(|e| ApiError::bad_request(e.to_string()))?;
        let total = resp.total_hits as usize;
        let paged = resp
            .hits
            .into_iter()
            .skip(offset)
            .take(limit)
            .collect::<Vec<_>>();
        (paged, total)
    } else {
        let r = mgr
            .search(&req.query, limit)
            .map_err(|e| ApiError::bad_request(e.to_string()))?;
        let len = r.len();
        (r, len)
    };
    let hits: Vec<Value> = hits_raw
        .into_iter()
        .map(|h| {
            let source = h.document.as_ref().map(|d| d.stored_fields());
            json!({
                "id": h.doc_id.to_string(),
                "score": h.score,
                "source": source,
            })
        })
        .collect();
    let link = pagination_links(
        &format!("/api/v1/search/indexes/{name}/query"),
        &[("limit", limit.to_string())],
        limit,
        offset,
        total_hits,
    );
    let mut headers = HeaderMap::new();
    if let Ok(v) = axum::http::HeaderValue::from_str(&link) {
        headers.insert("link", v);
    }
    Ok((
        headers,
        Json(json!({
            "index": name,
            "hits": hits,
            "total_hits": total_hits,
            "offset": offset,
            "limit": limit,
            "execution_time_ms": 0,
        })),
    ))
}

async fn index_stats(
    State(state): State<Arc<AdminState>>,
    Path(_name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let mgr = state
        .search_mgr
        .as_ref()
        .ok_or_else(|| ApiError::internal("Search not available"))?;
    let stats = mgr.stats();
    Ok(Json(json!({
        "num_docs": stats.num_docs,
        "num_terms": stats.num_terms,
        "field_count": stats.field_count,
    })))
}
