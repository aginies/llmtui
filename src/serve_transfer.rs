//! File-transfer API: authenticated push of tar.gz archives between servers.
//!
//! Lets one llm-manager server send files/directories to another (e.g. for
//! code review by the remote model) without going through the prompt.
//! All routes sit behind the same Bearer API-key auth as the proxy routes.
//!
//! Endpoints:
//!   POST /api/transfer/upload?label=...   upload a tar.gz, extracted to the
//!                                         transfer dir; returns id + file list
//!   GET  /api/transfer/list               list received transfers
//!   GET  /api/transfer/{id}/files         list files in one transfer

use std::path::{Component, Path, PathBuf};

use axum::Json;
use axum::extract::{Path as PathParam, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::io::AsyncWriteExt;
use tracing::{info, warn};

use crate::serve_api::ApiState;

/// Hard cap on upload size (2 GiB).
const MAX_UPLOAD_SIZE: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Deserialize)]
pub struct UploadQuery {
    label: Option<String>,
}

fn err(status: StatusCode, msg: &str) -> Response {
    (status, Json(json!({ "error": msg }))).into_response()
}

fn sanitize_label(label: &str) -> String {
    let clean: String = label
        .chars()
        .take(40)
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_' || *c == '.')
        .collect();
    if clean.is_empty() {
        "transfer".to_string()
    } else {
        clean
    }
}

/// Recursively list regular files under `root` as `{"path", "size"}` objects
/// with paths relative to `root`.
pub(crate) fn walk_files(root: &Path) -> Result<Vec<Value>, String> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let rd = std::fs::read_dir(&dir).map_err(|e| e.to_string())?;
        for entry in rd.flatten() {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                let size = entry.metadata().map_err(|e| e.to_string())?.len();
                let rel = p
                    .strip_prefix(root)
                    .unwrap_or(&p)
                    .to_string_lossy()
                    .to_string();
                out.push(json!({ "path": rel, "size": size }));
            }
        }
    }
    out.sort_by(|a, b| {
        a["path"]
            .as_str()
            .unwrap_or_default()
            .cmp(b["path"].as_str().unwrap_or_default())
    });
    Ok(out)
}

/// Extract a tar.gz archive into `dest`, rejecting absolute paths, `..`
/// components, and symlink/hardlink entries. Returns the extracted file list.
fn extract(archive: &Path, dest: &Path) -> Result<Vec<Value>, String> {
    let file = std::fs::File::open(archive).map_err(|e| e.to_string())?;
    let gz = flate2::read::GzDecoder::new(file);
    let mut ar = tar::Archive::new(gz);
    std::fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    for entry in ar.entries().map_err(|e| e.to_string())? {
        let mut entry = entry.map_err(|e| e.to_string())?;
        let p = entry.path().map_err(|e| e.to_string())?.to_path_buf();
        if p.is_absolute() {
            return Err("archive contains an absolute path".into());
        }
        if p.components().any(|c| matches!(c, Component::ParentDir)) {
            return Err("archive contains a parent-dir component".into());
        }
        let et = entry.header().entry_type();
        if et.is_symlink() || et.is_hard_link() {
            continue;
        }
        let p_display = p.display().to_string();
        entry
            .unpack_in(dest)
            .map_err(|e| format!("failed to extract {p_display}: {e}"))?;
    }
    walk_files(dest)
}

/// POST /api/transfer/upload?label=...
/// Body: raw tar.gz stream. Extracted to `<transfer_dir>/<timestamp>-<label>/`.
pub async fn upload(
    State(state): State<ApiState>,
    Query(q): Query<UploadQuery>,
    body: axum::body::Body,
) -> Response {
    let label = sanitize_label(q.label.as_deref().unwrap_or_default());
    let dir = state.transfer_dir.clone();
    // Two uploads with the same label in the same second would collide —
    // append a counter until the id is unique.
    let base_id = format!("{}-{}", chrono::Local::now().format("%Y%m%d-%H%M%S"), label);
    let mut id = base_id.clone();
    let mut n = 1;
    while dir.join(&id).exists() || dir.join(format!(".incoming-{id}")).exists() {
        id = format!("{base_id}-{n}");
        n += 1;
    }

    if let Err(e) = tokio::fs::create_dir_all(&dir).await {
        return err(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("cannot create transfer dir: {e}"),
        );
    }

    // Stream the body to a temp file with a hard size cap.
    let tmp = dir.join(format!(".incoming-{id}"));
    let mut file = match tokio::fs::File::create(&tmp).await {
        Ok(f) => f,
        Err(e) => {
            return err(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("cannot create temp file: {e}"),
            );
        }
    };
    let mut stream = body.into_data_stream();
    let mut size: u64 = 0;
    while let Some(chunk) = stream.next().await {
        let chunk = match chunk {
            Ok(c) => c,
            Err(_) => {
                let _ = tokio::fs::remove_file(&tmp).await;
                return err(StatusCode::BAD_REQUEST, "bad request body");
            }
        };
        size += chunk.len() as u64;
        if size > MAX_UPLOAD_SIZE {
            let _ = tokio::fs::remove_file(&tmp).await;
            return err(
                StatusCode::PAYLOAD_TOO_LARGE,
                &format!("upload exceeds the {MAX_UPLOAD_SIZE}-byte limit"),
            );
        }
        if let Err(e) = file.write_all(&chunk).await {
            let _ = tokio::fs::remove_file(&tmp).await;
            return err(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("write failed: {e}"),
            );
        }
    }

    // Extract (CPU-bound) off the async runtime.
    let dest = dir.join(&id);
    let dest_str = dest.to_string_lossy().to_string();
    let tmp_cleanup = tmp.clone();
    let dest_cleanup = dest.clone();
    let extract_result = tokio::task::spawn_blocking(move || extract(&tmp, &dest))
        .await
        .unwrap_or_else(|e| Err(format!("extraction task failed: {e}")));
    let files = match extract_result {
        Ok(f) => f,
        Err(e) => {
            warn!("transfer upload rejected: {e}");
            let _ = tokio::fs::remove_file(&tmp_cleanup).await;
            let _ = tokio::fs::remove_dir_all(&dest_cleanup).await;
            return err(StatusCode::BAD_REQUEST, &e);
        }
    };
    let _ = tokio::fs::remove_file(&tmp_cleanup).await;

    info!(
        "transfer received: {id} ({} files, {size} bytes) at {dest_str}",
        files.len()
    );
    Json(json!({
        "id": id,
        "path": dest_str,
        "files": files,
    }))
    .into_response()
}

/// GET /api/transfer/list
pub async fn list(State(state): State<ApiState>) -> Response {
    let dir = state.transfer_dir.clone();
    let transfers: Vec<Value> = match std::fs::read_dir(&dir) {
        Ok(rd) => rd
            .flatten()
            .filter_map(|e| {
                let p = e.path();
                if !p.is_dir() {
                    return None;
                }
                let files = walk_files(&p).ok()?;
                let total: u64 = files.iter().map(|f| f["size"].as_u64().unwrap_or(0)).sum();
                Some(json!({
                    "id": p.file_name()?.to_string_lossy(),
                    "files": files.len(),
                    "size": total,
                }))
            })
            .collect(),
        Err(_) => Vec::new(),
    };
    Json(json!({ "transfers": transfers })).into_response()
}

/// Validate a transfer id and resolve its directory under `transfer_dir`.
pub fn resolve_transfer(transfer_dir: &Path, id: &str) -> Result<PathBuf, String> {
    if id.is_empty()
        || id.contains("..")
        || id.contains('/')
        || id.contains('\\')
        || id.starts_with('.')
    {
        return Err("invalid transfer id".into());
    }
    let p = transfer_dir.join(id);
    if !p.is_dir() {
        return Err(format!("transfer '{id}' not found"));
    }
    Ok(p)
}

/// GET /api/transfer/{id}/files
pub async fn files(State(state): State<ApiState>, PathParam(id): PathParam<String>) -> Response {
    if id.is_empty()
        || id.contains("..")
        || id.contains('/')
        || id.contains('\\')
        || id.starts_with('.')
    {
        return err(StatusCode::BAD_REQUEST, "invalid transfer id");
    }
    let p: PathBuf = state.transfer_dir.join(&id);
    if !p.is_dir() {
        return err(StatusCode::NOT_FOUND, "transfer not found");
    }
    match walk_files(&p) {
        Ok(files) => Json(json!({
            "id": id,
            "path": p.to_string_lossy(),
            "files": files,
        }))
        .into_response(),
        Err(e) => err(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("failed to list files: {e}"),
        ),
    }
}
