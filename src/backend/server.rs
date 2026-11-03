use std::fmt::Display;
use std::process::Stdio;
use std::str::FromStr;
use std::sync::LazyLock;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::config::Config;
use crate::models::{
    DiscoveredModel, ModelSettings, ServerMetrics, clean_host, strip_gguf,
};

/// Client for health checks (short timeout).
static HEALTH_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(1))
        .build()
        .unwrap()
});

static HTTP_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(reqwest::Client::new);

/// Cached VRAM metrics from system tools (nvidia-smi / amdgpu_top).
struct VramCache {
    nvidia: Option<(u64, u64)>,
    amd: Option<(u64, u64)>,
    stale: bool,
}

static VRAM_CACHE: LazyLock<std::sync::Mutex<VramCache>> = LazyLock::new(|| {
    std::sync::Mutex::new(VramCache {
        nvidia: None,
        amd: None,
        stale: true,
    })
});

/// Invalidate the VRAM cache so the next metrics poll re-queries system tools.
pub fn invalidate_vram_cache() {
    let mut cache = VRAM_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    cache.nvidia = None;
    cache.amd = None;
    cache.stale = true;
}

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
    // Quote values that may contain spaces for shell safety
    if val_str.contains(' ') || val_str.contains(';') || val_str.contains('"') {
        parts.push(format!(
            "\"{}\"",
            val_str.replace('\\', "\\\\").replace('"', "\\\"")
        ));
    } else {
        parts.push(val_str);
    }
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

/// A single argument shared by the CLI (llama-server) and the
/// `--models-preset` `.ini` (llama.cpp router). The SAME settings→arg mapping
/// feeds both materializers, so the CLI and the preset can never drift apart.
#[derive(Clone, Debug)]
enum ArgKind {
    /// CLI: `--key value` / preset: `key = value`
    KeyValue { key: String, value: String },
    /// CLI: `--key` / preset: `key = true`
    Flag { key: String },
    /// CLI: `--no-key` / preset: `key = false`
    NegFlag { key: String },
    /// CLI: `from value` (from keeps its dashes, e.g. `-ngl`) /
    /// preset: `to = value` (the preset key can differ, e.g. `n-gpu-layers`).
    Renamed { from: String, to: String, value: String },
}

#[derive(Clone, Debug)]
struct Arg {
    kind: ArgKind,
}

fn arg_kv(key: impl Into<String>, value: impl std::fmt::Display) -> Arg {
    Arg {
        kind: ArgKind::KeyValue {
            key: key.into(),
            value: value.to_string(),
        },
    }
}

fn arg_flag(key: impl Into<String>) -> Arg {
    Arg {
        kind: ArgKind::Flag { key: key.into() },
    }
}

fn arg_neg(key: impl Into<String>) -> Arg {
    Arg {
        kind: ArgKind::NegFlag { key: key.into() },
    }
}

fn arg_renamed(
    from: impl Into<String>,
    to: impl Into<String>,
    value: impl std::fmt::Display,
) -> Arg {
    Arg {
        kind: ArgKind::Renamed {
            from: from.into(),
            to: to.into(),
            value: value.to_string(),
        },
    }
}

/// Quote a value in a display string when it contains shell-special
/// characters (preserves the historic `push_arg` quoting so the logged
/// command stays a valid one-liner).
fn push_quoted(parts: &mut Vec<String>, value: &str) {
    if value.contains(' ') || value.contains(';') || value.contains('"') {
        parts.push(format!(
            "\"{}\"",
            value.replace('\\', "\\\\").replace('"', "\\\"")
        ));
    } else {
        parts.push(value.to_string());
    }
}

/// Append arguments to a Command + display string as CLI flags.
fn append_cli(args: &[Arg], cmd: &mut Command, parts: &mut Vec<String>) {
    for a in args {
        match &a.kind {
            ArgKind::KeyValue { key, value } => {
                cmd.arg(format!("--{key}")).arg(value);
                parts.push(format!("--{key}"));
                push_quoted(parts, value);
            }
            ArgKind::Flag { key } => {
                cmd.arg(format!("--{key}"));
                parts.push(format!("--{key}"));
            }
            ArgKind::NegFlag { key } => {
                cmd.arg(format!("--no-{key}"));
                parts.push(format!("--no-{key}"));
            }
            ArgKind::Renamed { from, value, .. } => {
                cmd.arg(from).arg(value);
                parts.push(from.clone());
                push_quoted(parts, value);
            }
        }
    }
}

/// Quote a value for an `--models-preset` `.ini` line: raw when it is simple,
/// otherwise double-quote-wrapped with backslash escaping (matching the CLI so
/// values containing spaces never get silently truncated).
fn preset_value(value: &str) -> String {
    if value.contains(' ') || value.contains(';') || value.contains('"') || value.contains('\n') {
        format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        value.to_string()
    }
}

/// Append arguments to an `--models-preset` `.ini` body as `key = value` lines.
fn append_preset(body: &mut String, args: &[Arg]) {
    for a in args {
        match &a.kind {
            ArgKind::KeyValue { key, value } => {
                body.push_str(&format!("{key} = {}\n", preset_value(value)));
            }
            ArgKind::Flag { key } => body.push_str(&format!("{key} = true\n")),
            ArgKind::NegFlag { key } => body.push_str(&format!("{key} = false\n")),
            ArgKind::Renamed { to, value, .. } => {
                body.push_str(&format!("{to} = {}\n", preset_value(value)));
            }
        }
    }
}

/// Build the per-model generation arguments for one settings snapshot. These
/// materialize as CLI flags in Normal mode and as `--models-preset` `.ini` keys
/// in Router mode (see append_cli / append_preset). The `typical` key is used
/// both for the CLI flag `--typical` and for the preset key `typical`.
fn build_generation_args(
    settings: &ModelSettings,
    config: &Config,
    gguf_meta: Option<&crate::models::GgufMetadata>,
    model_config_used: bool,
) -> Vec<Arg> {
    let mut args: Vec<Arg> = Vec::new();

    // ── Loading ──────────────────────────────────────────────
    args.push(arg_kv("threads", settings.threads));
    args.push(arg_kv("threads-batch", settings.threads_batch));
    let effective_ctx = if settings.rope_yarn_enabled && settings.rope_scale > 1.0 {
        (settings.context_length as f64 * settings.rope_scale as f64) as u32
    } else {
        settings.context_length
    };
    if !model_config_used {
        args.push(arg_kv("ctx-size", effective_ctx));
    }
    if settings.cont_batching {
        args.push(arg_flag("cont-batching"));
    }
    // Always pass batch-size (Eval Batch) so the user's choice is never silently
    // replaced by llama-server's built-in default (2048).
    args.push(arg_kv("batch-size", settings.batch_size));
    // llama.cpp requires ubatch <= batch; clamp so a misconfiguration can't
    // prevent the server from starting.
    let ubatch_size = settings.ubatch_size.min(settings.batch_size);
    args.push(arg_kv("ubatch-size", ubatch_size));
    if let Some(n) = settings.max_concurrent_predictions {
        args.push(arg_kv("parallel", n));
    }
    // --no-warmup is always emitted (matches the historic CLI behavior).
    args.push(arg_neg("warmup"));

    // Speculative decoding
    if !settings.spec_type.is_empty() {
        args.push(arg_kv("spec-type", settings.spec_type.clone()));
        if settings.draft_tokens > 0 {
            args.push(arg_kv("spec-draft-n-max", settings.draft_tokens));
        }
    }

    if let Some(cache_k) = settings.cache_type_k {
        args.push(arg_kv("cache-type-k", cache_k));
    }
    if let Some(cache_v) = settings.cache_type_v {
        args.push(arg_kv("cache-type-v", cache_v));
    }

    if settings.keep != 0 {
        args.push(arg_kv("keep", settings.keep));
    }
    if settings.swa_full {
        args.push(arg_flag("swa-full"));
    }
    if settings.load_mode != crate::models::LoadMode::None {
        args.push(arg_kv("load-mode", settings.load_mode.to_string()));
    }
    if settings.numa != Default::default() {
        args.push(arg_kv("numa", settings.numa.to_string()));
    }
    if settings.kv_cache_offload {
        args.push(arg_flag("kv-offload"));
    }

    // ── GPU ──────────────────────────────────────────────────
    match settings.gpu_layers_mode {
        crate::models::GpuLayersMode::Specific(n) => {
            args.push(arg_renamed("-ngl", "n-gpu-layers", n))
        }
        crate::models::GpuLayersMode::All => {
            args.push(arg_renamed("-ngl", "n-gpu-layers", "999"))
        }
        crate::models::GpuLayersMode::Auto => {}
    }

    // Always pass split-mode so llama-server doesn't auto-detect other GPUs
    args.push(arg_kv("split-mode", settings.split_mode.to_string()));
    // Build tensor-split from local GPU + selected RPC workers
    let local_split = if settings.tensor_split.is_empty() {
        // Default: proportional to each GPU's VRAM (e.g. "24,16"); fall back
        // to a single slot when fewer than 2 GPUs could be detected.
        crate::backend::hardware::default_tensor_split(settings.backend)
            .unwrap_or_else(|| "1".to_string())
    } else {
        settings.tensor_split.clone()
    };
    let worker_splits: Vec<String> = config
        .rpc_workers
        .iter()
        .filter(|w| w.selected && std::net::IpAddr::from_str(&w.ip).is_ok())
        .map(|w| {
            if w.tensor_split.is_empty() {
                "1".to_string()
            } else {
                w.tensor_split.clone()
            }
        })
        .collect();
    // tensor-split is used for RPC workers, and also for local multi-GPU when
    // Split Mode is Tensor (default: each GPU's VRAM in GiB).
    if !worker_splits.is_empty() || settings.split_mode == crate::models::SplitMode::Tensor {
        let full_split = if worker_splits.is_empty() {
            local_split
        } else {
            format!("{},{}", local_split, worker_splits.join(","))
        };
        args.push(arg_kv("tensor-split", full_split));
    }
    // Restrict offloading to the selected GPU devices (from `--list-devices`).
    // Empty = let llama.cpp use all detected devices.
    if !settings.devices.is_empty() {
        args.push(arg_kv("device", settings.devices.join(",")));
    }
    if settings.fit {
        args.push(arg_kv("fit", "on"));
    }

    if let Some(ref lora) = settings.lora {
        args.push(arg_kv("lora", lora.display().to_string()));
    }
    if let Some((ref lora, scale)) = settings.lora_scaled {
        args.push(arg_kv(
            "lora-scaled",
            format!("{}:{}", lora.display(), scale),
        ));
    }

    let mut rpc_list = Vec::new();
    if !settings.rpc.is_empty() {
        rpc_list.push(settings.rpc.clone());
    }
    for worker in &config.rpc_workers {
        if worker.selected && std::net::IpAddr::from_str(&worker.ip).is_ok() {
            rpc_list.push(format!("{}:{}", worker.ip, worker.port));
        }
    }

    if !rpc_list.is_empty() {
        args.push(arg_kv("rpc", rpc_list.join(",")));
    }

    if settings.embedding {
        args.push(arg_flag("embedding"));
    }

    if settings.expert_count > 0 {
        let arch = gguf_meta
            .as_ref()
            .map(|m| m.arch.as_str())
            .unwrap_or("llama");
        let arch = arch
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect::<String>();
        args.push(arg_kv(
            "override-kv",
            format!("{}.expert_used_count=int:int:{}", arch, settings.expert_count),
        ));
    }

    args.push(arg_renamed(
        "-fa",
        "flash-attn",
        if settings.flash_attn { "on" } else { "off" },
    ));

    if settings.jinja {
        args.push(arg_flag("jinja"));
    }

    // Apply chat template: file path, built-in name, or auto-detect from arch
    if let Some(ref t) = settings.chat_template {
        if t.ends_with(".jinja") {
            args.push(arg_kv("chat-template-file", t.clone()));
        } else {
            args.push(arg_kv("chat-template", t.clone()));
        }
    } else if settings.auto_chat_template {
        let arch = gguf_meta
            .as_ref()
            .map(|m| m.arch.as_str())
            .unwrap_or("llama");
        if let Some(template) = crate::models::arch_to_chat_template(arch) {
            args.push(arg_kv("chat-template", template));
        }
    }

    // Inject system prompt via chat template kwargs when it is not empty
    if !settings.system_prompt.is_empty() {
        let mut merged = serde_json::Map::new();
        if let Some(ref kwargs) = settings.chat_template_kwargs
            && let Ok(obj) = serde_json::from_str::<serde_json::Value>(kwargs)
            && let serde_json::Value::Object(map) = obj
        {
            for (k, v) in map {
                merged.insert(k, v);
            }
        }
        // Warn if user's kwargs already defines system_prompt
        if merged.contains_key("system_prompt") {
            tracing::warn!(
                "chat_template_kwargs already contains 'system_prompt', will be overridden by settings"
            );
        }
        merged.insert(
            "system_prompt".to_string(),
            serde_json::Value::String(settings.system_prompt.clone()),
        );
        args.push(arg_kv(
            "chat-template-kwargs",
            serde_json::to_string(&merged).unwrap(),
        ));
    } else if let Some(ref kwargs) = settings.chat_template_kwargs {
        args.push(arg_kv("chat-template-kwargs", kwargs.clone()));
    }

    // ── Sampling ─────────────────────────────────────────────
    if settings.seed != -1 {
        args.push(arg_kv("seed", settings.seed));
    }
    if let Some(max_tokens) = settings.max_tokens {
        args.push(arg_kv("n-predict", max_tokens));
    }
    args.push(arg_kv("temp", format!("{:.2}", settings.temperature)));
    args.push(arg_kv("top-k", settings.top_k));
    args.push(arg_kv("top-p", format!("{:.2}", settings.top_p)));
    args.push(arg_kv("min-p", format!("{:.2}", settings.min_p)));
    // CLI flag is `--typical` (historic form); preset key is `typical`.
    args.push(arg_kv("typical", format!("{:.2}", settings.typical_p)));

    if settings.mirostat != Default::default() {
        args.push(arg_kv("mirostat", settings.mirostat.to_string()));
        args.push(arg_kv(
            "mirostat-lr",
            format!("{:.2}", settings.mirostat_lr),
        ));
        args.push(arg_kv(
            "mirostat-ent",
            format!("{:.2}", settings.mirostat_ent),
        ));
    }

    if settings.ignore_eos {
        args.push(arg_flag("ignore-eos"));
    }

    if !settings.samplers.0.is_empty() {
        args.push(arg_kv("samplers", settings.samplers.to_string()));
    }

    if let Some(frequency) = settings.frequency_penalty {
        args.push(arg_kv(
            "frequency-penalty",
            format!("{:.2}", frequency),
        ));
    }

    if settings.dry_multiplier != 0.0 {
        args.push(arg_kv(
            "dry-multiplier",
            format!("{:.2}", settings.dry_multiplier),
        ));
        args.push(arg_kv("dry-base", format!("{:.2}", settings.dry_base)));
        args.push(arg_kv(
            "dry-allowed-length",
            settings.dry_allowed_length,
        ));
        args.push(arg_kv(
            "dry-penalty-last-n",
            settings.dry_penalty_last_n,
        ));
    }

    // ── RoPE ─────────────────────────────────────────────────
    let rope_scaling = if settings.rope_yarn_enabled {
        crate::models::RopeScaling::Yarn
    } else {
        settings.rope_scaling
    };
    if rope_scaling != Default::default() {
        args.push(arg_kv("rope-scaling", rope_scaling.to_string()));
    }
    if settings.rope_scale != 1.0 {
        args.push(arg_kv("rope-scale", format!("{:.2}", settings.rope_scale)));
    }
    if settings.rope_freq_base != 0.0 {
        args.push(arg_kv(
            "rope-freq-base",
            format!("{:.2}", settings.rope_freq_base),
        ));
    }
    if settings.rope_freq_scale != 1.0 {
        args.push(arg_kv(
            "rope-freq-scale",
            format!("{:.2}", settings.rope_freq_scale),
        ));
    }

    if settings.rope_yarn_enabled
        && settings.rope_scale > 1.0
        && let Some(meta) = gguf_meta
    {
        args.push(arg_kv(
            "override-kv",
            format!("{}.context_length=int:{}", meta.arch, effective_ctx),
        ));
        args.push(arg_kv("yarn-orig-ctx", meta.n_ctx_train));
    }

    args
}

/// Build the full llama-server command line from settings.
/// Returns (Command, display_string, generation_args).
///
/// In Normal mode the generation arguments become CLI flags. In Router mode
/// they are returned separately and consumed as the `--models-preset` `.ini`
/// (the empty router carries no generation flags itself — each loaded model
/// inherits its settings from the preset, so models can differ). Infra and
/// model-binding args are always on the CLI.
pub fn build_server_cmd(
    binary: &std::path::Path,
    model: Option<&DiscoveredModel>,
    settings: &ModelSettings,
    config: &Config,
    server_mode: crate::models::ServerMode,
    router_max_models: u32,
    model_config_used: bool,
) -> (Command, String) {
    let gguf_meta = model.and_then(|m| crate::models::GgufMetadata::from_path(&m.path).ok());
    let gen_args = build_generation_args(settings, config, gguf_meta.as_ref(), model_config_used);

    // ── Infra (server-wide, always CLI) ──────────────────────
    let mut infra_args: Vec<Arg> = Vec::new();
    let resolved_host = clean_host(&settings.host);
    infra_args.push(arg_kv("host", resolved_host));
    infra_args.push(arg_kv("port", settings.port));
    infra_args.push(arg_kv("timeout", settings.timeout));
    infra_args.push(arg_flag("metrics"));
    // Prompt caching is a server-wide knob in the historic CLI (emitted only as
    // --no-cache-prompt when disabled, with --cache-reuse alongside it). Keep it
    // on the CLI so Normal mode stays byte-identical; in Router mode the empty
    // router carries these and every loaded model inherits them.
    if !settings.cache_prompt {
        infra_args.push(arg_neg("cache-prompt"));
    }
    if settings.cache_reuse != 0 {
        infra_args.push(arg_kv("cache-reuse", settings.cache_reuse));
    }
    if !settings.webui {
        infra_args.push(arg_neg("webui"));
    }
    let lv = match config.default.log_level.as_str() {
        "error" => 0,
        "warn" => 1,
        "info" => 2,
        "debug" => 3,
        "trace" => 4,
        _ => 1,
    };
    infra_args.push(arg_renamed("-lv", "log-level", lv));
    // Always pass --models-dir in router mode (global config setting). The
    // router's shallow scan only sees one level deep, so the app also lists
    // models explicitly via --models-preset; this dir is the fallback scan path.
    // (Not in Normal mode: there the server loads a single model via -m.)
    if server_mode == crate::models::ServerMode::Router
        && let Some(dir) = config.models_dirs.first()
    {
        infra_args.push(arg_kv("models-dir", dir.display()));
    }

    // ── Model binding (CLI only) ─────────────────────────────
    let mut model_args: Vec<Arg> = Vec::new();
    match server_mode {
        crate::models::ServerMode::Normal => {
            if let Some(model) = model {
                model_args.push(arg_renamed(
                    "-m",
                    "model",
                    model.path.display(),
                ));
                // Add alias for router mode identification (uses the unique
                // relative path)
                model_args.push(arg_kv("alias", model.display_name.clone()));
            }
        }
        crate::models::ServerMode::Router => {
            // Router mode: no model in CLI, use the /load API to load models.
            // Local models are registered via --models-preset (added by
            // spawn_server_inner from the preset built by the app layer),
            // because this app stores models two levels deep under models-dir
            // (<owner>/<repo>/<file>.gguf), which the router's shallow
            // --models-dir scan cannot discover.
            if router_max_models > 0 {
                model_args.push(arg_kv("models-max", router_max_models));
            }
        }
        crate::models::ServerMode::Bench | crate::models::ServerMode::BenchTune => {}
    }

    let mut cmd = Command::new(binary);
    let mut parts: Vec<String> = vec![binary.display().to_string()];
    // Model binding is always on the CLI.
    append_cli(&model_args, &mut cmd, &mut parts);
    // Generation settings: CLI in Normal mode, preset in Router mode.
    if server_mode != crate::models::ServerMode::Router {
        append_cli(&gen_args, &mut cmd, &mut parts);
    }
    // Infra is always on the CLI.
    append_cli(&infra_args, &mut cmd, &mut parts);

    let cmd_string = parts.join(" ");
    (cmd, cmd_string)
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

    push_flag(&mut cmd, &mut parts, "--progress");

    let display = parts.join(" ");
    (cmd, display)
}

/// Build the `--models-preset` `.ini` that registers every local model for
/// router mode, and write it to a stable path under the config directory.
///
/// The router only auto-discovers models one level deep under `--models-dir`
/// (`<dir>/*.gguf` or `<dir>/<model>/*.gguf`), but this app stores models as
/// `<owner>/<repo>/<file>.gguf` (two levels), so the router's scan finds none.
/// We register them explicitly instead: each section maps a router-safe id
/// (see `router_model_id`) to the model's absolute path. The section name is
/// what the router reports in `GET /models` and what the app uses to
/// load/unload. The file is regenerated on every router-mode spawn so it stays
/// in sync with the models the app actually discovered.
///
/// Each model section carries that model's *full* resolved generation settings
/// (see `config.resolve_settings`), so in router mode every loaded model runs
/// with its own config instead of a single server-wide set. A `[*]` baseline
/// section (`baseline`) holds the default generation settings every model
/// inherits for keys its own section does not override.
pub fn build_router_preset(
    models: &[DiscoveredModel],
    config: &Config,
    baseline: &ModelSettings,
) -> Result<std::path::PathBuf, String> {
    let mut body = String::new();

    // [*] baseline: default generation settings inherited by every model that
    // does not override a given key in its own section.
    append_preset(&mut body, &build_generation_args(baseline, config, None, false));

    for m in models {
        let id = crate::models::router_model_id(&m.display_name);
        let settings = config.resolve_settings(Some(m.display_name.as_str()), None);
        let meta = crate::models::GgufMetadata::from_path(&m.path).ok();
        body.push_str(&format!("[{id}]\n"));
        body.push_str(&format!("model = {}\n", m.path.display()));
        append_preset(&mut body, &build_generation_args(&settings, config, meta.as_ref(), false));
    }
    let path = crate::config::config_base_dir()
        .join("llm-manager")
        .join("router-preset.ini");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            format!(
                "failed to create preset directory {}: {e}",
                parent.display()
            )
        })?;
    }
    std::fs::write(&path, body)
        .map_err(|e| format!("failed to write preset {}: {e}", path.display()))?;
    Ok(path)
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
    // Router mode: path to the --models-preset .ini listing local models.
    // Generated by the app layer from its discovered models; None otherwise.
    pub router_preset: Option<std::path::PathBuf>,
}

pub async fn spawn_server(req: SpawnServerRequest<'_>) -> Result<(ServerHandle, String), String> {
    let SpawnServerRequest {
        config,
        model,
        settings,
        log_tx,
        progress_tx,
        server_mode,
        router_max_models,
        exit_tx,
        router_preset,
    } = req;
    // Turbo KV-cache types (turbo2/3/4) only exist in turboquant builds. If
    // the effective settings still carry one on a standard backend — a
    // leftover from a previous backend selection, a saved profile, or a
    // per-model override — downgrade to F16 instead of letting
    // llama-server fail to start with an unknown --cache-type-k value.
    if !settings.is_turboquant_backend()
        && (settings.cache_type_k.is_some_and(|t| t.is_turbo())
            || settings.cache_type_v.is_some_and(|t| t.is_turbo()))
    {
        let mut sanitized = settings.clone();
        let mut k = false;
        let mut v = false;
        if sanitized.cache_type_k.is_some_and(|t| t.is_turbo()) {
            sanitized.cache_type_k = Some(crate::models::CacheQuantType::F16);
            k = true;
        }
        if sanitized.cache_type_v.is_some_and(|t| t.is_turbo()) {
            sanitized.cache_type_v = Some(crate::models::CacheQuantType::F16);
            v = true;
        }
        let which = if k && v {
            "cache-type-k and cache-type-v".to_string()
        } else if k {
            "cache-type-k".to_string()
        } else {
            "cache-type-v".to_string()
        };
        log_tx
            .send(format!(
                "Note: turbo KV-cache type ({}) not supported by this backend; using F16",
                which
            ))
            .await
            .ok();
        let settings = &sanitized;
        return spawn_server_inner(SpawnServerRequest {
            config,
            model,
            settings,
            log_tx,
            progress_tx,
            server_mode,
            router_max_models,
            exit_tx,
            router_preset: router_preset.clone(),
        })
        .await;
    }
    spawn_server_inner(SpawnServerRequest {
        config,
        model,
        settings,
        log_tx,
        progress_tx,
        server_mode,
        router_max_models,
        exit_tx,
        router_preset,
    })
    .await
}

async fn spawn_server_inner(req: SpawnServerRequest<'_>) -> Result<(ServerHandle, String), String> {
    let SpawnServerRequest {
        config,
        model,
        settings,
        log_tx,
        progress_tx,
        server_mode,
        router_max_models,
        exit_tx,
        router_preset,
    } = req;
    if server_mode != crate::models::ServerMode::Bench
        && server_mode != crate::models::ServerMode::BenchTune
    {
        let port = settings.port;
        // Check if port is already in use (bind to the same host the server will use)
        let resolved_host = clean_host(&settings.host);
        if std::net::TcpListener::bind(format!("{}:{}", resolved_host, port)).is_err() {
            return Err(format!("Port {} is already in use", port));
        }
    }

    // BenchTune mode is handled separately in app.process_pending_spawn()
    // and should never reach this function.
    if server_mode == crate::models::ServerMode::BenchTune {
        return Err("BenchTune mode is not supported in spawn_server".to_string());
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

    // Custom backends run from a stored directory (no download / resolution).
    // Resolve via hub::get_backend_dir — the single source of truth shared
    // with the picker's install check and backend deletion — so the binary
    // that gets spawned is always the one the UI verified.
    let server_binary = if settings.backend == crate::models::Backend::Custom {
        let dir_str = settings
            .llama_cpp_version_custom
            .as_ref()
            .ok_or_else(|| "Custom backend has no directory configured".to_string())?;
        let dir = crate::backend::hub::get_backend_dir(crate::models::Backend::Custom, dir_str);
        let bin = dir.join(crate::backend::hub::binary_name());
        info!(
            "spawn_server: custom backend dir={}, binary={}",
            dir.display(),
            bin.display()
        );
        log_tx
            .send(format!("Using custom backend: {}", dir.display()))
            .await
            .ok();
        bin
    } else {
        match crate::backend::hub::resolve_backend_binary(
            settings.backend,
            version_param,
            settings.llama_cpp_strix_halo_rocm.as_deref(),
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
            false,
        )
    };

    // Router mode: register the local models via the preset file the app layer
    // built. build_server_cmd already omitted --models-dir for router mode
    // (the router's shallow scan cannot see this app's two-level model layout),
    // so we add --models-preset here and mirror it into the logged command.
    let mut cmd_string = cmd_string;
    if server_mode == crate::models::ServerMode::Router
        && let Some(preset) = &router_preset
    {
        cmd.arg("--models-preset").arg(preset);
        cmd_string.push_str(&format!(" --models-preset {}", preset.display()));
    }

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

        // Spawn a reader task for each stream.
        // Uses read_until + lossy UTF-8: `lines()` returns Err on invalid
        // UTF-8 (e.g. GPU/progress output), which would silently kill the
        // reader while the child is still alive.
        let mut std_out = Some(tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            let mut buf = Vec::with_capacity(8192);
            loop {
                buf.clear();
                match reader.read_until(b'\n', &mut buf).await {
                    Ok(0) => break, // EOF
                    Ok(_) => {
                        let line = String::from_utf8_lossy(&buf)
                            .trim_end_matches(['\r', '\n'])
                            .to_string();
                        if stdout_tx.send(line).await.is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        tracing::warn!("stdout reader error: {}", e);
                        break;
                    }
                }
            }
        }));

        let mut std_err = Some(tokio::spawn(async move {
            let mut reader = BufReader::new(stderr);
            let mut buf = Vec::with_capacity(8192);
            loop {
                buf.clear();
                match reader.read_until(b'\n', &mut buf).await {
                    Ok(0) => break, // EOF
                    Ok(_) => {
                        let line = String::from_utf8_lossy(&buf)
                            .trim_end_matches(['\r', '\n'])
                            .to_string();
                        if stderr_tx.send(line).await.is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        tracing::warn!("stderr reader error: {}", e);
                        break;
                    }
                }
            }
        }));

        // Merge loop: block on whichever channel has data.
        // Closed streams are DISABLED (conditional branches) instead of
        // breaking the loop, so the kill branch stays live until the child
        // actually exits. Breaking early used to buffer late kill messages
        // in the channel that were never consumed — kill_server returned Ok
        // but child.kill() was never called and the process was orphaned.
        // Once both output streams are closed, a child.wait() branch takes
        // over so a child that dies on its own (crash) is still detected —
        // the kill branch alone would block forever because the app keeps
        // the kill sender alive.
        let mut stdout_open = true;
        let mut stderr_open = true;
        let mut kill_open = true;
        loop {
            if !kill_open && !stdout_open && !stderr_open {
                break;
            }
            tokio::select! {
                msg = kill_rx.recv(), if kill_open => {
                    if msg.is_none() {
                        // All kill senders dropped without a request.
                        kill_open = false;
                        continue;
                    }
                    tracing::info!("spawn task: kill received for pid={}", pid);
                    let _ = log_tx_inner
                        .try_send(format!("[kill] kill requested for pid {}", pid));
                    let kill_res = child.kill().await;
                    tracing::info!(
                        "spawn task: child.kill() pid={} -> {:?}",
                        pid,
                        kill_res
                    );
                    let _ = log_tx_inner.try_send(format!(
                        "[kill] child.kill() pid {} -> {:?}",
                        pid, kill_res
                    ));
                    // Do NOT await the reader tasks here: a reader blocked on
                    // a full channel can never finish while this task is the
                    // only consumer (deadlock). Aborting drops its sender and
                    // the pipe, which is fine on the shutdown path.
                    if let Some(h) = std_out.take() { h.abort(); }
                    if let Some(h) = std_err.take() { h.abort(); }
                    break;
                }
                line = stdout_rx.recv(), if stdout_open => {
                    // Best-effort: never block on the UI log channel, or a
                    // full channel (e.g. at shutdown with no consumer) would
                    // stall this task and the kill branch above.
                    match line {
                        Some(line) => { let _ = log_tx_inner.try_send(line); }
                        None => {
                            stdout_open = false;
                            let alive = child.try_wait().map(|r| r.is_none()).unwrap_or(true);
                            tracing::warn!(
                                "spawn task: stdout stream closed, pid={} alive={} — kill branch stays active",
                                pid, alive
                            );
                            let _ = log_tx_inner.try_send(format!(
                                "[kill] stdout stream closed, pid {} alive={} — kill branch stays active",
                                pid, alive
                            ));
                        }
                    }
                }
                line = stderr_rx.recv(), if stderr_open => {
                    match line {
                        Some(line) => { let _ = log_tx_inner.try_send(line); }
                        None => {
                            stderr_open = false;
                            let alive = child.try_wait().map(|r| r.is_none()).unwrap_or(true);
                            tracing::warn!(
                                "spawn task: stderr stream closed, pid={} alive={} — kill branch stays active",
                                pid, alive
                            );
                            let _ = log_tx_inner.try_send(format!(
                                "[kill] stderr stream closed, pid {} alive={} — kill branch stays active",
                                pid, alive
                            ));
                        }
                    }
                }
                // Crash path: once both output streams are closed, wait for
                // the child to exit directly. Without this branch, a child
                // that dies on its own (crash, OOM, external kill) leaves
                // kill_rx.recv() blocking forever — the kill sender is still
                // alive in the app's ServerHandle — so child.wait() and the
                // exit notification below never run and the TUI keeps the
                // server state (and disabled settings) forever.
                status = child.wait(), if !stdout_open && !stderr_open => {
                    tracing::info!(
                        "spawn task: child exited on its own pid={} status={:?}",
                        pid, status
                    );
                    break;
                }
            }
        }

        // Wait for reader tasks to finish
        if let Some(h) = std_out.take() {
            let _ = h.await;
        }
        if let Some(h) = std_err.take() {
            let _ = h.await;
        }

        // Honor a kill that was buffered while the merge loop was draining.
        if kill_open && kill_rx.try_recv().is_ok() {
            tracing::info!("spawn task: late buffered kill for pid={}", pid);
            let _ = log_tx_inner.try_send(format!("[kill] late buffered kill for pid {}", pid));
            let _ = child.kill().await;
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

    match HEALTH_CLIENT.get(&url).send().await {
        Ok(resp) => resp.status().is_success(),
        Err(_) => false,
    }
}

/// Kill a running server.
pub async fn kill_server(handle: ServerHandle) -> Result<(), String> {
    tracing::info!("kill_server: sending kill for pid={}", handle.pid);
    handle
        .kill_tx
        .send(())
        .await
        .map_err(|_| {
            tracing::error!(
                "kill_server: kill channel CLOSED for pid={} — spawn task already exited, process may be orphaned",
                handle.pid
            );
            format!("Server already stopped (pid {} kill channel closed)", handle.pid)
        })
}

/// Outcome of verifying that a kill actually reached the server process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KillOutcome {
    /// The spawn task reaped the child (normal path).
    Exited,
    /// No exit notification, but the pid no longer exists.
    Gone,
    /// The pid was still alive after the grace period; a direct SIGKILL was
    /// sent and confirmed.
    Forced,
    /// The pid is still alive even after a direct SIGKILL attempt.
    Failed,
}

#[cfg(unix)]
fn pid_alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn pid_alive(_pid: u32) -> bool {
    false
}

/// Verify the server process is actually dead after a kill request.
///
/// `kill_server` only buffers a message in the kill channel; if the spawn
/// task's merge loop already exited, the message may never be consumed and
/// the process survives while the TUI believes it stopped. This waits up to
/// 5s for the spawn task's exit notification, then checks the pid directly
/// and sends SIGKILL as a last resort.
pub async fn ensure_killed(pid: u32, exit_rx: Option<mpsc::Receiver<()>>) -> KillOutcome {
    if let Some(mut rx) = exit_rx
        && tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv())
            .await
            .is_ok()
    {
        return KillOutcome::Exited;
    }
    if !pid_alive(pid) {
        return KillOutcome::Gone;
    }
    #[cfg(unix)]
    {
        tracing::warn!(
            "ensure_killed: pid {} still alive after kill — sending direct SIGKILL",
            pid
        );
        let sent = std::process::Command::new("kill")
            .args(["-9", &pid.to_string()])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if !sent {
            return KillOutcome::Failed;
        }
        for _ in 0..10 {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            if !pid_alive(pid) {
                return KillOutcome::Forced;
            }
        }
        KillOutcome::Failed
    }
    #[cfg(not(unix))]
    {
        // No portable direct-kill path; the exit wait above already timed out.
        KillOutcome::Failed
    }
}

/// Parse a Prometheus text exposition from llama-server's /metrics endpoint.
///
/// Handles both the legacy `llama_server_*` / `llama_kv_cache_*` names and
/// the renamed `llamacpp:*` names used by newer builds (b11000+).
pub fn parse_metrics_text(text: &str) -> ServerMetrics {
    let mut m = ServerMetrics::default();

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
            "llamacpp:requests_processing" | "llama_server_requests_processing" => {
                m.busy = m.busy || val > 0.0;
            }
            "llamacpp:kv_cache_usage_ratio" | "llama_server_kv_cache_usage_ratio" => {
                if !is_slot && ctx_max_global > 0 {
                    ctx_used_global = ctx_used_global.max((val * ctx_max_global as f64) as u32);
                }
            }
            _ => {
                tracing::debug!("Unknown metric: {}", name);
            }
        }
    }

    // Prefer global metrics (includes model weights + KV cache) over slot-only (KV cache subset).
    m.gpu_mem_used = if vram_used_global > 0 {
        vram_used_global
    } else if vram_used_slots > 0 {
        vram_used_slots
    } else {
        0
    };
    m.gpu_mem_total = if vram_total_global > 0 {
        vram_total_global
    } else if vram_total_slots > 0 {
        vram_total_slots
    } else {
        0
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

    m.loaded = ctx_max_global > 0 || vram_used_global > 0;

    m
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
        format!(
            "http://{}:{}/metrics?model={}",
            host,
            port,
            urlencoding::encode(name)
        )
    } else {
        format!("http://{}:{}/metrics", host, port)
    };

    let mut resp = HTTP_CLIENT
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to get metrics: {}", e))?;

    // If model-specific metrics fail with 404 or 400, try plain /metrics
    if (resp.status() == reqwest::StatusCode::NOT_FOUND
        || resp.status() == reqwest::StatusCode::BAD_REQUEST)
        && model_name.is_some()
    {
        url = format!("http://{}:{}/metrics", host, port);
        resp = HTTP_CLIENT
            .get(&url)
            .send()
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

    let mut m = parse_metrics_text(&text);

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

        let cached_nvidia;
        let cached_amd;
        {
            let cache = VRAM_CACHE.lock().unwrap_or_else(|e| e.into_inner());
            cached_nvidia = cache.nvidia;
            cached_amd = cache.amd;
        }

        if cached_nvidia.is_some() {
            if let Some((nv_used, nv_total)) = cached_nvidia {
                set_if_better(&mut m, nv_used, nv_total);
            }
        } else {
            let nv = tokio::task::spawn_blocking(get_nvidia_vram_metrics)
                .await
                .unwrap_or(Err("spawn join error".to_string()));
            if let Ok((used, total)) = nv {
                {
                    let mut cache = VRAM_CACHE.lock().unwrap_or_else(|e| e.into_inner());
                    cache.nvidia = Some((used, total));
                    cache.stale = false;
                }
                set_if_better(&mut m, used, total);
            }
        }

        if m.gpu_mem_total == 0 {
            if cached_amd.is_some() {
                if let Some((amd_used, amd_total)) = cached_amd {
                    set_if_better(&mut m, amd_used, amd_total);
                }
            } else {
                // AMD fallback when nvidia-smi is not available.
                let amd = tokio::task::spawn_blocking(get_amdgpu_vram_metrics)
                    .await
                    .unwrap_or(Err("spawn join error".to_string()));
                if let Ok((used, total)) = amd {
                    {
                        let mut cache = VRAM_CACHE.lock().unwrap_or_else(|e| e.into_inner());
                        cache.amd = Some((used, total));
                        cache.stale = false;
                    }
                    set_if_better(&mut m, used, total);
                }
            }
        }
    } else if m.gpu_mem_used == 0 {
        // KV-only queries: use system tools as a last resort.
        let cached_nvidia;
        let cached_amd;
        {
            let cache = VRAM_CACHE.lock().unwrap_or_else(|e| e.into_inner());
            cached_nvidia = cache.nvidia;
            cached_amd = cache.amd;
        }

        if cached_nvidia.is_some() {
            if let Some((used, total)) = cached_nvidia {
                m.gpu_mem_used = used;
                m.gpu_mem_total = total;
            }
        } else {
            let nv = tokio::task::spawn_blocking(get_nvidia_vram_metrics)
                .await
                .unwrap_or(Err("spawn join error".to_string()));
            if let Ok((used, total)) = nv {
                {
                    let mut cache = VRAM_CACHE.lock().unwrap_or_else(|e| e.into_inner());
                    cache.nvidia = Some((used, total));
                    cache.stale = false;
                }
                m.gpu_mem_used = used;
                m.gpu_mem_total = total;
            }
        }

        if m.gpu_mem_used == 0 {
            if cached_amd.is_some() {
                if let Some((used, total)) = cached_amd {
                    m.gpu_mem_used = used;
                    m.gpu_mem_total = total;
                }
            } else {
                let amd = tokio::task::spawn_blocking(get_amdgpu_vram_metrics)
                    .await
                    .unwrap_or(Err("spawn join error".to_string()));
                if let Ok((used, total)) = amd {
                    {
                        let mut cache = VRAM_CACHE.lock().unwrap_or_else(|e| e.into_inner());
                        cache.amd = Some((used, total));
                        cache.stale = false;
                    }
                    m.gpu_mem_used = used;
                    m.gpu_mem_total = total;
                }
            }
        }
    }

    // Fallback for RAM and CPU using sysinfo (cross-platform)
    if let Some(p) = pid
        && let Ok((ram, cpu)) = get_process_metrics(p)
    {
        if m.ram_used == 0 {
            m.ram_used = ram;
        }
        if m.cpu_usage == 0.0 {
            m.cpu_usage = cpu;
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
    let mut total_used: u64 = 0;
    let mut total_total: u64 = 0;
    for line in stdout.lines() {
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() >= 2 {
            let used = match parts[0].trim().parse::<u64>() {
                Ok(v) => v,
                Err(_) => {
                    warn!(
                        "nvidia-smi: failed to parse used memory from '{}'",
                        parts[0]
                    );
                    continue;
                }
            } * 1024
                * 1024;
            let total = match parts[1].trim().parse::<u64>() {
                Ok(v) => v,
                Err(_) => {
                    warn!(
                        "nvidia-smi: failed to parse total memory from '{}'",
                        parts[1]
                    );
                    continue;
                }
            } * 1024
                * 1024;
            total_used += used;
            total_total += total;
        }
    }
    if total_total > 0 {
        return Ok((total_used, total_total));
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
/// Uses a persistent System instance so sysinfo can compute accurate
/// CPU deltas across calls (first call on a fresh instance is always 0).
fn get_process_metrics(pid: u32) -> Result<(u64, f64), String> {
    use std::sync::{LazyLock, Mutex};
    use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};

    static SYS: LazyLock<Mutex<System>> = LazyLock::new(|| {
        Mutex::new(System::new_with_specifics(
            RefreshKind::everything()
                .with_processes(ProcessRefreshKind::nothing().with_cpu().with_memory()),
        ))
    });

    let mut sys = SYS.lock().unwrap_or_else(|e| e.into_inner());
    let pids = [Pid::from(pid as usize)];
    sys.refresh_processes_specifics(
        ProcessesToUpdate::Some(&pids),
        true,
        ProcessRefreshKind::nothing().with_cpu().with_memory(),
    );

    let sys_pid = Pid::from(pid as usize);

    if let Some(process) = sys.process(sys_pid) {
        let ram = process.memory(); // bytes
        let cpu = process.cpu_usage() as f64; // percentage
        return Ok((ram, cpu));
    }

    Err(format!("Process not found: pid={}", pid))
}

/// Load a model via the llama-server Router API.
/// Uses the canonical `/models/load` endpoint with the `"model"` JSON field.
/// Model is identified by its display_name (matches the `--alias` registered in llama.cpp).
pub async fn load_model(host: &str, port: u16, model_id: &str) -> Result<(), String> {
    let host = clean_host(host);

    let props_url = format!("http://{}:{}/props", host, port);
    if let Ok(res) = HTTP_CLIENT.get(&props_url).send().await
        && res.status().is_success()
        && let Ok(json) = res.json::<serde_json::Value>().await
        && json.get("role").and_then(|r| r.as_str()) != Some("router")
    {
        return Err(
            "Server is not in router mode. Start with --models-max to enable router mode."
                .to_string(),
        );
    }

    let url = format!("http://{}:{}/models/load", host, port);
    let body = serde_json::json!({ "model": model_id });

    match HTTP_CLIENT.post(&url).json(&body).send().await {
        Ok(res) if res.status().is_success() => Ok(()),
        Ok(res) => {
            let status = res.status();
            let error = res
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            if error.contains("model is already running") {
                Ok(())
            } else {
                Err(format!("Server returned {}: {}", status, error))
            }
        }
        Err(e) => Err(format!("Failed to send request: {}", e)),
    }
}

/// List all models and their status from the llama-server Router API.
/// Returns `(id, status, path, status_description)`; the description carries
/// the error text when the router reports a failed load (status `"error"`).
pub async fn list_models(
    host: &str,
    port: u16,
) -> Result<Vec<(String, String, Option<String>, Option<String>)>, String> {
    let host = clean_host(host);
    let url = format!("http://{}:{}/models", host, port);

    let res = HTTP_CLIENT
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
            let status_obj = model.get("status");
            let status = status_obj
                .and_then(|s| s.get("value").or(Some(s)))
                .and_then(|v| v.as_str())
                .unwrap_or("unloaded")
                .to_string();
            // When status is an object, "description" carries details (e.g. the
            // error message for a failed load).
            let description = status_obj
                .and_then(|s| s.get("description"))
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string());
            let path = model
                .get("path")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            results.push((id, status, path, description));
        }
    }

    Ok(results)
}

/// Unload a model via the llama-server Router API.
/// Uses the canonical `/models/unload` endpoint with the `"model"` JSON field.
pub async fn unload_model(host: &str, port: u16, model_id: &str) -> Result<(), String> {
    let host = clean_host(host);
    let url = format!("http://{}:{}/models/unload", host, port);
    let body = serde_json::json!({ "model": model_id });

    match HTTP_CLIENT.post(&url).json(&body).send().await {
        Ok(res) if res.status().is_success() => Ok(()),
        Ok(res) => {
            let status = res.status();
            let error = res
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            tracing::debug!(
                "Model unload failed (status {}, error: {}): this is expected if model was already unloaded",
                status,
                error
            );
            Ok(())
        }
        Err(e) => {
            tracing::debug!("Model unload request failed: {}", e);
            Ok(())
        }
    }
}
