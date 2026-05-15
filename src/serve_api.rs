use std::net::SocketAddr;
use std::time::Instant;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::Json;
use axum::Router;
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

/// Proxy a request to the llama-server backend.
async fn proxy_request(
    State(state): State<ApiState>,
    method: axum::http::Method,
    path: String,
    body: Option<String>,
) -> impl IntoResponse {
    let client = Client::new();
    let url = format!("{}{}", state.server_url, path);

    let mut request_builder = match method {
        axum::http::Method::GET => client.get(&url),
        axum::http::Method::POST => client.post(&url),
        axum::http::Method::PUT => client.put(&url),
        axum::http::Method::DELETE => client.delete(&url),
        _ => client.get(&url),
    };

    let response = match body {
        Some(body_str) => {
            request_builder = request_builder.header("Content-Type", "application/json");
            request_builder
                .body(body_str)
                .send()
                .await
        }
        None => request_builder.send().await,
    };

    match response {
        Ok(resp) => {
            let status = resp.status();
            let headers = resp.headers().clone();
            let bytes = resp.bytes().await.unwrap_or_default();
            (status, headers, bytes).into_response()
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

/// Proxy a POST request with JSON body.
async fn proxy_post(
    State(state): State<ApiState>,
    req: axum::extract::Request,
) -> impl IntoResponse {
    let path = req.uri().path().to_string();
    let body_bytes = axum::body::to_bytes(req.into_body(), 10 * 1024 * 1024)
        .await
        .unwrap_or_default();
    let body_str = String::from_utf8_lossy(&body_bytes).to_string();
    proxy_request(State(state), axum::http::Method::POST, path, Some(body_str)).await
}

/// Proxy a GET request.
async fn proxy_get(
    State(state): State<ApiState>,
    req: axum::extract::Request,
) -> impl IntoResponse {
    let path = req.uri().path().to_string();
    proxy_request(State(state), axum::http::Method::GET, path, None).await
}

/// Catch-all fallback: proxy any unmatched path to the llama-server backend.
async fn proxy_fallback(
    State(state): State<ApiState>,
    req: axum::extract::Request,
) -> impl IntoResponse {
    let path = req.uri().path().to_string();
    let method = req.method().clone();
    let body_bytes = axum::body::to_bytes(req.into_body(), 10 * 1024 * 1024)
        .await
        .unwrap_or_default();
    let body_str = String::from_utf8_lossy(&body_bytes).to_string();
    proxy_request(State(state), method, path, Some(body_str)).await
}

/// Custom status endpoint.
async fn status(State(state): State<ApiState>) -> impl IntoResponse {
    let client = Client::new();
    let uptime = state.start_time.elapsed();
    let uptime_secs = uptime.as_secs();

    // Try to get loaded models from llama-server
    let loaded_models = match client
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
    bind: SocketAddr,
    api_key: Option<String>,
    server_port: u16,
    model_name: String,
    pid: u32,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let start_time = Instant::now();
    let state = ApiState {
        server_url: format!("http://127.0.0.1:{}", server_port),
        api_key,
        model_name,
        pid,
        start_time,
        port: bind.port(),
    };

    let api_key_clone = state.api_key.clone();
    info!(
        "API server starting on http://{} (proxying to http://127.0.0.1:{})",
        bind, server_port
    );
    if api_key_clone.is_some() {
        info!("API key authentication is ENABLED");
    }

    let app = Router::new()
        .route("/health", get(proxy_get))
        .route("/metrics", get(proxy_get))
        .route("/v1/chat/completions", post(proxy_post))
        .route("/v1/completions", post(proxy_post))
        .route("/v1/embeddings", post(proxy_post))
        .route("/v1/models", get(proxy_get))
        .route("/api/status", get(status))
        .fallback(proxy_fallback)
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    axum::serve(tokio::net::TcpListener::bind(bind).await?, app).await?;
    Ok(())
}
