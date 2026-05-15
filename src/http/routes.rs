use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    http::StatusCode,
};
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::http::app::AppState;
use crate::models::DownloadState;
use crate::models::DownloadStateResponse;
use crate::config::DefaultParams;
use std::sync::Arc;

// ── Request/Response types ───────────────────────────────────

#[derive(Deserialize)]
#[allow(dead_code)]
pub struct SearchQuery {
    pub q: String,
    #[serde(default = "default_sort")]
    pub sort: String,
    #[serde(default)]
    pub offset: u32,
}

fn default_sort() -> String {
    "relevance".to_string()
}

#[derive(Serialize)]
pub struct DownloadResponse {
    pub job_id: String,
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub struct LoadRequest {
    pub model: String,
    #[serde(default)]
    pub settings: Option<crate::models::ModelSettings>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub struct UnloadRequest {
    pub model: String,
}

#[derive(Deserialize)]
pub struct GetLogsQuery {
    #[serde(default = "default_n")]
    pub n: usize,
}

fn default_n() -> usize {
    100
}

/// ── Auth middleware ──────────────────────────────────────────

fn extract_api_key(headers: &axum::http::HeaderMap) -> Option<String> {
    headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.to_string())
}

fn check_auth(state: &AppState, headers: &axum::http::HeaderMap) -> bool {
    match &state.api_key {
        Some(expected) => {
            let provided = extract_api_key(headers);
            match provided {
                Some(key) => key == *expected,
                None => false,
            }
        }
        None => true, // TODO: require auth
    }
}

/// Create the router with all routes
pub fn create_routes(state: AppState) -> axum::Router {
    use axum::routing;

    axum::Router::new()
        .route("/health", routing::get(health))
        .route("/models", routing::get(get_models).post(load_model))
        .route("/models/{id}", routing::get(get_model).post(unload_model))
        .route("/search", routing::get(search_models))
        .route("/models/{model_id}/file/{filename}", routing::post(download_file))
        .route("/downloads/{job_id}", routing::get(get_download_progress).post(cancel_download))
        .route("/metrics", routing::get(get_metrics))
        .route("/logs", routing::get(get_logs))
        .route("/settings", routing::get(get_settings).post(update_settings))
        .route("/profiles", routing::get(get_profiles).post(create_profile))
        .route("/profiles/route", routing::post(route_profile))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            logging_middleware,
        ))
        .with_state(state)
}

async fn logging_middleware(
    State(_state): State<AppState>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();

    // Get client IP from remote address
    let client_ip = req
        .extensions()
        .get::<std::net::SocketAddr>()
        .map(|a| a.to_string())
        .unwrap_or_else(|| "?".to_string());

    let start = std::time::Instant::now();

    let resp = next.run(req).await;
    let status = resp.status().as_u16();
    let elapsed = start.elapsed().as_millis();

    tracing::info!(
        method = %method,
        path,
        status,
        elapsed_ms = elapsed,
        client = %client_ip,
        "{} {} -> {} ({}ms)",
        method, path, status, elapsed,
    );

    resp
}

async fn auth_middleware(
    State(state): State<AppState>,
    mut req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let has_auth = req
        .headers()
        .get("Authorization")
        .is_some();
    let authed = check_auth(&state, req.headers());
    if !authed {
        if has_auth {
            tracing::warn!("Auth failed for client: headers present but key doesn't match");
        }
        let resp = axum::response::Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .header("content-type", "application/json")
            .body(axum::body::Body::from(
                serde_json::to_string(&serde_json::json!({"error": "Unauthorized"})).unwrap_or_default()
            ))
            .unwrap_or_default();
        tracing::info!("Auth denied: 401 (no api key or mismatch)");
        return resp;
    }
    req.extensions_mut().insert(std::net::SocketAddr::new(
        std::net::IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0)),
        0,
    ));
    next.run(req).await
}

// ── Health ────────────────────────────────────────────────────

pub async fn health(State(state): State<AppState>) -> impl IntoResponse {
    let ss = state.server_state.read().unwrap();
    let status = match *ss {
        crate::http::ServerState::Running { .. } => "running",
        crate::http::ServerState::Starting => "starting",
        crate::http::ServerState::Error { .. } => "error",
        crate::http::ServerState::Stopped => "stopped",
    };
    (
        StatusCode::OK,
        Json(serde_json::to_value(serde_json::json!({
            "status": status,
            "server": state.server_uuid,
        })).unwrap_or_default()),
    )
}

// ── Models ────────────────────────────────────────────────────

pub async fn get_models(State(state): State<AppState>) -> impl IntoResponse {
    let models = state.models.lock().unwrap();
    let names: Vec<String> = models.iter().map(|m| m.name.clone()).collect();
    (StatusCode::OK, Json(names))
}

pub async fn get_model(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    let models = state.models.lock().unwrap();
    match models.iter().find(|m| m.name == id) {
        Some(m) => {
            let v = serde_json::to_value(m).unwrap_or_default();
            (StatusCode::OK, Json(v))
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::to_value(serde_json::json!({"error": "Model not found"})).unwrap_or_default()),
        ),
    }
}

pub async fn load_model(State(_state): State<AppState>, Json(req): Json<LoadRequest>) -> impl IntoResponse {
    // TODO: load model via HTTP server
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(serde_json::to_value(serde_json::json!({
            "status": "not implemented",
            "model": req.model,
        })).unwrap_or_default()),
    )
}

pub async fn unload_model(State(_state): State<AppState>, Json(req): Json<UnloadRequest>) -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(serde_json::to_value(serde_json::json!({
            "status": "unloaded",
            "model": req.model,
        })).unwrap_or_default()),
    )
}

// ── Search ────────────────────────────────────────────────────

pub async fn search_models(Query(q): Query<SearchQuery>) -> impl IntoResponse {
    let results: Vec<_> = match crate::backend::hub::search_models(&q.q, 70, q.offset).await {
        Ok((r, _total)) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::to_value(serde_json::json!({"error": e.to_string()})).unwrap_or_default()),
            );
        }
    };
    let list: Vec<_> = results.iter().map(|r| serde_json::to_value(r).unwrap_or_default()).collect();
    (StatusCode::OK, Json(serde_json::Value::Array(list)))
}

// ── Download ──────────────────────────────────────────────────

pub async fn download_file(
    State(state): State<AppState>,
    Path((model_id, filename)): Path<(String, String)>,
) -> impl IntoResponse {
    // Get model files to find the one we want
    let files = match crate::backend::hub::list_gguf_files(&model_id).await {
        Ok(f) => f,
        Err(e) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::to_value(serde_json::json!({"error": e.to_string()})).unwrap_or_default()),
            );
        }
    };

    let file_entry = match files.iter().find(|(f, _, _)| f == &filename) {
        Some(f) => f.clone(),
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::to_value(serde_json::json!({"error": "File not found"})).unwrap_or_default()),
            );
        }
    };

    let job_id = Uuid::new_v4().to_string();
    let (tx, _rx) = tokio::sync::broadcast::channel(10);
    let dest = state.config.models_dir.join(&filename);
    let state_entry = DownloadState::new(model_id.clone(), filename.clone(), file_entry.1);

    {
        let mut downloads = state.downloads.write().unwrap();
        downloads.insert(job_id.clone(), state_entry.clone());
    }

    let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let cancelled_clone = cancelled.clone();
    let download_state_ref = state.downloads.clone();
    let job_id_ref = job_id.clone();

    let filename_clone = filename.clone();
    let dest_clone = dest.clone();
    let model_id_clone = model_id.clone();

    tokio::spawn(async move {
        let mut state = state_entry;
        state.cancel_token = Some(cancelled_clone.clone());
        let result = crate::backend::hub::download_file(
            &model_id_clone, &filename_clone, &file_entry.2,
            &dest_clone, &mut state, cancelled_clone, tx,
        ).await;
        if let Err(e) = result {
            state.status = crate::models::DownloadStatus::Error(e.to_string());
        }
        let mut downloads = download_state_ref.write().unwrap();
        downloads.insert(job_id_ref, state);
    });

    (
        StatusCode::OK,
        Json(serde_json::to_value(DownloadResponse { job_id }).unwrap_or_default()),
    )
}

pub async fn get_download_progress(
    State(state): State<AppState>,
    Path(job_id): Path<String>,
) -> impl IntoResponse {
    let downloads = state.downloads.read().unwrap();
    match downloads.get(&job_id) {
        Some(d) => {
            let resp = DownloadStateResponse {
                model_id: d.model_id.clone(),
                filename: d.filename.clone(),
                total_bytes: d.total_bytes,
                downloaded_bytes: d.downloaded_bytes,
                status: d.status.clone(),
                cancelled: d.cancelled,
                bytes_per_second: d.bytes_per_second,
            };
            let v = serde_json::to_value(resp).unwrap_or_default();
            (StatusCode::OK, Json(v))
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::to_value(serde_json::json!({"error": "Download not found"})).unwrap_or_default()),
        ),
    }
}

pub async fn cancel_download(
    State(state): State<AppState>,
    Path(job_id): Path<String>,
) -> impl IntoResponse {
    let mut downloads = state.downloads.write().unwrap();
    if let Some(d) = downloads.get_mut(&job_id) {
        if let Some(token) = &d.cancel_token {
            token.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        d.cancelled = true;
        d.status = crate::models::DownloadStatus::Error("Cancelled".into());
    }
    (
        StatusCode::OK,
        Json(serde_json::to_value(serde_json::json!({
            "status": "cancelled",
            "job_id": job_id,
        })).unwrap_or_default()),
    )
}

// ── Metrics ───────────────────────────────────────────────────

pub async fn get_metrics(State(state): State<AppState>) -> impl IntoResponse {
    let metrics = state.metrics.read().unwrap().clone();
    let v = serde_json::to_value(metrics).unwrap_or_default();
    (StatusCode::OK, Json(v))
}

// ── Logs ──────────────────────────────────────────────────────

pub async fn get_logs(
    State(state): State<AppState>,
    Query(q): Query<GetLogsQuery>,
) -> impl IntoResponse {
    let logs = state.log_entries.lock().unwrap();
    let vec: Vec<String> = logs.iter().rev().take(q.n).cloned().collect();
    // Add a log entry for the request
    {
        let mut logs = state.log_entries.lock().unwrap();
        logs.push_back(format!("[{}] Client accessed logs ({} entries)", chrono::Local::now().format("%H:%M:%S"), vec.len()));
    }
    (StatusCode::OK, Json(vec))
}

// ── Settings ──────────────────────────────────────────────────

pub async fn get_settings(State(state): State<AppState>) -> impl IntoResponse {
    let settings = state.settings.read().unwrap();
    let v = serde_json::to_value(settings.clone()).unwrap_or_default();
    (StatusCode::OK, Json(v))
}

pub async fn update_settings(
    State(state): State<AppState>,
    Json(settings): Json<DefaultParams>,
) -> impl IntoResponse {
    let mut s = state.settings.write().unwrap();
    *s = settings.into();
    (
        StatusCode::OK,
        Json(serde_json::to_value(serde_json::json!({"status": "updated"})).unwrap_or_default()),
    )
}

// ── Profiles ──────────────────────────────────────────────────

#[derive(Serialize)]
pub struct ProfileItem {
    #[serde(rename = "profileId")]
    pub profile_id: String,
    pub name: String,
    pub description: String,
    pub settings: Option<crate::models::ModelSettings>,
}

pub async fn get_profiles(State(state): State<AppState>) -> impl IntoResponse {
    let profiles: Vec<ProfileItem> = state
        .config
        .profiles
        .iter()
        .map(|p| ProfileItem {
            profile_id: p.name.clone(),
            name: p.name.clone(),
            description: p.description.clone(),
            settings: None,
        })
        .collect();
    (StatusCode::OK, Json(profiles))
}

pub async fn create_profile(
    _state: State<AppState>,
    Json(_profile): Json<crate::config::Profile>,
) -> impl IntoResponse {
    // TODO: store profile
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(serde_json::to_value(serde_json::json!({
            "status": "not implemented",
        })).unwrap_or_default()),
    )
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub struct RouteProfileRequest {
    pub profile_name: String,
}

pub async fn route_profile(
    State(state): State<AppState>,
    Json(req): Json<RouteProfileRequest>,
) -> impl IntoResponse {
    let profile_name = &req.profile_name;
    if let Some(profile) = state.config.profiles.iter().find(|p| p.name == *profile_name) {
        // TODO: Apply profile settings to active model
        let _ = profile;
    }
    (
        StatusCode::OK,
        Json(serde_json::to_value(serde_json::json!({
            "status": "routed",
            "profile": profile_name,
        })).unwrap_or_default()),
    )
}
