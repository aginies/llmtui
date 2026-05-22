use std::fmt::Display;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tracing::info;

use crate::models::{DiscoveredModel, ModelSettings, ServerMetrics, strip_gguf, clean_host};
use crate::config::Config;

/// Manages a single llama.cpp server process.
#[derive(Clone)]
pub struct ServerHandle {
    pub port: u16,
    pub host: String,
    pub pid: u32,
    pub kill_tx: mpsc::Sender<()>,
}

/// Helper: add an argument to both the Command and the display parts list.
fn push_arg(cmd: &mut Command, parts: &mut Vec<String>, name: &str, value: impl Display) {
    let val_str = value.to_string();
    cmd.arg(name).arg(&val_str);
    parts.push(name.to_string());
    parts.push(val_str);
}

/// Helper: add a flag (argument without value) to both the Command and display parts.
fn push_flag(cmd: &mut Command, parts: &mut Vec<String>, name: &str) {
    cmd.arg(name);
    parts.push(name.to_string());
}

/// Build the full llama-server command line from settings.
/// Returns (Command, display_string) where the string is suitable for logging.
pub fn build_server_cmd(binary: &std::path::Path, model: Option<&DiscoveredModel>, settings: &ModelSettings, config: &Config, server_mode: crate::models::ServerMode, router_max_models: u32) -> (Command, String) {
    let mut cmd = Command::new(binary);
    let mut parts: Vec<String> = vec![binary.display().to_string()];

    // ── Model ───────────────────────────────────────────────
    match server_mode {
        crate::models::ServerMode::Normal => {
            if let Some(model) = model {
                push_arg(&mut cmd, &mut parts, "-m", model.path.display());
                // Add alias for router mode identification (uses the unique relative path)
                push_arg(&mut cmd, &mut parts, "--alias", &model.display_name);
            }
        }
        crate::models::ServerMode::Router => {
            // Router mode: no model in CLI, use /load API to load models
            if router_max_models > 0 {
                push_arg(&mut cmd, &mut parts, "--models-max", router_max_models);
            }
            // Always pass --models-dir in router mode (global config setting)
            push_arg(&mut cmd, &mut parts, "--models-dir", config.models_dir.display());
        }
        crate::models::ServerMode::Bench => {
            // Should not be reached as Bench uses build_bench_cmd
        }
        crate::models::ServerMode::BenchTune => {
            // Should not be reached as BenchTune uses benchmark tuning function
        }
    }

    // ── Loading ──────────────────────────────────────────────
    push_arg(&mut cmd, &mut parts, "--threads", settings.threads);
    push_arg(&mut cmd, &mut parts, "--threads-batch", settings.threads_batch);
    push_arg(&mut cmd, &mut parts, "--ctx-size", settings.context_length);
    push_arg(&mut cmd, &mut parts, "--ubatch-size", settings.ubatch_size);
    if let Some(n) = settings.max_concurrent_predictions {
        push_arg(&mut cmd, &mut parts, "--parallel", n);
    }
    
    push_flag(&mut cmd, &mut parts, "--no-warmup");

    if settings.is_mtp {
        push_flag(&mut cmd, &mut parts, "--draft-mtp");
        if settings.draft_tokens > 0 {
            push_arg(&mut cmd, &mut parts, "-nd", settings.draft_tokens);
        }
    }

    if let Some(cache_k) = settings.cache_type_k {
        push_arg(&mut cmd, &mut parts, "--cache-type-k", cache_k);
    }
    if let Some(cache_v) = settings.cache_type_v {
        push_arg(&mut cmd, &mut parts, "--cache-type-v", cache_v);
    }

    if settings.keep != 0 {
        push_arg(&mut cmd, &mut parts, "--keep", settings.keep);
    }
    if settings.swa_full {
        push_flag(&mut cmd, &mut parts, "--swa-full");
    }
    if settings.mlock {
        push_flag(&mut cmd, &mut parts, "--mlock");
    }
    if !settings.mmap {
        push_flag(&mut cmd, &mut parts, "--no-mmap");
    }
    if settings.numa != Default::default() {
        push_arg(&mut cmd, &mut parts, "--numa", settings.numa.to_string());
    }
    if settings.kv_cache_offload {
        push_flag(&mut cmd, &mut parts, "--kv-offload");
    }

   // ── GPU ──────────────────────────────────────────────────
    if let crate::models::GpuLayersMode::Specific(n) = settings.gpu_layers_mode {
        push_arg(&mut cmd, &mut parts, "-ngl", n);
    }
    if matches!(settings.gpu_layers_mode, crate::models::GpuLayersMode::All) {
        push_arg(&mut cmd, &mut parts, "-ngl", "999");
    }
    
    if settings.split_mode != Default::default() {
        push_arg(&mut cmd, &mut parts, "--split-mode", settings.split_mode.to_string());
    }
    if !settings.tensor_split.is_empty() {
        push_arg(&mut cmd, &mut parts, "--tensor-split", &settings.tensor_split);
    }
    if settings.main_gpu != 0 {
        push_arg(&mut cmd, &mut parts, "--main-gpu", settings.main_gpu);
    }
    if !settings.fit {
        push_arg(&mut cmd, &mut parts, "--fit", "off");
    }

    if let Some(ref lora) = settings.lora {
        push_arg(&mut cmd, &mut parts, "--lora", lora.display());
    }
    if let Some((ref lora, scale)) = settings.lora_scaled {
        push_arg(&mut cmd, &mut parts, "--lora-scaled", format!("{}:{}", lora.display(), scale));
    }

    let mut rpc_list = Vec::new();
    if !settings.rpc.is_empty() {
        rpc_list.push(settings.rpc.clone());
    }
    for worker in &config.rpc_workers {
        if worker.selected {
            rpc_list.push(format!("{}:{}", worker.ip, worker.port));
        }
    }

    if !rpc_list.is_empty() {
        let joined_rpc = rpc_list.join(",");
        push_arg(&mut cmd, &mut parts, "--rpc", joined_rpc);
    }

    if settings.embedding {
        push_flag(&mut cmd, &mut parts, "--embedding");
    }

    if settings.expert_count > 0 {
        push_arg(&mut cmd, &mut parts, "--override-kv", format!("llama.expert_used_count=int:int:{}", settings.expert_count));
    }

    push_arg(&mut cmd, &mut parts, "-fa", if settings.flash_attn { "on" } else { "off" });

    if settings.jinja {
        push_flag(&mut cmd, &mut parts, "--jinja");
    }

    if let Some(ref template) = settings.chat_template {
        push_arg(&mut cmd, &mut parts, "--chat-template", template);
    }

    // ── Sampling ─────────────────────────────────────────────
    if settings.seed != -1 {
        push_arg(&mut cmd, &mut parts, "--seed", settings.seed);
    }
    if let Some(max_tokens) = settings.max_tokens {
        push_arg(&mut cmd, &mut parts, "--n-predict", max_tokens);
    }
    push_arg(&mut cmd, &mut parts, "--temp", format!("{:.2}", settings.temperature));

    push_arg(&mut cmd, &mut parts, "--top-k", settings.top_k);

    push_arg(&mut cmd, &mut parts, "--top-p", format!("{:.2}", settings.top_p));

    push_arg(&mut cmd, &mut parts, "--min-p", format!("{:.2}", settings.min_p));

    push_arg(&mut cmd, &mut parts, "--typical", format!("{:.2}", settings.typical_p));

    if settings.mirostat != Default::default() {
        push_arg(&mut cmd, &mut parts, "--mirostat", settings.mirostat.to_string());
        push_arg(&mut cmd, &mut parts, "--mirostat-lr", format!("{:.2}", settings.mirostat_lr));
        push_arg(&mut cmd, &mut parts, "--mirostat-ent", format!("{:.2}", settings.mirostat_ent));
    }

    if settings.ignore_eos {
        push_flag(&mut cmd, &mut parts, "--ignore-eos");
    }

    if !settings.samplers.0.is_empty() {
        push_arg(&mut cmd, &mut parts, "--samplers", settings.samplers.to_string());
    }

    if let Some(frequency) = settings.frequency_penalty {
        push_arg(&mut cmd, &mut parts, "--frequency-penalty", format!("{:.2}", frequency));
    }

    if settings.dry_multiplier != 0.0 {
        push_arg(&mut cmd, &mut parts, "--dry-multiplier", format!("{:.2}", settings.dry_multiplier));
        push_arg(&mut cmd, &mut parts, "--dry-base", format!("{:.2}", settings.dry_base));
        push_arg(&mut cmd, &mut parts, "--dry-allowed-length", settings.dry_allowed_length);
        push_arg(&mut cmd, &mut parts, "--dry-penalty-last-n", settings.dry_penalty_last_n);
    }

    // ── RoPE ─────────────────────────────────────────────────
    if settings.rope_scaling != Default::default() {
        push_arg(&mut cmd, &mut parts, "--rope-scaling", settings.rope_scaling.to_string());
    }
    if settings.rope_scale != 0.0 {
        push_arg(&mut cmd, &mut parts, "--rope-scale", format!("{:.2}", settings.rope_scale));
    }
    if settings.rope_freq_base != 0.0 {
        push_arg(&mut cmd, &mut parts, "--rope-freq-base", format!("{:.2}", settings.rope_freq_base));
    }
    if settings.rope_freq_scale != 1.0 {
        push_arg(&mut cmd, &mut parts, "--rope-freq-scale", format!("{:.2}", settings.rope_freq_scale));
    }

    let resolved_host = clean_host(&settings.host);
    push_arg(&mut cmd, &mut parts, "--host", resolved_host);
    push_arg(&mut cmd, &mut parts, "--port", settings.port);
    push_arg(&mut cmd, &mut parts, "--timeout", settings.timeout);

    push_flag(&mut cmd, &mut parts, "--metrics");
    if !settings.cache_prompt {
        push_flag(&mut cmd, &mut parts, "--no-cache-prompt");
    }
    if settings.cache_reuse != 0 {
        push_arg(&mut cmd, &mut parts, "--cache-reuse", settings.cache_reuse);
    }
    if !settings.webui {
        push_flag(&mut cmd, &mut parts, "--no-webui");
    }

    // ── General ──────────────────────────────────────────────

    let display = parts.join(" ");
    (cmd, display)
}

/// Build the full llama-bench command line.
pub fn build_bench_cmd(binary: &std::path::Path, model: &DiscoveredModel, settings: &ModelSettings) -> (Command, String) {
    let mut cmd = Command::new(binary);
    let mut parts: Vec<String> = vec![binary.display().to_string()];

    push_arg(&mut cmd, &mut parts, "-m", model.path.display());
    push_arg(&mut cmd, &mut parts, "-t", settings.threads);
    push_arg(&mut cmd, &mut parts, "-b", settings.batch_size);

    if let crate::models::GpuLayersMode::Specific(n) = settings.gpu_layers_mode {
        push_arg(&mut cmd, &mut parts, "-ngl", n);
    } else if matches!(settings.gpu_layers_mode, crate::models::GpuLayersMode::All) {
        push_arg(&mut cmd, &mut parts, "-ngl", "999");
    }

    if settings.flash_attn {
        push_arg(&mut cmd, &mut parts, "-fa", "1");
    }

    if settings.is_mtp {
        push_flag(&mut cmd, &mut parts, "--draft-mtp");
        if settings.draft_tokens > 0 {
            push_arg(&mut cmd, &mut parts, "-nd", settings.draft_tokens);
        }
    }

    push_flag(&mut cmd, &mut parts, "--progress");

    let display = parts.join(" ");
    (cmd, display)
}

/// Spawn a llama.cpp server process (single model or router).
/// Returns (ServerHandle, command_string) where command_string is the full CLI.
pub async fn spawn_server(
    config: &Config,
    model: Option<&DiscoveredModel>,
    settings: &ModelSettings,
    log_tx: mpsc::Sender<String>,
    progress_tx: Option<tokio::sync::broadcast::Sender<crate::models::DownloadState>>,
    server_mode: crate::models::ServerMode,
    router_max_models: u32,
) -> Result<(ServerHandle, String), String> {
    if server_mode != crate::models::ServerMode::Bench && server_mode != crate::models::ServerMode::BenchTune {
        let port = settings.port;
        // Check if port is already in use
        if std::net::TcpListener::bind(("127.0.0.1", port)).is_err() {
            return Err(format!("Port {} is already in use", port));
        }
    }

    // For BenchTune mode, we don't spawn a server process
    if server_mode == crate::models::ServerMode::BenchTune {
        // Handle benchmark tuning in main.rs instead
        return Err("BenchTune mode requires special handling in main.rs".to_string());
    }

    // Resolve the backend binary (downloads if needed)
    let backend_name = if server_mode == crate::models::ServerMode::Bench {
        "llama-bench"
    } else {
        "llama-server"
    };
    let version_display = settings.get_active_backend_version_display();
    log_tx.send(format!("Resolving {} (v{}) binary...", backend_name, version_display)).await.ok();
    let version_param = settings.get_active_backend_version().map(|s| s.as_str());

    let server_binary = match crate::backend::hub::resolve_backend_binary(settings.backend, version_param, Some(log_tx.clone()), progress_tx).await {
        Ok(path) => path,
        Err(e) => {
            return Err(format!("Failed to resolve backend binary: {}", e));
        }
    };

    let binary = if server_mode == crate::models::ServerMode::Bench {
        server_binary.parent().unwrap().join("llama-bench")
    } else {
        server_binary
    };

    if !binary.exists() {
        return Err(format!("Binary not found at: {}", binary.display()));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = binary.metadata()
            && metadata.permissions().mode() & 0o111 == 0 {
                return Err(format!("Binary is not executable: {}", binary.display()));
            }
    }

    let (mut cmd, cmd_string) = if server_mode == crate::models::ServerMode::Bench {
        if let Some(m) = model {
            build_bench_cmd(&binary, m, settings)
        } else {
            return Err("Model required for benchmark".to_string());
        }
    } else {
        build_server_cmd(&binary, model, settings, config, server_mode, router_max_models)
    };

    cmd.stdout(Stdio::piped())
       .stderr(Stdio::piped());

    // Set LD_LIBRARY_PATH so the binary can find its shared libraries
    let bin_dir = binary.parent().unwrap();
    if let Ok(current) = std::env::var("LD_LIBRARY_PATH") {
        cmd.env("LD_LIBRARY_PATH", format!("{}:{}", bin_dir.display(), current));
    } else {
        cmd.env("LD_LIBRARY_PATH", bin_dir);
    }

    info!("Spawning: {}", cmd_string);
    let _ = log_tx.send(format!("{}: {}", backend_name, cmd_string)).await;
    let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn process: {}", e))?;
    let pid = child.id().unwrap_or(0);

    let (kill_tx, mut kill_rx) = mpsc::channel(1);

    // Background task to manage the process
    let log_tx_inner = log_tx.clone();
    let backend_name_upper = backend_name.to_uppercase();
    tokio::spawn(async move {
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();

        let mut stdout_reader = BufReader::new(stdout).lines();
        let mut stderr_reader = BufReader::new(stderr).lines();

        loop {
            tokio::select! {
                _ = kill_rx.recv() => {
                    let _ = child.kill().await;
                    break;
                }
                res = stdout_reader.next_line() => {
                    if let Ok(Some(line)) = res {
                        let _ = log_tx_inner.send(line).await;
                    } else {
                        break;
                    }
                }
                res = stderr_reader.next_line() => {
                    if let Ok(Some(line)) = res {
                        let _ = log_tx_inner.send(line).await;
                    }
                }
            }
        }
        let _ = child.wait().await;
        let _ = log_tx_inner.send(format!("{} EXITED", backend_name_upper)).await;
    });

    Ok((ServerHandle {
        port: if server_mode == crate::models::ServerMode::Bench { 0 } else { settings.port },
        host: settings.host.clone(),
        pid,
        kill_tx,
    }, cmd_string))
}

/// Check if the server is healthy and responsive.
pub async fn check_health(host: &str, port: u16) -> bool {
    let host = clean_host(host);
    let url = format!("http://{}:{}/health", host, port);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(1))
        .build()
        .unwrap_or_default();
    
    match client.get(&url).send().await {
        Ok(resp) => resp.status().is_success(),
        Err(_) => false,
    }
}

/// Kill a running server.
pub async fn kill_server(handle: ServerHandle) -> Result<(), String> {
    handle.kill_tx.send(()).await.map_err(|_| "Server already stopped".to_string())
}

/// Poll metrics from the server.
pub async fn get_metrics(host: &str, port: u16, model_name: Option<&str>, pid: Option<u32>) -> Result<ServerMetrics, String> {
    let host = clean_host(host);
    // We prefer the /metrics endpoint as it's more stable for system info.
    // In router mode, we can specify the model via query parameter.
    let mut url = if let Some(model) = model_name {
        let name = strip_gguf(model);
        format!("http://{}:{}/metrics?model={}", host, port, name)
    } else {
        format!("http://{}:{}/metrics", host, port)
    };
    
    let mut resp = reqwest::get(&url).await.map_err(|e| format!("Failed to get metrics: {}", e))?;

    // If model-specific metrics fail with 404 or 400, try plain /metrics
    if (resp.status() == reqwest::StatusCode::NOT_FOUND || resp.status() == reqwest::StatusCode::BAD_REQUEST) && model_name.is_some() {
        url = format!("http://{}:{}/metrics", host, port);
        resp = reqwest::get(&url).await.map_err(|e| format!("Failed to get metrics: {}", e))?;
    }

    if !resp.status().is_success() {
        return Err(format!("Server returned {}", resp.status()));
    }

    let text = resp.text().await.map_err(|e| format!("Failed to read metrics: {}", e))?;
    
    let mut metrics = ServerMetrics::default();
    metrics.loaded = true;

    for line in text.lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }

        let name_with_labels = parts[0];
        // The value is usually the first numeric part after the name
        let mut val = 0.0;
        for part in parts.iter().skip(1) {
            if let Ok(v) = part.parse::<f64>() {
                val = v;
                break;
            }
        }

        // Strip labels from name for matching: llama_kv_cache_usage_bytes{pool="default"} -> llama_kv_cache_usage_bytes
        let name = name_with_labels.split('{').next().unwrap_or(name_with_labels);

        // Match common llama.cpp metric names (they evolve frequently)
        match name {
            // VRAM (usually KV cache usage)
            "llama_kv_cache_usage_bytes" | "kv_cache_usage_bytes" | "llama_server_kv_cache_usage_bytes" | "llama_server_kv_cache_used_bytes" => {
                metrics.gpu_mem_used = val as u64;
            }
            "llama_kv_cache_total_bytes" | "kv_cache_total_bytes" | "llama_server_kv_cache_total_bytes" => {
                metrics.gpu_mem_total = val as u64;
            }
            // RAM (Model weights and general memory)
            "llama_model_memory_usage_bytes" | "model_memory_usage_bytes" | "llama_server_model_memory_usage_bytes" | "llama_server_memory_usage_bytes" | "llama_server_ram_usage_bytes" | "llama_server_mem_used_bytes" => {
                metrics.ram_used = val as u64;
            }
            // Context Tokens
            "llama_kv_cache_tokens_used" | "kv_cache_tokens_used" | "llama_server_kv_cache_tokens_used" | "llamacpp:n_tokens_max" => {
                metrics.ctx_used = val as u32;
            }
            "llama_kv_cache_tokens_total" | "kv_cache_tokens_total" | "llama_server_kv_cache_tokens_total" | "llamacpp:n_ctx" => {
                metrics.ctx_max = val as u32;
            }
            // CPU
            "llama_server_cpu_usage_percentage" | "cpu_usage_percentage" | "llama_server_cpu_usage" | "llama_server_cpu_percent" => {
                metrics.cpu_usage = val;
            }
            // TPS / Throughput
            "llamacpp:predicted_tokens_seconds" => {
                metrics.tps = val;
            }
            "llamacpp:prompt_tokens_seconds" => {
                metrics.prompt_tps = val;
            }
            // KV Cache Ratio Fallback
            "llamacpp:kv_cache_usage_ratio" => {
                // If we don't have absolute VRAM but have the ratio, we can't show bytes
                // but we can at least ensure it's used as a fallback for ctx_used calculation later if needed.
                if metrics.ctx_used == 0 && metrics.ctx_max > 0 {
                    metrics.ctx_used = (val * metrics.ctx_max as f64) as u32;
                }
            }
            _ => {}
        }
    }

    // Try /health as last resort for context usage
    if metrics.ctx_used == 0
        && let Ok(health) = get_metrics_health(&host, port).await {
            metrics.ctx_used = health.ctx_used;
            metrics.ctx_max = health.ctx_max;
        }

    // Prefer actual GPU memory usage from nvidia-smi or amdgpu_top.
    // llama-server's kv_cache_usage_bytes only reports KV cache (typically 10%
    // of total VRAM); model weights are loaded into GPU memory but not tracked
    // by the server, so we use system-level tools to report what users see on GPUs.
    if model_name.is_none() {
        // Prefer system-level VRAM over llama-server's KV-only value.
        // System tools report actual GPU memory including model weights,
        // which is what users see on their GPUs and expect to read.
        let set_if_better = |out: &mut ServerMetrics, used: u64, total: u64| {
            if out.gpu_mem_used == 0 || used > out.gpu_mem_used {
                out.gpu_mem_used = used;
                out.gpu_mem_total = total;
            }
        };

        let (nv_used, nv_total) = get_nvidia_vram_metrics().unwrap_or((0, 0));
        set_if_better(&mut metrics, nv_used, nv_total);

        if metrics.gpu_mem_total == 0 {
            // AMD fallback when nvidia-smi is not available.
            let (amd_used, amd_total) = get_amdgpu_vram_metrics().unwrap_or((0, 0));
            set_if_better(&mut metrics, amd_used, amd_total);
        }
    } else if metrics.gpu_mem_used == 0 {
        // KV-only queries: use system tools as a last resort.
        if let Ok((used, total)) = get_nvidia_vram_metrics() {
            metrics.gpu_mem_used = used;
            metrics.gpu_mem_total = total;
        } else if let Ok((used, total)) = get_amdgpu_vram_metrics() {
            metrics.gpu_mem_used = used;
            metrics.gpu_mem_total = total;
        }
    }

    // Fallback for RAM and CPU using /proc if available (Linux)
    if let Some(p) = pid {
        let cpu_ticks_prev = metrics.cpu_ticks_prev;
        let system_uptime_prev = metrics.system_uptime_prev;
        if let Ok((ram, cpu)) = get_process_metrics(p, cpu_ticks_prev, system_uptime_prev) {
            if metrics.ram_used == 0 {
                metrics.ram_used = ram;
            }
            if metrics.cpu_usage == 0.0 {
                metrics.cpu_usage = cpu;
            }
            metrics.cpu_ticks_prev = cpu_ticks_prev;
            metrics.system_uptime_prev = system_uptime_prev;
        }
    }

    Ok(metrics)
}

/// Get VRAM usage using nvidia-smi
fn get_nvidia_vram_metrics() -> Result<(u64, u64), String> {
    let output = std::process::Command::new("nvidia-smi")
        .args(["--query-gpu=memory.used,memory.total", "--format=csv,noheader,nounits"])
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err("nvidia-smi failed".to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.lines().next().ok_or("No output from nvidia-smi")?;
    let parts: Vec<&str> = line.split(',').collect();
    if parts.len() >= 2 {
        let used = parts[0].trim().parse::<u64>().unwrap_or(0) * 1024 * 1024;
        let total = parts[1].trim().parse::<u64>().unwrap_or(0) * 1024 * 1024;
        return Ok((used, total));
    }

    Err("Invalid output from nvidia-smi".to_string())
}

/// Get VRAM usage using amdgpu_top
fn get_amdgpu_vram_metrics() -> Result<(u64, u64), String> {
    let output = std::process::Command::new("amdgpu_top")
        .args(["-d", "--json"])
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err("amdgpu_top failed".to_string());
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
    
    // amdgpu_top --json output has a "devices" array (or sometimes just a list of objects depending on version)
    let devices = if json.is_array() {
        json.as_array()
    } else {
        json.get("devices").and_then(|d| d.as_array())
    };

    if let Some(devices) = devices
        && let Some(device) = devices.first() {
            // Priority 1: Check root keys (newer amdgpu_top format as provided by user)
            // "VRAM Usage Size": 3070128128, "VRAM Size": 8589934592
            let root_used = device.get("VRAM Usage Size").and_then(|v| v.as_u64());
            let root_total = device.get("VRAM Size").and_then(|v| v.as_u64());
            
            if let (Some(used), Some(total)) = (root_used, root_total)
                && total > 0 {
                    return Ok((used, total));
                }

            // Priority 2: Check nested VRAM object (alternative format)
            let vram_obj = device.get("VRAM");
            if let Some(vram) = vram_obj {
                // Check if it's the "Total VRAM Usage" format (usually MiB)
                let nested_used = vram.get("Total VRAM Usage")
                    .and_then(|v| v.get("value").or(Some(v)))
                    .and_then(|v| v.as_u64());
                let nested_total = vram.get("Total VRAM")
                    .and_then(|v| v.get("value").or(Some(v)))
                    .and_then(|v| v.as_u64());

                if let (Some(used), Some(total)) = (nested_used, nested_total) {
                    // These are usually in MiB if they have a "unit" field
                    let multiplier = if vram.get("Total VRAM").and_then(|v| v.get("unit")).is_some() {
                        1024 * 1024
                    } else {
                        1
                    };
                    if total > 0 {
                        return Ok((used * multiplier, total * multiplier));
                    }
                }
            }
            
            // Priority 3: Check vram_usage key (older format)
            let vram_usage = device.get("vram_usage");
            if let Some(vram) = vram_usage {
                let used = vram.get("VRAM").or_else(|| vram.get("usage"))
                    .and_then(|v| v.get("value").or(Some(v)))
                    .and_then(|v| v.as_u64()).unwrap_or(0);
                let total = vram.get("TotalVRAM").or_else(|| vram.get("total"))
                    .and_then(|v| v.get("value").or(Some(v)))
                    .and_then(|v| v.as_u64()).unwrap_or(0);
                
                if total > 0 {
                    return Ok((used * 1024 * 1024, total * 1024 * 1024));
                }
            }
        }

    Err("Could not find VRAM info in amdgpu_top output".to_string())
}

/// Linux-specific: Get RAM (RSS) and CPU usage for a PID via /proc
fn get_process_metrics(pid: u32, cpu_ticks_prev: u64, system_uptime_prev: f64) -> Result<(u64, f64), String> {
    #[cfg(target_os = "linux")]
    {
        use std::fs;
        
        // RAM (RSS) from /proc/[pid]/statm (2nd field is RSS in pages)
        let statm = fs::read_to_string(format!("/proc/{}/statm", pid)).map_err(|e| e.to_string())?;
        let pages: u64 = statm.split_whitespace().nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
        let ram = pages * 4096; // assumes 4KB page size, typical for Linux

        // CPU from /proc/[pid]/stat - compute delta-based CPU usage
        let stat = fs::read_to_string(format!("/proc/{}/stat", pid)).map_err(|e| e.to_string())?;
        let parts: Vec<&str> = stat.split_whitespace().collect();
        if parts.len() > 14 {
            let utime: u64 = parts[13].parse().unwrap_or(0);
            let stime: u64 = parts[14].parse().unwrap_or(0);
            let start_time: u64 = parts[21].parse().unwrap_or(0);
            
            let uptime = fs::read_to_string("/proc/uptime").unwrap_or_default();
            let system_uptime: f64 = uptime.split_whitespace().next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
            
            let clk_tck = 100.0; // typical
            let total_time = (utime + stime) as f64 / clk_tck;
            let seconds = system_uptime - (start_time as f64 / clk_tck);
            
            let cpu = if cpu_ticks_prev > 0 && system_uptime_prev > 0.0 {
                let ticks_delta = (utime + stime) as f64 - cpu_ticks_prev as f64;
                let wall_delta = system_uptime - system_uptime_prev;
                if wall_delta > 0.0 {
                    (ticks_delta / clk_tck / wall_delta) * 100.0
                } else {
                    0.0
                }
            } else if seconds > 0.0 {
                // First call: fall back to average since start
                (total_time / seconds) * 100.0
            } else {
                0.0
            };
            return Ok((ram, cpu));
        }

        Ok((ram, 0.0))
    }

    #[cfg(not(target_os = "linux"))]
    Err("OS not supported for process metrics".to_string())
}

/// Poll /health for context info (newer llama.cpp)
async fn get_metrics_health(host: &str, port: u16) -> Result<ServerMetrics, String> {
    let host = clean_host(host);
    let url = format!("http://{}:{}/health", host, port);
    let resp = reqwest::get(&url).await.map_err(|e| format!("Failed to get health metrics: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Server returned {}", resp.status()));
    }

    let health: serde_json::Value = resp.json().await.map_err(|e| format!("Invalid response: {}", e))?;

    let mut metrics = ServerMetrics::default();
    metrics.loaded = true;

    // Newer llama.cpp has "slots" array in health
    if let Some(slots) = health.get("slots").and_then(|v| v.as_array()) {
        if let Some(slot) = slots.first() {
            metrics.ctx_used = slot.get("n_past").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            metrics.ctx_max = slot.get("n_ctx").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        }
    } else {
        // Fallback for older llama.cpp where it might be at the top level
        metrics.ctx_used = health.get("n_past").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        metrics.ctx_max = health.get("n_ctx").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    }

    Ok(metrics)
}

/// Load a model via the llama-server Router API.
pub async fn load_model(host: &str, port: u16, model_id: &str, model_path: Option<&str>) -> Result<(), String> {
    let client = reqwest::Client::new();
    let host = clean_host(host);
    
    // Try multiple endpoints
    let endpoints = ["/models/load", "/v1/models/load"];
    
    // Construct all possible identification variants
    let mut variants = Vec::new();
    
    // 1. Original ID (display_name / relative path)
    variants.push(model_id.to_string());
    variants.push(strip_gguf(model_id).to_string());
    
    // 2. Just the filename
    if let Some(filename) = std::path::Path::new(model_id).file_name().and_then(|f| f.to_str()) {
        variants.push(filename.to_string());
        variants.push(strip_gguf(filename).to_string());
    }

    // 3. Absolute path
    if let Some(path) = model_path {
        variants.push(path.to_string());
    }

    let mut last_status = reqwest::StatusCode::OK;
    let mut last_error = String::new();

    for endpoint in endpoints {
        let url = format!("http://{}:{}{}", host, port, endpoint);
        for variant in &variants {
            // Try both "model" and "alias" fields
            let bodies = vec![
                serde_json::json!({ "model": variant }),
                serde_json::json!({ "alias": variant }),
            ];

            for body in bodies {
                match client.post(&url).json(&body).send().await {
                    Ok(res) => {
                        if res.status().is_success() {
                            return Ok(());
                        }
                        last_status = res.status();
                        last_error = res.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                    }
                    Err(e) => {
                        last_error = e.to_string();
                    }
                }
            }
        }
    }

    Err(format!("Failed to load model (tried {} variants). Last status {}: {}", variants.len() * 2, last_status, last_error))
}

/// List all models and their status from the llama-server Router API.
pub async fn list_models(host: &str, port: u16) -> Result<Vec<(String, String, Option<String>)>, String> {
    let client = reqwest::Client::new();
    let host = clean_host(host);
    let url = format!("http://{}:{}/models", host, port);
    
    let res = client.get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to list models: {}", e))?;

    if !res.status().is_success() {
        return Err(format!("Server returned error {}", res.status()));
    }

    let json: serde_json::Value = res.json().await.map_err(|e| format!("Invalid JSON: {}", e))?;
    
    let mut results = Vec::new();
    if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
        for model in data {
            let id = model.get("id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            // Status can be a string or an object with a "value" field
            let status = model.get("status")
                .and_then(|s| s.get("value").or(Some(s)))
                .and_then(|v| v.as_str())
                .unwrap_or("unloaded")
                .to_string();
            let path = model.get("path").or_else(|| model.get("filename"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
                
            results.push((id, status, path));
        }
    }

    Ok(results)
}

/// Unload a model via the llama-server Router API.
pub async fn unload_model(host: &str, port: u16, model_id: &str, model_path: Option<&str>) -> Result<(), String> {
    let client = reqwest::Client::new();
    let host = clean_host(host);

    let endpoints = ["/models/unload", "/v1/models/unload"];
    let stripped = strip_gguf(model_id);
    let mut variants = vec![model_id.to_string(), stripped.to_string()];
    if let Some(path) = model_path {
        variants.push(path.to_string());
    }

    for endpoint in endpoints {
        let url = format!("http://{}:{}{}", host, port, endpoint);
        for variant in &variants {
            let body = serde_json::json!({
                "model": variant
            });

            if let Ok(res) = client.post(&url).json(&body).send().await
                && res.status().is_success() {
                    return Ok(());
                }
        }
    }

    Ok(()) // Silently ignore unload errors as it's often just a cleanup
}
