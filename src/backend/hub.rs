use anyhow::Result;
use futures_util::StreamExt;
use tokio::io::AsyncWriteExt;

/// Download state codes (stored as AtomicU8 for lock-free access)
///
/// NOTE: Pause only takes effect between chunks, not mid-chunk.
/// The download loop checks the state after writing each chunk to disk.
/// When paused, it polls the state every 100ms via `stream.next().await`.
/// For finer-grained control (mid-chunk pause), consider using
/// `tokio::sync::watch` instead of `AtomicU8`.
pub const DOWNLOAD_STATE_PAUSING: u8 = 4;
pub const DOWNLOAD_STATE_PAUSED: u8 = 2;
pub const DOWNLOAD_STATE_CANCELLED: u8 = 3;

/// Get the amount of free disk space (in bytes) at the given path.
/// Returns `None` if the free space cannot be determined (callers should
/// skip the space check in that case rather than assuming zero space).
pub fn get_free_space_bytes(path: &std::path::Path) -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let path_str = path.to_string_lossy();
        let c_path = std::ffi::CString::new(path_str.as_ref()).ok()?;

        let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
        let result = unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) };

        if result != 0 {
            return None;
        }

        // f_bavail = free blocks available to unprivileged user
        // f_frsize = fundamental filesystem block size
        Some(stat.f_bavail * stat.f_frsize)
    }
    #[cfg(target_os = "macos")]
    {
        let path_str = path.to_string_lossy();
        let c_path = std::ffi::CString::new(path_str.as_ref()).ok()?;

        let mut stat: libc::statfs = unsafe { std::mem::zeroed() };
        let result = unsafe { libc::statfs(c_path.as_ptr(), &mut stat) };

        if result != 0 {
            return None;
        }

        // f_bavail = free blocks available to unprivileged user
        // f_bsize = fundamental filesystem block size
        Some(stat.f_bavail as u64 * stat.f_bsize as u64)
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::ffi::OsStrExt;
        let wide: Vec<u16> = path
            .to_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        extern "system" {
            fn GetDiskFreeSpaceExW(
                lp_directory_name: *const u16,
                lp_free_bytes_available_to_caller: *mut u64,
                lp_total_number_of_bytes: *mut u64,
                lp_free_number_of_bytes: *mut u64,
            ) -> i32;
        }

        let mut free_bytes: u64 = 0;
        let result = unsafe {
            GetDiskFreeSpaceExW(
                wide.as_ptr(),
                &mut free_bytes,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };

        if result != 0 { Some(free_bytes) } else { None }
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        let _ = path;
        None
    }
}

fn default_tag(backend: &crate::models::Backend) -> String {
    match backend {
        crate::models::Backend::RocmLemonade => "b1273".to_string(),
        crate::models::Backend::Cuda
        | crate::models::Backend::CudaWindows12_4
        | crate::models::Backend::CudaWindows13_1 => "b9279".to_string(),
        _ => "b4100".to_string(),
    }
}

/// Extract the numeric part from a version tag (e.g. "v3081" -> "3081", "b1273" -> "1273").
fn extract_version_number(tag: &str) -> u64 {
    tag.chars()
        .filter(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse::<u64>()
        .unwrap_or(0)
}

/// Compare two version tags, returning the newer one.
/// Handles both "v1234" and "b1234" formats by extracting numeric parts.
fn compare_versions<'a>(a: &'a str, b: &'a str) -> &'a str {
    let a_num = extract_version_number(a);
    let b_num = extract_version_number(b);
    if a_num >= b_num { a } else { b }
}

/// Map a backend to the GitHub repo and asset name pattern to search for.
/// Returns (repo, asset_pattern) or None for backends that don't need asset lookup.
fn resolve_backend_key(backend: &crate::models::Backend) -> Option<(&'static str, &'static str)> {
    match backend {
        // Linux x64 backends from ggml-org/llama.cpp
        crate::models::Backend::Cpu => Some(("ggml-org/llama.cpp", "bin-ubuntu-x64.tar.gz")),
        crate::models::Backend::Vulkan => {
            Some(("ggml-org/llama.cpp", "bin-ubuntu-vulkan-x64.tar.gz"))
        }
        crate::models::Backend::Rocm => {
            // The exact ROCm asset name has changed over time
            // (bin-ubuntu-rocm-x64, bin-ubuntu-rocm-7.2-x64, ...) and recent
            // releases no longer publish the old name, so match any ubuntu
            // ROCm asset and resolve the exact name per release at download
            // time (see resolve_backend_binary).
            Some(("ggml-org/llama.cpp", "bin-ubuntu-rocm"))
        }
        // Linux ARM64
        crate::models::Backend::CpuArm64 => Some(("ggml-org/llama.cpp", "bin-ubuntu-arm64.tar.gz")),
        // ROCm Lemonade (separate repo)
        crate::models::Backend::RocmLemonade => Some(("lemonade-sdk/llamacpp-rocm", "rocm-")),
        // CUDA (separate repo)
        crate::models::Backend::Cuda => Some(("ai-dock/llama.cpp-cuda", "cuda-12.8")),
        // Windows CPU/Vulkan
        crate::models::Backend::CpuWindows => Some(("ggml-org/llama.cpp", "bin-win-cpu-x64.zip")),
        crate::models::Backend::VulkanWindows => {
            Some(("ggml-org/llama.cpp", "bin-win-vulkan-x64.zip"))
        }
        // Windows HIP (AMD)
        crate::models::Backend::HipWindows => {
            Some(("ggml-org/llama.cpp", "bin-win-hip-radeon-x64.zip"))
        }
        // Windows CUDA (different CUDA versions)
        crate::models::Backend::CudaWindows12_4 => {
            Some(("ggml-org/llama.cpp", "bin-win-cuda-12.4-x64.zip"))
        }
        crate::models::Backend::CudaWindows13_1 => {
            Some(("ggml-org/llama.cpp", "bin-win-cuda-13.1-x64.zip"))
        }
        // macOS (no Vulkan/CUDA; only CPU)
        crate::models::Backend::CpuMacosArm64 => Some(("ggml-org/llama.cpp", "macos-arm64.tar.gz")),
        crate::models::Backend::CpuMacosX64 => Some(("ggml-org/llama.cpp", "macos-x64.tar.gz")),
    }
}

/// Search models on HuggingFace.
///
/// `limit` is the number of results per page (default 10, max 200).
/// `offset` is the number of results to skip (for pagination).
/// Returns the (post-filter) results and the raw number of models returned
/// by the API (pre-filter), used for pagination `has_more` detection.
pub async fn search_models(
    query: &str,
    limit: u32,
    offset: u32,
) -> Result<(Vec<crate::models::SearchResult>, usize)> {
    let url = format!(
        "https://huggingface.co/api/models?search={}&limit={}&offset={}&filter=gguf&expand=config&expand=gguf&expand=downloads&expand=likes&expand=tags&expand=pipeline_tag&expand=trendingScore&expand=createdAt",
        urlencoding::encode(query),
        limit,
        offset
    );
    // println!("Search URL: {}", url);

    let resp = reqwest::Client::builder()
        .user_agent(super::USER_AGENT)
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap()
        .get(&url)
        .send()
        .await?
        .error_for_status()?;
    let models: Vec<serde_json::Value> = resp.json().await?;
    let raw_count = models.len();

    let query_words: Vec<String> = query.split_whitespace().map(|w| w.to_lowercase()).collect();
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
            let pipeline_tag = m
                .get("pipeline_tag")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let trending_score = m.get("trendingScore").and_then(|v| v.as_i64()).unwrap_or(0);
            let created_at = m
                .get("createdAt")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            // Extract quantization from tags (e.g. "gguf:Q4_K_M", "gguf:Q8_0")
            let quantization = tags
                .iter()
                .find(|t| t.starts_with("gguf:"))
                .and_then(|t| t.strip_prefix("gguf:"))
                .map(|s| s.to_string());

            // Extract license from tags (e.g. "license:apache-2.0")
            let license = tags
                .iter()
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

    Ok((results, raw_count))
}

/// Validate a HuggingFace model_id for safety.
pub fn validate_model_id(model_id: &str) -> Result<()> {
    if model_id.is_empty() || model_id.len() > 100 {
        anyhow::bail!("Invalid model_id");
    }
    if model_id.contains("..") {
        anyhow::bail!("model_id contains '..'");
    }
    for c in model_id.chars() {
        if c < ' ' || c > '~' || c == '`' || c == '$' || c == '\\' || c == '"' || c == '\'' {
            anyhow::bail!("model_id contains invalid characters");
        }
    }
    Ok(())
}

/// List all GGUF files for a model.
pub async fn list_gguf_files(model_id: &str) -> Result<Vec<(String, u64, String)>> {
    validate_model_id(model_id)?;
    let branch = "main";
    let url = format!(
        "https://huggingface.co/api/models/{}/tree/{}",
        model_id, branch
    );
    let client = reqwest::Client::builder()
        .user_agent(super::USER_AGENT)
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap();
    let resp = match client.get(&url).send().await {
        Ok(r) if r.status().is_success() => r,
        _ => {
            client
                .get(&format!(
                    "https://huggingface.co/api/models/{}/tree/master",
                    model_id
                ))
                .send()
                .await?
        }
    };
    let resp = resp.error_for_status()?;
    let files: Vec<serde_json::Value> = resp.json().await?;

    let mut gguf_files = Vec::new();
    for file in &files {
        let path = file.get("path").and_then(|p| p.as_str()).unwrap_or("");
        if path.ends_with(".gguf") {
            let size = file
                .get("lfs")
                .and_then(|l| l.get("size"))
                .and_then(|s| s.as_u64())
                .unwrap_or(0);
            let lfs_url = file
                .get("lfs")
                .and_then(|l| l.get("url"))
                .and_then(|u| u.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| {
                    format!(
                        "https://huggingface.co/{model_id}/resolve/{}/{}",
                        branch, path
                    )
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
    validate_model_id(model_id)?;
    let client = reqwest::Client::builder()
        .user_agent(super::USER_AGENT)
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap();
    let url = format!("https://huggingface.co/{}/raw/main/README.md", model_id);
    let url_master = format!("https://huggingface.co/{}/raw/master/README.md", model_id);
    let resp = match client.get(&url).send().await {
        Ok(r) if r.status().is_success() => r,
        _ => client.get(&url_master).send().await?,
    };
    let resp = resp.error_for_status()?;
    let text = resp.text().await?;
    Ok(text)
}

/// Download a file with progress tracking.
/// Returns the `sha2-256` header value from the response if present (from GitHub CDN).
pub async fn download_file(
    _model_id: &str,
    _filename: &str,
    url: &str,
    dest: &std::path::Path,
    progress: &mut crate::models::DownloadState,
    download_state: std::sync::Arc<std::sync::atomic::AtomicU8>,
    tx: tokio::sync::broadcast::Sender<crate::models::DownloadState>,
) -> Result<Option<String>> {
    // No overall request timeout: files are large and stream for minutes.
    // Instead bound the connect phase and detect stalled streams per chunk.
    let client = reqwest::Client::builder()
        .user_agent(super::USER_AGENT)
        .connect_timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to build HTTP client: {}", e))?;
    let resp = match tokio::time::timeout(
        std::time::Duration::from_secs(30),
        client.get(url).send(),
    )
    .await
    {
        Ok(r) => r?,
        Err(_) => anyhow::bail!("Connection timed out"),
    };
    let resp = resp.error_for_status()?;

    // Capture the sha2-256 header from the response (GitHub CDN provides this)
    let sha256 = resp
        .headers()
        .get("sha2-256")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_lowercase());

    // Get total size from content-length if available
    if let Some(len) = resp.content_length() {
        progress.total_bytes = len;
    }

    let mut stream = resp.bytes_stream();
    let mut file = tokio::fs::File::create(dest).await?;

    let mut last_update = std::time::Instant::now();
    let mut last_bytes = 0u64;
    let mut stream_done = false;

    loop {
        let state = download_state.load(std::sync::atomic::Ordering::Relaxed);
        if state == DOWNLOAD_STATE_CANCELLED {
            drop(file);
            let _ = tokio::fs::remove_file(dest).await;
            return Err(anyhow::anyhow!("Download cancelled"));
        }
        if stream_done {
            break;
        }
        if state == DOWNLOAD_STATE_PAUSING {
            download_state.store(DOWNLOAD_STATE_PAUSED, std::sync::atomic::Ordering::Relaxed);
            progress.status = crate::models::DownloadStatus::Paused;
            let _ = tx.send(progress.clone());
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            continue;
        }
        if state == DOWNLOAD_STATE_PAUSED {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            continue;
        }

        // Reset status when resuming from paused state
        if progress.status == crate::models::DownloadStatus::Paused {
            progress.status = crate::models::DownloadStatus::Downloading;
        }

        // Bound each chunk read: a stalled TCP stream (no data, no error)
        // would otherwise block here forever, and cancel/pause are only
        // checked at the top of this loop.
        let chunk =
            match tokio::time::timeout(std::time::Duration::from_secs(60), stream.next()).await {
                Ok(Some(Ok(c))) => c,
                Ok(Some(Err(e))) => {
                    drop(file);
                    let _ = tokio::fs::remove_file(dest).await;
                    return Err(anyhow::anyhow!("Stream error: {}", e));
                }
                Ok(None) => {
                    stream_done = true;
                    continue;
                }
                Err(_) => {
                    drop(file);
                    let _ = tokio::fs::remove_file(dest).await;
                    return Err(anyhow::anyhow!(
                        "Download stalled: no data received for 60s"
                    ));
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

        // Send progress update at most every 100ms and only if bytes changed
        if last_update.elapsed() >= std::time::Duration::from_millis(100)
            && progress.downloaded_bytes != last_bytes
        {
            let _ = tx.send(progress.clone());
            last_update = std::time::Instant::now();
            last_bytes = progress.downloaded_bytes;
        }
    }

    progress.status = crate::models::DownloadStatus::Complete;
    let _ = tx.send(progress.clone());

    // Check that we actually downloaded something. A 0-byte file is invalid
    // even when the server omitted Content-Length (total_bytes == 0).
    if progress.downloaded_bytes == 0 {
        drop(file);
        let _ = tokio::fs::remove_file(dest).await;
        if progress.total_bytes > 0 {
            anyhow::bail!(
                "Downloaded file is empty (0 bytes), expected {} bytes",
                progress.total_bytes
            );
        }
        anyhow::bail!("Downloaded file is empty (0 bytes)");
    }

    Ok(sha256)
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
                ("win", Some("cuda")) if parts.len() >= 4 && parts[2] == "12.4" => {
                    crate::models::Backend::CudaWindows12_4
                }
                ("win", Some("cuda")) if parts.len() >= 4 && parts[2] == "13.1" => {
                    crate::models::Backend::CudaWindows13_1
                }
                ("cpu", Some("arm64")) => crate::models::Backend::CpuArm64,
                ("macos", Some("arm64")) => crate::models::Backend::CpuMacosArm64,
                ("macos", Some("x64")) => crate::models::Backend::CpuMacosX64,
                ("win", Some("cpu")) => crate::models::Backend::CpuWindows,
                ("win", Some("vulkan")) => crate::models::Backend::VulkanWindows,
                ("win", Some("hip")) => crate::models::Backend::HipWindows,
                ("cpu", _) => crate::models::Backend::Cpu,
                ("vulkan", _) => crate::models::Backend::Vulkan,
                ("rocm", _) => crate::models::Backend::Rocm,
                ("cuda", _) => crate::models::Backend::Cuda,
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
    tracing::info!(
        "resolve_backend_binary: backend={}, version={:?}",
        backend,
        version
    );
    let tag = match version {
        Some(v) if !v.is_empty() => {
            tracing::info!("  -> using explicit version: {}", v);
            v.to_string()
        }
        _ => {
            // Check if we have any local version first
            let installed = list_installed_backends();
            let backend_versions: Vec<_> = installed
                .iter()
                .filter(|(b, _)| *b == backend)
                .map(|(_, t)| t.clone())
                .collect();
            tracing::info!(
                "  -> no explicit version, found {} installed versions for backend: {:?}",
                backend_versions.len(),
                backend
            );
            for v in &backend_versions {
                tracing::info!("     installed version: {}", v);
            }
            let latest_local = installed
                .iter()
                .filter(|(b, _)| *b == backend)
                .map(|(_, t)| t.clone())
                .next(); // list_installed_backends is already sorted by tag desc

            // Also check what's the latest available from GitHub
            let github_latest = if let Some((repo, pattern)) = resolve_backend_key(&backend) {
                tracing::info!(
                    "  -> fetching latest available version from GitHub repo '{}' with asset pattern '{}'",
                    repo,
                    pattern
                );
                let available =
                    latest_release_with_asset(repo, pattern, &default_tag(&backend)).await;
                tracing::info!("  -> latest available from GitHub: {}", available);
                Some(available)
            } else {
                None
            };

            match (latest_local, github_latest) {
                (Some(local), Some(available)) => {
                    let chosen = compare_versions(&local, &available);
                    if local != available {
                        tracing::info!(
                            "  -> using newer version: local={}, available={}",
                            local,
                            available
                        );
                    }
                    chosen.to_string()
                }
                (Some(local), None) => {
                    tracing::info!("  -> using latest installed version: {}", local);
                    local
                }
                (None, Some(available)) => {
                    tracing::info!("  -> using latest from GitHub: {}", available);
                    available
                }
                (None, None) => default_tag(&backend).to_string(),
            }
        }
    };

    let bin_dir = get_backend_dir(backend, &tag);
    let bin_name = binary_name();
    let bin_path = bin_dir.join(bin_name);
    tracing::info!(
        "  -> resolved tag={}, bin_dir={}, bin_path={}",
        tag,
        bin_dir.display(),
        bin_path.display()
    );

    // Check if both the binary and at least one shared library exist
    let lib_name = lib_sentinel_name();
    let lib_sentinel = bin_dir.join(lib_name);
    tracing::info!(
        "  -> checking binary existence: bin_path={} lib_sentinel={}",
        bin_path.exists(),
        lib_sentinel.exists()
    );

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
            format!(
                "https://github.com/ggml-org/llama.cpp/releases/download/{tag}/llama-{tag}-bin-ubuntu-x64.tar.gz"
            ),
            false,
        ),
        crate::models::Backend::Vulkan => (
            format!(
                "https://github.com/ggml-org/llama.cpp/releases/download/{tag}/llama-{tag}-bin-ubuntu-vulkan-x64.tar.gz"
            ),
            false,
        ),
        crate::models::Backend::Rocm => {
            // The ROCm asset name is not stable across releases (rocm-x64,
            // rocm-7.2-x64, ...) and recent releases dropped it entirely.
            // Look up the actual asset name for this tag; fall back to the
            // last known name if the lookup fails (e.g. offline).
            let asset_name =
                fetch_release_asset_name("ggml-org/llama.cpp", &tag, "bin-ubuntu-rocm")
                    .await
                    .unwrap_or_else(|| format!("llama-{tag}-bin-ubuntu-rocm-7.2-x64.tar.gz"));
            (
                format!(
                    "https://github.com/ggml-org/llama.cpp/releases/download/{tag}/{}",
                    asset_name
                ),
                false,
            )
        }
        crate::models::Backend::RocmLemonade => {
            use crate::backend::hardware::{detect_amd_gfx_target, get_lemonade_gfx_suffix};
            let gfx = detect_amd_gfx_target().unwrap_or_else(|| "gfx1100".to_string());
            let suffix = get_lemonade_gfx_suffix(&gfx);
            (
                format!(
                    "https://github.com/lemonade-sdk/llamacpp-rocm/releases/download/{tag}/llama-{tag}-ubuntu-rocm-{suffix}-x64.zip"
                ),
                true,
            )
        }
        crate::models::Backend::Cuda => (
            format!(
                "https://github.com/ai-dock/llama.cpp-cuda/releases/download/{tag}/llama.cpp-{tag}-cuda-12.8-amd64.tar.gz"
            ),
            false,
        ),
        // Linux ARM64
        crate::models::Backend::CpuArm64 => (
            format!(
                "https://github.com/ggml-org/llama.cpp/releases/download/{tag}/llama-{tag}-bin-ubuntu-arm64.tar.gz"
            ),
            false,
        ),
        // Windows backends
        crate::models::Backend::CpuWindows => (
            format!(
                "https://github.com/ggml-org/llama.cpp/releases/download/{tag}/llama-{tag}-bin-win-cpu-x64.zip"
            ),
            true,
        ),
        crate::models::Backend::VulkanWindows => (
            format!(
                "https://github.com/ggml-org/llama.cpp/releases/download/{tag}/llama-{tag}-bin-win-vulkan-x64.zip"
            ),
            true,
        ),
        crate::models::Backend::CudaWindows12_4 => (
            format!(
                "https://github.com/ggml-org/llama.cpp/releases/download/{tag}/llama-{tag}-bin-win-cuda-12.4-x64.zip"
            ),
            true,
        ),
        crate::models::Backend::CudaWindows13_1 => (
            format!(
                "https://github.com/ggml-org/llama.cpp/releases/download/{tag}/llama-{tag}-bin-win-cuda-13.1-x64.zip"
            ),
            true,
        ),
        crate::models::Backend::HipWindows => (
            format!(
                "https://github.com/ggml-org/llama.cpp/releases/download/{tag}/llama-{tag}-bin-win-hip-radeon-x64.zip"
            ),
            true,
        ),
        // macOS backends
        crate::models::Backend::CpuMacosArm64 => (
            format!(
                "https://github.com/ggml-org/llama.cpp/releases/download/{tag}/llama-{tag}-bin-macos-arm64.tar.gz"
            ),
            false,
        ),
        crate::models::Backend::CpuMacosX64 => (
            format!(
                "https://github.com/ggml-org/llama.cpp/releases/download/{tag}/llama-{tag}-bin-macos-x64.tar.gz"
            ),
            false,
        ),
    };

    if let Some(tx) = &log_tx {
        let _ = tx.send(format!("Download URL: {}", download_url)).await;
        let _ = tx
            .send(format!("Install path: {}", bin_dir.display()))
            .await;
    }

    // Download to temp file (GitHub requires User-Agent for releases)
    let tmp_ext = if is_zip { "zip" } else { "tar.gz" };
    let tmp_filename = format!("llama-server-{}-{}.tmp.{}", backend.slug(), tag, tmp_ext);
    let tmp_path = bin_dir.join(&tmp_filename);
    tracing::info!("  -> downloading to: {}", tmp_path.display());

    let expected_sha256 = if let Some(ref tx) = progress_tx {
        let mut progress =
            crate::models::DownloadState::new("llama-server".to_string(), tmp_filename.clone(), 0);
        let download_state = std::sync::Arc::new(std::sync::atomic::AtomicU8::new(1));
        download_file(
            "llama-server",
            &tmp_filename,
            &download_url,
            &tmp_path,
            &mut progress,
            download_state,
            tx.clone(),
        )
        .await?
    } else {
        let resp = client
            .get(&download_url)
            .header(
                "User-Agent",
                concat!("llm-manager/", env!("CARGO_PKG_VERSION")),
            )
            .send()
            .await?
            .error_for_status()?;

        // Capture the sha2-256 header from the response (GitHub CDN provides this)
        let sha256 = resp
            .headers()
            .get("sha2-256")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_lowercase());

        let mut stream = resp.bytes_stream();
        let mut file = tokio::fs::File::create(&tmp_path).await?;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk).await?;
        }
        sha256
    };

    // Verify archive file is not empty
    let archive_size = std::fs::metadata(&tmp_path)?.len();
    if archive_size == 0 {
        return Err(anyhow::anyhow!(
            "Downloaded archive is empty (0 bytes) from URL: {}",
            download_url
        ));
    }
    tracing::info!("  -> archive downloaded, size: {} bytes", archive_size);

    let extract_dir = bin_dir.join(format!("llama-server-{}-{}.extract", backend.slug(), tag));

    let setup_res: Result<std::path::PathBuf> = async {
        // Verify SHA256 if we received it from the GitHub CDN
        if let Some(expected) = &expected_sha256 {
            let actual = file_sha256(&tmp_path)?;
            if actual != expected.to_lowercase() {
                return Err(anyhow::anyhow!(
                    "SHA256 mismatch for downloaded binary ({}): expected {}, got {}",
                    download_url,
                    expected,
                    actual
                ));
            }
            tracing::info!("  -> SHA256 verified successfully");
        } else {
            tracing::warn!("  -> No sha2-256 header from GitHub, skipping integrity check");
        }

        tracing::info!("  -> download complete, extracting...");

        if let Some(tx) = &log_tx {
            let _ = tx.send("Extracting backend...".to_string()).await;
        }

        extract_archive(&tmp_path, &extract_dir)?;

        // Log extracted archive structure for debugging
        let mut file_count = 0usize;
        let mut dir_count = 0usize;
        walk_dir_recursive(&extract_dir, 0, 10, &mut |entry| {
            if entry.path().is_dir() {
                dir_count += 1;
            } else {
                file_count += 1;
            }
        });
        tracing::info!(
            "  -> archive extracted: {} files, {} directories in {}",
            file_count,
            dir_count,
            extract_dir.display()
        );

        if let Some(tx) = &log_tx {
            let _ = tx.send("Finalizing installation...".to_string()).await;
        }

        // The archive contains llama-xxx/bin/llama-server; find it and move into bin_dir
        let extracted_bin = extract_dir.join(bin_name);
        tracing::info!(
            "  -> looking for binary in extracted archive at: {}",
            extracted_bin.display()
        );
        if extracted_bin.exists() {
            tracing::info!(
                "  -> found binary at expected location, moving to {}",
                bin_path.display()
            );
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
                let path_display = path.display().to_string();
                std::fs::rename(&path, &bin_path)?;
                let bin_size = std::fs::metadata(&bin_path)?.len();
                if bin_size == 0 {
                    anyhow::bail!(
                        "Extracted {} is empty (0 bytes) at {}",
                        bin_name,
                        path_display
                    );
                }
                tracing::info!("  -> extracted {} ({} bytes)", bin_name, bin_size);
            } else {
                anyhow::bail!(
                    "Could not find {} binary in archive at {}",
                    bin_name,
                    extract_dir.display()
                );
            }
        }

        // Also try to extract llama-bench if it exists
        let bench_bin_path = bin_dir.join("llama-bench");
        let mut bench_found = None;
        walk_dir_recursive(&extract_dir, 0, 10, &mut |entry| {
            if entry
                .file_name()
                .to_str()
                .map(|n| n == "llama-bench")
                .unwrap_or(false)
            {
                bench_found = Some(entry.path().to_path_buf());
            }
        });
        if let Some(path) = bench_found {
            let _ = std::fs::rename(path, &bench_bin_path);
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(
                    &bench_bin_path,
                    std::fs::Permissions::from_mode(0o755),
                );
            }
        }

        // Also extract shared libraries from the archive into bin_dir
        let lib_ext = lib_extension();
        let lib_name = lib_sentinel_name();
        let mut libs_found = Vec::new();
        walk_dir_recursive(&extract_dir, 0, 10, &mut |entry| {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.ends_with(lib_ext)
                || name_str.contains(&format!(".{}", lib_ext.trim_start_matches('.')))
            {
                libs_found.push(name_str.to_string());
                let dest = bin_dir.join(name);
                // Preserve symlinks: if source is a symlink, create a symlink at dest
                // pointing to the same target (which is relative within the archive)
                if let Ok(metadata) = entry.path().symlink_metadata() {
                    if metadata.file_type().is_symlink() {
                        if let Ok(target) = std::fs::read_link(entry.path()) {
                            #[cfg(unix)]
                            {
                                let _ = std::os::unix::fs::symlink(&target, &dest);
                            }
                            #[cfg(windows)]
                            {
                                let _ = std::os::windows::fs::symlink_file(&target, &dest);
                            }
                        }
                    } else {
                        let _ = std::fs::copy(entry.path(), dest);
                    }
                } else {
                    let _ = std::fs::copy(entry.path(), dest);
                }
            }
        });
        tracing::info!(
            "  -> extracted {} shared libraries: {:?}",
            libs_found.len(),
            libs_found
        );
        if !bin_dir.join(lib_name).exists() {
            anyhow::bail!(
                "Expected library '{}' not found in archive (found: {:?})",
                lib_name,
                libs_found
            );
        }

        // Make executable (Unix-only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&bin_path, std::fs::Permissions::from_mode(0o755))?;
        }

        Ok(bin_path)
    }
    .await;

    // Clean up temp files
    let _ = tokio::fs::remove_file(&tmp_path).await;
    let _ = tokio::fs::remove_dir_all(&extract_dir).await;

    setup_res
}

/// Compute the SHA256 hash of a file.
pub fn file_sha256(path: &std::path::Path) -> Result<String> {
    use sha2::{Digest, Sha256};
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher)?;
    Ok(hex::encode(hasher.finalize()))
}

/// Extract a .tar.gz or .zip archive into a directory.
/// Lexically normalize a path (resolve `.` and `..` without touching the
/// filesystem). Only sound for absolute paths.
fn normalize_path(p: &std::path::Path) -> std::path::PathBuf {
    let mut out: Vec<&std::ffi::OsStr> = Vec::new();
    for comp in p.components() {
        match comp {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            c => out.push(c.as_os_str()),
        }
    }
    out.iter().copied().collect()
}

pub fn extract_archive(archive_path: &std::path::Path, dest_dir: &std::path::Path) -> Result<()> {
    let filename = archive_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");

    std::fs::create_dir_all(dest_dir)?;
    let dest_dir = dest_dir.canonicalize()?;

    if filename.ends_with(".zip") {
        let file = std::fs::File::open(archive_path)?;
        let mut archive = zip::ZipArchive::new(file)?;
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)?;
            let file_name = entry
                .enclosed_name()
                .ok_or(anyhow::anyhow!("Invalid path in zip"))?;

            let full_path = dest_dir.join(&file_name);
            if !full_path.starts_with(&dest_dir) {
                return Err(anyhow::anyhow!(
                    "Zip slip detected: {} tries to write to {}",
                    file_name.display(),
                    full_path.display()
                ));
            }

            if entry.is_file() {
                if let Some(parent) = full_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let mut outfile = std::fs::File::create(&full_path)?;
                std::io::copy(&mut entry, &mut outfile)?;
            } else if entry.is_dir() {
                std::fs::create_dir_all(&full_path)?;
            }
        }
    } else if filename.ends_with(".tar.gz") || filename.contains(".tar.gz") {
        use flate2::read::GzDecoder;
        use tar::Archive;

        let file = std::fs::File::open(archive_path)?;
        let decoder = GzDecoder::new(file);
        let mut archive = Archive::new(decoder);
        let entries = archive.entries()?;
        // Symlinks created so far: used to reject entries whose path passes
        // through a symlinked parent (which would write outside dest_dir).
        let mut symlinked: Vec<std::path::PathBuf> = Vec::new();
        for entry in entries {
            let mut entry = entry?;
            let entry_path = entry.path()?;
            let full_path = dest_dir.join(&entry_path);
            if !full_path.starts_with(&dest_dir) {
                return Err(anyhow::anyhow!(
                    "Tar slip detected: {} tries to write to {}",
                    entry_path.display(),
                    full_path.display()
                ));
            }
            // Reject entries that would follow a symlink created earlier in
            // the archive (e.g. "link" -> "../outside" then "link/file").
            if let Ok(rel) = full_path.strip_prefix(&dest_dir) {
                let mut ancestor = rel;
                while let Some(parent) = ancestor.parent() {
                    if parent.as_os_str().is_empty() {
                        break;
                    }
                    if symlinked.contains(&dest_dir.join(parent)) {
                        return Err(anyhow::anyhow!(
                            "Tar entry {} follows a symlink inside the archive",
                            entry_path.display()
                        ));
                    }
                    ancestor = parent;
                }
            }
            // Detect directory entries by tar header type, not path suffix
            let is_dir = entry.header().entry_type().is_dir();
            if is_dir {
                std::fs::create_dir_all(&full_path)?;
            } else if entry.header().entry_type().is_symlink() {
                // Preserve symlinks, but only if the target stays inside
                // dest_dir — an escaping target would let later entries (or
                // the user) read/write outside the extraction directory.
                let header = entry.header();
                if let Ok(Some(link_target)) = header.link_name() {
                    let link_target = link_target.into_owned();
                    let target = if link_target.is_absolute() {
                        link_target.clone()
                    } else if let Some(parent) = full_path.parent() {
                        parent.join(&link_target)
                    } else {
                        link_target.clone()
                    };
                    let normalized = normalize_path(&target);
                    if !normalized.starts_with(&dest_dir) {
                        return Err(anyhow::anyhow!(
                            "Tar symlink escape detected: {} -> {}",
                            full_path.display(),
                            link_target.display()
                        ));
                    }
                    if let Some(parent) = full_path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    #[cfg(unix)]
                    {
                        std::os::unix::fs::symlink(&link_target, &full_path)?;
                    }
                    #[cfg(windows)]
                    {
                        std::os::windows::fs::symlink_file(&link_target, &full_path)?;
                    }
                    symlinked.push(full_path.clone());
                }
            } else {
                if let Some(parent) = full_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let mut outfile = std::fs::File::create(&full_path)?;
                std::io::copy(&mut entry, &mut outfile)?;
            }
        }
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

/// Find the latest release tag that has an asset matching the given pattern.
/// Iterates through the last 100 releases and returns the first tag whose
/// release assets contain a file whose name includes `asset_pattern`.
/// Falls back to the provided default tag if no match is found.
async fn latest_release_with_asset(repo: &str, asset_pattern: &str, fallback: &str) -> String {
    let client = reqwest::Client::new();
    let url = format!(
        "https://api.github.com/repos/{}/releases?per_page=100",
        repo
    );
    latest_release_with_asset_inner(&client, &url, asset_pattern, fallback).await
}

/// Fetch the name of the first asset in a specific release whose name
/// contains `pattern`. Returns `None` on any error or when no asset matches.
async fn fetch_release_asset_name(repo: &str, tag: &str, pattern: &str) -> Option<String> {
    let client = reqwest::Client::new();
    let url = format!(
        "https://api.github.com/repos/{}/releases/tags/{}",
        repo, tag
    );
    fetch_release_asset_name_inner(&client, &url, pattern).await
}

async fn fetch_release_asset_name_inner(
    client: &reqwest::Client,
    url: &str,
    pattern: &str,
) -> Option<String> {
    let resp = client
        .get(url)
        .header("Accept", "application/vnd.github.v3+json")
        .header("User-Agent", super::USER_AGENT)
        .send()
        .await
        .ok()?;
    let release: serde_json::Value = resp.error_for_status().ok()?.json().await.ok()?;
    release.get("assets")?.as_array()?.iter().find_map(|asset| {
        asset
            .get("name")
            .and_then(|n| n.as_str())
            .filter(|n| n.contains(pattern))
            .map(|s| s.to_string())
    })
}

async fn latest_release_with_asset_inner(
    client: &reqwest::Client,
    url: &str,
    asset_pattern: &str,
    fallback: &str,
) -> String {
    match client
        .get(url)
        .header("Accept", "application/vnd.github.v3+json")
        .header("User-Agent", super::USER_AGENT)
        .send()
        .await
    {
        Ok(resp) => match resp.error_for_status() {
            Ok(resp) => match resp.json::<Vec<serde_json::Value>>().await {
                Ok(releases) => {
                    let count = releases.len();
                    for release in &releases {
                        if let Some(assets) = release.get("assets").and_then(|v| v.as_array()) {
                            let tag = release
                                .get("tag_name")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| fallback.to_string());
                            for asset in assets {
                                if let Some(name) = asset.get("name").and_then(|v| v.as_str())
                                    && name.contains(asset_pattern)
                                {
                                    tracing::info!(
                                        "  -> found asset '{}' in release '{}'",
                                        name,
                                        tag
                                    );
                                    return tag;
                                }
                            }
                        }
                    }
                    tracing::info!(
                        "  -> no asset matching '{}' found in {} releases, using fallback '{}'",
                        asset_pattern,
                        count,
                        fallback
                    );
                    fallback.to_string()
                }
                Err(_) => fallback.to_string(),
            },
            Err(_) => fallback.to_string(),
        },
        Err(_) => fallback.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn make_releases_response(releases: serde_json::Value) -> ResponseTemplate {
        ResponseTemplate::new(200).set_body_string(releases.to_string())
    }

    #[tokio::test]
    async fn test_latest_release_with_asset_finds_vulkan() {
        let server = MockServer::start().await;

        let releases = serde_json::json!([
            {
                "tag_name": "v3081",
                "assets": [
                    {"name": "llama-v3081-bin-ubuntu-x64.tar.gz"},
                    {"name": "llama-v3081-bin-ubuntu-rocm-7.2-x64.tar.gz"}
                ]
            },
            {
                "tag_name": "v3080",
                "assets": [
                    {"name": "llama-v3080-bin-ubuntu-x64.tar.gz"},
                    {"name": "llama-v3080-bin-ubuntu-vulkan-x64.tar.gz"},
                    {"name": "llama-v3080-bin-ubuntu-rocm-7.2-x64.tar.gz"}
                ]
            }
        ]);

        Mock::given(method("GET"))
            .and(path("/org/repo/releases"))
            .respond_with(make_releases_response(releases))
            .mount(&server)
            .await;

        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .build()
            .unwrap();

        let url = format!("{}/org/repo/releases?per_page=100", server.uri());
        let result = latest_release_with_asset_inner(
            &client,
            &url,
            "bin-ubuntu-vulkan-x64.tar.gz",
            "fallback-tag",
        )
        .await;

        assert_eq!(result, "v3080");
    }

    #[tokio::test]
    async fn test_latest_release_with_asset_no_match_fallback() {
        let server = MockServer::start().await;

        let releases = serde_json::json!([
            {
                "tag_name": "v3081",
                "assets": [
                    {"name": "llama-v3081-bin-ubuntu-x64.tar.gz"}
                ]
            },
            {
                "tag_name": "v3080",
                "assets": [
                    {"name": "llama-v3080-bin-ubuntu-x64.tar.gz"}
                ]
            }
        ]);

        Mock::given(method("GET"))
            .and(path("/org/repo/releases"))
            .respond_with(make_releases_response(releases))
            .mount(&server)
            .await;

        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .build()
            .unwrap();

        let url = format!("{}/org/repo/releases?per_page=100", server.uri());
        let result = latest_release_with_asset_inner(
            &client,
            &url,
            "bin-ubuntu-vulkan-x64.tar.gz",
            "fallback-tag",
        )
        .await;

        assert_eq!(result, "fallback-tag");
    }

    #[tokio::test]
    async fn test_latest_release_with_asset_empty_repo_fallback() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/org/repo/releases"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .build()
            .unwrap();

        let url = format!("{}/org/repo/releases?per_page=100", server.uri());
        let result = latest_release_with_asset_inner(
            &client,
            &url,
            "bin-ubuntu-vulkan-x64.tar.gz",
            "fallback-tag",
        )
        .await;

        assert_eq!(result, "fallback-tag");
    }

    #[tokio::test]
    async fn test_latest_release_with_asset_uses_first_match() {
        let server = MockServer::start().await;

        // Vulkan asset appears in both releases; should pick the first (most recent)
        let releases = serde_json::json!([
            {
                "tag_name": "v3081",
                "assets": [
                    {"name": "llama-v3081-bin-ubuntu-vulkan-x64.tar.gz"}
                ]
            },
            {
                "tag_name": "v3080",
                "assets": [
                    {"name": "llama-v3080-bin-ubuntu-vulkan-x64.tar.gz"}
                ]
            }
        ]);

        Mock::given(method("GET"))
            .and(path("/org/repo/releases"))
            .respond_with(make_releases_response(releases))
            .mount(&server)
            .await;

        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .build()
            .unwrap();

        let url = format!("{}/org/repo/releases?per_page=100", server.uri());
        let result = latest_release_with_asset_inner(
            &client,
            &url,
            "bin-ubuntu-vulkan-x64.tar.gz",
            "fallback-tag",
        )
        .await;

        assert_eq!(result, "v3081");
    }

    #[tokio::test]
    async fn test_fetch_release_asset_name_finds_rocm() {
        let server = MockServer::start().await;
        let release = serde_json::json!({
            "tag_name": "b1234",
            "assets": [
                {"name": "llama-b1234-bin-ubuntu-x64.tar.gz"},
                {"name": "llama-b1234-bin-ubuntu-rocm-x64.tar.gz"}
            ]
        });
        Mock::given(method("GET"))
            .and(path("/org/repo/releases/tags/b1234"))
            .respond_with(ResponseTemplate::new(200).set_body_string(release.to_string()))
            .mount(&server)
            .await;

        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .build()
            .unwrap();
        let url = format!("{}/org/repo/releases/tags/b1234", server.uri());
        let name = fetch_release_asset_name_inner(&client, &url, "bin-ubuntu-rocm").await;
        assert_eq!(
            name,
            Some("llama-b1234-bin-ubuntu-rocm-x64.tar.gz".to_string())
        );
    }

    #[tokio::test]
    async fn test_fetch_release_asset_name_no_match() {
        let server = MockServer::start().await;
        let release = serde_json::json!({
            "tag_name": "b1234",
            "assets": [
                {"name": "llama-b1234-bin-ubuntu-x64.tar.gz"}
            ]
        });
        Mock::given(method("GET"))
            .and(path("/org/repo/releases/tags/b1234"))
            .respond_with(ResponseTemplate::new(200).set_body_string(release.to_string()))
            .mount(&server)
            .await;

        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .build()
            .unwrap();
        let url = format!("{}/org/repo/releases/tags/b1234", server.uri());
        let name = fetch_release_asset_name_inner(&client, &url, "bin-ubuntu-rocm").await;
        assert_eq!(name, None);
    }

    #[tokio::test]
    async fn test_fetch_release_asset_name_error_returns_none() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/org/repo/releases/tags/b1234"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .build()
            .unwrap();
        let url = format!("{}/org/repo/releases/tags/b1234", server.uri());
        let name = fetch_release_asset_name_inner(&client, &url, "bin-ubuntu-rocm").await;
        assert_eq!(name, None);
    }

    #[test]
    fn test_extract_version_number() {
        assert_eq!(extract_version_number("v3081"), 3081);
        assert_eq!(extract_version_number("b4100"), 4100);
        assert_eq!(extract_version_number("v1.2.3"), 123);
        assert_eq!(extract_version_number("abc"), 0);
    }

    #[test]
    fn test_compare_versions() {
        assert_eq!(compare_versions("v3081", "v3080"), "v3081");
        assert_eq!(compare_versions("v3080", "v3081"), "v3081");
        assert_eq!(compare_versions("v3081", "v3081"), "v3081");
        assert_eq!(compare_versions("b9266", "b9279"), "b9279");
        assert_eq!(compare_versions("b9279", "b9266"), "b9279");
    }

    fn tmp_dir(tag: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "llm-manager-test-{}-{}-{}",
            std::process::id(),
            tag,
            nanos
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn append_symlink(tar: &mut tar::Builder<impl std::io::Write>, path: &str, target: &str) {
        let mut hdr = tar::Header::new_gnu();
        hdr.set_path(path).unwrap();
        hdr.set_link_name(target).unwrap();
        hdr.set_entry_type(tar::EntryType::Symlink);
        hdr.set_size(0);
        hdr.set_cksum();
        tar.append(&hdr, std::io::empty()).unwrap();
    }

    fn append_file(tar: &mut tar::Builder<impl std::io::Write>, path: &str, content: &[u8]) {
        let mut hdr = tar::Header::new_gnu();
        hdr.set_path(path).unwrap();
        hdr.set_size(content.len() as u64);
        hdr.set_cksum();
        tar.append(&hdr, content).unwrap();
    }

    fn build_tar_gz(
        path: &std::path::Path,
        prepare: impl FnOnce(&mut tar::Builder<flate2::write::GzEncoder<std::fs::File>>),
    ) {
        let file = std::fs::File::create(path).unwrap();
        let enc = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        let mut tar = tar::Builder::new(enc);
        prepare(&mut tar);
        tar.into_inner().unwrap().finish().unwrap();
    }

    #[test]
    fn test_extract_rejects_symlink_escape() {
        let dir = tmp_dir("tar-symlink-escape");
        let archive = dir.join("evil.tar.gz");
        let dest = dir.join("dest");
        std::fs::create_dir_all(&dest).unwrap();

        build_tar_gz(&archive, |tar| {
            // "link" -> "../outside" escapes the extraction directory
            append_symlink(tar, "link", "../outside");
            // "link/evil.txt" would follow the symlink and write outside
            append_file(tar, "link/evil.txt", b"pwned");
        });

        let res = extract_archive(&archive, &dest);
        assert!(res.is_err(), "symlink escape must be rejected: {:?}", res);
        assert!(
            !dir.join("outside").exists(),
            "nothing may be written outside dest"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_extract_rejects_absolute_symlink_target() {
        let dir = tmp_dir("tar-symlink-abs");
        let archive = dir.join("evil.tar.gz");
        let dest = dir.join("dest");
        std::fs::create_dir_all(&dest).unwrap();

        build_tar_gz(&archive, |tar| {
            append_symlink(tar, "etc", "/etc");
        });

        let res = extract_archive(&archive, &dest);
        assert!(
            res.is_err(),
            "absolute symlink target must be rejected: {:?}",
            res
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_extract_allows_internal_symlink() {
        let dir = tmp_dir("tar-symlink-ok");
        let archive = dir.join("ok.tar.gz");
        let dest = dir.join("dest");
        std::fs::create_dir_all(&dest).unwrap();

        build_tar_gz(&archive, |tar| {
            append_file(tar, "a.txt", b"hello");
            append_symlink(tar, "link", "a.txt");
        });

        let res = extract_archive(&archive, &dest);
        assert!(res.is_ok(), "internal symlink must be allowed: {:?}", res);
        assert_eq!(std::fs::read_to_string(dest.join("link")).unwrap(), "hello");
        std::fs::remove_dir_all(&dir).ok();
    }
}
