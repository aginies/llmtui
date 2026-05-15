use std::fmt::Display;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tracing::info;

use crate::models::{Backend, DiscoveredModel, ModelSettings, ServerMetrics};
use crate::config::Config;

/// Manages a single llama.cpp server process.
#[derive(Clone)]
pub struct ServerHandle {
    pub port: u16,
    pub host: String,
    pub pid: u32,
    pub kill_tx: mpsc::Sender<()>,
}

/// Helper: add an argument if the value differs from default.
fn add_arg(cmd: &mut Command, name: &str, value: impl Display) {
    cmd.arg(name).arg(value.to_string());
}

/// Helper: ensure host string is valid for URL construction.
/// Handles empty strings (defaults to 127.0.0.1), strips display suffixes,
/// and wraps IPv6 addresses in brackets.
fn clean_host(host: &str) -> String {
    let host = host.trim();
    if host.is_empty() {
        return "127.0.0.1".to_string();
    }
    // Remove (xxx) suffixes often used in display, e.g. "localhost (127.0.0.1)"
    let host = host.split_whitespace().next().unwrap_or(host);
    if host.contains(':') && !host.starts_with('[') {
        format!("[{}]", host)
    } else {
        host.to_string()
    }
}

/// Build the full llama-server command line from settings.
/// Returns (Command, display_string) where the string is suitable for logging.
pub fn build_server_cmd(binary: &std::path::Path, model: Option<&DiscoveredModel>, settings: &ModelSettings, _config: &Config) -> (Command, String) {
    let mut cmd = Command::new(binary);
    let mut parts: Vec<String> = vec![binary.display().to_string()];

    // ── Model ───────────────────────────────────────────────
    if let Some(model) = model {
        if settings.server_mode == crate::models::ServerMode::Normal {
            cmd.arg("-m").arg(&model.path);
            parts.push("-m".to_string());
            parts.push(model.path.display().to_string());

            // Add alias for router mode identification (uses the unique relative path)
            cmd.arg("--alias").arg(&model.display_name);
            parts.push("--alias".to_string());
            parts.push(model.display_name.clone());
        } else {
            // Router mode: use --models-max instead of loading a specific model
            if settings.router_max_models > 0 {
                add_arg(&mut cmd, "--models-max", settings.router_max_models);
                parts.push("--models-max".to_string());
                parts.push(settings.router_max_models.to_string());
            }
        }
    } else {
        // Pure router mode
        if settings.router_max_models > 0 {
            add_arg(&mut cmd, "--models-max", settings.router_max_models);
            parts.push("--models-max".to_string());
            parts.push(settings.router_max_models.to_string());
        }
    }

    // ── Loading ──────────────────────────────────────────────
    add_arg(&mut cmd, "--threads", settings.threads);
    parts.push("--threads".to_string());
    parts.push(settings.threads.to_string());
    add_arg(&mut cmd, "--threads-batch", settings.threads_batch);
    parts.push("--threads-batch".to_string());
    parts.push(settings.threads_batch.to_string());
    add_arg(&mut cmd, "--ctx-size", settings.context_length);
    parts.push("--ctx-size".to_string());
    parts.push(settings.context_length.to_string());
    add_arg(&mut cmd, "--ubatch-size", settings.ubatch_size);
    parts.push("--ubatch-size".to_string());
    parts.push(settings.ubatch_size.to_string());
    if settings.parallel > 1 {
        add_arg(&mut cmd, "--parallel", settings.parallel);
        parts.push("--parallel".to_string());
        parts.push(settings.parallel.to_string());
    }
    
    cmd.arg("--no-warmup");
    parts.push("--no-warmup".to_string());

    if let Some(cache_k) = settings.cache_type_k {
        add_arg(&mut cmd, "--cache-type-k", cache_k);
        parts.push("--cache-type-k".to_string());
        parts.push(cache_k.to_string());
    }
    if let Some(cache_v) = settings.cache_type_v {
        add_arg(&mut cmd, "--cache-type-v", cache_v);
        parts.push("--cache-type-v".to_string());
        parts.push(cache_v.to_string());
    }

    if settings.keep != 0 {
        cmd.arg("--keep").arg(settings.keep.to_string());
        parts.push("--keep".to_string());
        parts.push(settings.keep.to_string());
    }
    if settings.swa_full {
        cmd.arg("--swa-full");
        parts.push("--swa-full".to_string());
    }
    if settings.mlock {
        cmd.arg("--mlock");
        parts.push("--mlock".to_string());
    }
    if !settings.mmap {
        cmd.arg("--no-mmap");
        parts.push("--no-mmap".to_string());
    }
    if settings.numa != Default::default() {
        cmd.arg("--numa").arg(settings.numa.to_string());
        parts.push("--numa".to_string());
        parts.push(settings.numa.to_string());
    }
    if settings.kv_cache_offload {
        cmd.arg("--kv-offload");
        parts.push("--kv-offload".to_string());
    }

    // ── GPU ──────────────────────────────────────────────────
    let gpu_layers = if settings.gpu_layers < 0 { 999 } else { settings.gpu_layers };
    cmd.arg("-ngl").arg(gpu_layers.to_string());
    parts.push("-ngl".to_string());
    parts.push(gpu_layers.to_string());
    
    if settings.split_mode != Default::default() {
        cmd.arg("--split-mode").arg(settings.split_mode.to_string());
        parts.push("--split-mode".to_string());
        parts.push(settings.split_mode.to_string());
    }
    if !settings.tensor_split.is_empty() {
        cmd.arg("--tensor-split").arg(&settings.tensor_split);
        parts.push("--tensor-split".to_string());
        parts.push(settings.tensor_split.clone());
    }
    if settings.main_gpu != 0 {
        cmd.arg("--main-gpu").arg(settings.main_gpu.to_string());
        parts.push("--main-gpu".to_string());
        parts.push(settings.main_gpu.to_string());
    }
    if !settings.fit {
        cmd.arg("--fit").arg("off");
        parts.push("--fit".to_string());
        parts.push("off".to_string());
    }

    if let Some(ref lora) = settings.lora {
        add_arg(&mut cmd, "--lora", lora.display());
        parts.push("--lora".to_string());
        parts.push(lora.display().to_string());
    }
    if let Some((ref lora, scale)) = settings.lora_scaled {
        add_arg(&mut cmd, "--lora-scaled", format!("{}:{}", lora.display(), scale));
        parts.push("--lora-scaled".to_string());
        parts.push(format!("{}:{}", lora.display(), scale));
    }
    if !settings.rpc.is_empty() {
        cmd.arg("--rpc").arg(&settings.rpc);
        parts.push("--rpc".to_string());
        parts.push(settings.rpc.clone());
    }
    if settings.embedding {
        cmd.arg("--embedding");
        parts.push("--embedding".to_string());
    }

    if settings.expert_count > 0 {
        cmd.arg("--override-kv").arg(format!("llama.expert_used_count=int:int:{}", settings.expert_count));
        parts.push("--override-kv".to_string());
        parts.push(format!("llama.expert_used_count=int:int:{}", settings.expert_count));
    }

    cmd.arg("-fa").arg(if settings.flash_attn { "on" } else { "off" });
    parts.push("-fa".to_string());
    parts.push(if settings.flash_attn { "on" } else { "off" }.to_string());

    if settings.jinja {
        cmd.arg("--jinja");
        parts.push("--jinja".to_string());
    }

    if let Some(ref template) = settings.chat_template {
        cmd.arg("--chat-template").arg(template);
        parts.push("--chat-template".to_string());
        parts.push(template.clone());
    }

    // ── Sampling ─────────────────────────────────────────────
    if settings.seed != -1 {
        add_arg(&mut cmd, "--seed", settings.seed);
        parts.push("--seed".to_string());
        parts.push(settings.seed.to_string());
    }
    add_arg(&mut cmd, "--temp", format!("{:.2}", settings.temperature));
    parts.push("--temp".to_string());
    parts.push(format!("{:.2}", settings.temperature));

    add_arg(&mut cmd, "--top-k", settings.top_k);
    parts.push("--top-k".to_string());
    parts.push(settings.top_k.to_string());

    add_arg(&mut cmd, "--top-p", format!("{:.2}", settings.top_p));
    parts.push("--top-p".to_string());
    parts.push(format!("{:.2}", settings.top_p));

    add_arg(&mut cmd, "--min-p", format!("{:.2}", settings.min_p));
    parts.push("--min-p".to_string());
    parts.push(format!("{:.2}", settings.min_p));

    add_arg(&mut cmd, "--typical", format!("{:.2}", settings.typical_p));
    parts.push("--typical".to_string());
    parts.push(format!("{:.2}", settings.typical_p));

    if settings.mirostat != Default::default() {
        add_arg(&mut cmd, "--mirostat", settings.mirostat.to_string());
        parts.push("--mirostat".to_string());
        parts.push(settings.mirostat.to_string());

        add_arg(&mut cmd, "--mirostat-lr", format!("{:.2}", settings.mirostat_lr));
        parts.push("--mirostat-lr".to_string());
        parts.push(format!("{:.2}", settings.mirostat_lr));

        add_arg(&mut cmd, "--mirostat-ent", format!("{:.2}", settings.mirostat_ent));
        parts.push("--mirostat-ent".to_string());
        parts.push(format!("{:.2}", settings.mirostat_ent));
    }

    if settings.ignore_eos {
        cmd.arg("--ignore-eos");
        parts.push("--ignore-eos".to_string());
    }

    if !settings.samplers.0.is_empty() {
        cmd.arg("--samplers").arg(&settings.samplers.to_string());
        parts.push("--samplers".to_string());
        parts.push(settings.samplers.to_string());
    }

    if let Some(frequency) = settings.frequency_penalty {
        add_arg(&mut cmd, "--frequency-penalty", format!("{:.2}", frequency));
        parts.push("--frequency-penalty".to_string());
        parts.push(format!("{:.2}", frequency));
    }

    if settings.dry_multiplier != 0.0 {
        add_arg(&mut cmd, "--dry-multiplier", format!("{:.2}", settings.dry_multiplier));
        parts.push("--dry-multiplier".to_string());
        parts.push(format!("{:.2}", settings.dry_multiplier));

        add_arg(&mut cmd, "--dry-base", format!("{:.2}", settings.dry_base));
        parts.push("--dry-base".to_string());
        parts.push(format!("{:.2}", settings.dry_base));

        add_arg(&mut cmd, "--dry-allowed-length", settings.dry_allowed_length);
        parts.push("--dry-allowed-length".to_string());
        parts.push(settings.dry_allowed_length.to_string());

        add_arg(&mut cmd, "--dry-penalty-last-n", settings.dry_penalty_last_n);
        parts.push("--dry-penalty-last-n".to_string());
        parts.push(settings.dry_penalty_last_n.to_string());
    }

    // ── RoPE ─────────────────────────────────────────────────
    if settings.rope_scaling != Default::default() {
        cmd.arg("--rope-scaling").arg(settings.rope_scaling.to_string());
        parts.push("--rope-scaling".to_string());
        parts.push(settings.rope_scaling.to_string());
    }
    if settings.rope_scale != 0.0 {
        cmd.arg("--rope-scale").arg(format!("{:.2}", settings.rope_scale));
        parts.push("--rope-scale".to_string());
        parts.push(format!("{:.2}", settings.rope_scale));
    }
    if settings.rope_freq_base != 0.0 {
        cmd.arg("--rope-freq-base").arg(format!("{:.2}", settings.rope_freq_base));
        parts.push("--rope-freq-base".to_string());
        parts.push(format!("{:.2}", settings.rope_freq_base));
    }
    if settings.rope_freq_scale != 1.0 {
        cmd.arg("--rope-freq-scale").arg(format!("{:.2}", settings.rope_freq_scale));
        parts.push("--rope-freq-scale".to_string());
        parts.push(format!("{:.2}", settings.rope_freq_scale));
    }

    // ── Server ───────────────────────────────────────────────
    let resolved_host = clean_host(&settings.host);
    cmd.arg("--host").arg(&resolved_host);
    parts.push("--host".to_string());
    parts.push(resolved_host);

    cmd.arg("--port").arg(settings.port.to_string());
    parts.push("--port".to_string());
    parts.push(settings.port.to_string());

    add_arg(&mut cmd, "--timeout", settings.timeout);
    parts.push("--timeout".to_string());
    parts.push(settings.timeout.to_string());

    cmd.arg("--metrics");
    parts.push("--metrics".to_string());
    if !settings.cache_prompt {
        cmd.arg("--no-cache-prompt");
        parts.push("--no-cache-prompt".to_string());
    }
    if settings.cache_reuse != 0 {
        cmd.arg("--cache-reuse").arg(settings.cache_reuse.to_string());
        parts.push("--cache-reuse".to_string());
        parts.push(settings.cache_reuse.to_string());
    }
    if !settings.webui {
        cmd.arg("--no-webui");
        parts.push("--no-webui".to_string());
    }

    // ── General ──────────────────────────────────────────────

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
) -> Result<(ServerHandle, String), String> {
    let port = settings.port;

    // Check if port is already in use
    if std::net::TcpListener::bind(("127.0.0.1", port)).is_err() {
        return Err(format!("Port {} is already in use", port));
    }

    // Resolve the backend binary (downloads if needed)
    let binary = match crate::backend::hub::resolve_backend_binary(settings.backend).await {
        Ok(path) => {
            if !path.exists() {
                return Err(format!("llama-server binary not found at: {}", path.display()));
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(metadata) = path.metadata() {
                    if metadata.permissions().mode() & 0o111 == 0 {
                        return Err(format!("llama-server binary is not executable: {}", path.display()));
                    }
                }
            }
            if settings.backend != Backend::Cpu {
                info!("Using backend: {} ({} bytes)", settings.backend, path.metadata().map(|m| m.len()).unwrap_or(0));
            }
            path
        }
        Err(e) => {
            return Err(format!("Failed to resolve backend binary: {}", e));
        }
    };

    let (mut cmd, cmd_string) = build_server_cmd(&binary, model, settings, config);
    cmd.stdout(Stdio::piped())
       .stderr(Stdio::piped());

    // Set LD_LIBRARY_PATH so the binary can find its shared libraries
    let bin_dir = binary.parent().unwrap();
    if let Ok(current) = std::env::var("LD_LIBRARY_PATH") {
        cmd.env("LD_LIBRARY_PATH", format!("{}:{}", bin_dir.display(), current));
    } else {
        cmd.env("LD_LIBRARY_PATH", bin_dir);
    }

    let full_cmd = cmd_string;
    info!("Command: {}", full_cmd);

    let (kill_tx, mut kill_rx) = mpsc::channel::<()>(1);

    let mut child = cmd.spawn().map_err(|e| format!("Failed to start llama-server: {}", e))?;
    let pid = child.id().unwrap_or(0);

    let resolved_host = clean_host(&settings.host);
    info!("Started llama-server on {} port {} (pid={})", resolved_host, port, pid);

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let log_tx_stdout = log_tx.clone();
    tokio::spawn(async move {
        let mut reader = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            let _ = log_tx_stdout.send(line).await;
        }
    });

    let log_tx_stderr = log_tx.clone();
    tokio::spawn(async move {
        let mut reader = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            let _ = log_tx_stderr.send(line).await;
        }
    });

    let log_tx_exit = log_tx.clone();
    tokio::spawn(async move {
        tokio::select! {
            _ = kill_rx.recv() => {
                let _ = child.kill().await;
            }
            status = child.wait() => {
                let msg = format!("ERROR: llama-server (pid={}) exited with status {:?}", pid, status);
                info!("{}", msg);
                let _ = log_tx_exit.send(msg).await;
            }
        }
    });

    Ok((ServerHandle {
        port,
        host: resolved_host,
        pid,
        kill_tx,
    }, full_cmd))
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
        let name = model.strip_suffix(".gguf").unwrap_or(model);
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
    if metrics.ctx_used == 0 {
        if let Ok(health) = get_metrics_health(&host, port).await {
            metrics.ctx_used = health.ctx_used;
            metrics.ctx_max = health.ctx_max;
        }
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
        if let Ok((ram, cpu)) = get_process_metrics(p) {
            if metrics.ram_used == 0 {
                metrics.ram_used = ram;
            }
            if metrics.cpu_usage == 0.0 {
                metrics.cpu_usage = cpu;
            }
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

    if let Some(devices) = devices {
        if let Some(device) = devices.first() {
            // Priority 1: Check root keys (newer amdgpu_top format as provided by user)
            // "VRAM Usage Size": 3070128128, "VRAM Size": 8589934592
            let root_used = device.get("VRAM Usage Size").and_then(|v| v.as_u64());
            let root_total = device.get("VRAM Size").and_then(|v| v.as_u64());
            
            if let (Some(used), Some(total)) = (root_used, root_total) {
                if total > 0 {
                    return Ok((used, total));
                }
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
    }

    Err("Could not find VRAM info in amdgpu_top output".to_string())
}

/// Linux-specific: Get RAM (RSS) and CPU usage for a PID via /proc
fn get_process_metrics(pid: u32) -> Result<(u64, f64), String> {
    #[cfg(target_os = "linux")]
    {
        use std::fs;
        
        // RAM (RSS) from /proc/[pid]/statm (2nd field is RSS in pages)
        let statm = fs::read_to_string(format!("/proc/{}/statm", pid)).map_err(|e| e.to_string())?;
        let pages: u64 = statm.split_whitespace().nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
        let ram = pages * 4096; // assumes 4KB page size, typical for Linux

        // CPU from /proc/[pid]/stat (approximate, since we don't track deltas here yet)
        // For now, we'll just return RAM if CPU delta tracking is too complex for a single call.
        // We can get a rough "average since start" CPU usage.
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
            
            let cpu = if seconds > 0.0 { (total_time / seconds) * 100.0 } else { 0.0 };
            return Ok((ram, cpu));
        }

        return Ok((ram, 0.0));
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
    variants.push(model_id.strip_suffix(".gguf").unwrap_or(model_id).to_string());
    
    // 2. Just the filename
    if let Some(filename) = std::path::Path::new(model_id).file_name().and_then(|f| f.to_str()) {
        variants.push(filename.to_string());
        variants.push(filename.strip_suffix(".gguf").unwrap_or(filename).to_string());
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
    let stripped = model_id.strip_suffix(".gguf").unwrap_or(model_id);
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

            if let Ok(res) = client.post(&url).json(&body).send().await {
                if res.status().is_success() {
                    return Ok(());
                }
            }
        }
    }

    Ok(()) // Silently ignore unload errors as it's often just a cleanup
}
