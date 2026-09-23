//! Small review agent: runs the local llama.cpp model in a tool loop,
//! giving it file tools scoped to a transfer directory.
//!
//! The remote side uploads a tarball via the transfer API, then calls
//! POST /api/agent/run with the transfer id and a task. The prompt carries
//! only the task — the agent lets the model page through the extracted
//! files (LIST / READ / GREP) and returns its final answer, so there is no
//! prompt-size wall.
//!
//! Endpoint (behind the same Bearer auth as the proxy routes, registered
//! only when the transfer API is enabled):
//!   POST /api/agent/run  {"transfer_id": "...", "task": "...", "max_rounds": 20}

use std::path::{Component, Path, PathBuf};

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use regex::Regex;
use serde::Deserialize;
use serde_json::{Value, json};
use tracing::{info, warn};

use crate::serve_api::ApiState;
use crate::serve_transfer::{resolve_transfer, walk_files};

/// Default / hard cap on tool-loop rounds.
const DEFAULT_MAX_ROUNDS: usize = 20;
const MAX_ROUNDS_HARD: usize = 50;
/// Per-read cap and total tool-output budget per run.
const MAX_READ_BYTES: usize = 200 * 1024;
const MAX_INJECTED_BYTES: usize = 1024 * 1024;
/// Max grep hits returned to the model.
const MAX_GREP_HITS: usize = 50;

#[derive(Deserialize)]
pub struct AgentRunBody {
    transfer_id: String,
    task: String,
    #[serde(default)]
    max_rounds: Option<usize>,
}

fn err(status: StatusCode, msg: &str) -> Response {
    (status, Json(json!({ "error": msg }))).into_response()
}

/// Confine `rel` to `root`: reject absolute paths and `..` components.
fn safe_join(root: &Path, rel: &str) -> Result<PathBuf, String> {
    let rel = rel.trim();
    if rel.is_empty() {
        return Ok(root.to_path_buf());
    }
    let p = Path::new(rel);
    if p.is_absolute() || p.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err("path escapes the transfer directory".into());
    }
    Ok(root.join(p))
}

/// File manifest (relative path + size, one per line) for a directory tree.
fn manifest(root: &Path) -> Result<String, String> {
    let files = walk_files(root)?;
    let mut out = String::new();
    for f in &files {
        out.push_str(&format!(
            "{}  ({})\n",
            f["path"].as_str().unwrap_or_default(),
            f["size"].as_u64().unwrap_or(0)
        ));
    }
    Ok(out)
}

fn tool_list(root: &Path, arg: &str) -> Result<String, String> {
    let dir = safe_join(root, arg)?;
    if !dir.is_dir() {
        return Err(format!("not a directory: {arg}"));
    }
    let m = manifest(&dir)?;
    Ok(if m.is_empty() { "(empty)".into() } else { m })
}

fn tool_read(root: &Path, arg: &str) -> Result<String, String> {
    let p = safe_join(root, arg)?;
    let bytes = std::fs::read(&p).map_err(|e| format!("cannot read {arg}: {e}"))?;
    if bytes.iter().take(8192).any(|&b| b == 0) {
        return Ok(format!("[binary file, {} bytes — not shown]", bytes.len()));
    }
    let text = String::from_utf8_lossy(&bytes);
    if text.len() > MAX_READ_BYTES {
        let cut = text.floor_char_boundary(MAX_READ_BYTES);
        Ok(format!(
            "{}\n... [truncated, {} bytes total]",
            &text[..cut],
            text.len()
        ))
    } else {
        Ok(text.to_string())
    }
}

fn tool_grep(root: &Path, arg: &str) -> Result<String, String> {
    let re = Regex::new(arg).map_err(|e| format!("invalid regex: {e}"))?;
    let files = walk_files(root)?;
    let mut hits: Vec<String> = Vec::new();
    for f in &files {
        let rel = f["path"].as_str().unwrap_or_default().to_string();
        let Ok(bytes) = std::fs::read(root.join(&rel)) else {
            continue;
        };
        if bytes.iter().take(8192).any(|&b| b == 0) {
            continue;
        }
        let text = String::from_utf8_lossy(&bytes);
        for (i, line) in text.lines().enumerate() {
            if re.is_match(line) {
                hits.push(format!("{rel}:{}: {}", i + 1, line.trim()));
                if hits.len() >= MAX_GREP_HITS {
                    return Ok(hits.join("\n"));
                }
            }
        }
    }
    Ok(if hits.is_empty() {
        "(no matches)".into()
    } else {
        hits.join("\n")
    })
}

/// One non-streaming chat completion against the local llama-server.
async fn call_llm(state: &ApiState, messages: &[Value]) -> Result<String, String> {
    let url = format!("{}/v1/chat/completions", state.server_url);
    let body = json!({
        "model": state.model_name,
        "messages": messages,
        "stream": false,
        "temperature": 0.2,
    });
    let resp = state
        .client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("llama-server request failed: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("llama-server returned {status}: {text}"));
    }
    let v: Value = resp.json().await.map_err(|e| e.to_string())?;
    v.pointer("/choices/0/message/content")
        .and_then(|c| c.as_str())
        .map(str::to_string)
        .ok_or_else(|| "no content in llama-server response".into())
}

/// POST /api/agent/run — run the tool loop and return the final answer.
pub async fn run(State(state): State<ApiState>, Json(body): Json<AgentRunBody>) -> Response {
    let root = match resolve_transfer(&state.transfer_dir, &body.transfer_id) {
        Ok(r) => r,
        Err(e) => return err(StatusCode::BAD_REQUEST, &e),
    };
    let max_rounds = body
        .max_rounds
        .unwrap_or(DEFAULT_MAX_ROUNDS)
        .min(MAX_ROUNDS_HARD);
    // Reject empty transfers, but do NOT embed the manifest in the prompt —
    // the model lists the files itself (LIST) and reads what it needs.
    let file_count = match walk_files(&root) {
        Ok(files) => files.len(),
        Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, &e),
    };
    if file_count == 0 {
        return err(StatusCode::BAD_REQUEST, "transfer contains no files");
    }

    let system = format!(
        "You are a code-review agent running on the machine where the files are stored.\n\
         Project root: {root}\n\n\
         Keep any internal thinking brief (a few sentences at most) — get straight to the \
         command or FINAL answer; do not deliberate at length.\n\n\
         Inspect the project with exactly ONE command per reply, on the first line:\n\
           LIST [subdir]         list files with sizes (whole project or a subdirectory)\n\
           READ <relative/path>  show one file's content\n\
           GREP <regex>          search all files; returns path:line: text\n\n\
         Start with LIST, then READ or GREP the files relevant to the task.\n\
         When you have enough information, reply with your final answer: the first \
         line starts with \"FINAL:\" followed by the answer (the answer may span \
         multiple lines).",
        root = root.display()
    );

    let mut messages: Vec<Value> = vec![
        json!({ "role": "system", "content": system }),
        json!({
            "role": "user",
            "content": format!(
                "Task: {}\n\nRead the files you need from the project directory (start with LIST).",
                body.task
            ),
        }),
    ];

    let mut injected = 0usize;
    let mut files_read: Vec<String> = Vec::new();

    for round in 1..=max_rounds {
        let reply = match call_llm(&state, &messages).await {
            Ok(r) => r,
            Err(e) => {
                warn!("agent run failed at round {round}: {e}");
                return err(StatusCode::BAD_GATEWAY, &e);
            }
        };
        let first_line = reply.lines().find(|l| !l.trim().is_empty()).unwrap_or("");

        if first_line.starts_with("FINAL:") {
            let answer = reply.trim().to_string();
            info!(
                "agent run finished: {} round(s), {} file(s) read",
                round,
                files_read.len()
            );
            return Json(json!({
                "answer": answer,
                "rounds": round,
                "files_read": files_read,
            }))
            .into_response();
        }

        let tool_result: Result<String, String> = if let Some(arg) =
            first_line.strip_prefix("READ ")
        {
            let arg = arg.trim().to_string();
            match tool_read(&root, &arg) {
                Ok(out) => {
                    files_read.push(arg.clone());
                    Ok(format!("--- {arg} ---\n{out}"))
                }
                Err(e) => Err(e),
            }
        } else if first_line == "LIST" || first_line.starts_with("LIST ") {
            let arg = first_line.strip_prefix("LIST").unwrap_or("").trim();
            tool_list(&root, arg)
        } else if let Some(arg) = first_line.strip_prefix("GREP ") {
            tool_grep(&root, arg.trim())
        } else {
            // Neither a command nor FINAL — nudge the model back on track.
            messages.push(json!({
                    "role": "user",
                    "content": "Reply with exactly one command line (LIST, READ, GREP) or start with FINAL:",
                }));
            continue;
        };

        // Tool errors go back to the model so it can self-correct.
        let out = match tool_result {
            Ok(o) => o,
            Err(e) => format!("ERROR: {e}"),
        };
        if injected + out.len() > MAX_INJECTED_BYTES {
            return err(
                StatusCode::BAD_REQUEST,
                &format!("tool output budget of {MAX_INJECTED_BYTES} bytes exceeded"),
            );
        }
        injected += out.len();
        messages.push(json!({
            "role": "user",
            "content": format!("<tool_result>\n{out}\n</tool_result>"),
        }));
    }

    err(
        StatusCode::INTERNAL_SERVER_ERROR,
        &format!("agent did not finish within {max_rounds} rounds"),
    )
}
