use anyhow::Result;
use futures_util::StreamExt;
use tokio::io::AsyncWriteExt;

/// Search models on HuggingFace.
///
/// `limit` is the number of results per page (default 70, max 200).
/// `offset` is the number of results to skip (for pagination).
pub async fn search_models(query: &str, limit: u32, offset: u32) -> Result<(Vec<crate::models::SearchResult>, usize)> {
    let url = format!(
        "https://huggingface.co/api/models?search={}&limit={}&offset={}&filter=gguf",
        urlencoding::encode(query),
        limit,
        offset
    );

    let resp = reqwest::get(&url).await?.error_for_status()?;
    let models: Vec<serde_json::Value> = resp.json().await?;

    let mut results: Vec<crate::models::SearchResult> = models
        .into_iter()
        .filter_map(|m| {
            let model_id = m.get("modelId")?.as_str()?.to_string();
            let model_name = model_id.clone();

            let tags: Vec<String> = m
                .get("tags")
                .and_then(|t| t.as_array())
                .map(|t| {
                    t.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();

            let downloads = m.get("downloads")?.as_u64().unwrap_or(0);
            let likes = m.get("likes")?.as_u64().unwrap_or(0);
            let pipeline_tag = m.get("pipeline_tag").and_then(|v| v.as_str()).map(|s| s.to_string());
            let trending_score = m.get("trendingScore").and_then(|v| v.as_i64()).unwrap_or(0);
            let created_at = m.get("createdAt").and_then(|v| v.as_str()).map(|s| s.to_string());

            // Extract quantization from tags (e.g. "gguf:Q4_K_M", "gguf:Q8_0")
            let quantization = tags.iter()
                .find(|t| t.starts_with("gguf:"))
                .and_then(|t| t.strip_prefix("gguf:"))
                .map(|s| s.to_string());

            // Extract license from tags (e.g. "license:apache-2.0")
            let license = tags.iter()
                .find(|t| t.starts_with("license:"))
                .and_then(|t| t.strip_prefix("license:"))
                .map(|s| s.to_string());

            Some(crate::models::SearchResult {
                model_id: model_id.clone(),
                model_name,
                tags,
                downloads,
                likes,
                pipeline_tag,
                size: None, // filled in below
                parameters: None,
                capabilities: vec![],
                readme: None,
                quantization,
                license,
                trending_score,
                created_at,
            })
        })
        .collect();

    // Enrich with parameters, capabilities, and GGUF size from model detail API
    // Use semaphore to limit concurrent requests to 8 at a time
    let model_ids: Vec<String> = results.iter().map(|r| r.model_id.clone()).collect();
    let client = reqwest::Client::new();
    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(8));
    let mut handles = Vec::new();

    for model_id in &model_ids {
        let client = client.clone();
        let model_id = model_id.clone();
        let permit = semaphore.clone();

        handles.push(tokio::spawn(async move {
            let _permit = permit.acquire().await.ok()?;
            let url = format!("https://huggingface.co/api/models/{}", model_id);
            match client.get(&url).send().await {
                Ok(resp) => {
                    if let Ok(m) = resp.json::<serde_json::Value>().await {
                        let config = m.get("config");

                        // Parameters from config.n_params
                        let parameters = config
                            .and_then(|c| c.get("n_params"))
                            .and_then(|v| {
                                if v.is_number() {
                                    Some(v.to_string())
                                } else {
                                    v.as_str().map(|s| s.to_string())
                                }
                            });

                        // Capabilities: prefer config.model_type, fall back to gguf.architecture
                        let capabilities: Vec<String> = config
                            .and_then(|c| c.get("model_type"))
                            .and_then(|v| v.as_str())
                            .map(|s| vec![s.to_string()])
                            .or_else(|| {
                                m.get("gguf")
                                    .and_then(|g| g.get("architecture"))
                                    .and_then(|v| v.as_str())
                                    .map(|s| vec![s.to_string()])
                            })
                            .unwrap_or_default();

                        // Size from gguf.total or gguf.totalFileSize
                        let size = m.get("gguf")
                            .and_then(|g| g.get("total"))
                            .and_then(|v| v.as_u64())
                            .or_else(|| {
                                m.get("gguf")
                                    .and_then(|g| g.get("totalFileSize"))
                                    .and_then(|v| v.as_u64())
                            });

                        Some((model_id, parameters, capabilities, size))
                    } else {
                        None
                    }
                }
                Err(_) => None,
            }
        }));
    }
    for handle in handles {
        if let Ok(Some((model_id, parameters, capabilities, size))) = handle.await
            && let Some(result) = results.iter_mut().find(|r| r.model_id == model_id) {
                result.parameters = parameters;
                result.capabilities = capabilities;
                if let Some(s) = size {
                    result.size = Some(s);
                }
            }
    }

    Ok((results, 1))
}

/// List all GGUF files for a model.
pub async fn list_gguf_files(model_id: &str) -> Result<Vec<(String, u64, String)>> {
    let url = format!("https://huggingface.co/api/models/{}/tree/main", model_id);
    let resp = reqwest::get(&url).await?.error_for_status()?;
    let files: Vec<serde_json::Value> = resp.json().await?;

    let mut gguf_files = Vec::new();
    for file in &files {
        let path = file.get("path").and_then(|p| p.as_str()).unwrap_or("");
        if path.ends_with(".gguf") {
            let size = file.get("lfs")
                .and_then(|l| l.get("size"))
                .and_then(|s| s.as_u64())
                .unwrap_or(0);
            let lfs_url = file
                .get("lfs")
                .and_then(|l| l.get("url"))
                .and_then(|u| u.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| {
                    format!("https://huggingface.co/{model_id}/resolve/main/{path}")
                });
            gguf_files.push((path.to_string(), size, lfs_url));
        }
    }

    if gguf_files.is_empty() {
        anyhow::bail!("No .gguf files found in {}", model_id);
    }

    Ok(gguf_files)
}

/// Fetch the README for a model from HuggingFace.
pub async fn fetch_readme(model_id: &str) -> Result<String> {
    let url = format!("https://huggingface.co/{}/raw/main/README.md", model_id);
    let resp = reqwest::Client::new()
        .get(&url)
        .header("User-Agent", "llm-manager/0.1.0")
        .send()
        .await?
        .error_for_status()?;
    let text = resp.text().await?;
    Ok(text)
}

/// Download a file with progress tracking.
pub async fn download_file(
    _model_id: &str,
    _filename: &str,
    url: &str,
    dest: &std::path::Path,
    progress: &mut crate::models::DownloadState,
    cancelled: std::sync::Arc<std::sync::atomic::AtomicBool>,
    tx: tokio::sync::broadcast::Sender<crate::models::DownloadState>,
) -> Result<()> {
    let client = reqwest::Client::new();
    let resp = client.get(url).send().await?.error_for_status()?;

    // Get total size from content-length if available
    if let Some(len) = resp.content_length() {
        progress.total_bytes = len;
    }

    let mut stream = resp.bytes_stream();
    let mut file = tokio::fs::File::create(dest).await?;

    let mut last_update = std::time::Instant::now();
    let mut last_bytes = 0u64;

    while let Some(chunk) = stream.next().await {
        let chunk = match chunk {
            Ok(c) => c,
            Err(e) => {
                drop(file);
                let _ = tokio::fs::remove_file(dest).await;
                return Err(anyhow::anyhow!("Stream error: {}", e));
            }
        };

        if let Err(e) = file.write_all(&chunk).await {
            drop(file);
            let _ = tokio::fs::remove_file(dest).await;
            return Err(anyhow::anyhow!("Write error: {}", e));
        }

        progress.downloaded_bytes += chunk.len() as u64;

        // Calculate speed
        let elapsed = progress.start_time.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            progress.bytes_per_second = progress.downloaded_bytes as f64 / elapsed;
        }

        if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
            drop(file);
            let _ = tokio::fs::remove_file(dest).await;
            return Err(anyhow::anyhow!("Download cancelled"));
        }

        // Send progress update at most every 100ms and only if bytes changed
        if last_update.elapsed() >= std::time::Duration::from_millis(100)
            && progress.downloaded_bytes != last_bytes {
                let _ = tx.send(progress.clone());
                last_update = std::time::Instant::now();
                last_bytes = progress.downloaded_bytes;
            }
    }

    progress.status = crate::models::DownloadStatus::Complete;
    let _ = tx.send(progress.clone());

    Ok(())
}

use std::os::unix::fs::PermissionsExt;

/// Resolve the llama-server binary path for a given backend.
/// Downloads the binary from GitHub releases if not already cached.
pub async fn resolve_backend_binary(backend: crate::models::Backend, version: Option<&str>) -> Result<std::path::PathBuf> {
    let bin_base = dirs::data_local_dir()
        .unwrap_or_default()
        .join("llm-manager")
        .join("bin");

    let tag = match version {
        Some(v) if !v.is_empty() => v.to_string(),
        _ => {
            // Fetch latest release tag (best-effort; falls back to hardcoded tag)
            let client = reqwest::Client::new();
            match client
                .get("https://api.github.com/repos/ggml-org/llama.cpp/releases/latest")
                .header("Accept", "application/vnd.github.v3+json")
                .send()
                .await
            {
                Ok(resp) => match resp.error_for_status() {
                    Ok(resp) => match resp.json::<serde_json::Value>().await {
                        Ok(json) => json
                            .get("tag_name")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| "b9128".to_string()),
                        Err(_) => "b9128".to_string(),
                    },
                    Err(_) => "b9128".to_string(),
                },
                Err(_) => "b9128".to_string(),
            }
        }
    };

    let bin_name = format!("llama-server-{}-{}", match backend {
        crate::models::Backend::Cpu => "cpu",
        crate::models::Backend::Vulkan => "vulkan",
        crate::models::Backend::Rocrm => "rocm",
    }, tag);
    let bin_dir = bin_base.join(&bin_name);
    let bin_path = bin_dir.join("llama-server");

    // Check if both the binary and at least one shared library exist
    let lib_sentinel = bin_dir.join("libllama.so");

    if bin_path.exists() && lib_sentinel.exists() {
        return Ok(bin_path);
    }

    // Create bin directory
    std::fs::create_dir_all(&bin_dir)?;

    let client = reqwest::Client::new();

    // Construct asset name
    let asset_name = match backend {
        crate::models::Backend::Cpu => format!("llama-{tag}-bin-ubuntu-x64.tar.gz"),
        crate::models::Backend::Vulkan => format!("llama-{tag}-bin-ubuntu-vulkan-x64.tar.gz"),
        crate::models::Backend::Rocrm => format!("llama-{tag}-bin-ubuntu-rocm-7.2-x64.tar.gz"),
    };

    let download_url = format!("https://github.com/ggml-org/llama.cpp/releases/download/{tag}/{asset_name}");

    // Download to temp file (GitHub requires User-Agent for releases)
    let tmp_path = bin_dir.join(format!("{bin_name}.tmp"));
    let resp = client
        .get(&download_url)
        .header("User-Agent", "llm-manager/0.1.0")
        .send()
        .await?
        .error_for_status()?;
    let bytes = resp.bytes().await?;
    tokio::fs::write(&tmp_path, &bytes).await?;

    // Extract the archive to a temp directory, then pull out the binary and shared libs
    let extract_dir = bin_dir.join(format!("{bin_name}.extract"));
    extract_tar_gz_to(&tmp_path, &extract_dir)?;

    // The archive contains llama-xxx/bin/llama-server; find it and move into bin_dir
    let extracted_bin = extract_dir.join("llama-server");
    if extracted_bin.exists() {
        std::fs::rename(&extracted_bin, &bin_path)?;
    } else {
        // Try searching recursively
        let mut found = None;
        for entry in &walk_dir(&extract_dir) {
            if entry.file_name().to_str().map(|n| n == "llama-server").unwrap_or(false) {
                found = Some(entry.path().to_path_buf());
                break;
            }
        }
        if let Some(path) = found {
            std::fs::rename(path, &bin_path)?;
        } else {
            anyhow::bail!("Could not find llama-server binary in archive");
        }
    }

    // Also extract shared libraries (*.so*) from the archive into bin_dir
    for entry in &walk_dir(&extract_dir) {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.ends_with(".so") || name_str.contains(".so.") {
            let dest = bin_dir.join(name);
            // Use std::fs::copy which follows symlinks and creates a regular file at dest
            let _ = std::fs::copy(entry.path(), dest);
        }
    }

    // Make executable
    std::fs::set_permissions(&bin_path, std::fs::Permissions::from_mode(0o755))?;

    // Clean up temp files
    let _ = tokio::fs::remove_file(&tmp_path).await;
    let _ = tokio::fs::remove_dir_all(&extract_dir).await;

    Ok(bin_path)
}

/// Extract the entire tar.gz archive into a directory.
fn extract_tar_gz_to(archive_path: &std::path::Path, dest_dir: &std::path::Path) -> Result<()> {
    use flate2::read::GzDecoder;
    use tar::Archive;

    let file = std::fs::File::open(archive_path)?;
    let decoder = GzDecoder::new(file);
    let mut archive = Archive::new(decoder);
    archive.unpack(dest_dir)?;
    Ok(())
}

/// Recursively walk a directory.
/// Returns entries directly without collecting entire tree into memory.
fn walk_dir(dir: &std::path::Path) -> Vec<std::fs::DirEntry> {
    let mut entries = Vec::new();
    walk_dir_impl(dir, &mut entries, 0, 10); // Max depth of 10 to prevent stack overflow
    entries
}

fn walk_dir_impl(dir: &std::path::Path, entries: &mut Vec<std::fs::DirEntry>, depth: usize, max_depth: usize) {
    if depth >= max_depth {
        return;
    }

    if let Ok(read) = std::fs::read_dir(dir) {
        for entry in read.flatten() {
            let path = entry.path();
            entries.push(entry);
            if path.is_dir() {
                walk_dir_impl(&path, entries, depth + 1, max_depth);
            }
        }
    }
}
