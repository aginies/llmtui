use anyhow::Result;
use futures_util::StreamExt;
use tokio::io::AsyncWriteExt;

/// Download state codes (stored as AtomicU8 for lock-free access)
pub const DOWNLOAD_STATE_PAUSED: u8 = 2;
pub const DOWNLOAD_STATE_CANCELLED: u8 = 3;

/// Get the amount of free disk space (in bytes) at the given path.
/// Uses `statvfs` on Unix systems.
pub fn get_free_space_bytes(path: &std::path::Path) -> u64 {
    let path_str = path.to_string_lossy();
    let c_path = std::ffi::CString::new(path_str.as_ref()).unwrap();

    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    let result = unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) };

    if result != 0 {
        return 0;
    }

    // f_bavail = free blocks available to unprivileged user
    // f_frsize = fundamental filesystem block size
    stat.f_bavail as u64 * stat.f_frsize as u64
}

fn default_tag(repo: &str) -> String {
    if repo.contains("lemonade") {
        "b1273".to_string()
    } else if repo.contains("cuda") {
        "b9279".to_string()
    } else {
        "b4100".to_string()
    }
}

/// Search models on HuggingFace.
///
/// `limit` is the number of results per page (default 10, max 200).
/// `offset` is the number of results to skip (for pagination).
pub async fn search_models(query: &str, limit: u32, offset: u32) -> Result<(Vec<crate::models::SearchResult>, usize, Vec<String>)> {
    let url = format!(
        "https://huggingface.co/api/models?search={}&limit={}&offset={}&filter=gguf&expand=config&expand=gguf&expand=downloads&expand=likes&expand=tags&expand=pipeline_tag&expand=trendingScore&expand=createdAt",
        urlencoding::encode(query),
        limit,
        offset
    );
    // println!("Search URL: {}", url);

    let resp = reqwest::get(&url).await?.error_for_status()?;
    let models: Vec<serde_json::Value> = resp.json().await?;

    let query_words: Vec<String> = query.trim().split_whitespace().map(|w| w.to_lowercase()).collect();
    let raw_ids: Vec<String> = models.iter().filter_map(|m| m.get("id").and_then(|v| v.as_str())).map(|s| s.to_string()).collect();
    let results: Vec<crate::models::SearchResult> = models
        .into_iter()
        .filter_map(|m| {
            let model_id = m.get("id")?.as_str()?.to_string();
            // Post-filter: only keep results where the model_id contains each search word.
            // The HF API does full-text search across descriptions/tags, so unrelated
            // models can appear. We check each word case-insensitively (AND logic).
            let model_lower = model_id.to_lowercase();
            if !query_words.is_empty() && !query_words.iter().all(|w| model_lower.contains(w)) {
                return None;
            }
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

            let downloads = m.get("downloads").and_then(|v| v.as_u64()).unwrap_or(0);
            let likes = m.get("likes").and_then(|v| v.as_u64()).unwrap_or(0);
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

            let gguf = m.get("gguf");
            let parameters = gguf
                .and_then(|g| g.get("architecture"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let capabilities: Vec<String> = gguf
                .and_then(|g| g.get("architecture"))
                .and_then(|v| v.as_str())
                .map(|s| vec![s.to_string()])
                .unwrap_or_default();
            let size = gguf
                .and_then(|g| g.get("total"))
                .and_then(|v| v.as_u64())
                .or_else(|| {
                    gguf.and_then(|g| g.get("totalFileSize"))
                        .and_then(|v| v.as_u64())
                });
            let context_length = gguf
                .and_then(|g| g.get("context_length"))
                .and_then(|v| v.as_u64())
                .map(|v| v as u32);

            Some(crate::models::SearchResult {
                model_id: model_id.clone(),
                model_name,
                tags,
                downloads,
                likes,
                pipeline_tag,
                size,
                parameters,
                capabilities,
                context_length,
                readme: None,
                quantization,
                license,
                trending_score,
                created_at,
                downloaded: false,
            })
        })
        .collect();

    Ok((results, 1, raw_ids))
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
        .header("User-Agent", "llm-manager/1.0.0")
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
    download_state: std::sync::Arc<std::sync::atomic::AtomicU8>,
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

        let state = download_state.load(std::sync::atomic::Ordering::Relaxed);
        if state == DOWNLOAD_STATE_CANCELLED {
            drop(file);
            let _ = tokio::fs::remove_file(dest).await;
            return Err(anyhow::anyhow!("Download cancelled"));
        }
        if state == DOWNLOAD_STATE_PAUSED {
            // Pause: wait until resumed (state changes back to DOWNLOADING)
            // Also check download_state_arc if present for UI consistency
            let should_pause = if let Some(arc) = &progress.download_state_arc {
                arc.load(std::sync::atomic::Ordering::Relaxed) == DOWNLOAD_STATE_PAUSED
            } else {
                true
            };
            if should_pause {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                continue;
            }
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

pub fn get_bin_base() -> std::path::PathBuf {
    dirs::data_local_dir()
        .unwrap_or_default()
        .join("llm-manager")
        .join("bin")
}

/// Get the binary sentinel name for a platform (llama-server, llama-server.exe, etc.)
pub fn binary_name() -> &'static str {
    match std::env::consts::OS {
        "windows" => "llama-server.exe",
        _ => "llama-server",
    }
}

/// Get the shared library sentinel patterns for a platform
pub fn lib_sentinel_name() -> &'static str {
    match std::env::consts::OS {
        "windows" => "libllama.dll",
        "macos" => "libllama.dylib",
        _ => "libllama.so",
    }
}

/// Get the shared library extension for matching during extraction
pub fn lib_extension() -> &'static str {
    match std::env::consts::OS {
        "windows" => ".dll",
        "macos" => ".dylib",
        _ => ".so",
    }
}

/// Get the directory path for a specific backend version.
pub fn get_backend_dir(backend: crate::models::Backend, tag: &str) -> std::path::PathBuf {
    get_bin_base().join(format!("llama-server-{}-{}", backend.slug(), tag))
}

/// Check if any version of the specified backend is already installed.
pub fn is_backend_any_version_installed(backend: crate::models::Backend) -> bool {
    let bin_base = get_bin_base();
    if !bin_base.exists() {
        return false;
    }

    let prefix = format!("llama-server-{}-", backend.slug());

    let bin_name = binary_name();
    let lib_name = lib_sentinel_name();

    if let Ok(entries) = std::fs::read_dir(bin_base) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with(&prefix) {
                let bin_path = entry.path().join(bin_name);
                let lib_sentinel = entry.path().join(lib_name);
                if bin_path.exists() && lib_sentinel.exists() {
                    return true;
                }
            }
        }
    }

    false
}

/// Check if a specific version of the specified backend is already installed.
pub fn is_backend_version_installed(backend: crate::models::Backend, tag: Option<&str>) -> bool {
    // If tag is None, we don't know the exact version yet (latest), so we can't be sure it's installed
    // unless we check for ANY version, but here we want to know if the target is ready.
    // For "latest", we should probably always "resolve" it to check for updates.
    let tag = match tag {
        Some(t) => t,
        None => return false,
    };

    let bin_dir = get_backend_dir(backend, tag);
    let bin_name = binary_name();
    let lib_name = lib_sentinel_name();
    let bin_path = bin_dir.join(bin_name);
    let lib_sentinel = bin_dir.join(lib_name);

    bin_path.exists() && lib_sentinel.exists()
}

/// List all installed backends and their versions.
/// Returns a list of (Backend, VersionTag) pairs.
pub fn list_installed_backends() -> Vec<(crate::models::Backend, String)> {
    let bin_base = get_bin_base();
    let mut installed = Vec::new();
    if !bin_base.exists() {
        return installed;
    }

    let bin_name = binary_name();

    if let Ok(entries) = std::fs::read_dir(bin_base) {
        for entry in entries.flatten() {
            if !entry.path().is_dir() {
                continue;
            }
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            
            // Expected format: llama-server-{backend}-{tag}
            if !name_str.starts_with("llama-server-") {
                continue;
            }

            // Strip the prefix and split the rest
            let suffix = name_str.strip_prefix("llama-server-").unwrap_or("");
            let parts: Vec<&str> = suffix.split('-').collect();
            
            if parts.len() < 2 {
                continue;
            }

            // The tag is always the last segment
            let tag = parts[parts.len() - 1].to_string();
            let backend = match (parts[0], parts.get(1).copied()) {
                ("rocm", Some("lemonade")) => crate::models::Backend::RocmLemonade,
                ("win", Some("cuda")) if parts.len() >= 4 && parts[2] == "12.4" => crate::models::Backend::CudaWindows12_4,
                ("win", Some("cuda")) if parts.len() >= 4 && parts[2] == "13.1" => crate::models::Backend::CudaWindows13_1,
                ("cpu", Some("arm64")) => crate::models::Backend::CpuArm64,
                ("macos", Some("arm64")) => crate::models::Backend::CpuMacosArm64,
                ("macos", Some("x64")) => crate::models::Backend::CpuMacosX64,
                ("cpu", _) => crate::models::Backend::Cpu,
                ("vulkan", _) => crate::models::Backend::Vulkan,
                ("rocm", _) => crate::models::Backend::Rocm,
                ("cuda", _) => crate::models::Backend::Cuda,
                ("win-cpu", _) => crate::models::Backend::CpuWindows,
                ("win-vulkan", _) => crate::models::Backend::VulkanWindows,
                ("win-hip", _) => crate::models::Backend::HipWindows,
                _ => continue,
            };

            // Verify it actually contains the binary
            if entry.path().join(bin_name).exists() {
                installed.push((backend, tag));
            }
        }
    }
    
    // Sort by backend then tag descending (usually tag contains version number)
    installed.sort_by(|a, b| {
        let b_cmp = format!("{:?}", a.0).cmp(&format!("{:?}", b.0));
        if b_cmp == std::cmp::Ordering::Equal {
            b.1.cmp(&a.1) // descending tags
        } else {
            b_cmp
        }
    });

    installed
}

/// Resolve the llama-server binary path for a given backend.
/// Downloads the binary from GitHub releases if not already cached.
pub async fn resolve_backend_binary(
    backend: crate::models::Backend,
    version: Option<&str>,
    log_tx: Option<tokio::sync::mpsc::Sender<String>>,
    progress_tx: Option<tokio::sync::broadcast::Sender<crate::models::DownloadState>>,
) -> Result<std::path::PathBuf> {
    tracing::info!("resolve_backend_binary: backend={}, version={:?}", backend, version);
    let tag = match version {
        Some(v) if !v.is_empty() => {
            tracing::info!("  -> using explicit version: {}", v);
            v.to_string()
        }
        _ => {
            // Check if we have any local version first before asking GitHub
            let installed = list_installed_backends();
            let backend_versions: Vec<_> = installed.iter()
                .filter(|(b, _)| *b == backend)
                .map(|(_, t)| t.clone())
                .collect();
            tracing::info!("  -> no explicit version, found {} installed versions for backend: {:?}", backend_versions.len(), backend);
            for v in &backend_versions {
                tracing::info!("     installed version: {}", v);
            }
            let latest_local = installed.iter()
                .filter(|(b, _)| *b == backend)
                .map(|(_, t)| t.clone())
                .next(); // list_installed_backends is already sorted by tag desc

            if let Some(t) = &latest_local {
                tracing::info!("  -> using latest installed version: {}", t);
                t.clone()
            } else {
                // Fetch latest release tag (best-effort; falls back to hardcoded tag)
                let repo = match backend {
                    crate::models::Backend::RocmLemonade => "lemonade-sdk/llamacpp-rocm",
                    crate::models::Backend::Cuda => "ai-dock/llama.cpp-cuda",
                    _ => "ggml-org/llama.cpp",
                };
                tracing::info!("  -> no local version, fetching latest from GitHub repo: {}", repo);
                fetch_latest_release_tag(repo, &default_tag(repo)).await
            }
        }
    };

    let bin_dir = get_backend_dir(backend, &tag);
    let bin_name = binary_name();
    let bin_path = bin_dir.join(bin_name);
    tracing::info!("  -> resolved tag={}, bin_dir={}, bin_path={}", tag, bin_dir.display(), bin_path.display());

    // Check if both the binary and at least one shared library exist
    let lib_name = lib_sentinel_name();
    let lib_sentinel = bin_dir.join(lib_name);
    tracing::info!("  -> checking binary existence: bin_path={} lib_sentinel={}", bin_path.exists(), lib_sentinel.exists());

    if bin_path.exists() && lib_sentinel.exists() {
        tracing::info!("  -> binary already exists, returning cached path");
        return Ok(bin_path);
    }

    tracing::info!("  -> binary not found, will download");

    // Create bin directory
    std::fs::create_dir_all(&bin_dir)?;

    let client = reqwest::Client::new();

    // Construct asset name and URL
    let (download_url, is_zip) = match backend {
        // Linux x64 backends
        crate::models::Backend::Cpu => (
            format!("https://github.com/ggml-org/llama.cpp/releases/download/{tag}/llama-{tag}-bin-ubuntu-x64.tar.gz"),
            false
        ),
        crate::models::Backend::Vulkan => (
            format!("https://github.com/ggml-org/llama.cpp/releases/download/{tag}/llama-{tag}-bin-ubuntu-vulkan-x64.tar.gz"),
            false
        ),
        crate::models::Backend::Rocm => (
            format!("https://github.com/ggml-org/llama.cpp/releases/download/{tag}/llama-{tag}-bin-ubuntu-rocm-7.2-x64.tar.gz"),
            false
        ),
        crate::models::Backend::RocmLemonade => {
            use crate::backend::hardware::{detect_amd_gfx_target, get_lemonade_gfx_suffix};
            let gfx = detect_amd_gfx_target().unwrap_or_else(|| "gfx1100".to_string());
            let suffix = get_lemonade_gfx_suffix(&gfx);
            (
                format!("https://github.com/lemonade-sdk/llamacpp-rocm/releases/download/{tag}/llama-{tag}-ubuntu-rocm-{suffix}-x64.zip"),
                true
            )
        }
        crate::models::Backend::Cuda => (
            format!("https://github.com/ai-dock/llama.cpp-cuda/releases/download/{tag}/llama.cpp-{tag}-cuda-12.8-amd64.tar.gz"),
            false
        ),
        // Linux ARM64
        crate::models::Backend::CpuArm64 => (
            format!("https://github.com/ggml-org/llama.cpp/releases/download/{tag}/llama-{tag}-bin-ubuntu-arm64.tar.gz"),
            false
        ),
        // Windows backends
        crate::models::Backend::CpuWindows => (
            format!("https://github.com/ggml-org/llama.cpp/releases/download/{tag}/llama-{tag}-bin-win-cpu-x64.zip"),
            true
        ),
        crate::models::Backend::VulkanWindows => (
            format!("https://github.com/ggml-org/llama.cpp/releases/download/{tag}/llama-{tag}-bin-win-vulkan-x64.zip"),
            true
        ),
        crate::models::Backend::CudaWindows12_4 => (
            format!("https://github.com/ggml-org/llama.cpp/releases/download/{tag}/llama-{tag}-bin-win-cuda-12.4-x64.zip"),
            true
        ),
        crate::models::Backend::CudaWindows13_1 => (
            format!("https://github.com/ggml-org/llama.cpp/releases/download/{tag}/llama-{tag}-bin-win-cuda-13.1-x64.zip"),
            true
        ),
        crate::models::Backend::HipWindows => (
            format!("https://github.com/ggml-org/llama.cpp/releases/download/{tag}/llama-{tag}-bin-win-hip-radeon-x64.zip"),
            true
        ),
        // macOS backends
        crate::models::Backend::CpuMacosArm64 => (
            format!("https://github.com/ggml-org/llama.cpp/releases/download/{tag}/llama-{tag}-bin-macos-arm64.tar.gz"),
            false
        ),
        crate::models::Backend::CpuMacosX64 => (
            format!("https://github.com/ggml-org/llama.cpp/releases/download/{tag}/llama-{tag}-bin-macos-x64.tar.gz"),
            false
        ),
    };

    if let Some(tx) = &log_tx {
        let _ = tx.send(format!("Download URL: {}", download_url)).await;
        let _ = tx.send(format!("Install path: {}", bin_dir.display())).await;
    }

    // Download to temp file (GitHub requires User-Agent for releases)
    let tmp_ext = if is_zip { "zip" } else { "tar.gz" };
    let tmp_filename = format!("llama-server-{}-{}.tmp.{}", backend.slug(), tag, tmp_ext);
    let tmp_path = bin_dir.join(&tmp_filename);
    tracing::info!("  -> downloading to: {}", tmp_path.display());
    
    if let Some(ref tx) = progress_tx {
        let mut progress = crate::models::DownloadState::new("llama-server".to_string(), tmp_filename.clone(), 0);
        let download_state = std::sync::Arc::new(std::sync::atomic::AtomicU8::new(1));
        download_file("llama-server", &tmp_filename, &download_url, &tmp_path, &mut progress, download_state, tx.clone()).await?;
    } else {
        let resp = client
            .get(&download_url)
.header("User-Agent", "llm-manager/0.9.9")
            .send()
            .await?
            .error_for_status()?;
        let mut stream = resp.bytes_stream();
        let mut file = tokio::fs::File::create(&tmp_path).await?;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk).await?;
        }
    }
    tracing::info!("  -> download complete, extracting...");

    // Extract the archive to a temp directory, then pull out the binary and shared libs
    let extract_dir = bin_dir.join(format!("llama-server-{}-{}.extract", backend.slug(), tag));
    
    if let Some(tx) = &log_tx {
        let _ = tx.send("Extracting backend...".to_string()).await;
    }

    extract_archive(&tmp_path, &extract_dir)?;

    if let Some(tx) = &log_tx {
        let _ = tx.send("Finalizing installation...".to_string()).await;
    }

    // The archive contains llama-xxx/bin/llama-server; find it and move into bin_dir
    let extracted_bin = extract_dir.join(bin_name);
    tracing::info!("  -> looking for binary in extracted archive at: {}", extracted_bin.display());
    if extracted_bin.exists() {
        tracing::info!("  -> found binary at expected location, moving to {}", bin_path.display());
        std::fs::rename(&extracted_bin, &bin_path)?;
    } else {
        // Try searching recursively for the binary name
        tracing::info!("  -> binary not at expected location, searching recursively...");
        let mut found = None;
        walk_dir_recursive(&extract_dir, 0, 10, &mut |entry| {
            if entry.file_name().to_str() == Some(bin_name) {
                tracing::info!("  -> found binary at: {}", entry.path().display());
                found = Some(entry.path().to_path_buf());
            }
        });
        if let Some(path) = found {
            std::fs::rename(path, &bin_path)?;
        } else {
            anyhow::bail!("Could not find {} binary in archive at {}", bin_name, extract_dir.display());
        }
    }

    // Also try to extract llama-bench if it exists
    let bench_bin_path = bin_dir.join("llama-bench");
    let mut bench_found = None;
    walk_dir_recursive(&extract_dir, 0, 10, &mut |entry| {
        if entry.file_name().to_str().map(|n| n == "llama-bench").unwrap_or(false) {
            bench_found = Some(entry.path().to_path_buf());
        }
    });
    if let Some(path) = bench_found {
        let _ = std::fs::rename(path, &bench_bin_path);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&bench_bin_path, std::fs::Permissions::from_mode(0o755));
        }
    }

    // Also extract shared libraries from the archive into bin_dir
    let lib_ext = lib_extension();
    walk_dir_recursive(&extract_dir, 0, 10, &mut |entry| {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.ends_with(lib_ext) || name_str.contains(&format!(".{}", lib_ext.trim_start_matches('.'))) {
            let dest = bin_dir.join(name);
            // Use std::fs::copy which follows symlinks and creates a regular file at dest
            let _ = std::fs::copy(entry.path(), dest);
        }
    });

    // Make executable (Unix-only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&bin_path, std::fs::Permissions::from_mode(0o755))?;
    }

    // Clean up temp files
    let _ = tokio::fs::remove_file(&tmp_path).await;
    let _ = tokio::fs::remove_dir_all(&extract_dir).await;

    Ok(bin_path)
}

/// Extract a .tar.gz or .zip archive into a directory.
pub fn extract_archive(archive_path: &std::path::Path, dest_dir: &std::path::Path) -> Result<()> {
    let filename = archive_path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");

    if filename.ends_with(".zip") {
        let file = std::fs::File::open(archive_path)?;
        let mut archive = zip::ZipArchive::new(file)?;
        archive.extract(dest_dir)?;
    } else if filename.ends_with(".tar.gz") || filename.contains(".tar.gz") {
        use flate2::read::GzDecoder;
        use tar::Archive;

        let file = std::fs::File::open(archive_path)?;
        let decoder = GzDecoder::new(file);
        let mut archive = Archive::new(decoder);
        archive.unpack(dest_dir)?;
    } else {
        anyhow::bail!("Unsupported archive format: {}", filename);
    }
    
    Ok(())
}

  /// Recursively walk a directory and call a closure for each entry.
pub fn walk_dir_recursive<F>(dir: &std::path::Path, depth: usize, max_depth: usize, f: &mut F)
where
    F: FnMut(&std::fs::DirEntry),
{
    if depth >= max_depth {
        return;
    }

    if let Ok(read) = std::fs::read_dir(dir) {
        for entry in read.flatten() {
            let path = entry.path();
            f(&entry);
            if path.is_dir() {
                walk_dir_recursive(&path, depth + 1, max_depth, f);
            }
        }
    }
}

/// Fetch the latest release tag from a GitHub repository.
/// Returns the tag_name from the API, or falls back to a hardcoded default.
async fn fetch_latest_release_tag(repo: &str, fallback: &str) -> String {
    let client = reqwest::Client::new();
    let url = format!("https://api.github.com/repos/{}/releases/latest", repo);
    match client
        .get(&url)
        .header("Accept", "application/vnd.github.v3+json")
        .header("User-Agent", "llm-manager/1.0.0")
        .send()
        .await
    {
        Ok(resp) => match resp.error_for_status() {
            Ok(resp) => match resp.json::<serde_json::Value>().await {
                Ok(json) => json
                    .get("tag_name")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| fallback.to_string()),
                Err(_) => fallback.to_string(),
            },
            Err(_) => fallback.to_string(),
        },
        Err(_) => fallback.to_string(),
    }
}
