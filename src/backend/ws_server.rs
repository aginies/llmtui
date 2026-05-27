use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{
    ws::{Message, WebSocket, WebSocketUpgrade},
};
use axum::response::IntoResponse;
use axum::{response::Html, routing::get, Router};
use axum::http::StatusCode;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tower_http::trace::TraceLayer;
use tracing::{error, info, warn};

use crate::models::WsMetrics;

#[derive(Clone)]
pub struct WsAppState {
    pub metrics_rx: Arc<broadcast::Receiver<WsMetrics>>,
    pub auth_key: Option<String>,
}

pub async fn start_ws_server(
    port: u16,
    metrics_rx: Arc<broadcast::Receiver<WsMetrics>>,
    auth_key: Option<String>,
) -> JoinHandle<()> {
    let state = WsAppState { metrics_rx, auth_key };

    let app = Router::new()
        .route("/dashboard", get(serve_dashboard))
        .route("/ws", get(ws_handler))
        .route("/health", get(|| async { "OK" }))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await;
    match listener {
        Ok(listener) => {
            let handle = tokio::spawn(async move {
                if let Err(e) = axum::serve(listener, app).await {
                    error!("WebSocket server error: {e}");
                }
            });
            info!("WebSocket server listening on {addr}");
            handle
        }
        Err(e) => {
            error!("Failed to bind WebSocket server to {addr}: {e}");
            tokio::spawn(async move {
                loop {
                    warn!("WebSocket server failed to bind, retrying in 60s...");
                    tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                }
            })
        }
    }
}

pub fn stop_ws_server(handle: JoinHandle<()>) {
    handle.abort();
}

async fn serve_dashboard() -> Html<&'static str> {
    Html(include_str!("../dashboard.html"))
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    axum::extract::State(state): axum::extract::State<WsAppState>,
    axum::extract::Query(query): axum::extract::Query<HashMap<String, String>>,
) -> impl IntoResponse {
    if let Some(ref expected) = state.auth_key {
        if let Some(provided) = query.get("auth").and_then(|v| urlencoding::decode(v).ok()) {
            if provided != *expected {
                return StatusCode::UNAUTHORIZED.into_response();
            }
        } else {
            return StatusCode::UNAUTHORIZED.into_response();
        }
    }
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: WsAppState) {
    let mut rx = state.metrics_rx.resubscribe();
    info!("WebSocket client connected");

    let (mut sender, mut receiver) = socket.split();

    loop {
        tokio::select! {
            biased;
            _ = receiver.next() => {
                info!("WebSocket client disconnected");
                break;
            }
            metrics = rx.recv() => match metrics {
                Ok(m) => {
                    let json = match serde_json::to_string(&m) {
                        Ok(j) => j,
                        Err(e) => {
                            error!("Failed to serialize metrics: {e}");
                            continue;
                        }
                    };
                    if sender.send(Message::Text(json.into())).await.is_err() {
                        info!("WebSocket client disconnected");
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!("WebSocket client lagged behind, skipped {n} metrics");
                }
                Err(broadcast::error::RecvError::Closed) => {
                    break;
                }
            },
        }
    }
}
