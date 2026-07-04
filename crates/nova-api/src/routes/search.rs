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
        .route("/indexes", post(create_index))
        .route("/indexes", get(list_indexes))
        .route("/indexes/{name}", get(get_index))
        .route("/indexes/{name}", delete(delete_index))
        .route("/indexes/{name}/documents", post(index_documents))
        .route("/indexes/{name}/query", post(search_query))
        .route("/indexes/{name}/stats", get(index_stats))
        .with_state(state)
}

#[derive(Deserialize)]
struct CreateIndexRequest {
    name: String,
    fields: Option<Vec<IndexFieldDef>>,
}

#[derive(Deserialize)]
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
) -> Result<Json<Value>, ApiError> {
    let _mgr = state.search_mgr.as_ref()
        .ok_or_else(|| ApiError::internal("Search not available"))?;
    Ok(Json(json!({
        "id": format!("idx_{}", &req.name),
        "name": req.name,
        "status": "created",
    })))
}

async fn list_indexes(
    State(state): State<Arc<AdminState>>,
) -> Result<Json<Value>, ApiError> {
    let _mgr = state.search_mgr.as_ref()
        .ok_or_else(|| ApiError::internal("Search not available"))?;
    Ok(Json(json!({
        "data": [],
        "pagination": {"cursor": null, "limit": 50, "has_more": false}
    })))
}

async fn get_index(
    State(state): State<Arc<AdminState>>,
    Path(name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let mgr = state.search_mgr.as_ref()
        .ok_or_else(|| ApiError::internal("Search not available"))?;
    let name = name;
    let stats = mgr.stats();
    Ok(Json(json!({
        "name": name,
        "num_docs": stats.num_docs,
        "num_terms": stats.num_terms,
        "field_count": stats.field_count,
    })))
}

async fn delete_index(
    State(state): State<Arc<AdminState>>,
    Path(_name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let _mgr = state.search_mgr.as_ref()
        .ok_or_else(|| ApiError::internal("Search not available"))?;
    Ok(Json(json!({"status": "deleted"})))
}

#[derive(Deserialize)]
struct IndexDocumentsRequest {
    documents: Vec<Value>,
}

async fn index_documents(
    State(state): State<Arc<AdminState>>,
    Path(_name): Path<String>,
    Json(req): Json<IndexDocumentsRequest>,
) -> Result<Json<Value>, ApiError> {
    let mgr = state.search_mgr.as_ref()
        .ok_or_else(|| ApiError::internal("Search not available"))?;
    for doc_val in &req.documents {
        let doc_id = doc_val.get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("doc");
        let doc = nova_search::IndexedDocument::new(doc_id);
        if let Some(obj) = doc_val.as_object() {
            for (field_name, field_val) in obj {
                if field_name == "id" { continue; }
                let doc = match field_val {
                    Value::String(s) => doc.clone().add_text(field_name, s.clone()),
                    Value::Number(n) => {
                        if let Some(f) = n.as_f64() {
                            doc.clone().add_float(field_name, f)
                        } else if let Some(i) = n.as_i64() {
                            doc.clone().add_integer(field_name, i)
                        } else { continue; }
                    }
                    Value::Bool(b) => doc.clone().add_text(field_name, b.to_string()),
                    _ => continue,
                };
                if let Err(e) = mgr.index_document(doc) {
                    return Err(ApiError::internal(e.to_string()));
                }
            }
        }
    }
    Ok(Json(json!({
        "status": "indexed",
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
    Path(_name): Path<String>,
    Json(req): Json<SearchQueryRequest>,
) -> Result<Json<Value>, ApiError> {
    let mgr = state.search_mgr.as_ref()
        .ok_or_else(|| ApiError::internal("Search not available"))?;
    let limit = req.limit.unwrap_or(10);
    let result = mgr.search(&req.query, limit)
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    let hits: Vec<Value> = result.into_iter().map(|h| {
        let source = h.document.as_ref().map(|d| d.stored_fields());
        json!({
            "id": h.doc_id.to_string(),
            "score": h.score,
            "source": source,
        })
    }).collect();
    Ok(Json(json!({
        "hits": hits,
        "total_hits": hits.len(),
        "execution_time_ms": 0,
    })))
}

async fn index_stats(
    State(state): State<Arc<AdminState>>,
    Path(_name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let mgr = state.search_mgr.as_ref()
        .ok_or_else(|| ApiError::internal("Search not available"))?;
    let stats = mgr.stats();
    Ok(Json(json!({
        "num_docs": stats.num_docs,
        "num_terms": stats.num_terms,
        "field_count": stats.field_count,
    })))
}
