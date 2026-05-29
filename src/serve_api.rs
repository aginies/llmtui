use std::net::SocketAddr;
use std::time::Instant;

use axum::body::Body;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::Json;
use axum::Router;
use futures_util::{stream, StreamExt};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::info;

use reqwest::Client;

#[derive(Clone)]
pub struct ApiState {
    pub server_url: String,
    pub api_key: Option<String>,
    pub model_name: String,
    pub pid: u32,
    pub start_time: Instant,
    pub port: u16,
    pub client: reqwest::Client,
}

fn extract_api_key(headers: &axum::http::HeaderMap) -> Option<String> {
    headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.to_string())
}

async fn auth_middleware(
    State(state): State<ApiState>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    if let Some(expected) = &state.api_key {
        let provided = extract_api_key(req.headers());
        if provided.as_deref() != Some(expected) {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "Unauthorized"})),
            )
                .into_response();
        }
    }
    next.run(req).await
}

/// Proxy a request to the llama-server backend with SSE streaming support.
/// Checks Content-Type: if text/event-stream, streams the body; otherwise buffers.
async fn proxy_streaming(
    State(state): State<ApiState>,
    req: axum::extract::Request,
) -> impl IntoResponse {
    let path = req.uri().path().to_string();
    let method = req.method().clone();
    let headers = req.headers().clone();

    let url = format!("{}{}", state.server_url, path);

    // Convert request body to a stream for reqwest
    let body_bytes = match axum::body::to_bytes(req.into_body(), 10 * 1024 * 1024)
        .await
    {
        Ok(b) => b,
        Err(e) => {
            info!("Failed to read request body for {}: {}", path, e);
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": format!("Failed to read request body: {}", e)})),
            )
                .into_response();
        }
    };
    let body_stream = stream::iter(vec![Ok::<_, reqwest::Error>(body_bytes.clone())]);

    let mut request_builder = match method {
        axum::http::Method::GET => state.client.get(&url),
        axum::http::Method::POST => state.client.post(&url),
        axum::http::Method::PUT => state.client.put(&url),
        axum::http::Method::DELETE => state.client.delete(&url),
        _ => state.client.get(&url),
    };

    if matches!(method, axum::http::Method::POST | axum::http::Method::PUT) {
        let content_type = headers
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/json");
        request_builder = request_builder.header("Content-Type", content_type);
    }

    let response = request_builder
        .body(reqwest::Body::wrap_stream(body_stream))
        .send()
        .await;

    match response {
        Ok(resp) => {
            let status = resp.status();
            let headers = resp.headers().clone();
            let is_sse = resp
                .headers()
                .get(axum::http::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .map(|v| v.contains("text/event-stream"))
                .unwrap_or(false);

            if is_sse {
                let mut response = axum::response::Response::new(Body::from_stream(
                    resp.bytes_stream().map(|result| {
                        result.map_err(|e| {
                            std::io::Error::new(std::io::ErrorKind::Other, e)
                        })
                    }),
                ));
                *response.status_mut() = status;
                *response.headers_mut() = headers;
                response
            } else {
                let bytes = match resp.bytes().await {
                    Ok(b) => b,
                    Err(e) => {
                        info!("Failed to read response body for {}: {}", path, e);
                        return (
                            StatusCode::BAD_GATEWAY,
                            Json(serde_json::json!({"error": format!("Failed to read backend response: {}", e)})),
                        )
                            .into_response();
                    }
                };
                (status, headers, bytes).into_response()
            }
        }
        Err(e) => {
            info!("Proxy error for {}: {}", path, e);
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({"error": format!("Backend unavailable: {}", e)})),
            )
                .into_response()
        }
    }
}

/// Custom status endpoint.
async fn status(State(state): State<ApiState>) -> impl IntoResponse {
    let uptime = state.start_time.elapsed();
    let uptime_secs = uptime.as_secs();

    // Try to get loaded models from llama-server
    let loaded_models = match state.client
        .get(format!("{}/models", state.server_url))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            let json: serde_json::Value = match resp.json().await {
                Ok(v) => v,
                Err(_) => serde_json::json!([]),
            };
            json.get("data")
                .and_then(|d| d.as_array())
                .map(|a| a.len())
                .unwrap_or(0)
        }
        _ => 0,
    };

    Json(serde_json::json!({
        "status": "running",
        "pid": state.pid,
        "port": state.port,
        "model": state.model_name,
        "uptime_seconds": uptime_secs,
        "loaded_models": loaded_models,
    }))
}

pub async fn start_api_server(
    addr: SocketAddr,
    api_key: Option<String>,
    server_port: u16,
    model_name: String,
    pid: u32,
    mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
    host: String,
    tls_config: Option<axum_server::tls_rustls::RustlsConfig>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let bind = addr;
    let start_time = Instant::now();
    let client = Client::new();
    let state = ApiState {
        server_url: format!("http://127.0.0.1:{}", server_port),
        api_key,
        model_name,
        pid,
        start_time,
        port: bind.port(),
        client,
    };

    let cors = CorsLayer::new()
        .allow_origin(tower_http::cors::Any)
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::PUT,
            axum::http::Method::DELETE,
            axum::http::Method::OPTIONS,
        ])
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::AUTHORIZATION,
        ]);

    let api_key_clone = state.api_key.clone();
    let protocol = if tls_config.is_some() { "https" } else { "http" };
    info!(
        "API server starting on {protocol}://{} (proxying to http://127.0.0.1:{})",
        host, server_port
    );
    if api_key_clone.is_some() {
        info!("API key authentication is ENABLED");
    }

    let app = Router::new()
        .route("/health", get(proxy_streaming))
        .route("/metrics", get(proxy_streaming))
        .nest(
            "/",
            Router::new()
                .route("/v1/chat/completions", post(proxy_streaming))
                .route("/v1/completions", post(proxy_streaming))
                .route("/v1/embeddings", post(proxy_streaming))
                .route("/v1/models", get(proxy_streaming))
                .route("/api/status", get(status))
                .fallback(proxy_streaming)
                .layer(cors)
                .layer(axum::middleware::from_fn_with_state(
                    state.clone(),
                    auth_middleware,
                ))
                .layer(TraceLayer::new_for_http()),
        )
        .with_state(state);

    match tls_config {
        Some(tls_cfg) => {
            let tls_listener = axum_server::bind_rustls(bind, tls_cfg);
            let shutdown_fut = async {
                let _ = shutdown_rx.wait_for(|v| *v).await;
            };
            let _ = tokio::select! {
                result = tls_listener.serve(app.into_make_service()) => result,
                _ = shutdown_fut => Ok(()),
            };
        }
        None => {
            axum::serve(tokio::net::TcpListener::bind(bind).await?, app)
                .with_graceful_shutdown(async move {
                    let _ = shutdown_rx.wait_for(|v| *v).await;
                })
                .await?;
        }
    }
    Ok(())
}
