use crate::admin::AdminState;
use crate::error::ApiError;
use axum::extract::{Path, State};
use axum::response::Json;
use axum::{routing::{get, post}, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

pub fn routes(state: Arc<AdminState>) -> Router {
    Router::new()
        .route("/query", post(sql_query))
        .route("/execute", post(sql_execute))
        .route("/tables", get(list_tables))
        .route("/tables/{table}/schema", get(get_table_schema))
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
    let engine = state.sql_engine.as_ref()
        .ok_or_else(|| ApiError::internal("SQL engine not available"))?;

    let result = engine.execute(&req.query)
        .map_err(|e| ApiError::bad_request(e.to_string()))?;

    match result {
        nova_sql::SQLResult::Query { batches, stats } => {
            let mut rows = Vec::new();
            let mut columns = Vec::new();
            for batch in &batches {
                for col in &batch.columns {
                    let col_name = match col {
                        nova_sql::Column::Integer(_) => "integer",
                        nova_sql::Column::Float(_) => "float",
                        nova_sql::Column::Boolean(_) => "boolean",
                        nova_sql::Column::String(_) => "text",
                        nova_sql::Column::Null(_) => "null",
                    };
                    if !columns.contains(&col_name.to_string()) {
                        columns.push(col_name.to_string());
                    }
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
                                row.push(v.get(i).as_ref().map(|x| json!(x)).unwrap_or(Value::Null));
                            }
                            nova_sql::Column::Null(_) => {
                                row.push(Value::Null);
                            }
                        }
                    }
                    rows.push(row);
                }
            }
            Ok(Json(json!({
                "columns": columns,
                "types": columns,
                "rows": rows,
                "row_count": rows.len(),
                "truncated": false,
                "execution_time_ms": stats.execution_time_ms,
            })))
        }
        nova_sql::SQLResult::Exec { .. } => {
            Ok(Json(json!({
                "columns": [],
                "types": [],
                "rows": [],
                "row_count": 0,
                "truncated": false,
                "execution_time_ms": 0,
            })))
        }
    }
}

async fn sql_execute(
    State(state): State<Arc<AdminState>>,
    Json(req): Json<ExecuteRequest>,
) -> Result<Json<Value>, ApiError> {
    let engine = state.sql_engine.as_ref()
        .ok_or_else(|| ApiError::internal("SQL engine not available"))?;

    let result = engine.execute(&req.query)
        .map_err(|e| ApiError::bad_request(e.to_string()))?;

    match result {
        nova_sql::SQLResult::Exec { rows_affected, stats } => {
            Ok(Json(json!({
                "affected_rows": rows_affected,
                "execution_time_ms": stats.execution_time_ms,
            })))
        }
        nova_sql::SQLResult::Query { .. } => {
            Ok(Json(json!({
                "affected_rows": 0,
                "execution_time_ms": 0,
            })))
        }
    }
}

async fn list_tables(
    State(state): State<Arc<AdminState>>,
) -> Result<Json<Value>, ApiError> {
    let _engine = state.sql_engine.as_ref()
        .ok_or_else(|| ApiError::internal("SQL engine not available"))?;
    Ok(Json(json!({
        "data": [],
        "pagination": {"cursor": null, "limit": 50, "has_more": false}
    })))
}

async fn get_table_schema(
    State(state): State<Arc<AdminState>>,
    Path(table): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let _engine = state.sql_engine.as_ref()
        .ok_or_else(|| ApiError::internal("SQL engine not available"))?;
    Ok(Json(json!({
        "table": table,
        "columns": [],
    })))
}
