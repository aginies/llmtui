use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Router, response::Html, routing::get};
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
    host: String,
    mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
) -> Result<JoinHandle<()>> {
    let state = WsAppState {
        metrics_rx,
        auth_key,
    };

    let app = Router::new()
        .route("/dashboard", get(serve_dashboard))
        .route("/ws", get(ws_handler))
        .route("/health", get(|| async { "OK" }))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = format!("{host}:{port}");

    match tls_config {
        Some(tls_cfg) => {
            let socket_addr: std::net::SocketAddr = addr
                .parse()
                .map_err(|e| anyhow!("Invalid bind address {addr} for TLS: {e}"))?;
            let tls_listener = axum_server::bind_rustls(socket_addr, tls_cfg);
            let shutdown_fut = async move {
                let _ = shutdown_rx.wait_for(|v| *v).await;
            };
            let handle = tokio::spawn(async move {
                tokio::select! {
                    result = tls_listener.serve(app.into_make_service()) => {
                        if let Err(e) = result {
                            error!("WebSocket server error: {e}");
                        }
                    }
                    _ = shutdown_fut => {},
                }
            });
            info!("WebSocket server listening on https://{addr}");
            Ok(handle)
        }
        None => {
            let listener = tokio::net::TcpListener::bind(&addr)
                .await
                .with_context(|| format!("Failed to bind WebSocket server to {addr}"))?;
            let handle = tokio::spawn(async move {
                let _ = axum::serve(listener, app)
                    .with_graceful_shutdown(async move {
                        let _ = shutdown_rx.wait_for(|v| *v).await;
                    })
                    .await;
            });
            info!("WebSocket server listening on http://{addr}");
            Ok(handle)
        }
    }
}

pub fn stop_ws_server(handle: JoinHandle<()>) {
    handle.abort();
}

async fn serve_dashboard(
    axum::extract::State(state): axum::extract::State<WsAppState>,
) -> Html<String> {
    let auth_json = serde_json::to_string(&state.auth_key).unwrap_or("null".to_string());
    let auth_script = format!("<script>window.__WS_AUTH={};</script>", auth_json);
    let html = include_str!("../dashboard.html");
    Html(html.replacen("</body>", &format!("{}\n</body>", auth_script), 1))
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
