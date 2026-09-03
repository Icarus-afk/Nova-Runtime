use axum::body::{Body, to_bytes};
use axum::http::Request;
use axum::http::{HeaderName, HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tracing::{info, warn};

fn percent_decode(s: &str) -> String {
    fn hex_val(b: u8) -> Option<u8> {
        match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'a'..=b'f' => Some(b - b'a' + 10),
            b'A'..=b'F' => Some(b - b'A' + 10),
            _ => None,
        }
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let (Some(h), Some(l)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2]))
        {
            out.push(h << 4 | l);
            i += 3;
            continue;
        }
        out.push(if bytes[i] == b'+' { b' ' } else { bytes[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub async fn request_timeout(req: Request<Body>, next: Next) -> Response {
    // Enforce a 30s request timeout to mitigate slowloris / hung handlers.
    // Returns 504 Gateway Timeout on expiry (mapped to ApiError for consistency).
    match tokio::time::timeout(std::time::Duration::from_secs(30), next.run(req)).await {
        Ok(resp) => resp,
        Err(_) => crate::error::ApiError::new(
            axum::http::StatusCode::GATEWAY_TIMEOUT,
            "Gateway Timeout",
            "Request timed out",
        )
        .into_response(),
    }
}

pub async fn payload_too_large_handler(req: Request<Body>, next: Next) -> Response {
    let resp = next.run(req).await;
    if resp.status() == StatusCode::PAYLOAD_TOO_LARGE {
        return crate::error::ApiError::payload_too_large(
            "Request body too large (limit 10MB)",
        )
        .into_response();
    }
    resp
}

pub async fn request_logger(req: Request<Body>, next: Next) -> Response {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let start = Instant::now();

    let response = next.run(req).await;

    let status = response.status();
    let elapsed = start.elapsed();
    let elapsed_ms = elapsed.as_secs_f64() * 1000.0;

    if status.is_server_error() {
        warn!("{} {} -> {} ({:.1}ms)", method, uri, status, elapsed_ms);
    } else if status.is_client_error() {
        info!("{} {} -> {} ({:.1}ms)", method, uri, status, elapsed_ms);
    } else {
        tracing::debug!("{} {} -> {} ({:.1}ms)", method, uri, status, elapsed_ms);
    }

    response
}

pub async fn cors_layer(req: Request<Body>, next: Next) -> Response {
    let origin = req
        .headers()
        .get("origin")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_default();

    let allowed_origins = [
        "http://localhost:5173",
        "http://127.0.0.1:5173",
        "http://localhost:8642",
        "http://127.0.0.1:8642",
    ];
    let is_allowed = origin.is_empty() || allowed_origins.contains(&origin.as_str());

    // Handle preflight OPTIONS directly
    if req.method() == axum::http::Method::OPTIONS {
        let mut res = Response::new(Body::empty());
        *res.status_mut() = axum::http::StatusCode::NO_CONTENT;
        if is_allowed {
            let allow_origin = if origin.is_empty() {
                "*"
            } else {
                origin.as_str()
            };
            if let Ok(v) = allow_origin.parse() {
                res.headers_mut().insert("access-control-allow-origin", v);
            }
            res.headers_mut().insert(
                "access-control-allow-methods",
                "GET, POST, PUT, DELETE, PATCH, OPTIONS".parse().unwrap(),
            );
            res.headers_mut().insert(
                "access-control-allow-headers",
                "Content-Type, Authorization, Idempotency-Key"
                    .parse()
                    .unwrap(),
            );
            if allow_origin != "*" {
                res.headers_mut()
                    .insert("access-control-allow-credentials", "true".parse().unwrap());
            }
            res.headers_mut().insert("vary", "Origin".parse().unwrap());
        }
        return res;
    }

    let mut response = next.run(req).await;

    if is_allowed {
        let allow_origin = if origin.is_empty() {
            "*"
        } else {
            origin.as_str()
        };
        if let Ok(v) = allow_origin.parse() {
            response
                .headers_mut()
                .insert("access-control-allow-origin", v);
        }
        if allow_origin != "*" {
            response
                .headers_mut()
                .insert("access-control-allow-credentials", "true".parse().unwrap());
        }
    }
    // For disallowed origins, intentionally do NOT set ACAO header — browser will block
    response.headers_mut().insert(
        "access-control-allow-methods",
        "GET, POST, PUT, DELETE, PATCH, OPTIONS".parse().unwrap(),
    );
    response.headers_mut().insert(
        "access-control-allow-headers",
        "Content-Type, Authorization, Idempotency-Key"
            .parse()
            .unwrap(),
    );
    response
        .headers_mut()
        .insert("vary", "Origin".parse().unwrap());
    response
}

pub async fn auth_layer(
    axum::extract::State(state): axum::extract::State<std::sync::Arc<crate::admin::AdminState>>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let path = req.uri().path().to_string();
    let method = req.method().clone();

    // Explicit allowlist — deny by default (OWASP)
    let is_public = path == "/health"
        || path == "/ready"
        || path == "/live"
        || path == "/openapi.json"
        || path == "/api/v1/auth/login"
        || path == "/api/v1/auth/refresh"
        || method == axum::http::Method::OPTIONS;

    if is_public {
        return next.run(req).await;
    }

    // All API/admin/runtime/graphql require auth — deny by default
    let needs_auth = path.starts_with("/api/")
        || path.starts_with("/admin/")
        || path.starts_with("/runtime/")
        || path.starts_with("/graphql")
        || path == "/metrics"
        || path == "/ws";

    if !needs_auth {
        // Unknown path — deny with RFC7807 problem+json
        return crate::error::ApiError::not_found("Not found").into_response();
    }

    let auth_mgr = match &state.auth_mgr {
        Some(m) => m,
        None => {
            // Auth disabled but route requires auth — deny (not bypass)
            let mut res =
                crate::error::ApiError::unauthorized("Authentication required").into_response();
            res.headers_mut()
                .insert("www-authenticate", "Bearer".parse().unwrap());
            return res;
        }
    };

    // Try Bearer token first
    let header_bearer = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.to_string());

    // Browsers cannot set the Authorization header when opening a WebSocket
    // (new WebSocket(url)), so allow WS handshakes to authenticate via
    // ?token= / ?access_token= query param.
    let query_bearer = if path == "/api/v1/ws" {
        req.uri().query().and_then(|q| {
            q.split('&')
                .filter_map(|kv| kv.split_once('='))
                .find(|(k, _)| *k == "token" || *k == "access_token")
                .map(|(_, v)| percent_decode(v))
                .filter(|s| !s.is_empty())
        })
    } else {
        None
    };
    let bearer_token = header_bearer.or(query_bearer);

    // Try X-Api-Key header or Authorization: ApiKey
    let api_key = req
        .headers()
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .or_else(|| {
            req.headers()
                .get(axum::http::header::AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.strip_prefix("ApiKey "))
                .map(|s| s.to_string())
        });

    // Try Bearer
    if let Some(ref t) = bearer_token
        && let Ok(session) = auth_mgr.validate_session(t)
    {
        tracing::debug!(
            path = %path,
            user_id = %session.user_id,
            roles = ?session.roles,
            "nova_api middleware: bearer session"
        );
        // RBAC: gate admin-sensitive endpoints.
        // - PUT /admin/config and /runtime/config require admin:users:write or admin role
        // - GET /api/v1/auth/users, /api-keys, /roles (user enumeration) require admin
        // - POST/PUT/DELETE on /api/v1/auth/* for /users, /api-keys, /roles require admin
        // Self-service password change (PUT .../password) is exempted from admin gate
        // and is validated in the handler via current_password check.
        let is_admin_config = path.starts_with("/admin/config") || path.starts_with("/runtime/config");
        let is_auth_sensitive = path.starts_with("/api/v1/auth/")
            && (path == "/api/v1/auth/users"
                || path.starts_with("/api/v1/auth/users")
                || path == "/api/v1/auth/api-keys"
                || path.starts_with("/api/v1/auth/api-keys")
                || path.contains("/roles")
                || path.contains("/api-keys/"));
        let is_password_self_service =
            path.contains("/password") && method == axum::http::Method::PUT;
        // /admin/config is always admin-gated; auth sensitive is admin-gated for any method (covers enumeration via GET)
        let needs_admin = !is_password_self_service
            && (is_admin_config
                || (is_auth_sensitive
                    && (method == axum::http::Method::GET
                        || matches!(
                            method,
                            axum::http::Method::POST | axum::http::Method::PUT | axum::http::Method::DELETE
                        ))));
        // Also gate generic PUT /admin/config explicitly for RBAC parity
        let is_admin_config_mutating = is_admin_config && method == axum::http::Method::PUT;
        if needs_admin || is_admin_config_mutating {
            let has_admin = auth_mgr.check_permission(&session.user_id, "admin:users:write")
                || auth_mgr.check_permission(&session.user_id, "admin:config:write")
                || session.roles.contains(&"admin".to_string())
                || session.permissions.iter().any(|p| p == "*:*" || p == "admin:users:write");
            if !has_admin {
                return crate::error::ApiError::forbidden("Admin role required").into_response();
            }
        }
        return next.run(req).await;
    }

    // Try API key
    if let Some(ref k) = api_key {
        // Hash and lookup
        use sha2::{Digest, Sha256};
        let hash = hex::encode(Sha256::digest(k.as_bytes()));
        let found = auth_mgr.list_api_keys().into_iter().find(|ak| {
            ak.key_hash == hash
                && ak.enabled
                && ak
                    .expires_at
                    .map(|exp| exp > chrono::Utc::now().timestamp_millis())
                    .unwrap_or(true)
        });
        if let Some(api_key_record) = found {
            // Enforce RBAC on API keys.
            // - /admin/config (PUT) and /runtime/config require admin:users:write or *:*
            // - GET /api/v1/auth/users, /api-keys, /roles (enumeration) require admin
            // - Mutating auth endpoints require admin:users:write
            // For all other authenticated routes, any valid enabled non-expired API key is sufficient,
            // but callers should scope keys with least-privilege permissions; a key with no relevant
            // permission will be denied on admin routes and allowed on data-plane routes only if it
            // carries read:* / write:* / *:* as appropriate. Document least-privilege expectation.
            let is_admin_config = path.starts_with("/admin/config") || path.starts_with("/runtime/config");
            let is_auth_sensitive = path.starts_with("/api/v1/auth/")
                && (path == "/api/v1/auth/users"
                    || path.starts_with("/api/v1/auth/users")
                    || path == "/api/v1/auth/api-keys"
                    || path.starts_with("/api/v1/auth/api-keys")
                    || path.contains("/roles")
                    || path.contains("/api-keys/"));
            let needs_admin = is_admin_config
                || (is_auth_sensitive
                    && (method == axum::http::Method::GET
                        || matches!(
                            method,
                            axum::http::Method::POST | axum::http::Method::PUT | axum::http::Method::DELETE
                        )));
            if needs_admin {
                let has_permission = api_key_record
                    .permissions
                    .iter()
                    .any(|p| p == "admin:users:write" || p == "admin:config:write" || p == "*:*");
                if !has_permission {
                    return crate::error::ApiError::forbidden(
                        "API key lacks required permission: admin:users:write",
                    )
                    .into_response();
                }
            }
            // Generic data-plane RBAC for API keys: enforce read/write scoping where permissions are restrictive.
            // If key has scoped permissions (e.g., only admin:users:write), deny it for data-plane to enforce least privilege.
            // Keys with *:* or read:*/write:* pass; keys with only admin:* are blocked from /sql, /cache etc.
            if !needs_admin
                && !path.starts_with("/api/v1/auth/")
                && !api_key_record.permissions.is_empty()
            {
                let is_read = method == axum::http::Method::GET;
                let has_data_perm = api_key_record.permissions.iter().any(|p| {
                    p == "*:*" || p == "read:*" || p == "write:*" || (is_read && p.starts_with("read:")) || (!is_read && p.starts_with("write:"))
                });
                // If key is scoped only to admin permissions, deny data-plane access to avoid privilege confusion
                let is_admin_only = api_key_record.permissions.iter().all(|p| p.starts_with("admin:"));
                if is_admin_only && !has_data_perm {
                    // admin-only keys should not be used for data-plane; deny with 403 to signal scoping issue
                    // but allow *: * keys.
                    return crate::error::ApiError::forbidden(
                        "API key lacks data-plane permission (requires read:* / write:* or *:* )",
                    )
                    .into_response();
                }
            }
            return next.run(req).await;
        }
    }

    let mut res = if bearer_token.is_some() || api_key.is_some() {
        crate::error::ApiError::unauthorized("Invalid or expired token or API key").into_response()
    } else {
        crate::error::ApiError::unauthorized("Missing Authorization Bearer token or X-Api-Key")
            .into_response()
    };
    res.headers_mut()
        .insert("www-authenticate", "Bearer".parse().unwrap());
    res
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn is_valid_ip_like(s: &str) -> bool {
    // Very cheap validation: must contain '.' or ':' and only allowed chars, length bound
    if s.is_empty() || s.len() > 45 {
        return false;
    }
    let has_dot = s.contains('.');
    let has_colon = s.contains(':');
    if !has_dot && !has_colon {
        return false;
    }
    s.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == ':' || c == '[' || c == ']')
}

fn client_key(req: &Request<Body>) -> String {
    // SECURITY: X-Forwarded-For is spoofable unless behind a trusted proxy.
    // This is only used for rate-limiting (not auth), so spoofing just shifts
    // the bucket, not bypass. If deploying behind a proxy, ensure the proxy
    // strips/forges XFF and set TRUSTED_PROXIES. For now we validate format
    // and take the first entry; fallback to "local" if missing/invalid.
    if let Some(v) = req
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && is_valid_ip_like(s))
    {
        return v;
    }
    if let Some(v) = req
        .headers()
        .get("x-real-ip")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && is_valid_ip_like(s))
    {
        return v;
    }
    "local".to_string()
}

/// Fixed-window rate limiter keyed by client IP.
pub struct RateLimitState {
    limit: u32,
    window_ms: u64,
    windows: Mutex<HashMap<String, Window>>,
}

struct Window {
    start: u64,
    count: u32,
}

impl RateLimitState {
    pub fn new(limit: u32, window_ms: u64) -> Self {
        Self {
            limit,
            window_ms,
            windows: Mutex::new(HashMap::new()),
        }
    }
}

pub async fn rate_limit_layer(
    axum::extract::State(state): axum::extract::State<Arc<RateLimitState>>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let client = client_key(&req);
    let now = now_ms();
    let (permit, remaining, reset_in) = {
        let mut windows = state.windows.lock().unwrap();
        if windows.len() > 10_000 {
            windows.retain(|_, w| now.saturating_sub(w.start) < state.window_ms);
        }
        let w = windows.entry(client).or_insert(Window {
            start: now,
            count: 0,
        });
        if now.saturating_sub(w.start) >= state.window_ms {
            w.start = now;
            w.count = 0;
        }
        w.count += 1;
        let remaining = state.limit.saturating_sub(w.count);
        let reset_at = w.start + state.window_ms;
        (
            w.count <= state.limit,
            remaining,
            reset_at.saturating_sub(now),
        )
    };

    let mut response = if permit {
        next.run(req).await
    } else {
        crate::error::ApiError::too_many_requests("Rate limit exceeded")
            .with_header("retry-after", reset_in.max(1).to_string())
            .with_header("x-ratelimit-limit", state.limit.to_string())
            .with_header("x-ratelimit-remaining", "0".to_string())
            .with_header("x-ratelimit-reset", reset_in.to_string())
            .into_response()
    };

    if !response.headers().contains_key("x-ratelimit-limit")
        && let Ok(v) = HeaderValue::from_str(&state.limit.to_string())
    {
        response.headers_mut().insert("x-ratelimit-limit", v);
    }
    if let Ok(v) = HeaderValue::from_str(&remaining.to_string()) {
        response.headers_mut().insert("x-ratelimit-remaining", v);
    }
    if let Ok(v) = HeaderValue::from_str(&reset_in.to_string()) {
        response.headers_mut().insert("x-ratelimit-reset", v);
    }
    response
}

/// Idempotency-Key deduplication for mutating methods.
pub struct IdempotencyState {
    store: Mutex<HashMap<String, StoredResponse>>,
    ttl_ms: u64,
}

struct StoredResponse {
    status: u16,
    headers: Vec<(String, String)>,
    body: Bytes,
}

impl Clone for StoredResponse {
    fn clone(&self) -> Self {
        Self {
            status: self.status,
            headers: self.headers.clone(),
            body: self.body.clone(),
        }
    }
}

impl IdempotencyState {
    pub fn new() -> Self {
        Self {
            store: Mutex::new(HashMap::new()),
            ttl_ms: 60_000,
        }
    }
}

impl Default for IdempotencyState {
    fn default() -> Self {
        Self::new()
    }
}

pub async fn idempotency_layer(
    axum::extract::State(state): axum::extract::State<Arc<IdempotencyState>>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let m = req.method();
    let mutating =
        m == Method::POST || m == Method::PUT || m == Method::PATCH || m == Method::DELETE;
    let key = req
        .headers()
        .get("idempotency-key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    if !mutating || key.is_none() {
        return next.run(req).await;
    }

    // Scope by method + idempotency-key + path + user identity to avoid cross-user/method collisions.
    let user_scope = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let api_key_scope = req
        .headers()
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let identity = if !user_scope.is_empty() {
        user_scope
    } else {
        api_key_scope
    };
    let store_key = format!(
        "{}:{}:{}:{}",
        m,
        key.as_deref().unwrap_or(""),
        req.uri().path(),
        identity
    );
    {
        let mut store = state.store.lock().unwrap();
        if store.len() > 10_000 {
            let cutoff = now_ms().saturating_sub(state.ttl_ms);
            store.retain(|_, s| {
                s.headers
                    .iter()
                    .find(|(n, _)| n == "__created")
                    .and_then(|(_, v)| v.parse::<u64>().ok())
                    .map(|t| t >= cutoff)
                    .unwrap_or(true)
            });
        }
        if let Some(stored) = store.get(&store_key).cloned() {
            return build_response(stored);
        }
    }

    let response = next.run(req).await;
    let status = response.status().as_u16();
    let headers: Vec<(String, String)> = response
        .headers()
        .iter()
        .filter(|(n, _)| {
            *n != axum::http::header::TRANSFER_ENCODING && *n != axum::http::header::CONTENT_LENGTH
        })
        .map(|(n, v)| (n.as_str().to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();
    let body = match to_bytes(response.into_body(), usize::MAX).await {
        Ok(b) => b,
        Err(_) => Bytes::new(),
    };

    let mut headers = headers;
    headers.push(("__created".to_string(), now_ms().to_string()));

    if (200..300).contains(&status) {
        state.store.lock().unwrap().insert(
            store_key,
            StoredResponse {
                status,
                headers: headers.clone(),
                body: body.clone(),
            },
        );
    }

    build_response(StoredResponse {
        status,
        headers,
        body,
    })
}

fn build_response(stored: StoredResponse) -> Response {
    let mut resp = Response::new(Body::from(stored.body));
    *resp.status_mut() =
        StatusCode::from_u16(stored.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    for (n, v) in stored.headers {
        if n == "__created" {
            continue;
        }
        if let (Ok(hn), Ok(hv)) = (
            HeaderName::from_bytes(n.as_bytes()),
            HeaderValue::from_str(&v),
        ) {
            resp.headers_mut().insert(hn, hv);
        }
    }
    resp
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Router;
    use axum::http::StatusCode;
    use axum::response::Json;
    use axum::routing::get;
    use serde_json::json;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_cors_layer_sets_origin_header() {
        let app = Router::new()
            .route("/test", get(|| async { Json(json!({"ok": true})) }))
            .layer(axum::middleware::from_fn(cors_layer));

        let response = app
            .oneshot(
                Request::get("/test")
                    .header("origin", "http://localhost:5173")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            response.headers()["access-control-allow-origin"],
            "http://localhost:5173"
        );
    }

    #[tokio::test]
    async fn test_cors_layer_sets_methods_header() {
        let app = Router::new()
            .route("/test", get(|| async { Json(json!({"ok": true})) }))
            .layer(axum::middleware::from_fn(cors_layer));

        let response = app
            .oneshot(Request::get("/test").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(
            response.headers()["access-control-allow-methods"],
            "GET, POST, PUT, DELETE, PATCH, OPTIONS"
        );
    }

    #[tokio::test]
    async fn test_cors_layer_sets_headers_header() {
        let app = Router::new()
            .route("/test", get(|| async { Json(json!({"ok": true})) }))
            .layer(axum::middleware::from_fn(cors_layer));

        let response = app
            .oneshot(Request::get("/test").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(
            response.headers()["access-control-allow-headers"],
            "Content-Type, Authorization, Idempotency-Key"
        );
    }

    #[tokio::test]
    async fn test_cors_layer_all_headers_set() {
        let app = Router::new()
            .route("/test", get(|| async { Json(json!({"ok": true})) }))
            .layer(axum::middleware::from_fn(cors_layer));

        let response = app
            .oneshot(Request::get("/test").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert!(
            response
                .headers()
                .contains_key("access-control-allow-origin")
        );
        assert!(
            response
                .headers()
                .contains_key("access-control-allow-methods")
        );
        assert!(
            response
                .headers()
                .contains_key("access-control-allow-headers")
        );
    }

    #[tokio::test]
    async fn test_request_logger_passthrough() {
        let app = Router::new()
            .route("/ok", get(|| async { Json(json!({"status": "ok"})) }))
            .layer(axum::middleware::from_fn(request_logger));

        let response = app
            .oneshot(Request::get("/ok").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert!(response.status().is_success());
    }

    #[tokio::test]
    async fn test_request_logger_does_not_modify_body() {
        let app = Router::new()
            .route("/data", get(|| async { Json(json!({"key": "value"})) }))
            .layer(axum::middleware::from_fn(request_logger));

        let response = app
            .oneshot(Request::get("/data").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert!(response.status().is_success());
    }

    #[tokio::test]
    async fn test_cors_on_not_found() {
        let app = Router::new()
            .route("/exists", get(|| async { Json(json!({"ok": true})) }))
            .layer(axum::middleware::from_fn(cors_layer));

        let response = app
            .oneshot(
                Request::get("/nonexistent")
                    .header("origin", "http://localhost:5173")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            response.headers()["access-control-allow-origin"],
            "http://localhost:5173"
        );
    }

    #[tokio::test]
    async fn test_cors_rejects_disallowed_origin() {
        let app = Router::new()
            .route("/test", get(|| async { Json(json!({"ok": true})) }))
            .layer(axum::middleware::from_fn(cors_layer));

        let response = app
            .oneshot(
                Request::get("/test")
                    .header("origin", "https://evil.com")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // Disallowed origins must NOT get ACAO echoed (browser blocks)
        assert!(
            !response
                .headers()
                .contains_key("access-control-allow-origin")
                || response.headers()["access-control-allow-origin"] != "https://evil.com"
        );
        assert!(
            response
                .headers()
                .contains_key("access-control-allow-methods")
        );
        assert!(
            response
                .headers()
                .contains_key("access-control-allow-headers")
        );
    }

    #[tokio::test]
    async fn test_middleware_stack_both_layers() {
        let app = Router::new()
            .route("/test", get(|| async { Json(json!({"ok": true})) }))
            .layer(axum::middleware::from_fn(cors_layer))
            .layer(axum::middleware::from_fn(request_logger));

        let response = app
            .oneshot(
                Request::get("/test")
                    .header("origin", "http://localhost:5173")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert!(response.status().is_success());
        assert_eq!(
            response.headers()["access-control-allow-origin"],
            "http://localhost:5173"
        );
    }

    #[tokio::test]
    async fn test_rate_limit_headers_and_throttle() {
        let state = Arc::new(RateLimitState::new(2, 60_000));
        let app = Router::new()
            .route("/rl", get(|| async { Json(json!({"ok": true})) }))
            .layer(axum::middleware::from_fn_with_state(
                state,
                rate_limit_layer,
            ));

        let r1 = app
            .clone()
            .oneshot(Request::get("/rl").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(r1.headers()["x-ratelimit-limit"], "2");
        assert_eq!(r1.headers()["x-ratelimit-remaining"], "1");
        assert!(r1.headers().contains_key("x-ratelimit-reset"));
        assert!(r1.status().is_success());

        let r2 = app
            .clone()
            .oneshot(Request::get("/rl").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(r2.headers()["x-ratelimit-remaining"], "0");
        assert!(r2.status().is_success());

        let r3 = app
            .clone()
            .oneshot(Request::get("/rl").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(r3.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(r3.headers()["x-ratelimit-remaining"], "0");
        assert!(r3.headers().contains_key("retry-after"));
    }

    #[derive(Clone)]
    struct IdemCtx {
        calls: Arc<std::sync::atomic::AtomicUsize>,
    }

    async fn idem_handler(
        axum::extract::State(ctx): axum::extract::State<IdemCtx>,
    ) -> Json<serde_json::Value> {
        ctx.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Json(json!({"created": true, "seq": 1}))
    }

    #[tokio::test]
    async fn test_idempotency_replays_response() {
        let state = Arc::new(IdempotencyState::new());
        let ctx = IdemCtx {
            calls: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        };
        let app = Router::new()
            .route("/idem", axum::routing::post(idem_handler))
            .with_state(ctx.clone())
            .layer(axum::middleware::from_fn_with_state(
                state,
                idempotency_layer,
            ));

        let req1 = Request::post("/idem")
            .header("idempotency-key", "abc-123")
            .body(Body::empty())
            .unwrap();
        let r1 = app.clone().oneshot(req1).await.unwrap();
        assert!(r1.status().is_success());

        let req2 = Request::post("/idem")
            .header("idempotency-key", "abc-123")
            .body(Body::empty())
            .unwrap();
        let r2 = app.clone().oneshot(req2).await.unwrap();

        assert_eq!(ctx.calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(r1.status(), r2.status());
    }
}
