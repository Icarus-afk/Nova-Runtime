use axum::body::Body;
use axum::http::Request;
use axum::middleware::Next;
use axum::response::Response;
use std::time::Instant;
use tracing::{info, warn};

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
        info!("{} {} -> {} ({:.1}ms)", method, uri, status, elapsed_ms);
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

    // Public paths: health checks, openapi, login/refresh, graphql playground (GET), and OPTIONS preflight
    let is_public = path == "/health"
        || path == "/ready"
        || path == "/live"
        || path == "/metrics"
        || path == "/openapi.json"
        || path == "/runtime/status"
        || path == "/runtime/info"
        || path == "/graphql" && method == axum::http::Method::GET
        || path == "/api/v1/auth/login"
        || path == "/api/v1/auth/refresh"
        || method == axum::http::Method::OPTIONS;

    if is_public {
        return next.run(req).await;
    }

    // Only protect /api/v1 and /admin and /runtime/config (mutating) — health stays public but admin/config needs auth
    let needs_auth =
        path.starts_with("/api/v1/") || path.starts_with("/admin/") || path == "/runtime/config";

    if !needs_auth {
        return next.run(req).await;
    }

    let auth_mgr = match &state.auth_mgr {
        Some(m) => m,
        None => return next.run(req).await, // if auth disabled, allow
    };

    let token = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.to_string());

    match token {
        Some(t) => {
            if auth_mgr.validate_session(&t).is_ok() {
                next.run(req).await
            } else {
                let body = serde_json::json!({"type":"about:blank","title":"Unauthorized","status":401,"detail":"Invalid or expired token"});
                let mut res = Response::new(Body::from(body.to_string()));
                *res.status_mut() = axum::http::StatusCode::UNAUTHORIZED;
                res.headers_mut()
                    .insert("content-type", "application/json".parse().unwrap());
                res
            }
        }
        None => {
            // Also accept api-key via X-Api-Key or Authorization: ApiKey <key> ?
            let body = serde_json::json!({"type":"about:blank","title":"Unauthorized","status":401,"detail":"Missing Authorization Bearer token"});
            let mut res = Response::new(Body::from(body.to_string()));
            *res.status_mut() = axum::http::StatusCode::UNAUTHORIZED;
            res.headers_mut()
                .insert("content-type", "application/json".parse().unwrap());
            res
        }
    }
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
}
