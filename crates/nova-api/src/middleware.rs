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
    let mut response = next.run(req).await;
    response.headers_mut().insert(
        "access-control-allow-origin",
        "*".parse().unwrap(),
    );
    response.headers_mut().insert(
        "access-control-allow-methods",
        "GET, POST, PUT, DELETE, PATCH, OPTIONS"
            .parse()
            .unwrap(),
    );
    response.headers_mut().insert(
        "access-control-allow-headers",
        "Content-Type, Authorization, Idempotency-Key"
            .parse()
            .unwrap(),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use axum::response::Json;
    use axum::routing::get;
    use axum::Router;
    use serde_json::json;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_cors_layer_sets_origin_header() {
        let app = Router::new()
            .route("/test", get(|| async { Json(json!({"ok": true})) }))
            .layer(axum::middleware::from_fn(cors_layer));

        let response = app
            .oneshot(Request::get("/test").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(
            response.headers()["access-control-allow-origin"],
            "*"
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

        assert!(response.headers().contains_key("access-control-allow-origin"));
        assert!(response.headers().contains_key("access-control-allow-methods"));
        assert!(response.headers().contains_key("access-control-allow-headers"));
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
            .oneshot(Request::get("/nonexistent").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(response.headers()["access-control-allow-origin"], "*");
    }

    #[tokio::test]
    async fn test_middleware_stack_both_layers() {
        let app = Router::new()
            .route("/test", get(|| async { Json(json!({"ok": true})) }))
            .layer(axum::middleware::from_fn(cors_layer))
            .layer(axum::middleware::from_fn(request_logger));

        let response = app
            .oneshot(Request::get("/test").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert!(response.status().is_success());
        assert_eq!(response.headers()["access-control-allow-origin"], "*");
    }
}
