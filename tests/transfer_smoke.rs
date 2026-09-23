//! Integration smoke test for the file-transfer API (/api/transfer/...):
//! boots the API server with a dead backend and exercises upload/list/files,
//! auth, traversal rejection, and the disabled-by-default behavior.
use std::io::Write;
use std::sync::Arc;

use llm_manager::serve_api::{WebSearchConfig, start_api_server};

fn make_tar_gz() -> Vec<u8> {
    use flate2::Compression;
    use flate2::write::GzEncoder;
    let mut tar = tar::Builder::new(Vec::new());
    let content = b"fn main() { println!(\"hello\"); }\n";
    let mut headers = tar::Header::new_gnu();
    headers.set_size(content.len() as u64);
    headers.set_mode(0o644);
    headers.set_cksum();
    tar.append_data(&mut headers, "src/main.rs", &content[..])
        .unwrap();
    // a directory entry
    let mut dir_headers = tar::Header::new_gnu();
    dir_headers.set_size(0);
    dir_headers.set_entry_type(tar::EntryType::Directory);
    dir_headers.set_mode(0o755);
    dir_headers.set_cksum();
    tar.append_data(&mut dir_headers, "docs/", &b""[..])
        .unwrap();
    let tar_bytes = tar.into_inner().unwrap();
    let mut gz = GzEncoder::new(Vec::new(), Compression::default());
    gz.write_all(&tar_bytes).unwrap();
    gz.finish().unwrap()
}

async fn boot(
    transfer_enabled: bool,
    backend_port: u16,
) -> (
    std::net::SocketAddr,
    tokio::sync::watch::Sender<bool>,
    tokio::task::JoinHandle<()>,
) {
    let port = {
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        l.local_addr().unwrap().port()
    };
    let addr: std::net::SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    let (tx, rx) = tokio::sync::watch::channel(false);
    let handle = tokio::spawn(async move {
        let _ = start_api_server(
            addr,
            Some("secret-key".to_string()),
            backend_port,
            "test-model".to_string(),
            1,
            rx,
            "127.0.0.1".to_string(),
            None,
            String::new(),
            Arc::new(std::sync::RwLock::new(WebSearchConfig {
                engine: String::new(),
                engine_url: String::new(),
                enabled: false,
                api_key: None,
            })),
            Arc::new(std::sync::Mutex::new(None)),
            None,
            0,
            None,
            0,
            false,
            transfer_enabled,
        )
        .await;
    });
    // wait for the listener, then give axum a moment to start accepting
    for _ in 0..100 {
        if tokio::net::TcpStream::connect(addr).await.is_ok() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    (addr, tx, handle)
}

#[tokio::test]
async fn transfer_api_smoke() {
    let tmp = std::env::temp_dir().join(format!(
        "llm-mgr-transfer-test-{}-smoke",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&tmp);
    unsafe { std::env::set_var("LLM_MANAGER_TRANSFER_DIR", &tmp) };
    let (addr, _tx, handle) = boot(true, 59999).await;
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();

    // No auth -> 401
    let r = client
        .get(format!("{base}/api/transfer/list"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 401, "expected 401 without API key");

    let auth = reqwest::header::HeaderValue::from_static("Bearer secret-key");

    // Upload
    let body = make_tar_gz();
    let r = client
        .post(format!("{base}/api/transfer/upload?label=smoke"))
        .header(reqwest::header::AUTHORIZATION, auth.clone())
        .header(reqwest::header::CONTENT_TYPE, "application/gzip")
        .body(body)
        .send()
        .await
        .unwrap();
    assert_eq!(
        r.status(),
        200,
        "upload failed: {}",
        r.text().await.unwrap()
    );
    let v: serde_json::Value = r.json().await.unwrap();
    let id = v["id"].as_str().unwrap().to_string();
    assert!(id.ends_with("-smoke"), "id {id}");
    let paths: Vec<&str> = v["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["path"].as_str().unwrap())
        .collect();
    assert!(paths.contains(&"src/main.rs"), "files: {paths:?}");
    let dest = v["path"].as_str().unwrap().to_string();
    assert!(
        std::path::Path::new(&dest).join("src/main.rs").exists(),
        "extracted file missing at {dest}"
    );

    // List
    let r = client
        .get(format!("{base}/api/transfer/list"))
        .header(reqwest::header::AUTHORIZATION, auth.clone())
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let v: serde_json::Value = r.json().await.unwrap();
    assert!(
        v["transfers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["id"] == id),
        "list: {v}"
    );

    // Files
    let r = client
        .get(format!("{base}/api/transfer/{id}/files"))
        .header(reqwest::header::AUTHORIZATION, auth.clone())
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let v: serde_json::Value = r.json().await.unwrap();
    assert!(v["files"].as_array().unwrap().len() >= 1);

    // Bad id rejected
    let r = client
        .get(format!("{base}/api/transfer/..%2F..%2Fetc/files"))
        .header(reqwest::header::AUTHORIZATION, auth.clone())
        .send()
        .await
        .unwrap();
    assert!(
        r.status() == 400 || r.status() == 404,
        "traversal: {}",
        r.status()
    );

    // Malformed archive rejected
    let r = client
        .post(format!("{base}/api/transfer/upload?label=bad"))
        .header(reqwest::header::AUTHORIZATION, auth.clone())
        .body("not a tarball".to_string())
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 400, "malformed archive should be 400");

    // Cleanup + shutdown
    let _ = std::fs::remove_dir_all(&dest);
    let _ = std::fs::remove_dir_all(&tmp);
    handle.abort();
}

#[tokio::test]
async fn transfer_api_disabled() {
    let tmp = std::env::temp_dir().join(format!(
        "llm-mgr-transfer-test-{}-disabled",
        std::process::id()
    ));
    unsafe { std::env::set_var("LLM_MANAGER_TRANSFER_DIR", &tmp) };
    let (addr, _tx, handle) = boot(false, 59999).await;
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let auth = reqwest::header::HeaderValue::from_static("Bearer secret-key");

    // Route not registered -> falls through to proxy (dead backend) -> not 200
    let r = client
        .get(format!("{base}/api/transfer/list"))
        .header(reqwest::header::AUTHORIZATION, auth)
        .send()
        .await
        .unwrap();
    assert_ne!(r.status(), 200, "transfer API should be off by default");
    handle.abort();
}

/// Fake llama-server: scripted replies — first call asks for a file,
/// second call gives the final answer. Captures the forwarded bodies so
/// the test can verify the tool result was injected.
async fn boot_fake_backend() -> (u16, Arc<std::sync::Mutex<Vec<String>>>) {
    let bodies: Arc<std::sync::Mutex<Vec<String>>> = Arc::new(std::sync::Mutex::new(Vec::new()));
    let b2 = bodies.clone();
    let app = axum::Router::new().route(
        "/v1/chat/completions",
        axum::routing::post(move |bytes: axum::body::Bytes| {
            let b2 = b2.clone();
            async move {
                let mut guard = b2.lock().unwrap();
                guard.push(String::from_utf8_lossy(&bytes).to_string());
                let content = if guard.len() == 1 {
                    "READ src/main.rs"
                } else {
                    "FINAL: reviewed src/main.rs, no bugs found"
                };
                axum::Json(serde_json::json!({
                    "id": "c",
                    "object": "chat.completion",
                    "choices": [{
                        "index": 0,
                        "message": { "role": "assistant", "content": content },
                        "finish_reason": "stop"
                    }]
                }))
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    (port, bodies)
}

#[tokio::test]
async fn agent_run_smoke() {
    let tmp = std::env::temp_dir().join(format!(
        "llm-mgr-transfer-test-{}-agent",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&tmp);
    unsafe { std::env::set_var("LLM_MANAGER_TRANSFER_DIR", &tmp) };
    let (backend_port, bodies) = boot_fake_backend().await;
    let (addr, _tx, handle) = boot(true, backend_port).await;
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let auth = reqwest::header::HeaderValue::from_static("Bearer secret-key");

    // Upload a tarball
    let body = make_tar_gz();
    let r = client
        .post(format!("{base}/api/transfer/upload?label=agent"))
        .header(reqwest::header::AUTHORIZATION, auth.clone())
        .body(body)
        .send()
        .await
        .unwrap();
    assert_eq!(
        r.status(),
        200,
        "upload failed: {}",
        r.text().await.unwrap()
    );
    let v: serde_json::Value = r.json().await.unwrap();
    let id = v["id"].as_str().unwrap().to_string();
    let dest = v["path"].as_str().unwrap().to_string();

    // Unknown transfer id -> 400
    let r = client
        .post(format!("{base}/api/agent/run"))
        .header(reqwest::header::AUTHORIZATION, auth.clone())
        .json(&serde_json::json!({
            "transfer_id": "nope",
            "task": "review"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        r.status(),
        400,
        "unknown transfer should be 400: {}",
        r.text().await.unwrap()
    );

    // Agent run: fake model reads the file, then answers
    let r = client
        .post(format!("{base}/api/agent/run"))
        .header(reqwest::header::AUTHORIZATION, auth.clone())
        .json(&serde_json::json!({
            "transfer_id": id,
            "task": "review this code"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        r.status(),
        200,
        "agent run failed: {}",
        r.text().await.unwrap()
    );
    let v: serde_json::Value = r.json().await.unwrap();
    assert!(
        v["answer"].as_str().unwrap().contains("no bugs found"),
        "answer: {v}"
    );
    assert_eq!(v["rounds"].as_u64().unwrap(), 2, "answer: {v}");
    assert!(
        v["files_read"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f == "src/main.rs"),
        "files_read: {v}"
    );

    // The second LLM call must contain the tool result (file content)
    let sent = bodies.lock().unwrap();
    assert!(sent.len() >= 2, "expected 2 LLM calls, got {}", sent.len());
    assert!(
        sent[1].contains("fn main()"),
        "tool result not injected into second call: {}",
        &sent[1][..sent[1].len().min(500)]
    );

    // Cleanup + shutdown
    let _ = std::fs::remove_dir_all(&dest);
    let _ = std::fs::remove_dir_all(&tmp);
    handle.abort();
}
