use std::net::SocketAddr;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

use axum::Json;
use axum::Router;
use axum::body::{Body, Bytes, to_bytes};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use futures_util::StreamExt;
use tower_http::trace::TraceLayer;
use tracing::info;

use crate::models::clean_host;

use reqwest::Client;

use crate::backend::web_context;

/// HTTP hop-by-hop headers to strip when proxying.
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
    "content-length",
];

pub struct StatusCache {
    pub models: usize,
    pub cached_at: Instant,
    pub metrics: Option<crate::models::ServerMetrics>,
    pub metrics_at: Instant,
    pub log_metrics: crate::backend::server_logs::ServerLogMetrics,
    pub log_metrics_at: Instant,
    /// Last raw log line, kept for cross-line parsing (e.g. tokens-per-second).
    pub log_prev_line: Option<String>,
}

pub struct WebSearchConfig {
    pub engine: String,
    pub engine_url: String,
    pub enabled: bool,
    pub api_key: Option<String>,
}

#[derive(Clone)]
pub struct ApiState {
    pub server_url: String,
    pub server_host: String,
    pub server_port: u16,
    pub api_key: Option<String>,
    pub model_name: String,
    pub pid: u32,
    pub start_time: Instant,
    pub port: u16,
    pub client: reqwest::Client,
    pub status_cache: Arc<RwLock<StatusCache>>,
    pub log_rx: Option<Arc<Mutex<tokio::sync::mpsc::Receiver<String>>>>,
    pub system_prompt_preset_name: String,
    pub web_search_config: Arc<RwLock<WebSearchConfig>>,
    pub log_callback: Arc<Mutex<Option<Box<dyn Fn(String) + Send + Sync>>>>,
    /// WebSocket dashboard port (0 = disabled). Exposed via /api/status so
    /// the web chat can connect to the WS metrics channel for full stats.
    pub ws_port: u16,
    /// WebSocket dashboard auth key (None = no auth).
    pub ws_auth: Option<String>,
    /// Effective context length (context_length * rope_scale), 0 = use raw ctx_max.
    pub effective_ctx: u32,
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
    tracing::debug!("auth_middleware: api_key={:?}", state.api_key.is_some());
    if let Some(expected) = &state.api_key {
        let provided = extract_api_key(req.headers());
        let expected_bytes = expected.as_bytes();
        let not_equal = if let Some(provided_str) = provided {
            constant_time_not_eq(provided_str.as_bytes(), expected_bytes)
        } else {
            true
        };
        if not_equal {
            tracing::debug!(
                "auth_middleware: rejecting request, not_equal={}",
                not_equal
            );
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "Unauthorized"})),
            )
                .into_response();
        }
    }
    next.run(req).await
}

/// Constant-time byte comparison: returns true if a != b.
/// Always processes all bytes regardless of where the first difference occurs.
fn constant_time_not_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return true;
    }
    let mut result: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result != 0
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

    // For chat completions and completions, drain body and optionally inject web search
    if (path == "/v1/chat/completions" || path == "/v1/completions")
        && method == axum::http::Method::POST
    {
        info!("API: proxying {} {}", method, path);
        {
            let cb = state.log_callback.lock().unwrap();
            if let Some(c) = cb.as_ref() {
                c(format!("API: proxying {} {}", method, path));
            }
        }
        let body_bytes = match to_bytes(req.into_body(), 10 * 1024 * 1024).await {
            Ok(b) => b,
            Err(e) => {
                info!("Failed to collect request body for {}: {}", path, e);
                return (
                    StatusCode::BAD_GATEWAY,
                    Json(
                        serde_json::json!({"error": format!("Failed to read request body: {}", e)}),
                    ),
                )
                    .into_response();
            }
        };

        let body_bytes = body_bytes;
        let mut request_json: serde_json::Value = match serde_json::from_slice(&body_bytes) {
            Ok(j) => j,
            Err(e) => {
                info!("Failed to parse request JSON for {}: {}", path, e);
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": format!("Invalid JSON: {}", e)})),
                )
                    .into_response();
            }
        };

        let (ws_enabled, ws_engine, ws_engine_url, ws_api_key) = {
            let ws_cfg = state.web_search_config.read().unwrap();
            (
                ws_cfg.enabled,
                ws_cfg.engine.clone(),
                ws_cfg.engine_url.clone(),
                ws_cfg.api_key.clone().unwrap_or_default(),
            )
        };
        info!(
            "API: web_search_enabled={}, preset='{}', engine='{}'",
            ws_enabled, state.system_prompt_preset_name, ws_engine
        );
        {
            let cb = state.log_callback.lock().unwrap();
            if let Some(c) = cb.as_ref() {
                c(format!(
                    "API: web_search_enabled={}, preset='{}', engine='{}'",
                    ws_enabled, state.system_prompt_preset_name, ws_engine
                ));
            }
        }

        let result = web_context::build_injected_prompt(
            &state.system_prompt_preset_name,
            &request_json,
            ws_enabled,
            &ws_engine,
            &ws_engine_url,
            &ws_api_key,
            &state.log_callback,
        )
        .await;

        info!(
            "API: web search performed={}, content_len={}",
            result.performed,
            result.content.len()
        );
        {
            let cb = state.log_callback.lock().unwrap();
            if let Some(c) = cb.as_ref() {
                c(format!(
                    "API: web search performed={}, content_len={}",
                    result.performed,
                    result.content.len()
                ));
            }
        }
        if result.performed
            && !result.content.is_empty()
            && let Some(obj) = request_json.as_object_mut()
            && let Some(messages) = obj.get_mut("messages").and_then(|m| m.as_array_mut())
            && let Some(last) = messages.last_mut()
            && let Some(content_val) = last.get_mut("content")
        {
            *content_val = serde_json::Value::String(result.content);
        }

        let modified_body = request_json.clone();

        if let Some(messages) = modified_body.get("messages").and_then(|m| m.as_array()) {
            let last_content = messages
                .last()
                .and_then(|m| m.get("content").and_then(|c| c.as_str()))
                .unwrap_or("");
            info!(
                "Prompt to llama-server: {} messages, last content ({} chars):\n{}",
                messages.len(),
                last_content.len(),
                last_content
            );
        }

        let body_stream = futures_util::stream::once(async move {
            Ok::<Bytes, std::convert::Infallible>(Bytes::from(
                serde_json::to_vec(&modified_body).unwrap_or(body_bytes.to_vec()),
            ))
        });

        let mut request_builder = state.client.post(&url);

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

        let response = handle_response(response, &path).await;
        return response.into_response();
    }

    // Stream request body directly to backend (no drain to memory)
    let body_stream = req
        .into_body()
        .into_data_stream()
        .map(|r| r.map_err(|e| std::io::Error::other(format!("{}", e))));

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

    let response = handle_response(response, &path).await;
    response.into_response()
}

async fn handle_response(
    response: Result<reqwest::Response, reqwest::Error>,
    path: &str,
) -> impl IntoResponse {
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
async fn security_headers(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> impl IntoResponse {
    let mut resp = next.run(req).await;
    resp.headers_mut()
        .entry(axum::http::header::X_CONTENT_TYPE_OPTIONS)
        .or_insert("nosniff".parse().unwrap());
    resp.headers_mut()
        .entry(axum::http::header::X_FRAME_OPTIONS)
        .or_insert("DENY".parse().unwrap());
    resp.headers_mut()
        .entry(axum::http::header::CONTENT_SECURITY_POLICY)
        .or_insert("default-src 'self' blob:; script-src 'self' 'unsafe-inline' https://cdn.jsdelivr.net https://cdnjs.cloudflare.com; style-src 'self' 'unsafe-inline' https://cdnjs.cloudflare.com; img-src 'self' data:; connect-src 'self' ws: wss:".parse().unwrap());
    resp
}

/// Dynamic CORS middleware: validates Origin header against allowed hosts.
/// Allows localhost, 127.0.0.1, and the configured bind host.
async fn cors_middleware(
    State(allowed_origins): State<Arc<Vec<String>>>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> impl IntoResponse {
    let origin = req
        .headers()
        .get(axum::http::header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let allowed = origin
        .as_ref()
        .map(|o| {
            let origin_host = o
                .strip_prefix("http://")
                .or_else(|| o.strip_prefix("https://"))
                .and_then(|u| u.split('/').next())
                .map(|h| h.strip_suffix(':').unwrap_or(h))
                .unwrap_or("");
            allowed_origins.iter().any(|a| a == origin_host)
        })
        .unwrap_or(false);

    if req.method() == axum::http::Method::OPTIONS {
        if allowed {
            let mut resp = axum::response::Response::new(Body::empty());
            resp.headers_mut().insert(
                axum::http::header::ACCESS_CONTROL_ALLOW_ORIGIN,
                origin.unwrap().parse().unwrap(),
            );
            resp.headers_mut().insert(
                axum::http::header::ACCESS_CONTROL_ALLOW_METHODS,
                "GET, POST, PUT, DELETE, OPTIONS".parse().unwrap(),
            );
            resp.headers_mut().insert(
                axum::http::header::ACCESS_CONTROL_ALLOW_HEADERS,
                "Content-Type, Authorization".parse().unwrap(),
            );
            resp
        } else {
            StatusCode::METHOD_NOT_ALLOWED.into_response()
        }
    } else {
        let mut resp = next.run(req).await;
        if allowed {
            if let Some(o) = origin {
                resp.headers_mut().insert(
                    axum::http::header::ACCESS_CONTROL_ALLOW_ORIGIN,
                    o.parse().unwrap(),
                );
                resp.headers_mut().insert(
                    axum::http::header::ACCESS_CONTROL_ALLOW_HEADERS,
                    "Content-Type, Authorization".parse().unwrap(),
                );
            }
        }
        resp
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
            match state
                .client
                .get(format!("{}/models", state.server_url))
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => {
                    let val: Option<serde_json::Value> = resp.json().await.ok();
                    let data = val
                        .as_ref()
                        .and_then(|v| v.get("data"))
                        .and_then(|d| d.as_array());
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
            }
        }
    };

    let metrics = {
        let stale = state.status_cache.read().unwrap().metrics_at.elapsed() >= STATUS_CACHE_TTL;
        if state.server_port == 0 {
            // No backend server port yet — nothing to fetch.
            None
        } else if !stale {
            state.status_cache.read().unwrap().metrics.clone()
        } else {
            let m = crate::backend::server::get_metrics(
                &state.server_host,
                state.server_port,
                Some(&state.model_name),
                Some(state.pid),
            )
            .await
            .ok();
            let mut cache = state.status_cache.write().unwrap();
            cache.metrics = m.clone();
            cache.metrics_at = Instant::now();
            m
        }
    };

    // Drain llama-server log lines (if wired) and parse metrics from them.
    // Log-parsed ctx_used/decoded_tokens/gen_tps are the source of truth when
    // the /metrics API returns 0 (same strategy as TUI tick_metrics).
    // Values expire after STATUS_CACHE_TTL so a stopped server doesn't show stale ctx.
    let mut log_metrics = crate::backend::server_logs::ServerLogMetrics::default();
    let log_stale = state.status_cache.read().unwrap().log_metrics_at.elapsed() >= STATUS_CACHE_TTL;
    if let Some(rx_arc) = &state.log_rx {
        // Seed prev_line from cache so cross-line patterns (e.g. tokens-per-second)
        // keep working across polls, like the TUI tick_server_logs does.
        let mut prev_line = state.status_cache.read().unwrap().log_prev_line.clone();
        let mut any = false;
        let mut rx = rx_arc.lock().unwrap();
        while let Ok(line) = rx.try_recv() {
            let (m, _is_gen) =
                crate::backend::server_logs::parse_log_line(&line, prev_line.as_deref());
            // Merge field-by-field so a line with only ctx_used doesn't clobber gen_tps.
            if m.ctx_used.is_some() {
                log_metrics.ctx_used = m.ctx_used;
                any = true;
            }
            if m.decoded_tokens.is_some() {
                log_metrics.decoded_tokens = m.decoded_tokens;
                any = true;
            }
            if m.gen_tps.is_some() {
                log_metrics.gen_tps = m.gen_tps;
                any = true;
            }
            prev_line = Some(line);
        }
        // Always persist prev_line (even when no line parsed) so cross-line
        // patterns (e.g. tokens-per-second) keep working across polls.
        {
            let mut cache = state.status_cache.write().unwrap();
            if any {
                cache.log_metrics = log_metrics.clone();
                cache.log_metrics_at = Instant::now();
            }
            cache.log_prev_line = prev_line;
        }
        if !any && !log_stale {
            log_metrics = state.status_cache.read().unwrap().log_metrics.clone();
        }
    } else if !log_stale {
        log_metrics = state.status_cache.read().unwrap().log_metrics.clone();
    }

    let metrics_json = metrics.as_ref().map(|m| {
        let ctx_used = log_metrics.ctx_used.unwrap_or(m.ctx_used);
        let decoded_tokens = log_metrics.decoded_tokens.unwrap_or(m.decoded_tokens);
        let gen_tps = log_metrics.gen_tps.unwrap_or(m.gen_tps);
        let ctx_max = if state.effective_ctx > 0 { state.effective_ctx } else { m.ctx_max };
        serde_json::json!({
            "tps": m.tps,
            "prompt_tps": m.prompt_tps,
            "gen_tps": gen_tps,
            "latency_ms": if gen_tps > 0.0 { 1000.0 / gen_tps } else if m.tps > 0.0 { 1000.0 / m.tps } else { 0.0 },
            "latency_per_token_ms": if gen_tps > 0.0 { 1000.0 / gen_tps } else if m.tps > 0.0 { 1000.0 / m.tps } else { 0.0 },
            "ctx_used": ctx_used,
            "ctx_max": ctx_max,
            "gpu_mem_used": m.gpu_mem_used,
            "gpu_mem_total": m.gpu_mem_total,
            "ram_used": m.ram_used,
            "cpu_usage": m.cpu_usage,
            "decoded_tokens": decoded_tokens,
            "prompt_progress": 0.0,
            "prompt_tps_eval": 0.0,
        })
    });

    Json(serde_json::json!({
        "status": "running",
        "pid": state.pid,
        "port": state.port,
        "model": state.model_name,
        "uptime_seconds": uptime_secs,
        "loaded_models": loaded_models,
        "metrics": metrics_json,
        "ws_port": state.ws_port,
        "ws_auth": state.ws_auth,
    }))
}

/// Serve the chat HTML page.
async fn chat_handler() -> impl IntoResponse {
    (
        [(axum::http::header::CACHE_CONTROL, "no-store")],
        axum::response::Html(include_str!("chat.html").to_string()),
    )
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
    system_prompt_preset_name: String,
    web_search_config: Arc<RwLock<WebSearchConfig>>,
    log_callback: Arc<Mutex<Option<Box<dyn Fn(String) + Send + Sync>>>>,
    log_rx: Option<Arc<Mutex<tokio::sync::mpsc::Receiver<String>>>>,
    ws_port: u16,
    ws_auth: Option<String>,
    effective_ctx: u32,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let bind = addr;
    let start_time = Instant::now();
    // No overall request timeout: it applies to the whole response, which
    // would cut off streaming (SSE) completions running longer than the
    // limit. Only bound the connect phase to llama-server.
    let client = Client::builder()
        .pool_max_idle_per_host(20)
        .connect_timeout(std::time::Duration::from_secs(30))
        .build()?;
    let state = ApiState {
        server_url: format!("http://{}:{}", clean_host(&host), server_port),
        server_host: clean_host(&host),
        server_port,
        api_key,
        model_name,
        pid,
        start_time,
        port: bind.port(),
        client,
        status_cache: Arc::new(RwLock::new(StatusCache {
            models: 0,
            cached_at: Instant::now() - std::time::Duration::from_secs(10),
            metrics: None,
            metrics_at: Instant::now() - std::time::Duration::from_secs(10),
            log_metrics: crate::backend::server_logs::ServerLogMetrics::default(),
            log_metrics_at: Instant::now() - std::time::Duration::from_secs(10),
            log_prev_line: None,
        })),
        log_rx,
        system_prompt_preset_name,
        web_search_config,
        log_callback,
        ws_port,
        ws_auth,
        effective_ctx,
    };

    let allowed_origins: Arc<Vec<String>> = Arc::new({
        let mut origins = vec!["127.0.0.1".into(), "localhost".into()];
        if host != "127.0.0.1" && host != "localhost" && host != "0.0.0.0" {
            origins.push(host.clone());
        }
        origins
    });

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
        // /health, /metrics, /chat stay open (no auth);
        // auth applies only to the proxied API routes below.
        .route("/health", get(health))
        .route("/metrics", get(proxy_streaming))
        .route("/chat", get(chat_handler))
        .merge(
            Router::new()
                .route("/v1/chat/completions", post(proxy_streaming))
                .route("/v1/completions", post(proxy_streaming))
                .route("/v1/embeddings", post(proxy_streaming))
                .route("/v1/models", get(proxy_streaming))
                .route("/api/status", get(status))
                .fallback(proxy_streaming)
                .layer(axum::middleware::from_fn_with_state(
                    allowed_origins.clone(),
                    cors_middleware,
                ))
                .layer(axum::middleware::from_fn_with_state(
                    state.clone(),
                    auth_middleware,
                ))
                .layer(TraceLayer::new_for_http()),
        )
        .layer(axum::middleware::from_fn(security_headers))
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
