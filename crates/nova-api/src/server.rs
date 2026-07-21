use crate::admin::{self, AdminState};
use crate::middleware::{request_logger, cors_layer};
use crate::routes;
use axum::http::StatusCode;
use axum::response::{Html, Json};
use axum::{Router, middleware};
use serde_json::json;
use std::sync::Arc;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing::info;

pub async fn start_server(
    addr: &str,
    admin_state: Arc<AdminState>,
    shutdown_rx: tokio::sync::watch::Receiver<bool>,
    graphql_router: Option<Router>,
) -> Result<(), Box<dyn std::error::Error>> {
    let fallback = || async {
        (StatusCode::NOT_FOUND, Json(json!({
            "error": "not_found",
            "message": "The requested resource was not found"
        })))
    };

    let mut app = Router::new()
        .nest("/", admin::routes(admin_state.clone()))
        .nest("/api/v1", routes::v1_routes(admin_state.clone()))
        .nest("/api/v1", routes::ws_router().with_state(admin_state))
        .fallback(fallback)
        .layer(middleware::from_fn(cors_layer))
        .layer(middleware::from_fn(request_logger))
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
