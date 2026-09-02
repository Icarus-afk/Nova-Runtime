use crate::admin::AdminState;
use crate::error::ApiError;
use crate::routes::http::{Created, created};
use axum::extract::{Path, State};
use axum::response::Json;
use axum::{
    Router,
    routing::{delete, get, post, put},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use uuid::Uuid;

static LOGIN_ATTEMPTS: OnceLock<parking_lot::RwLock<HashMap<String, Vec<Instant>>>> =
    OnceLock::new();
fn login_attempts() -> &'static parking_lot::RwLock<HashMap<String, Vec<Instant>>> {
    LOGIN_ATTEMPTS.get_or_init(|| parking_lot::RwLock::new(HashMap::new()))
}

#[allow(clippy::result_large_err)]
fn check_login_rate_limit(ip: &str) -> Result<(), ApiError> {
    const WINDOW: Duration = Duration::from_secs(60);
    const MAX_ATTEMPTS: usize = 5;
    let now = Instant::now();
    let mut map = login_attempts().write();
    let entry = map.entry(ip.to_string()).or_default();
    entry.retain(|t| now.duration_since(*t) < WINDOW);
    if entry.len() >= MAX_ATTEMPTS {
        let oldest = entry.first().copied().unwrap_or(now);
        let retry_after = WINDOW
            .as_secs()
            .saturating_sub(now.duration_since(oldest).as_secs());
        return Err(ApiError::too_many_requests(format!(
            "too many login attempts, retry in {retry_after}s"
        ))
        .with_extra(json!({"retry_after_secs": retry_after}))
        .with_header("Retry-After", retry_after.to_string()));
    }
    entry.push(now);
    Ok(())
}

pub fn routes(state: Arc<AdminState>) -> Router {
    Router::new()
        .route("/login", post(auth_login))
        .route("/refresh", post(auth_refresh))
        .route("/logout", post(auth_logout))
        .route("/api-keys", post(create_api_key))
        .route("/api-keys", get(list_api_keys))
        .route("/api-keys/:id", delete(revoke_api_key))
        .route("/users", post(create_user))
        .route("/users", get(list_users))
        .route("/users/:id", get(get_user))
        .route("/users/:id", delete(delete_user))
        .route("/users/:id/roles", put(update_user_roles))
        .route("/users/:id/password", put(change_password))
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
    headers: axum::http::HeaderMap,
    Json(req): Json<LoginRequest>,
) -> Result<Json<Value>, ApiError> {
    // Per-IP rate limiting: 5/min
    let ip = headers
        .get("x-forwarded-for")
        .or_else(|| headers.get("x-real-ip"))
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    check_login_rate_limit(&ip)?;

    let mgr = state
        .auth_mgr
        .as_ref()
        .ok_or_else(|| ApiError::internal("Auth not available"))?;
    let mut creds = HashMap::new();
    creds.insert("username".to_string(), req.username);
    creds.insert("password".to_string(), req.password);
    let result = mgr
        .authenticate("local", creds)
        .await
        .map_err(|e| ApiError::unauthorized(e.to_string()))?;
    let session = result
        .session
        .as_ref()
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
    Json(req): Json<RefreshRequest>,
) -> Result<Json<Value>, ApiError> {
    let mgr = state
        .auth_mgr
        .as_ref()
        .ok_or_else(|| ApiError::internal("Auth not available"))?;
    let session = mgr
        .validate_session(&req.refresh_token)
        .map_err(|_| ApiError::unauthorized("Invalid or expired session"))?;
    Ok(Json(json!({
        "token_type": "Bearer",
        "access_token": session.token,
        "expires_in": 3600,
    })))
}

async fn auth_logout(
    State(state): State<Arc<AdminState>>,
    headers: axum::http::HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let mgr = state
        .auth_mgr
        .as_ref()
        .ok_or_else(|| ApiError::internal("Auth not available"))?;
    let token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| ApiError::unauthorized("Missing authorization header"))?;
    mgr.revoke_session(token)
        .map_err(|_| ApiError::unauthorized("Invalid session"))?;
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
) -> Result<Created, ApiError> {
    let mgr = state
        .auth_mgr
        .as_ref()
        .ok_or_else(|| ApiError::internal("Auth not available"))?;
    let (mut record, full_key) = mgr.create_api_key(&req.name, req.permissions);
    // Honor expires_at if provided (RFC3339 or millis)
    if let Some(exp) = req.expires_at {
        let parsed = if let Ok(ms) = exp.parse::<i64>() {
            Some(ms)
        } else if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&exp) {
            Some(dt.timestamp_millis())
        } else if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(&exp, "%Y-%m-%d %H:%M:%S") {
            Some(dt.and_utc().timestamp_millis())
        } else {
            return Err(ApiError::bad_request(format!(
                "invalid expires_at: {exp}, expected RFC3339 or millis"
            )));
        };
        if let Some(ms) = parsed {
            if ms <= chrono::Utc::now().timestamp_millis() {
                return Err(ApiError::bad_request("expires_at must be in the future"));
            }
            // Patch record in store
            record.expires_at = Some(ms);
            if let Some(mut entry) = mgr.api_keys().get_mut(&record.id.to_string()) {
                entry.expires_at = Some(ms);
            }
        }
    }
    Ok(created(
        &format!("/api/v1/auth/api-keys/{}", record.id),
        json!({
            "id": record.id.to_string(),
            "name": record.name,
            "key": full_key,
            "prefix": record.prefix,
            "permissions": record.permissions,
            "created_at": record.created_at,
            "expires_at": record.expires_at,
        }),
    ))
}

async fn list_api_keys(State(state): State<Arc<AdminState>>) -> Result<Json<Value>, ApiError> {
    let mgr = state
        .auth_mgr
        .as_ref()
        .ok_or_else(|| ApiError::internal("Auth not available"))?;
    let keys: Vec<Value> = mgr
        .list_api_keys()
        .into_iter()
        .map(|k| {
            json!({
                "id": k.id.to_string(),
                "name": k.name,
                "prefix": k.prefix,
                "permissions": k.permissions,
                "created_at": k.created_at,
                "expires_at": k.expires_at,
                "enabled": k.enabled,
            })
        })
        .collect();
    Ok(Json(json!({
        "data": keys,
        "pagination": {"cursor": null, "limit": 50, "has_more": false}
    })))
}

async fn revoke_api_key(
    State(state): State<Arc<AdminState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let mgr = state
        .auth_mgr
        .as_ref()
        .ok_or_else(|| ApiError::internal("Auth not available"))?;
    let key_id = Uuid::parse_str(&id).map_err(|_| ApiError::bad_request("Invalid API key ID"))?;
    if mgr.revoke_api_key(&key_id) {
        Ok(Json(json!({"status": "revoked", "id": id})))
    } else {
        Err(ApiError::not_found("API key not found"))
    }
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
) -> Result<Created, ApiError> {
    let mgr = state
        .auth_mgr
        .as_ref()
        .ok_or_else(|| ApiError::internal("Auth not available"))?;
    mgr.password_policy()
        .validate(&req.password)
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    let user = mgr
        .create_user(&req.username, &req.password, req.roles.unwrap_or_default())
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    let user_id = user.id.to_string();
    Ok(created(
        &format!("/api/v1/auth/users/{user_id}"),
        json!({
            "id": user_id,
            "username": user.username,
            "roles": user.roles,
            "status": "created",
        }),
    ))
}

async fn list_users(State(state): State<Arc<AdminState>>) -> Result<Json<Value>, ApiError> {
    let mgr = state
        .auth_mgr
        .as_ref()
        .ok_or_else(|| ApiError::internal("Auth not available"))?;
    let users: Vec<Value> = mgr
        .list_users()
        .into_iter()
        .map(|u| {
            json!({
                "id": u.id.to_string(),
                "username": u.username,
                "roles": u.roles,
                "created_at": u.created_at,
            })
        })
        .collect();
    Ok(Json(json!({
        "data": users,
        "pagination": {"cursor": null, "limit": 50, "has_more": false}
    })))
}

async fn get_user(
    State(state): State<Arc<AdminState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let mgr = state
        .auth_mgr
        .as_ref()
        .ok_or_else(|| ApiError::internal("Auth not available"))?;
    let user_id = Uuid::parse_str(&id).map_err(|_| ApiError::bad_request("Invalid user ID"))?;
    let user = mgr
        .get_user_by_id(&user_id)
        .ok_or_else(|| ApiError::not_found("User not found"))?;
    Ok(Json(json!({
        "id": user.id.to_string(),
        "username": user.username,
        "roles": user.roles,
        "created_at": user.created_at,
    })))
}

async fn delete_user(
    State(state): State<Arc<AdminState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let mgr = state
        .auth_mgr
        .as_ref()
        .ok_or_else(|| ApiError::internal("Auth not available"))?;
    let user_id = Uuid::parse_str(&id).map_err(|_| ApiError::bad_request("Invalid user ID"))?;
    if mgr.delete_user(&user_id) {
        Ok(Json(json!({"status": "deleted", "id": id})))
    } else {
        Err(ApiError::not_found("User not found"))
    }
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
    let mgr = state
        .auth_mgr
        .as_ref()
        .ok_or_else(|| ApiError::internal("Auth not available"))?;
    let user_id = Uuid::parse_str(&id).map_err(|_| ApiError::bad_request("Invalid user ID"))?;
    mgr.update_user_roles(&user_id, req.roles.clone())
        .map_err(|_| ApiError::not_found("User not found"))?;
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
    Json(req): Json<ChangePasswordRequest>,
) -> Result<Json<Value>, ApiError> {
    let mgr = state
        .auth_mgr
        .as_ref()
        .ok_or_else(|| ApiError::internal("Auth not available"))?;
    let user_id = Uuid::parse_str(&id).map_err(|_| ApiError::bad_request("Invalid user ID"))?;
    let user = mgr
        .get_user_by_id(&user_id)
        .ok_or_else(|| ApiError::not_found("User not found"))?;
    if !bcrypt::verify(&req.current_password, &user.password_hash).unwrap_or(false) {
        return Err(ApiError::unauthorized("Current password is incorrect"));
    }
    mgr.password_policy()
        .validate(&req.new_password)
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    mgr.change_password(&user_id, &req.new_password)
        .map_err(|_| ApiError::not_found("User not found"))?;
    Ok(Json(json!({
        "status": "changed",
        "user_id": id,
    })))
}
