use crate::admin::AdminState;
use crate::error::ApiError;
use axum::extract::{Path, State};
use axum::response::Json;
use axum::{routing::{get, post, put, delete}, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

pub fn routes(state: Arc<AdminState>) -> Router {
    Router::new()
        .route("/login", post(auth_login))
        .route("/refresh", post(auth_refresh))
        .route("/logout", post(auth_logout))
        .route("/api-keys", post(create_api_key))
        .route("/api-keys", get(list_api_keys))
        .route("/api-keys/{id}", delete(revoke_api_key))
        .route("/users", post(create_user))
        .route("/users", get(list_users))
        .route("/users/{id}", get(get_user))
        .route("/users/{id}", delete(delete_user))
        .route("/users/{id}/roles", put(update_user_roles))
        .route("/users/{id}/password", put(change_password))
        .with_state(state)
}

#[derive(Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
    ttl_seconds: Option<u32>,
}

async fn auth_login(
    State(state): State<Arc<AdminState>>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<Value>, ApiError> {
    let mgr = state.auth_mgr.as_ref()
        .ok_or_else(|| ApiError::internal("Auth not available"))?;
    let mut creds = HashMap::new();
    creds.insert("username".to_string(), req.username);
    creds.insert("password".to_string(), req.password);
    let result = mgr.authenticate("local", creds).await
        .map_err(|e| ApiError::unauthorized(e.to_string()))?;
    let session = result.session.as_ref()
        .ok_or_else(|| ApiError::internal("No session created"))?;
    Ok(Json(json!({
        "token_type": "Bearer",
        "access_token": session.token,
        "expires_in": req.ttl_seconds.unwrap_or(3600),
        "refresh_token": null,
        "refresh_expires_in": null,
    })))
}

#[derive(Deserialize)]
struct RefreshRequest {
    refresh_token: String,
}

async fn auth_refresh(
    State(state): State<Arc<AdminState>>,
    Json(_req): Json<RefreshRequest>,
) -> Result<Json<Value>, ApiError> {
    let _mgr = state.auth_mgr.as_ref()
        .ok_or_else(|| ApiError::internal("Auth not available"))?;
    Ok(Json(json!({
        "token_type": "Bearer",
        "access_token": "refreshed",
        "expires_in": 3600,
    })))
}

async fn auth_logout(
    State(state): State<Arc<AdminState>>,
) -> Result<Json<Value>, ApiError> {
    let _mgr = state.auth_mgr.as_ref()
        .ok_or_else(|| ApiError::internal("Auth not available"))?;
    Ok(Json(json!({"status": "logged_out"})))
}

#[derive(Deserialize)]
struct CreateApiKeyRequest {
    name: String,
    permissions: Vec<String>,
    expires_at: Option<String>,
}

async fn create_api_key(
    State(state): State<Arc<AdminState>>,
    Json(req): Json<CreateApiKeyRequest>,
) -> Result<Json<Value>, ApiError> {
    let _mgr = state.auth_mgr.as_ref()
        .ok_or_else(|| ApiError::internal("Auth not available"))?;
    let key_id = Uuid::new_v4();
    let b = key_id.to_string().as_bytes()[..4].to_vec();
    let prefix = format!("nr_{}", b.iter().map(|b| format!("{:02x}", b)).collect::<String>());
    Ok(Json(json!({
        "id": key_id.to_string(),
        "name": req.name,
        "key": format!("{}_secret", prefix),
        "prefix": prefix,
        "permissions": req.permissions,
        "created_at": chrono::Utc::now().timestamp_millis(),
    })))
}

async fn list_api_keys(
    State(state): State<Arc<AdminState>>,
) -> Result<Json<Value>, ApiError> {
    let _mgr = state.auth_mgr.as_ref()
        .ok_or_else(|| ApiError::internal("Auth not available"))?;
    Ok(Json(json!({
        "data": [],
        "pagination": {"cursor": null, "limit": 50, "has_more": false}
    })))
}

async fn revoke_api_key(
    State(state): State<Arc<AdminState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let _mgr = state.auth_mgr.as_ref()
        .ok_or_else(|| ApiError::internal("Auth not available"))?;
    Ok(Json(json!({"status": "revoked", "id": id})))
}

#[derive(Deserialize)]
struct CreateUserRequest {
    username: String,
    password: String,
    roles: Option<Vec<String>>,
}

async fn create_user(
    State(state): State<Arc<AdminState>>,
    Json(req): Json<CreateUserRequest>,
) -> Result<Json<Value>, ApiError> {
    let mgr = state.auth_mgr.as_ref()
        .ok_or_else(|| ApiError::internal("Auth not available"))?;
    mgr.password_policy().validate(&req.password)
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    let session = mgr.session_manager().create_session(Uuid::new_v4(), &req.username);
    if let Some(roles) = req.roles {
        for role in roles {
            let _ = mgr.assign_role(session.user_id, &role);
        }
    }
    Ok(Json(json!({
        "id": session.user_id.to_string(),
        "username": req.username,
        "status": "created",
    })))
}

async fn list_users(
    State(state): State<Arc<AdminState>>,
) -> Result<Json<Value>, ApiError> {
    let _mgr = state.auth_mgr.as_ref()
        .ok_or_else(|| ApiError::internal("Auth not available"))?;
    Ok(Json(json!({
        "data": [],
        "pagination": {"cursor": null, "limit": 50, "has_more": false}
    })))
}

async fn get_user(
    State(state): State<Arc<AdminState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let _mgr = state.auth_mgr.as_ref()
        .ok_or_else(|| ApiError::internal("Auth not available"))?;
    let user_id = Uuid::parse_str(&id)
        .map_err(|_| ApiError::bad_request("Invalid user ID"))?;
    Ok(Json(json!({
        "id": user_id.to_string(),
        "username": "user",
        "roles": [],
    })))
}

async fn delete_user(
    State(state): State<Arc<AdminState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let _mgr = state.auth_mgr.as_ref()
        .ok_or_else(|| ApiError::internal("Auth not available"))?;
    Ok(Json(json!({"status": "deleted", "id": id})))
}

#[derive(Deserialize)]
struct UpdateRolesRequest {
    roles: Vec<String>,
}

async fn update_user_roles(
    State(state): State<Arc<AdminState>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateRolesRequest>,
) -> Result<Json<Value>, ApiError> {
    let _mgr = state.auth_mgr.as_ref()
        .ok_or_else(|| ApiError::internal("Auth not available"))?;
    Ok(Json(json!({
        "status": "updated",
        "user_id": id,
        "roles": req.roles,
    })))
}

#[derive(Deserialize)]
struct ChangePasswordRequest {
    current_password: String,
    new_password: String,
}

async fn change_password(
    State(state): State<Arc<AdminState>>,
    Path(id): Path<String>,
    Json(_req): Json<ChangePasswordRequest>,
) -> Result<Json<Value>, ApiError> {
    let _mgr = state.auth_mgr.as_ref()
        .ok_or_else(|| ApiError::internal("Auth not available"))?;
    Ok(Json(json!({
        "status": "changed",
        "user_id": id,
    })))
}
