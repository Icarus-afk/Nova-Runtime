use crate::admin::AdminState;
use crate::error::ApiError;
use crate::routes::http::pagination_links;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::Json;
use axum::{
    Router,
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

pub fn routes(state: Arc<AdminState>) -> Router {
    Router::new()
        .route("/query", post(sql_query))
        .route("/execute", post(sql_execute))
        .route("/tables", get(list_tables))
        .route("/tables/:table/schema", get(get_table_schema))
        .with_state(state)
}

#[derive(Deserialize)]
struct QueryRequest {
    query: String,
    params: Option<Vec<Value>>,
    limit: Option<usize>,
    format: Option<String>,
}

#[derive(Deserialize)]
struct ExecuteRequest {
    query: String,
    params: Option<Vec<Value>>,
}

async fn sql_query(
    State(state): State<Arc<AdminState>>,
    Json(req): Json<QueryRequest>,
) -> Result<Json<Value>, ApiError> {
    let engine = state
        .sql_engine
        .as_ref()
        .ok_or_else(|| ApiError::internal("SQL engine not available"))?;

    // Honor params: simple $1,$2 interpolation for prepared-like queries
    let mut query = req.query.clone();
    if let Some(params) = &req.params {
        for (i, v) in params.iter().enumerate() {
            let placeholder = format!("${}", i + 1);
            let replacement = match v {
                Value::String(s) => format!("'{}'", s.replace('\'', "''")),
                Value::Number(n) => n.to_string(),
                Value::Bool(b) => b.to_string().to_uppercase(),
                Value::Null => "NULL".to_string(),
                _ => serde_json::to_string(v).unwrap_or_else(|_| "NULL".to_string()),
            };
            query = query.replace(&placeholder, &replacement);
        }
        if query.contains('$') && query.chars().any(|c| c == '$') {
            tracing::warn!(
                "query still contains $ placeholder after params interpolation: {}",
                query
            );
        }
    }
    // Honor format validation
    if let Some(fmt) = &req.format {
        let allowed = ["json", "csv", "arrow"];
        if !allowed.contains(&fmt.as_str()) {
            return Err(ApiError::bad_request(format!(
                "unsupported format: {fmt}, allowed: json,csv,arrow"
            )));
        }
    }

    let result = engine
        .execute(&query)
        .map_err(|e| ApiError::bad_request(e.to_string()))?;

    match result {
        nova_sql::SQLResult::Query { batches, stats } => {
            let mut rows = Vec::new();
            let mut column_names = Vec::new();
            let mut types = Vec::new();
            for batch in &batches {
                column_names = batch.column_names.clone();
                for col in &batch.columns {
                    let col_name = match col {
                        nova_sql::Column::Integer(_) => "integer",
                        nova_sql::Column::Float(_) => "float",
                        nova_sql::Column::Boolean(_) => "boolean",
                        nova_sql::Column::String(_) => "text",
                        nova_sql::Column::Null(_) => "null",
                    };
                    types.push(col_name.to_string());
                }
                for i in 0..batch.num_rows {
                    let mut row = Vec::new();
                    for col in &batch.columns {
                        match col {
                            nova_sql::Column::Integer(v) => {
                                row.push(v.get(i).map(|x| json!(x)).unwrap_or(Value::Null));
                            }
                            nova_sql::Column::Float(v) => {
                                row.push(v.get(i).map(|x| json!(x)).unwrap_or(Value::Null));
                            }
                            nova_sql::Column::Boolean(v) => {
                                row.push(v.get(i).map(|x| json!(x)).unwrap_or(Value::Null));
                            }
                            nova_sql::Column::String(v) => {
                                row.push(
                                    v.get(i).as_ref().map(|x| json!(x)).unwrap_or(Value::Null),
                                );
                            }
                            nova_sql::Column::Null(_) => {
                                row.push(Value::Null);
                            }
                        }
                    }
                    rows.push(row);
                }
            }
            // Honor limit truncation
            let limit = req.limit.unwrap_or(usize::MAX);
            let truncated = rows.len() > limit;
            if truncated {
                rows.truncate(limit);
            }
            let is_truncated = truncated;
            Ok(Json(json!({
                "columns": column_names,
                "column_names": column_names,
                "types": types,
                "rows": rows,
                "row_count": rows.len(),
                "truncated": is_truncated,
                "execution_time_ms": stats.execution_time_ms,
                "format": req.format.clone().unwrap_or_else(|| "json".to_string()),
            })))
        }
        nova_sql::SQLResult::Exec { .. } => Ok(Json(json!({
            "columns": [],
            "types": [],
            "rows": [],
            "row_count": 0,
            "truncated": false,
            "execution_time_ms": 0,
        }))),
    }
}

async fn sql_execute(
    State(state): State<Arc<AdminState>>,
    Json(req): Json<ExecuteRequest>,
) -> Result<Json<Value>, ApiError> {
    let engine = state
        .sql_engine
        .as_ref()
        .ok_or_else(|| ApiError::internal("SQL engine not available"))?;

    let mut query = req.query.clone();
    if let Some(params) = &req.params {
        for (i, v) in params.iter().enumerate() {
            let placeholder = format!("${}", i + 1);
            let replacement = match v {
                Value::String(s) => format!("'{}'", s.replace('\'', "''")),
                Value::Number(n) => n.to_string(),
                Value::Bool(b) => b.to_string().to_uppercase(),
                Value::Null => "NULL".to_string(),
                _ => serde_json::to_string(v).unwrap_or_else(|_| "NULL".to_string()),
            };
            query = query.replace(&placeholder, &replacement);
        }
    }

    let result = engine
        .execute(&query)
        .map_err(|e| ApiError::bad_request(e.to_string()))?;

    match result {
        nova_sql::SQLResult::Exec {
            rows_affected,
            stats,
        } => Ok(Json(json!({
            "affected_rows": rows_affected,
            "execution_time_ms": stats.execution_time_ms,
        }))),
        nova_sql::SQLResult::Query { .. } => Ok(Json(json!({
            "affected_rows": 0,
            "execution_time_ms": 0,
        }))),
    }
}

#[derive(Deserialize)]
struct ListTablesParams {
    limit: Option<usize>,
    offset: Option<usize>,
}

async fn list_tables(
    State(state): State<Arc<AdminState>>,
    Query(params): Query<ListTablesParams>,
) -> Result<(HeaderMap, Json<Value>), ApiError> {
    let engine = state
        .sql_engine
        .as_ref()
        .ok_or_else(|| ApiError::internal("SQL engine not available"))?;
    let tables = engine.table_names();
    let total = tables.len();
    let offset = params.offset.unwrap_or(0);
    let limit = params.limit.unwrap_or(50).clamp(1, 1000);
    let has_more = offset + limit < total;
    let data: Vec<Value> = tables
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(|name| {
            let count = engine.num_rows(&name).unwrap_or(0);
            json!({ "name": name, "document_count": count })
        })
        .collect();
    let link = pagination_links(
        "/api/v1/sql/tables",
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

async fn get_table_schema(
    State(state): State<Arc<AdminState>>,
    Path(table): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let engine = state
        .sql_engine
        .as_ref()
        .ok_or_else(|| ApiError::internal("SQL engine not available"))?;
    let schema = engine
        .get_table_schema(&table)
        .map_err(|e| ApiError::not_found(e.to_string()))?;
    let columns: Vec<Value> = schema
        .columns
        .iter()
        .map(|c| {
            json!({
                "name": c.name,
                "type": format!("{:?}", c.sql_type),
                "nullable": c.nullable,
                "is_primary_key": c.is_primary_key,
                "unique": c.unique,
            })
        })
        .collect();
    Ok(Json(json!({
        "table": table,
        "columns": columns,
    })))
}
