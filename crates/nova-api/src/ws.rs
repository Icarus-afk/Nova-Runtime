use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::response::IntoResponse;

use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use tracing::info;

use crate::admin::AdminState;

#[derive(Deserialize, Default)]
pub struct WsQuery {
    topics: Option<String>,
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AdminState>>,
    Query(query): Query<WsQuery>,
) -> impl IntoResponse {
    let topics = query.topics.clone().unwrap_or_default();
    ws.on_upgrade(move |socket| handle_socket(socket, state, topics))
}

async fn handle_socket(mut socket: WebSocket, state: Arc<AdminState>, _topics: String) {
    let event_bus = match state.event_bus.as_ref() {
        Some(bus) => bus.clone(),
        None => {
            let _ = socket.send(Message::Text(
                json!({"error": "event bus not available"}).to_string().into(),
            )).await;
            return;
        }
    };

    let (tx, rx) = crossbeam::channel::bounded::<nova_event::Event>(256);

    let topic = match nova_event::TopicPattern::new("db.table.*") {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!("invalid topic pattern: {e}");
            return;
        }
    };

    let sub_id = uuid::Uuid::new_v4();
    let sub = nova_event::Subscription {
        id: sub_id,
        subscriber: nova_event::SubscriberId {
            id: format!("ws-{sub_id}"),
            subsystem: nova_event::Subsystem::System,
            name: "websocket".into(),
        },
        topic,
        content_filter: None,
        delivery_guarantee: nova_event::DeliveryGuarantee::AtMostOnce,
        max_retries: 0,
        retry_backoff_ms: 0,
        max_backoff_ms: 0,
        queue_capacity: 256,
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before UNIX_EPOCH")
            .as_millis() as u64,
        active: true,
        consumer_group: None,
        sender: tx,
    };

    if let Err(e) = event_bus.subscribe(sub) {
        tracing::warn!("ws subscription failed: {e}");
        let _ = socket.send(Message::Text(
            json!({"error": "subscription failed"}).to_string().into(),
        )).await;
        return;
    }

    info!("WS client connected (sub_id={sub_id})");

    // Spawn a task to poll the crossbeam channel and forward to a tokio mpsc
    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel::<nova_event::Event>();
    tokio::task::spawn_blocking(move || {
        loop {
            match rx.recv() {
                Ok(event) => {
                    if event_tx.send(event).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    loop {
        tokio::select! {
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) => break,
                    Some(Ok(Message::Ping(_))) => {
                        let _ = socket.send(Message::Pong(vec![])).await;
                    }
                    Some(Ok(_)) => {}
                    Some(Err(_)) => break,
                    None => break,
                }
            }
            event = event_rx.recv() => {
                match event {
                    Some(event) => {
                        let event_id_hex = event.metadata.event_id.0.iter()
                            .map(|b| format!("{:02x}", b))
                            .collect::<Vec<_>>()
                            .join("");
                        let payload = json!({
                            "event": event.metadata.event_type.canonical,
                            "timestamp": event.metadata.timestamp,
                            "event_id": event_id_hex,
                        });
                        if socket.send(Message::Text(payload.to_string().into())).await.is_err() {
                            break;
                        }
                    }
                    None => break,
                }
            }
        }
    }

    event_bus.unsubscribe(sub_id);
    info!("WS client disconnected (sub_id={sub_id})");
}


