use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{
    ws::{Message, WebSocket, WebSocketUpgrade},
    ConnectInfo,
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
    tls_config: Option<axum_server::tls_rustls::RustlsConfig>,
) -> JoinHandle<()> {
    let state = WsAppState { metrics_rx, auth_key };

    let app = Router::new()
        .route("/dashboard", get(serve_dashboard))
        .route("/ws", get(ws_handler))
        .route("/health", get(|| async { "OK" }))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = format!("0.0.0.0:{port}");

    match tls_config {
        Some(tls_cfg) => {
            let socket_addr: std::net::SocketAddr = match addr.parse() {
                Ok(a) => a,
                Err(e) => {
                    error!("Invalid bind address {addr}: {e}");
                    return tokio::spawn(async move {
                        loop {
                            warn!("TLS server failed to bind, retrying in 60s...");
                            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                        }
                    });
                }
            };
            let tls_listener = axum_server::bind_rustls(socket_addr, tls_cfg);
            let handle = tokio::spawn(async move {
                if let Err(e) = tls_listener.serve(app.into_make_service()).await {
                    error!("WebSocket server error: {e}");
                }
            });
            info!("WebSocket server listening on https://{addr}");
            handle
        }
        None => {
            let listener = match tokio::net::TcpListener::bind(&addr).await {
                Ok(l) => l,
                Err(e) => {
                    error!("Failed to bind WebSocket server to {addr}: {e}");
                    return tokio::spawn(async move {
                        loop {
                            warn!("WebSocket server failed to bind, retrying in 60s...");
                            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                        }
                    });
                }
            };
            let handle = tokio::spawn(async move {
                if let Err(e) = axum::serve(listener, app).await {
                    error!("WebSocket server error: {e}");
                }
            });
            info!("WebSocket server listening on http://{addr}");
            handle
        }
    }
}

pub fn stop_ws_server(handle: JoinHandle<()>) {
    handle.abort();
}

async fn serve_dashboard(
    axum::extract::State(state): axum::extract::State<WsAppState>,
) -> Html<String> {
    let auth_script = match &state.auth_key {
        Some(key) => format!(
            r#"<script>window.__WS_AUTH='{}';</script>"#,
            key.replace('\\', "\\\\").replace('\'', "\\'"),
        ),
        None => "<script>window.__WS_AUTH=null;</script>".to_string(),
    };
    let html = include_str!("../dashboard.html");
    Html(html.replacen("</body>", &format!("{}\n</body>", auth_script), 1))
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    axum::extract::State(state): axum::extract::State<WsAppState>,
    axum::extract::Query(query): axum::extract::Query<HashMap<String, String>>,
    ConnectInfo(addr): ConnectInfo<std::net::SocketAddr>,
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
    ws.on_upgrade(move |socket| handle_socket(socket, state, addr))
}

async fn handle_socket(socket: WebSocket, state: WsAppState, addr: std::net::SocketAddr) {
    let mut rx = state.metrics_rx.resubscribe();
    info!("WebSocket client connected from {addr}");

    let (mut sender, mut receiver) = socket.split();

    loop {
        tokio::select! {
            biased;
            _ = receiver.next() => {
                info!("WebSocket client disconnected from {addr}");
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
