use axum::extract::{
    ws::{Message, WebSocket},
    State, WebSocketUpgrade,
};
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tracing::info;
use uuid::Uuid;

use super::app::AppState;

/// WebSocket upgrade handler — accepts the connection and spawns a
/// handler task that keeps the connection alive and forwards events.
pub async fn ws_handler(
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

/// Handle an active WebSocket connection.
async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut tx, mut rx) = socket.split();
    let client_id = Uuid::new_v4().to_string();

    let (ws_tx, mut ws_rx) = tokio::sync::mpsc::channel::<Value>(32);

    info!("WS client connected: {} (ip={:?})", client_id, state.ws_clients.read().unwrap().len());

    // Register the client
    {
        let mut clients = state.ws_clients.write().unwrap();
        clients.insert(client_id.clone(), ws_tx);
    }

    // Spawn a task to send messages to the client
    let send_task = tokio::spawn(async move {
        while let Some(msg) = ws_rx.recv().await {
            if tx.send(Message::Text(msg.to_string().into())).await.is_err() {
                break;
            }
        }
    });

    // Handle incoming messages
    while let Some(msg) = rx.next().await {
        match msg {
            Ok(Message::Close(_)) | Err(_) => break,
            Ok(Message::Text(t)) => {
                tracing::debug!("WS client {} received: {} chars", client_id, t.len());
            }
            Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => {}
            Ok(_) => {}
        }
    }

    info!("WS client disconnected: {} (connected: {})", client_id, state.ws_clients.read().unwrap().len());

    // Clean up
    {
        let mut clients = state.ws_clients.write().unwrap();
        clients.remove(&client_id);
    }
    send_task.abort();
}
