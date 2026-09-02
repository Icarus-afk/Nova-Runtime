use crate::admin::{self, AdminState};
use crate::error::ApiError;
use crate::middleware::{
    IdempotencyState, RateLimitState, auth_layer, cors_layer, idempotency_layer, rate_limit_layer,
    request_logger,
};
use crate::routes;
use axum::http::StatusCode;
use axum::{Router, middleware};
use std::sync::Arc;
use tokio::net::TcpListener;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;
use tracing::info;

pub async fn start_server(
    addr: &str,
    admin_state: Arc<AdminState>,
    shutdown_rx: tokio::sync::watch::Receiver<bool>,
    graphql_router: Option<Router>,
) -> Result<(), Box<dyn std::error::Error>> {
    let fallback = || async {
        (
            StatusCode::NOT_FOUND,
            axum::response::IntoResponse::into_response(ApiError::not_found(
                "The requested resource was not found",
            )),
        )
    };

    let rate_limit_state = Arc::new(RateLimitState::new(600, 60_000));
    let idempotency_state = Arc::new(IdempotencyState::new());

    let mut app = Router::new()
        .nest("/", admin::routes(admin_state.clone()))
        .nest("/api/v1", routes::v1_routes(admin_state.clone()))
        .nest(
            "/api/v1",
            routes::ws_router().with_state(admin_state.clone()),
        )
        .fallback(fallback)
        .layer(middleware::from_fn_with_state(
            admin_state.clone(),
            auth_layer,
        ))
        .layer(middleware::from_fn_with_state(
            rate_limit_state,
            rate_limit_layer,
        ))
        .layer(middleware::from_fn_with_state(
            idempotency_state,
            idempotency_layer,
        ))
        .layer(middleware::from_fn(cors_layer))
        .layer(middleware::from_fn(request_logger))
        .layer(RequestBodyLimitLayer::new(10 * 1024 * 1024)) // 10MB max request body
        .layer(TraceLayer::new_for_http());

    if let Some(gql) = graphql_router {
        app = app.merge(gql);
    }

    let listener = TcpListener::bind(addr).await?;
    info!("HTTP server listening on {}", addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(wait_for_shutdown(shutdown_rx))
        .await?;

    Ok(())
}

async fn wait_for_shutdown(mut rx: tokio::sync::watch::Receiver<bool>) {
    while !*rx.borrow() {
        if rx.changed().await.is_err() {
            break;
        }
    }
    info!("Shutdown signal received, starting graceful shutdown...");
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::watch;

    #[tokio::test]
    async fn test_wait_for_shutdown_returns_on_signal() {
        let (tx, rx) = watch::channel(false);
        let handle = tokio::spawn(wait_for_shutdown(rx));

        tx.send(true).ok();
        let _ = tokio::time::timeout(std::time::Duration::from_secs(5), handle)
            .await
            .expect("wait_for_shutdown did not return within 5s")
            .expect("wait_for_shutdown panicked");
    }

    #[tokio::test]
    async fn test_wait_for_shutdown_returns_on_drop() {
        let (tx, rx) = watch::channel(false);
        let handle = tokio::spawn(wait_for_shutdown(rx));

        drop(tx);
        let _ = tokio::time::timeout(std::time::Duration::from_secs(5), handle)
            .await
            .expect("wait_for_shutdown did not return within 5s")
            .expect("wait_for_shutdown panicked");
    }

    #[tokio::test]
    async fn test_wait_for_shutdown_starts_false() {
        let (tx, rx) = watch::channel(false);
        assert!(!*rx.borrow());
        drop(tx);
    }
}
