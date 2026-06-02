use std::fmt::Display;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tracing::info;

use crate::config::Config;
use crate::models::{
    DiscoveredModel, ModelSettings, RopeScaling, ServerMetrics, clean_host, strip_gguf,
};

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

fn push_gpu_layers(cmd: &mut Command, parts: &mut Vec<String>, settings: &ModelSettings) {
    match settings.gpu_layers_mode {
        crate::models::GpuLayersMode::Specific(n) => push_arg(cmd, parts, "-ngl", n),
        crate::models::GpuLayersMode::All => push_arg(cmd, parts, "-ngl", "999"),
        crate::models::GpuLayersMode::Auto => {}
    }
}

fn push_spec_decoding(cmd: &mut Command, parts: &mut Vec<String>, settings: &ModelSettings) {
    if !settings.spec_type.is_empty() {
        push_arg(cmd, parts, "--spec-type", &settings.spec_type);
        if settings.draft_tokens > 0 {
            push_arg(cmd, parts, "--spec-draft-n-max", settings.draft_tokens);
        }
    }
}

/// Build the full llama-server command line from settings.
/// Returns (Command, display_string) where the string is suitable for logging.
pub fn build_server_cmd(
    binary: &std::path::Path,
    model: Option<&DiscoveredModel>,
    settings: &ModelSettings,
    config: &Config,
    server_mode: crate::models::ServerMode,
    router_max_models: u32,
) -> (Command, String) {
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
            if let Some(dir) = config.models_dirs.first() {
                push_arg(&mut cmd, &mut parts, "--models-dir", dir.display());
            }
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
    push_arg(
        &mut cmd,
        &mut parts,
        "--threads-batch",
        settings.threads_batch,
    );
    let effective_ctx = (settings.context_length as f32 * settings.rope_scale) as u32;
    push_arg(&mut cmd, &mut parts, "--ctx-size", effective_ctx);
    push_arg(&mut cmd, &mut parts, "--ubatch-size", settings.ubatch_size);
    if let Some(n) = settings.max_concurrent_predictions {
        push_arg(&mut cmd, &mut parts, "--parallel", n);
    }

    push_flag(&mut cmd, &mut parts, "--no-warmup");

    push_spec_decoding(&mut cmd, &mut parts, settings);

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
    push_gpu_layers(&mut cmd, &mut parts, settings);

    if settings.split_mode != Default::default() {
        push_arg(
            &mut cmd,
            &mut parts,
            "--split-mode",
            settings.split_mode.to_string(),
        );
    }
    if !settings.tensor_split.is_empty() {
        push_arg(
            &mut cmd,
            &mut parts,
            "--tensor-split",
            &settings.tensor_split,
        );
    }
    if settings.main_gpu != 0 {
        push_arg(&mut cmd, &mut parts, "--main-gpu", settings.main_gpu);
    }
    if settings.fit {
        push_arg(&mut cmd, &mut parts, "--fit", "on");
    } else {
        push_arg(&mut cmd, &mut parts, "--fit", "off");
    }

    if let Some(ref lora) = settings.lora {
        push_arg(&mut cmd, &mut parts, "--lora", lora.display());
    }
    if let Some((ref lora, scale)) = settings.lora_scaled {
        push_arg(
            &mut cmd,
            &mut parts,
            "--lora-scaled",
            format!("{}:{}", lora.display(), scale),
        );
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
        push_arg(
            &mut cmd,
            &mut parts,
            "--override-kv",
            format!("llama.expert_used_count=int:int:{}", settings.expert_count),
        );
    }

    push_arg(
        &mut cmd,
        &mut parts,
        "-fa",
        if settings.flash_attn { "on" } else { "off" },
    );

    if settings.jinja {
        push_flag(&mut cmd, &mut parts, "--jinja");
    }

    if let Some(ref template) = settings.chat_template {
        push_arg(&mut cmd, &mut parts, "--chat-template", template);
    }

    // Inject system prompt via chat template kwargs when it differs from default
    if settings.system_prompt != "You are a helpful assistant." {
        let escaped = settings
            .system_prompt
            .replace('\\', "\\\\")
            .replace('"', "\\\"");
        let mut merged = serde_json::Map::new();
        if let Some(ref kwargs) = settings.chat_template_kwargs
            && let Ok(obj) = serde_json::from_str::<serde_json::Value>(kwargs)
                && let serde_json::Value::Object(map) = obj {
                    for (k, v) in map {
                        merged.insert(k, v);
                    }
                }
        merged.insert(
            "system_prompt".to_string(),
            serde_json::Value::String(escaped),
        );
        push_arg(
            &mut cmd,
            &mut parts,
            "--chat-template-kwargs",
            serde_json::to_string(&merged).unwrap(),
        );
    } else if let Some(ref kwargs) = settings.chat_template_kwargs {
        push_arg(&mut cmd, &mut parts, "--chat-template-kwargs", kwargs);
    }

    // ── Sampling ─────────────────────────────────────────────
    if settings.seed != -1 {
        push_arg(&mut cmd, &mut parts, "--seed", settings.seed);
    }
    if let Some(max_tokens) = settings.max_tokens {
        push_arg(&mut cmd, &mut parts, "--n-predict", max_tokens);
    }
    push_arg(
        &mut cmd,
        &mut parts,
        "--temp",
        format!("{:.2}", settings.temperature),
    );

    push_arg(&mut cmd, &mut parts, "--top-k", settings.top_k);

    push_arg(
        &mut cmd,
        &mut parts,
        "--top-p",
        format!("{:.2}", settings.top_p),
    );

    push_arg(
        &mut cmd,
        &mut parts,
        "--min-p",
        format!("{:.2}", settings.min_p),
    );

    push_arg(
        &mut cmd,
        &mut parts,
        "--typical",
        format!("{:.2}", settings.typical_p),
    );

    if settings.mirostat != Default::default() {
        push_arg(
            &mut cmd,
            &mut parts,
            "--mirostat",
            settings.mirostat.to_string(),
        );
        push_arg(
            &mut cmd,
            &mut parts,
            "--mirostat-lr",
            format!("{:.2}", settings.mirostat_lr),
        );
        push_arg(
            &mut cmd,
            &mut parts,
            "--mirostat-ent",
            format!("{:.2}", settings.mirostat_ent),
        );
    }

    if settings.ignore_eos {
        push_flag(&mut cmd, &mut parts, "--ignore-eos");
    }

    if !settings.samplers.0.is_empty() {
        push_arg(
            &mut cmd,
            &mut parts,
            "--samplers",
            settings.samplers.to_string(),
        );
    }

    if let Some(frequency) = settings.frequency_penalty {
        push_arg(
            &mut cmd,
            &mut parts,
            "--frequency-penalty",
            format!("{:.2}", frequency),
        );
    }

    if settings.dry_multiplier != 0.0 {
        push_arg(
            &mut cmd,
            &mut parts,
            "--dry-multiplier",
            format!("{:.2}", settings.dry_multiplier),
        );
        push_arg(
            &mut cmd,
            &mut parts,
            "--dry-base",
            format!("{:.2}", settings.dry_base),
        );
        push_arg(
            &mut cmd,
            &mut parts,
            "--dry-allowed-length",
            settings.dry_allowed_length,
        );
        push_arg(
            &mut cmd,
            &mut parts,
            "--dry-penalty-last-n",
            settings.dry_penalty_last_n,
        );
    }

    // ── RoPE ─────────────────────────────────────────────────
    let rope_scaling = if settings.rope_yarn_enabled {
        RopeScaling::Yarn
    } else {
        settings.rope_scaling
    };
    if rope_scaling != Default::default() {
        push_arg(
            &mut cmd,
            &mut parts,
            "--rope-scaling",
            rope_scaling.to_string(),
        );
    }
    if settings.rope_scale != 1.0 {
        push_arg(
            &mut cmd,
            &mut parts,
            "--rope-scale",
            format!("{:.2}", settings.rope_scale),
        );
    }
    if settings.rope_freq_base != 0.0 {
        push_arg(
            &mut cmd,
            &mut parts,
            "--rope-freq-base",
            format!("{:.2}", settings.rope_freq_base),
        );
    }
    if settings.rope_freq_scale != 1.0 {
        push_arg(
            &mut cmd,
            &mut parts,
            "--rope-freq-scale",
            format!("{:.2}", settings.rope_freq_scale),
        );
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
pub fn build_bench_cmd(
    binary: &std::path::Path,
    model: &DiscoveredModel,
    settings: &ModelSettings,
) -> (Command, String) {
    let mut cmd = Command::new(binary);
    let mut parts: Vec<String> = vec![binary.display().to_string()];

    push_arg(&mut cmd, &mut parts, "-m", model.path.display());
    push_arg(&mut cmd, &mut parts, "-t", settings.threads);
    push_arg(&mut cmd, &mut parts, "-b", settings.batch_size);

    push_gpu_layers(&mut cmd, &mut parts, settings);

    if settings.flash_attn {
        push_arg(&mut cmd, &mut parts, "-fa", "1");
    }

    push_spec_decoding(&mut cmd, &mut parts, settings);

    push_flag(&mut cmd, &mut parts, "--progress");

    let display = parts.join(" ");
    (cmd, display)
}

/// Spawn a llama.cpp server process (single model or router).
/// Returns (ServerHandle, command_string) where command_string is the full CLI.
pub struct SpawnServerRequest<'a> {
    pub config: &'a Config,
    pub model: Option<&'a DiscoveredModel>,
    pub settings: &'a ModelSettings,
    pub log_tx: mpsc::Sender<String>,
    pub progress_tx: Option<tokio::sync::broadcast::Sender<crate::models::DownloadState>>,
    pub server_mode: crate::models::ServerMode,
    pub router_max_models: u32,
    pub exit_tx: mpsc::Sender<()>,
}

pub async fn spawn_server(
    req: SpawnServerRequest<'_>,
) -> Result<(ServerHandle, String), String> {
    let SpawnServerRequest {
        config,
        model,
        settings,
        log_tx,
        progress_tx,
        server_mode,
        router_max_models,
        exit_tx,
    } = req;
    if server_mode != crate::models::ServerMode::Bench
        && server_mode != crate::models::ServerMode::BenchTune
    {
        let port = settings.port;
        // Check if port is already in use
        if std::net::TcpListener::bind(("127.0.0.1", port)).is_err() {
            return Err(format!("Port {} is already in use", port));
        }
    }

    // BenchTune mode is handled separately in app.start_pending_spawn()
    // and should never reach this function.
    if server_mode == crate::models::ServerMode::BenchTune {
        unreachable!("BenchTune mode must be handled before calling spawn_server")
    }

    // Resolve the backend binary (downloads if needed)
    let backend_name = if server_mode == crate::models::ServerMode::Bench {
        "llama-bench"
    } else {
        "llama-server"
    };
    let version_display = settings.get_active_backend_version_display();
    info!(
        "spawn_server: backend={}, requested_version={:?}, version_display={}",
        settings.backend,
        settings.get_active_backend_version(),
        version_display
    );
    log_tx
        .send(format!(
            "Resolving {} (v{}) binary...",
            backend_name, version_display
        ))
        .await
        .ok();
    let version_param = settings.get_active_backend_version().map(|s| s.as_str());

    let server_binary = match crate::backend::hub::resolve_backend_binary(
        settings.backend,
        version_param,
        Some(log_tx.clone()),
        progress_tx,
    )
    .await
    {
        Ok(path) => {
            info!("spawn_server: resolved binary path={}", path.display());
            path
        }
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
            && metadata.permissions().mode() & 0o111 == 0
        {
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
        build_server_cmd(
            &binary,
            model,
            settings,
            config,
            server_mode,
            router_max_models,
        )
    };

    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    // Set platform-specific env vars so the binary can find its shared libraries
    let bin_dir = binary.parent().unwrap();
    match std::env::consts::OS {
        "windows" => {
            // On Windows, add bin_dir to PATH so llama-server.exe finds libllama.dll
            if let Ok(current) = std::env::var("PATH") {
                cmd.env("PATH", format!("{};{}", bin_dir.display(), current));
            } else {
                cmd.env("PATH", bin_dir);
            }
        }
        "macos" => {
            // On macOS, set DYLD_LIBRARY_PATH for dylib loading
            if let Ok(current) = std::env::var("DYLD_LIBRARY_PATH") {
                cmd.env(
                    "DYLD_LIBRARY_PATH",
                    format!("{}:{}", bin_dir.display(), current),
                );
            } else {
                cmd.env("DYLD_LIBRARY_PATH", bin_dir);
            }
        }
        _ => {
            // On Linux, set LD_LIBRARY_PATH for so loading
            if let Ok(current) = std::env::var("LD_LIBRARY_PATH") {
                cmd.env(
                    "LD_LIBRARY_PATH",
                    format!("{}:{}", bin_dir.display(), current),
                );
            } else {
                cmd.env("LD_LIBRARY_PATH", bin_dir);
            }
        }
    }

    info!("Spawning: {}", cmd_string);
    let _ = log_tx
        .send(format!("{}: {}", backend_name, cmd_string))
        .await;
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn process: {}", e))?;
    let pid = child.id().unwrap_or(0);

    let (kill_tx, mut kill_rx) = mpsc::channel(1);

    // Background task: read stdout and stderr concurrently via separate tasks.
    // Each stream gets its own task + mpsc channel so neither can block the other.
    let log_tx_inner = log_tx.clone();
    let exit_tx_inner = exit_tx.clone();
    let backend_name_upper = backend_name.to_uppercase();
    tokio::spawn(async move {
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();

        let (stdout_tx, mut stdout_rx) = mpsc::channel::<String>(64);
        let (stderr_tx, mut stderr_rx) = mpsc::channel::<String>(64);

        // Spawn a reader task for each stream
        let mut std_out = Some(tokio::spawn(async move {
            let reader = BufReader::new(stdout).lines();
            tokio::pin!(reader);
            while let Ok(Some(line)) = reader.next_line().await {
                if stdout_tx.send(line).await.is_err() {
                    break;
                }
            }
        }));

        let mut std_err = Some(tokio::spawn(async move {
            let reader = BufReader::new(stderr).lines();
            tokio::pin!(reader);
            while let Ok(Some(line)) = reader.next_line().await {
                if stderr_tx.send(line).await.is_err() {
                    break;
                }
            }
        }));

        // Merge loop: block on whichever channel has data.
        // When both are empty, select! sleeps with zero CPU cost.
        loop {
            tokio::select! {
                _ = kill_rx.recv() => {
                    let _ = child.kill().await;
                    if let Some(h) = std_out.take() { let _ = h.await; }
                    if let Some(h) = std_err.take() { let _ = h.await; }
                    break;
                }
                line = stdout_rx.recv() => {
                    if let Some(line) = line { let _ = log_tx_inner.send(line).await; } else { break; }
                }
                line = stderr_rx.recv() => {
                    if let Some(line) = line { let _ = log_tx_inner.send(line).await; } else { break; }
                }
                else => break,
            }
        }

        // Wait for reader tasks to finish
        if let Some(h) = std_out.take() {
            let _ = h.await;
        }
        if let Some(h) = std_err.take() {
            let _ = h.await;
        }

        let exit_code = child.wait().await.ok().and_then(|s| s.code());
        let _ = exit_tx_inner.send(()).await;
        let _ = log_tx_inner
            .send(format!(
                "{} exited with code {:?}",
                backend_name_upper, exit_code
            ))
            .await;
    });

    Ok((
        ServerHandle {
            port: if server_mode == crate::models::ServerMode::Bench {
                0
            } else {
                settings.port
            },
            host: settings.host.clone(),
            pid,
            kill_tx,
        },
        cmd_string,
    ))
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
    handle
        .kill_tx
        .send(())
        .await
        .map_err(|_| "Server already stopped".to_string())
}

/// Poll metrics from the server.
pub async fn get_metrics(
    host: &str,
    port: u16,
    model_name: Option<&str>,
    pid: Option<u32>,
) -> Result<ServerMetrics, String> {
    let host = clean_host(host);
    // We prefer the /metrics endpoint as it's more stable for system info.
    // In router mode, we can specify the model via query parameter.
    let mut url = if let Some(model) = model_name {
        let name = strip_gguf(model);
        format!("http://{}:{}/metrics?model={}", host, port, name)
    } else {
        format!("http://{}:{}/metrics", host, port)
    };

    let mut resp = reqwest::get(&url)
        .await
        .map_err(|e| format!("Failed to get metrics: {}", e))?;

    // If model-specific metrics fail with 404 or 400, try plain /metrics
    if (resp.status() == reqwest::StatusCode::NOT_FOUND
        || resp.status() == reqwest::StatusCode::BAD_REQUEST)
        && model_name.is_some()
    {
        url = format!("http://{}:{}/metrics", host, port);
        resp = reqwest::get(&url)
            .await
            .map_err(|e| format!("Failed to get metrics: {}", e))?;
    }

    if !resp.status().is_success() {
        return Err(format!("Server returned {}", resp.status()));
    }

    let text = resp
        .text()
        .await
        .map_err(|e| format!("Failed to read metrics: {}", e))?;

    let mut m = ServerMetrics { loaded: true, ..Default::default() };

    let mut ctx_max_slots = 0u32;
    let mut ctx_used_slots = 0u32;
    let mut ctx_used_global = 0u32;
    let mut ctx_max_global = 0u32;

    let mut vram_used_slots = 0u64;
    let mut vram_total_slots = 0u64;
    let mut vram_used_global = 0u64;
    let mut vram_total_global = 0u64;

    for line in text.lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }

        let name_with_labels = parts[0];
        let mut val = 0.0;
        for part in parts.iter().skip(1) {
            if let Ok(v) = part.parse::<f64>() {
                val = v;
                break;
            }
        }

        let is_slot = name_with_labels.contains("slot=\"") || name_with_labels.contains("pool=\"");
        let name = name_with_labels
            .split('{')
            .next()
            .unwrap_or(name_with_labels);

        match name {
            "llama_kv_cache_usage_bytes"
            | "kv_cache_usage_bytes"
            | "llama_server_kv_cache_usage_bytes"
            | "llama_server_kv_cache_used_bytes"
            | "llama_server_vram_used_bytes" => {
                if is_slot {
                    vram_used_slots += val as u64;
                } else {
                    vram_used_global = vram_used_global.max(val as u64);
                }
            }
            "llama_kv_cache_total_bytes"
            | "kv_cache_total_bytes"
            | "llama_server_kv_cache_total_bytes"
            | "llama_server_vram_total_bytes" => {
                if is_slot {
                    vram_total_slots += val as u64;
                } else {
                    vram_total_global = vram_total_global.max(val as u64);
                }
            }
            "llama_model_memory_usage_bytes"
            | "model_memory_usage_bytes"
            | "llama_server_model_memory_usage_bytes"
            | "llama_server_memory_usage_bytes"
            | "llama_server_ram_usage_bytes"
            | "llama_server_mem_used_bytes" => {
                m.ram_used = m.ram_used.max(val as u64);
            }
            "llama_kv_cache_tokens_used"
            | "kv_cache_usage_tokens"
            | "kv_cache_tokens_used"
            | "llama_server_kv_cache_tokens_used"
            | "llamacpp:n_tokens_used"
            | "llama_server_n_tokens_used"
            | "llama_server_n_past"
            | "llamacpp:n_past" => {
                if is_slot {
                    ctx_used_slots += val as u32;
                } else {
                    ctx_used_global = ctx_used_global.max(val as u32);
                }
            }
            "llama_kv_cache_tokens_total"
            | "kv_cache_total_tokens"
            | "kv_cache_tokens_total"
            | "llama_server_kv_cache_tokens_total"
            | "llamacpp:n_ctx"
            | "llamacpp:n_tokens_max"
            | "llama_server_n_ctx"
            | "llama_server_n_tokens_max" => {
                if is_slot {
                    ctx_max_slots += val as u32;
                } else {
                    ctx_max_global = ctx_max_global.max(val as u32);
                }
            }
            "llama_server_cpu_usage_percentage"
            | "cpu_usage_percentage"
            | "llama_server_cpu_usage"
            | "llama_server_cpu_percent" => {
                m.cpu_usage = m.cpu_usage.max(val);
            }
            "llamacpp:predicted_tokens_seconds"
            | "llama_server_predicted_tokens_seconds"
            | "llama_server_tps" => {
                m.tps += val;
            }
            "llamacpp:prompt_tokens_seconds"
            | "llama_server_prompt_tokens_seconds"
            | "llama_server_prompt_tps" => {
                m.prompt_tps += val;
            }
            "llamacpp:kv_cache_usage_ratio" | "llama_server_kv_cache_usage_ratio" => {
                if !is_slot && ctx_max_global > 0 {
                    ctx_used_global = ctx_used_global.max((val * ctx_max_global as f64) as u32);
                }
            }
            _ => {}
        }
    }

    m.gpu_mem_used = if vram_used_slots > 0 {
        vram_used_slots
    } else {
        vram_used_global
    };
    m.gpu_mem_total = if vram_total_slots > 0 {
        vram_total_slots
    } else {
        vram_total_global
    };

    // ctx_used = tokens currently in the KV cache.
    // ctx_max = the total context window size allocated by the server.
    m.ctx_used = if ctx_used_slots > 0 {
        ctx_used_slots
    } else {
        ctx_used_global
    };
    m.ctx_max = if ctx_max_slots > 0 {
        ctx_max_slots
    } else {
        ctx_max_global
    };
    // ctx_max may be overridden in poll_metrics() by the user-configured value.

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
        set_if_better(&mut m, nv_used, nv_total);

        if m.gpu_mem_total == 0 {
            // AMD fallback when nvidia-smi is not available.
            let (amd_used, amd_total) = get_amdgpu_vram_metrics().unwrap_or((0, 0));
            set_if_better(&mut m, amd_used, amd_total);
        }
    } else if m.gpu_mem_used == 0 {
        // KV-only queries: use system tools as a last resort.
        if let Ok((used, total)) = get_nvidia_vram_metrics() {
            m.gpu_mem_used = used;
            m.gpu_mem_total = total;
        } else if let Ok((used, total)) = get_amdgpu_vram_metrics() {
            m.gpu_mem_used = used;
            m.gpu_mem_total = total;
        }
    }

    // Fallback for RAM and CPU using sysinfo (cross-platform)
    if let Some(p) = pid {
        if let Ok((ram, cpu)) = get_process_metrics(p) {
            if m.ram_used == 0 {
                m.ram_used = ram;
            }
            if m.cpu_usage == 0.0 {
                m.cpu_usage = cpu;
            }
        }
    }

    Ok(m)
}

/// Get VRAM usage using nvidia-smi
fn get_nvidia_vram_metrics() -> Result<(u64, u64), String> {
    let output = std::process::Command::new("nvidia-smi")
        .args([
            "--query-gpu=memory.used,memory.total",
            "--format=csv,noheader,nounits",
        ])
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

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;

    // amdgpu_top --json output has a "devices" array (or sometimes just a list of objects depending on version)
    let devices = if json.is_array() {
        json.as_array()
    } else {
        json.get("devices").and_then(|d| d.as_array())
    };

    if let Some(devices) = devices
        && let Some(device) = devices.first()
    {
        // Priority 1: Check root keys (newer amdgpu_top format as provided by user)
        // "VRAM Usage Size": 3070128128, "VRAM Size": 8589934592
        let root_used = device.get("VRAM Usage Size").and_then(|v| v.as_u64());
        let root_total = device.get("VRAM Size").and_then(|v| v.as_u64());

        if let (Some(used), Some(total)) = (root_used, root_total)
            && total > 0
        {
            return Ok((used, total));
        }

        // Priority 2: Check nested VRAM object (alternative format)
        let vram_obj = device.get("VRAM");
        if let Some(vram) = vram_obj {
            // Check if it's the "Total VRAM Usage" format (usually MiB)
            let nested_used = vram
                .get("Total VRAM Usage")
                .and_then(|v| v.get("value").or(Some(v)))
                .and_then(|v| v.as_u64());
            let nested_total = vram
                .get("Total VRAM")
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
            let used = vram
                .get("VRAM")
                .or_else(|| vram.get("usage"))
                .and_then(|v| v.get("value").or(Some(v)))
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let total = vram
                .get("TotalVRAM")
                .or_else(|| vram.get("total"))
                .and_then(|v| v.get("value").or(Some(v)))
                .and_then(|v| v.as_u64())
                .unwrap_or(0);

            if total > 0 {
                return Ok((used * 1024 * 1024, total * 1024 * 1024));
            }
        }
    }

    Err("Could not find VRAM info in amdgpu_top output".to_string())
}

/// Cross-platform: Get RAM (RSS) and CPU usage for a PID.
fn get_process_metrics(
    pid: u32,
) -> Result<(u64, f64), String> {
    use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};

    let mut sys = System::new_with_specifics(
        RefreshKind::new().with_processes(ProcessRefreshKind::new().with_cpu().with_memory()),
    );

    std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);

    let pids = [Pid::from(pid as usize)];
    sys.refresh_processes_specifics(
        ProcessesToUpdate::Some(&pids),
        true,
        ProcessRefreshKind::new().with_cpu().with_memory(),
    );

    let sys_pid = Pid::from(pid as usize);

    if let Some(process) = sys.process(sys_pid) {
        let ram = process.memory(); // bytes
        let cpu = process.cpu_usage() as f64; // percentage
        return Ok((ram, cpu));
    }

    Err("Process not found".to_string())
}

/// Load a model via the llama-server Router API.
pub async fn load_model(
    host: &str,
    port: u16,
    model_id: &str,
    model_path: Option<&str>,
) -> Result<(), String> {
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
    if let Some(filename) = std::path::Path::new(model_id)
        .file_name()
        .and_then(|f| f.to_str())
    {
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
                        last_error = res
                            .text()
                            .await
                            .unwrap_or_else(|_| "Unknown error".to_string());
                    }
                    Err(e) => {
                        last_error = e.to_string();
                    }
                }
            }
        }
    }

    Err(format!(
        "Failed to load model (tried {} variants). Last status {}: {}",
        variants.len() * 2,
        last_status,
        last_error
    ))
}

/// List all models and their status from the llama-server Router API.
pub async fn list_models(
    host: &str,
    port: u16,
) -> Result<Vec<(String, String, Option<String>)>, String> {
    let client = reqwest::Client::new();
    let host = clean_host(host);
    let url = format!("http://{}:{}/models", host, port);

    let res = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to list models: {}", e))?;

    if !res.status().is_success() {
        return Err(format!("Server returned error {}", res.status()));
    }

    let json: serde_json::Value = res
        .json()
        .await
        .map_err(|e| format!("Invalid JSON: {}", e))?;

    let mut results = Vec::new();
    if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
        for model in data {
            let id = model
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            // Status can be a string or an object with a "value" field
            let status = model
                .get("status")
                .and_then(|s| s.get("value").or(Some(s)))
                .and_then(|v| v.as_str())
                .unwrap_or("unloaded")
                .to_string();
            let path = model
                .get("path")
                .or_else(|| model.get("filename"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            results.push((id, status, path));
        }
    }

    Ok(results)
}

/// Unload a model via the llama-server Router API.
pub async fn unload_model(
    host: &str,
    port: u16,
    model_id: &str,
    model_path: Option<&str>,
) -> Result<(), String> {
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
                && res.status().is_success()
            {
                return Ok(());
            }
        }
    }

    Ok(()) // Silently ignore unload errors as it's often just a cleanup
}
