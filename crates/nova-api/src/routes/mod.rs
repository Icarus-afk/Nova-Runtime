pub mod sql;
pub mod cache;
pub mod queue;
pub mod scheduler;
pub mod search;
pub mod blob;
pub mod auth;

use crate::admin::AdminState;
use crate::ws;
use axum::routing::get;
use axum::Router;
use std::sync::Arc;

pub fn v1_routes(state: Arc<AdminState>) -> Router {
    Router::new()
        .nest("/sql", sql::routes(state.clone()))
        .nest("/cache", cache::routes(state.clone()))
        .nest("/queues", queue::routes(state.clone()))
        .nest("/scheduler", scheduler::routes(state.clone()))
        .nest("/search", search::routes(state.clone()))
        .nest("/blobs", blob::routes(state.clone()))
        .nest("/auth", auth::routes(state.clone()))
}

pub fn ws_routes(state: Arc<AdminState>) -> Router {
    Router::new()
        .route("/ws", get(ws::ws_handler))
        .with_state(state)
}
