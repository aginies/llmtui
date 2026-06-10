use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use std::time::Instant;

use axum::Json;
use axum::Router;
use axum::body::Body;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};

use futures_util::StreamExt;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::info;

use reqwest::Client;

pub struct StatusCache {
    pub models: usize,
    pub cached_at: Instant,
}

#[derive(Clone)]
pub struct ApiState {
    pub server_url: String,
    pub api_key: Option<String>,
    pub model_name: String,
    pub pid: u32,
    pub start_time: Instant,
    pub port: u16,
    pub client: reqwest::Client,
    pub status_cache: Arc<RwLock<StatusCache>>,
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

    // Stream request body directly to backend (no drain to memory)
    let body_stream = req.into_body().into_data_stream().map(|r| {
        r.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{}", e)))
    });

    let mut request_builder = match method {
        axum::http::Method::GET => state.client.get(&url),
        axum::http::Method::POST => state.client.post(&url),
        axum::http::Method::PUT => state.client.put(&url),
        axum::http::Method::DELETE => state.client.delete(&url),
        _ => {
            return (
                StatusCode::METHOD_NOT_ALLOWED,
                Json(serde_json::json!({"error": "Method not supported"})),
            )
                .into_response();
        }
    };

    const HOP_BY_HOP: &[&str] = &[
        "connection",
        "keep-alive",
        "proxy-authenticate",
        "proxy-authorization",
        "te",
        "trailer",
        "transfer-encoding",
        "upgrade",
        "host",
    ];
    let mut filtered = axum::http::HeaderMap::new();
    for (name, value) in headers.iter() {
        let n = name.as_str();
        if !HOP_BY_HOP.contains(&n) && n != "authorization" {
            filtered.insert(name, value.clone());
        }
    }
    request_builder = request_builder.headers(filtered);

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
                    resp.bytes_stream()
                        .map(|result| result.map_err(std::io::Error::other)),
                ));
                *response.status_mut() = status;
                for (name, value) in headers.iter() {
                    response.headers_mut().insert(name, value.clone());
                }
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

/// Simple health check endpoint - no auth, verifies backend
async fn health(State(state): State<ApiState>) -> impl IntoResponse {
    let resp = state
        .client
        .get(format!("{}/health", state.server_url))
        .send()
        .await;

    match resp {
        Ok(response) if response.status().is_success() => Json(serde_json::json!({
            "status": "ok",
            "backend": "healthy"
        })),
        Ok(_) => Json(serde_json::json!({
            "status": "degraded",
            "backend": "unreachable"
        })),
        Err(_) => Json(serde_json::json!({
            "status": "degraded",
            "backend": "unreachable"
        })),
    }
}

/// Custom status endpoint.
const STATUS_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(5);

#[axum::debug_handler]
async fn status(State(state): State<ApiState>) -> impl IntoResponse {
    let uptime = state.start_time.elapsed();
    let uptime_secs = uptime.as_secs();

    let loaded_models = {
        let (is_stale, cached_models) = {
            let cache = state.status_cache.read().unwrap();
            (cache.cached_at.elapsed() >= STATUS_CACHE_TTL, cache.models)
        };
        if !is_stale {
            cached_models
        } else {
            let count = match state
                .client
                .get(format!("{}/models", state.server_url))
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => {
                    let val: Option<serde_json::Value> = resp.json().await.ok();
                    let data = val.as_ref().and_then(|v| v.get("data")).and_then(|d| d.as_array());
                    let c = data.map(|a| a.len()).unwrap_or(0);
                    let mut cache = state.status_cache.write().unwrap();
                    cache.models = c;
                    cache.cached_at = Instant::now();
                    c
                }
                _ => {
                    let mut cache = state.status_cache.write().unwrap();
                    cache.models = 0;
                    cache.cached_at = Instant::now();
                    0
                }
            };
            count
        }
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
    let client = Client::builder()
        .pool_max_idle_per_host(20)
        .timeout(std::time::Duration::from_secs(60))
        .build()?;
    let state = ApiState {
        server_url: format!("http://127.0.0.1:{}", server_port),
        api_key,
        model_name,
        pid,
        start_time,
        port: bind.port(),
        client,
        status_cache: Arc::new(RwLock::new(StatusCache {
            models: 0,
            cached_at: Instant::now() - std::time::Duration::from_secs(10),
        })),
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
    let protocol = if tls_config.is_some() {
        "https"
    } else {
        "http"
    };
    info!(
        "API server starting on {protocol}://{} (proxying to http://127.0.0.1:{})",
        host, server_port
    );
    if api_key_clone.is_some() {
        info!("API key authentication is ENABLED");
    }

    let app = Router::new()
        .route("/health", get(health))
        .route("/metrics", get(proxy_streaming))
        .merge(
            Router::new()
                .route("/v1/chat/completions", post(proxy_streaming))
                .route("/v1/completions", post(proxy_streaming))
                .route("/v1/embeddings", post(proxy_streaming))
                .route("/v1/models", get(proxy_streaming))
                .route("/api/status", get(status))
                .fallback(proxy_streaming)
                .layer(cors)
                .layer(TraceLayer::new_for_http())
                .layer(axum::middleware::from_fn_with_state(
                    state.clone(),
                    auth_middleware,
                )),
        )
        .with_state(state);

    match tls_config {
        Some(tls_cfg) => {
            let tls_listener = axum_server::bind_rustls(bind, tls_cfg);
            let shutdown_fut = async {
                let _ = shutdown_rx.wait_for(|v| *v).await;
            };
            tokio::select! {
                result = tls_listener.serve(app.into_make_service()) => result?,
                _ = shutdown_fut => {},
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
