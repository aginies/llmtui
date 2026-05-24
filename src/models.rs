use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// The state of a model in the manager.
#[derive(Debug, Clone, PartialEq)]
pub enum ModelState {
    Available,
    Loading,
    Benchmarking,
    Loaded {
        port: u16,
        pid: u32,
    },
    Failed {
        error: String,
    },
}

/// Sort order for search results.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchSort {
    Relevance,
    Downloads,
    Likes,
    Trending,
    CreatedAt,
}

impl SearchSort {
    pub fn next(self) -> Self {
        match self {
            SearchSort::Relevance => SearchSort::Downloads,
            SearchSort::Downloads => SearchSort::Likes,
            SearchSort::Likes => SearchSort::Trending,
            SearchSort::Trending => SearchSort::CreatedAt,
            SearchSort::CreatedAt => SearchSort::Relevance,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SearchSort::Relevance => "Relevance",
            SearchSort::Downloads => "Downloads",
            SearchSort::Likes => "Likes",
            SearchSort::Trending => "Trending",
            SearchSort::CreatedAt => "Created",
        }
    }
}

/// A model found via HuggingFace search.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchResult {
    pub model_id: String,
    pub model_name: String,
    pub tags: Vec<String>,
    pub downloads: u64,
    pub likes: u64,
    pub pipeline_tag: Option<String>,
    pub size: Option<u64>,
    pub parameters: Option<String>,
    pub capabilities: Vec<String>,
    pub context_length: Option<u32>,
    pub readme: Option<String>,
    /// Quantization type extracted from GGUF metadata (e.g. "Q4_K_M", "Q8_0").
    pub quantization: Option<String>,
    /// License extracted from tags (e.g. "apache-2.0", "llama3.1").
    pub license: Option<String>,
    /// HuggingFace trending score.
    pub trending_score: i64,
    /// Creation timestamp string.
    pub created_at: Option<String>,
}

/// Download progress information.
#[derive(Debug, Clone)]
pub struct DownloadState {
    pub model_id: String,
    pub filename: String,
    pub total_bytes: u64,
    pub downloaded_bytes: u64,
    pub status: DownloadStatus,
    pub cancelled: bool,
    pub cancel_token: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    /// Download control: 1=downloading, 2=paused, 3=cancelled
    pub download_state: u8,
    /// Shared atomic state for pausing/resuming the download loop
    pub download_state_arc: Option<std::sync::Arc<std::sync::atomic::AtomicU8>>,
    pub start_time: std::time::Instant,
    pub bytes_per_second: f64,
}

impl DownloadState {
    pub fn new(model_id: String, filename: String, total_bytes: u64) -> Self {
        Self {
            model_id,
            filename,
            total_bytes,
            downloaded_bytes: 0,
            status: DownloadStatus::Downloading,
            cancelled: false,
            cancel_token: None,
            download_state: 1,
            download_state_arc: None,
            start_time: std::time::Instant::now(),
            bytes_per_second: 0.0,
        }
    }
}

impl ModelSettings {
    /// Get the version string for the currently active backend.
    pub fn get_active_backend_version(&self) -> Option<&String> {
        match self.backend {
            Backend::Cpu => self.llama_cpp_version_cpu.as_ref(),
            Backend::Vulkan => self.llama_cpp_version_vulkan.as_ref(),
            Backend::Rocm => self.llama_cpp_version_rocm.as_ref(),
            Backend::RocmLemonade => self.llama_cpp_version_rocm_lemonade.as_ref(),
            Backend::Cuda => self.llama_cpp_version_cuda.as_ref(),
        }
    }

    /// Get the display version string for the currently active backend (defaults to "latest").
    pub fn get_active_backend_version_display(&self) -> &str {
        self.get_active_backend_version()
            .map(|s| s.as_str())
            .unwrap_or("latest")
    }

    /// Set the version string for the currently active backend.
    pub fn set_active_backend_version(&mut self, tag: Option<String>) {
        match self.backend {
            Backend::Cpu => self.llama_cpp_version_cpu = tag,
            Backend::Vulkan => self.llama_cpp_version_vulkan = tag,
            Backend::Rocm => self.llama_cpp_version_rocm = tag,
            Backend::RocmLemonade => self.llama_cpp_version_rocm_lemonade = tag,
            Backend::Cuda => self.llama_cpp_version_cuda = tag,
        }
    }
}

/// Strip the .gguf extension from a model name.
pub fn strip_gguf(name: &str) -> &str {
    name.strip_suffix(".gguf")
        .or_else(|| name.strip_suffix(".GGUF"))
        .unwrap_or(name)
}

/// Ensure host string is valid for URL construction and CLI arguments.
/// Handles empty strings (defaults to 127.0.0.1), strips display suffixes,
/// and wraps IPv6 addresses in brackets.
pub fn clean_host(host: &str) -> String {
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

/// Format a host string for display (e.g. "" or "127.0.0.1" -> "localhost (127.0.0.1)").
pub fn format_host(host: &str) -> &str {
    match host {
        "" | "127.0.0.1" => "localhost (127.0.0.1)",
        _ => host,
    }
}

impl From<crate::config::DefaultParams> for ModelSettings {
    fn from(dp: crate::config::DefaultParams) -> Self {
        Self {
            context_length: dp.context_length,
            threads: dp.threads,
            threads_batch: dp.threads_batch,
            batch_size: dp.batch_size,
            ubatch_size: dp.ubatch_size,
            parallel: dp.parallel,
            max_concurrent_predictions: dp.max_concurrent_predictions,
            uniform_cache: dp.uniform_cache,
            kv_cache_offload: dp.kv_cache_offload,
            cache_type_k: dp.cache_type_k,
            cache_type_v: dp.cache_type_v,
            keep: dp.keep,
            swa_full: dp.swa_full,
            mlock: dp.mlock,
            mmap: dp.mmap,
            numa: dp.numa,
            system_prompt: dp.system_prompt,
            system_prompt_preset_name: dp.system_prompt_preset_name,
            reasoning_mode: dp.reasoning_mode,
            gpu_layers_mode: match dp.gpu_layers {
                n if n < 0 => GpuLayersMode::All,
                _ => dp.gpu_layers_mode,
            },
            split_mode: dp.split_mode,
            tensor_split: dp.tensor_split,
            main_gpu: dp.main_gpu,
            fit: dp.fit,
            lora: dp.lora,
            lora_scaled: dp.lora_scaled,
            rpc: dp.rpc,
            embedding: dp.embedding,
            flash_attn: dp.flash_attn,
            expert_count: dp.expert_count,
            jinja: dp.jinja,
            chat_template: dp.chat_template,
            seed: dp.seed,
            temperature: dp.temperature,
            top_k: dp.top_k,
            top_p: dp.top_p,
            min_p: dp.min_p,
            typical_p: dp.typical_p,
            mirostat: dp.mirostat,
            mirostat_lr: dp.mirostat_lr,
            mirostat_ent: dp.mirostat_ent,
            ignore_eos: dp.ignore_eos,
            samplers: dp.samplers,
            repeat_penalty: dp.repeat_penalty,
            repeat_last_n: dp.repeat_last_n,
            presence_penalty: dp.presence_penalty,
            frequency_penalty: dp.frequency_penalty,
            dry_multiplier: dp.dry_multiplier,
            dry_base: dp.dry_base,
            dry_allowed_length: dp.dry_allowed_length,
            dry_penalty_last_n: dp.dry_penalty_last_n,
            rope_scaling: dp.rope_scaling,
            rope_scale: dp.rope_scale,
            rope_freq_base: dp.rope_freq_base,
            rope_freq_scale: dp.rope_freq_scale,
            host: dp.host,
            port: dp.port,
            timeout: dp.timeout,
            cache_prompt: dp.cache_prompt,
            cache_reuse: dp.cache_reuse,
            webui: dp.webui,
            max_tokens: dp.max_tokens,
            cache_type: dp.cache_type,
            backend: dp.backend,
            llama_cpp_version_cpu: dp.llama_cpp_version_cpu,
            llama_cpp_version_vulkan: dp.llama_cpp_version_vulkan,
            llama_cpp_version_rocm: dp.llama_cpp_version_rocm,
           llama_cpp_version_rocm_lemonade: dp.llama_cpp_version_rocm_lemonade,
            llama_cpp_version_cuda: dp.llama_cpp_version_cuda,
            api_endpoint_enabled: dp.api_endpoint_enabled,
            api_endpoint_port: dp.api_endpoint_port,
            is_mtp: dp.is_mtp,
            draft_tokens: dp.draft_tokens,
            tags: dp.tags,
        }
    }
}

/// How to handle GPU layer offloading.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Hash)]
pub enum GpuLayersMode {
    Auto,
    Specific(u32),
    All,
}

impl Default for GpuLayersMode {
    fn default() -> Self {
        GpuLayersMode::Auto
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadStatus {
    Downloading,
    Paused,
    Complete,
    Error(String),
}

// ── Cache type enums ──────────────────────────────────────────

/// Main KV cache data type.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Hash)]
#[derive(Default)]
pub enum CacheType {
    #[serde(rename = "f16")]
    #[default]
    F16,
    #[serde(rename = "bf16")]
    BF16,
    #[serde(rename = "fq8_0")]
    Fq8_0,
    #[serde(rename = "fq4_1")]
    Fq4_1,
}


impl std::fmt::Display for CacheType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CacheType::F16 => write!(f, "f16"),
            CacheType::BF16 => write!(f, "bf16"),
            CacheType::Fq8_0 => write!(f, "fq8_0"),
            CacheType::Fq4_1 => write!(f, "fq4_1"),
        }
    }
}

/// KV cache quantization type.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default, Hash)]
pub enum CacheQuantType {
    #[serde(rename = "f32")]
    F32,
    #[serde(rename = "f16")]
    #[default]
    F16,
    #[serde(rename = "bf16")]
    BF16,
    #[serde(rename = "q8_0")]
    Q8_0,
    #[serde(rename = "q4_0")]
    Q4_0,
    #[serde(rename = "q4_1")]
    Q4_1,
    #[serde(rename = "iq4_nl")]
    Iq4Nl,
    #[serde(rename = "q5_0")]
    Q5_0,
    #[serde(rename = "q5_1")]
    Q5_1,
}

pub type CacheTypeK = CacheQuantType;
pub type CacheTypeV = CacheQuantType;

impl CacheQuantType {
    pub fn from_u8(n: u8) -> Self {
        match n {
            0 => Self::F32,
            1 => Self::F16,
            2 => Self::BF16,
            3 => Self::Q8_0,
            4 => Self::Q5_1,
            5 => Self::Q5_0,
            6 => Self::Q4_1,
            7 => Self::Q4_0,
            8 => Self::Iq4Nl,
            _ => Self::F16,
        }
    }
    pub fn next(&self) -> Self {
        match self {
            Self::F32 => Self::F16,
            Self::F16 => Self::BF16,
            Self::BF16 => Self::Q8_0,
            Self::Q8_0 => Self::Q5_1,
            Self::Q5_1 => Self::Q5_0,
            Self::Q5_0 => Self::Q4_1,
            Self::Q4_1 => Self::Q4_0,
            Self::Q4_0 => Self::Iq4Nl,
            Self::Iq4Nl => Self::F32,
        }
    }
    pub fn prev(&self) -> Self {
        match self {
            Self::F32 => Self::Iq4Nl,
            Self::F16 => Self::F32,
            Self::BF16 => Self::F16,
            Self::Q8_0 => Self::BF16,
            Self::Q5_1 => Self::Q8_0,
            Self::Q5_0 => Self::Q5_1,
            Self::Q4_1 => Self::Q5_0,
            Self::Q4_0 => Self::Q4_1,
            Self::Iq4Nl => Self::Q4_0,
        }
    }
}


impl std::fmt::Display for CacheQuantType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::F32 => write!(f, "f32"),
            Self::F16 => write!(f, "f16"),
            Self::BF16 => write!(f, "bf16"),
            Self::Q8_0 => write!(f, "q8_0"),
            Self::Q4_0 => write!(f, "q4_0"),
            Self::Q4_1 => write!(f, "q4_1"),
            Self::Iq4Nl => write!(f, "iq4_nl"),
            Self::Q5_0 => write!(f, "q5_0"),
            Self::Q5_1 => write!(f, "q5_1"),
        }
    }
}

impl From<&str> for CacheQuantType {
    fn from(s: &str) -> Self {
        match s {
            "F32" => Self::F32,
            "F16" => Self::F16,
            "BF16" => Self::BF16,
            "Q8_0" => Self::Q8_0,
            "Q4_0" => Self::Q4_0,
            "Q4_1" => Self::Q4_1,
            "Iq4Nl" => Self::Iq4Nl,
            "Q5_0" => Self::Q5_0,
            "Q5_1" => Self::Q5_1,
            _ => Self::F16, // Default or error handling
        }
    }
}

/// Split mode for multi-GPU.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Hash)]
#[derive(Default)]
pub enum SplitMode {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "layer")]
    #[default]
    Layer,
    #[serde(rename = "row")]
    Row,
    #[serde(rename = "tensor")]
    Tensor,
}


impl std::fmt::Display for SplitMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SplitMode::None => write!(f, "none"),
            SplitMode::Layer => write!(f, "layer"),
            SplitMode::Row => write!(f, "row"),
            SplitMode::Tensor => write!(f, "tensor"),
        }
    }
}

/// NUMA optimization mode.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Hash)]
#[derive(Default)]
pub enum NumMode {
    #[serde(rename = "none")]
    #[default]
    None,
    #[serde(rename = "distribute")]
    Distribute,
    #[serde(rename = "isolate")]
    Isolate,
    #[serde(rename = "numactl")]
    Numactl,
}


impl std::fmt::Display for NumMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NumMode::None => write!(f, "none"),
            NumMode::Distribute => write!(f, "distribute"),
            NumMode::Isolate => write!(f, "isolate"),
            NumMode::Numactl => write!(f, "numactl"),
        }
    }
}

/// RoPE frequency scaling method.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Hash)]
#[derive(Default)]
pub enum RopeScaling {
    #[serde(rename = "none")]
    #[default]
    None,
    #[serde(rename = "linear")]
    Linear,
    #[serde(rename = "yarn")]
    Yarn,
}


impl std::fmt::Display for RopeScaling {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RopeScaling::None => write!(f, "none"),
            RopeScaling::Linear => write!(f, "linear"),
            RopeScaling::Yarn => write!(f, "yarn"),
        }
    }
}

/// Mirostat version.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Hash)]
#[derive(Default)]
pub enum Mirostat {
    #[serde(rename = "0")]
    #[default]
    Off,
    #[serde(rename = "1")]
    Mirostat,
    #[serde(rename = "2")]
    Mirostat2,
}


impl std::fmt::Display for Mirostat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Mirostat::Off => write!(f, "off"),
            Mirostat::Mirostat => write!(f, "1"),
            Mirostat::Mirostat2 => write!(f, "2"),
        }
    }
}

/// Sampler order string (semicolon-separated).
/// Common types: penalties, dry, top_n_sigma, top_k, typ_p, top_p, min_p, xtc, temperature
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Samplers(pub String);

impl Default for Samplers {
    fn default() -> Self {
        Self("penalties;dry;top_n_sigma;top_k;typ_p;top_p;min_p;xtc;temperature".to_string())
    }
}

impl std::fmt::Display for Samplers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Backend used to run the llama.cpp server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[derive(Default)]
pub enum Backend {
    #[serde(rename = "cpu")]
    #[default]
    Cpu,
    #[serde(rename = "vulkan")]
    Vulkan,
    #[serde(rename = "rocm")]
    Rocm,
    #[serde(rename = "rocm_lemonade")]
    RocmLemonade,
    #[serde(rename = "cuda")]
    Cuda,
}


impl Backend {
    /// Get the identifier used for directory names and asset prefixes.
    pub fn slug(&self) -> &'static str {
        match self {
            Backend::Cpu => "cpu",
            Backend::Vulkan => "vulkan",
            Backend::Rocm => "rocm",
            Backend::RocmLemonade => "rocm-lemonade",
            Backend::Cuda => "cuda",
        }
    }
}

impl std::fmt::Display for Backend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.slug())
    }
}


/// Server mode: normal (single model) or router (multiple models).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[derive(Default)]
pub enum ServerMode {
    #[serde(rename = "normal")]
    #[default]
    Normal,
    #[serde(rename = "router")]
    Router,
    #[serde(rename = "bench_gpu", alias = "bench")]
    Bench,
    #[serde(rename = "bench_tune")]
    BenchTune,
}


impl std::fmt::Display for ServerMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServerMode::Normal => write!(f, "Normal"),
            ServerMode::Router => write!(f, "Router (XP!)"),
            ServerMode::Bench => write!(f, "Bench GPU"),
            ServerMode::BenchTune => write!(f, "BenchTune"),
        }
    }
}

/// Mode for parsing reasoning tags from model responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[derive(Default)]
pub enum ReasoningMode {
    #[serde(rename = "default")]
    #[default]
    Default, // DeepSeek/OpenAI style: <think> ... </think>
    #[serde(rename = "gemma")]
    Gemma,   // Gemma style: <|channel>thought <channel|>
}


impl std::fmt::Display for ReasoningMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReasoningMode::Default => write!(f, "default"),
            ReasoningMode::Gemma => write!(f, "gemma"),
        }
    }
}

// ── ModelSettings ─────────────────────────────────────────────

/// Settings for loading a model via llama.cpp server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelSettings {
    // ── Loading ──────────────────────────────────────────────

    /// Size of the prompt context.
    pub context_length: u32,
    /// Number of CPU threads for generation.
    pub threads: u32,
    /// Number of CPU threads for batch processing.
    pub threads_batch: u32,
    /// Logical maximum batch size.
    pub batch_size: u32,
    /// Physical maximum batch size (micro-batch).
    pub ubatch_size: u32,
    /// Max concurrent predictions (sequences).
    pub parallel: u32,
    /// Max concurrent predictions (requests in flight). None means no --parallel argument.
    pub max_concurrent_predictions: Option<u32>,
    /// Use uniform (unified) KV cache across all sequences.
    pub uniform_cache: bool,
    /// Offload KV cache to system RAM.
    pub kv_cache_offload: bool,
    /// KV cache data type for K.
    pub cache_type_k: Option<CacheTypeK>,
    /// KV cache data type for V.
    pub cache_type_v: Option<CacheTypeV>,
    /// Keep N tokens from the initial prompt.
    pub keep: i32,
    /// Use full-size SWA cache.
    pub swa_full: bool,
    /// Force system to keep model in RAM.
    pub mlock: bool,
    /// Memory-map the model.
    pub mmap: bool,
    /// NUMA optimization.
    pub numa: NumMode,
    /// System prompt.
    pub system_prompt: String,
    /// Name of the system prompt preset currently selected.
    pub system_prompt_preset_name: String,
    /// How to parse reasoning tags from model responses.
    pub reasoning_mode: ReasoningMode,

    // ── GPU ──────────────────────────────────────────────────

    /// GPU layer offloading mode.
    pub gpu_layers_mode: GpuLayersMode,
    /// Split mode across multiple GPUs.
    pub split_mode: SplitMode,
    /// Fraction of model offloaded to each GPU (comma-separated).
    pub tensor_split: String,
    /// Main GPU index.
    pub main_gpu: i32,
    /// Whether to adjust arguments to fit device memory.
    pub fit: bool,
    /// Path to LoRA adapter.
    pub lora: Option<PathBuf>,
    /// Path to LoRA adapter with scale.
    pub lora_scaled: Option<(PathBuf, f32)>,
    /// RPC servers.
    pub rpc: String,
    /// Restrict to embedding use case.
    pub embedding: bool,
    /// Enable Flash Attention.
    pub flash_attn: bool,
    /// Active experts per token (MoE models, -1 = model default).
    pub expert_count: i32,
    /// Use Jinja template engine for chat.
    pub jinja: bool,
    /// Custom chat template string.
    pub chat_template: Option<String>,

    // ── Sampling ─────────────────────────────────────────────

    /// RNG seed (-1 = random).
    pub seed: i32,
    /// Temperature.
    pub temperature: f32,
    /// Top-k sampling (0 = disabled).
    pub top_k: i32,
    /// Top-p sampling (1.0 = disabled).
    pub top_p: f32,
    /// Minimum probability for a token.
    pub min_p: f32,
    /// Locally typical sampling parameter p.
    pub typical_p: f32,
    /// Mirostat version (0=off, 1=Mirostat, 2=Mirostat2).
    pub mirostat: Mirostat,
    /// Mirostat learning rate (eta).
    pub mirostat_lr: f32,
    /// Mirostat target entropy (tau).
    pub mirostat_ent: f32,
    /// Ignore end-of-stream token.
    pub ignore_eos: bool,
    /// Sampler order string.
    pub samplers: Samplers,

    // ── Repetition Control ───────────────────────────────────

    /// Penalize repeat sequence of tokens.
    pub repeat_penalty: f32,
    /// Last N tokens to consider for repeat penalty.
    pub repeat_last_n: i32,
    /// Repeat alpha presence penalty.
    pub presence_penalty: Option<f32>,
    /// Repeat alpha frequency penalty.
    pub frequency_penalty: Option<f32>,
    /// DRY sampling multiplier.
    pub dry_multiplier: f32,
    /// DRY sampling base value.
    pub dry_base: f32,
    /// DRY allowed length.
    pub dry_allowed_length: i32,
    /// DRY penalty last N.
    pub dry_penalty_last_n: i32,

    // ── RoPE ─────────────────────────────────────────────────

    /// RoPE frequency scaling method.
    pub rope_scaling: RopeScaling,
    /// RoPE context scaling factor.
    pub rope_scale: f32,
    /// RoPE base frequency.
    pub rope_freq_base: f32,
    /// RoPE frequency scaling factor.
    pub rope_freq_scale: f32,

    // ── Server ───────────────────────────────────────────────

    /// Host address.
    pub host: String,
    /// Port.
    pub port: u16,
    /// Server timeout in seconds.
    pub timeout: u32,
    /// Whether to enable prompt caching.
    pub cache_prompt: bool,
    /// Min chunk size for cache reuse.
    pub cache_reuse: u32,
    /// Whether to enable WebUI.
    pub webui: bool,

    // ── Other ────────────────────────────────────────────────

    /// Max tokens to predict.
    pub max_tokens: Option<u32>,
    /// Cache type (legacy, kept for compatibility).
    pub cache_type: CacheType,
    /// Backend (cpu/vulkan).
    pub backend: Backend,
    /// llama.cpp release tag for CPU backend (e.g. "b1234" or None for latest).
    pub llama_cpp_version_cpu: Option<String>,
    /// llama.cpp release tag for Vulkan backend (e.g. "b1234" or None for latest).
    pub llama_cpp_version_vulkan: Option<String>,
    /// llama.cpp release tag for ROCm backend (e.g. "b1234" or None for latest).
    pub llama_cpp_version_rocm: Option<String>,
    /// Lemonade llama.cpp release tag for ROCm backend.
    pub llama_cpp_version_rocm_lemonade: Option<String>,
    /// llama.cpp release tag for CUDA backend.
    pub llama_cpp_version_cuda: Option<String>,
    /// Whether to enable the API proxy server.
    pub api_endpoint_enabled: bool,
    /// Port for the API proxy server.
    pub api_endpoint_port: u16,
    /// Whether this model uses MTP (Multi-Token Prediction) architecture.
    pub is_mtp: bool,
    /// Number of draft tokens for MTP.
    pub draft_tokens: u32,
    /// Tags for the model.
    pub tags: Vec<String>,
}

impl Default for ModelSettings {
    fn default() -> Self {
        Self {
            // Loading
            context_length: 32096,
            threads: 8,
            threads_batch: 8,
            batch_size: 512,
            ubatch_size: 512,
            parallel: 1,
            max_concurrent_predictions: None,
            uniform_cache: false,
            kv_cache_offload: true,
            cache_type_k: Some(CacheTypeK::F16),
            cache_type_v: Some(CacheTypeV::F16),
            keep: 0,
            swa_full: false,
            mlock: false,
            mmap: true,
            numa: NumMode::None,
            system_prompt: "You are a helpful assistant.".to_string(),
            system_prompt_preset_name: "General".to_string(),
            reasoning_mode: ReasoningMode::Default,

            // GPU
            gpu_layers_mode: GpuLayersMode::Auto,
            split_mode: SplitMode::Layer,
            tensor_split: String::new(),
            main_gpu: 0,
            fit: true,
            lora: None,
            lora_scaled: None,
            rpc: String::new(),
            embedding: false,
            flash_attn: true,
            expert_count: -1,
            jinja: true,
            chat_template: None,

            // Sampling
            seed: -1,
            temperature: 0.8,
            top_k: 40,
            top_p: 0.95,
            min_p: 0.0,
            typical_p: 1.0,
            mirostat: Mirostat::Off,
            mirostat_lr: 0.1,
            mirostat_ent: 5.0,
            ignore_eos: false,
            samplers: Samplers::default(),

            // Repetition
            repeat_penalty: 1.1,
            repeat_last_n: 64,
            presence_penalty: Some(0.0),
            frequency_penalty: Some(0.0),
            dry_multiplier: 0.0,
            dry_base: 1.75,
            dry_allowed_length: 2,
            dry_penalty_last_n: -1,

            // RoPE
            rope_scaling: RopeScaling::None,
            rope_scale: 1.0,
            rope_freq_base: 0.0, // 0 = loaded from model
            rope_freq_scale: 1.0,

            // Server
            host: "127.0.0.1".to_string(),
            port: 8080,
            timeout: 600,
            cache_prompt: true,
            cache_reuse: 0,
            webui: false,

          // Other
            max_tokens: None,
            cache_type: CacheType::default(),
            backend: Backend::Cpu,
            llama_cpp_version_cpu: None,
            llama_cpp_version_vulkan: None,
            llama_cpp_version_rocm: None,
           llama_cpp_version_rocm_lemonade: None,
            llama_cpp_version_cuda: None,
            api_endpoint_enabled: false,
            api_endpoint_port: 49222,
            is_mtp: false,
            draft_tokens: 0,
            tags: Vec::new(),
        }
    }
}

impl ModelSettings {
    /// Create ModelSettings from config defaults, applying model-specific overrides.
    pub fn from_config(config: &crate::config::Config) -> Self {
        config.default.clone().into()
    }
}

/// A discovered model file.
#[derive(Debug, Clone)]
pub struct DiscoveredModel {
    pub path: PathBuf,
    pub name: String,
    pub file_size: u64,
    pub display_name: String, // path relative to model_dir for display
}

/// Parsed GGUF metadata for a model, cached to avoid re-parsing the file.
#[derive(Debug, Clone)]
pub struct GgufMetadata {
    pub layers: u32,
    pub hidden_size: u32,
    pub n_ctx_train: u32,
    pub n_head: u32,
    pub n_kv_head: u32,
    pub arch: String,
    pub file_type: String,
    pub quantization: String,
    pub model_parameters: String,
    pub domain: String,
    pub capabilities: Vec<String>,
    pub tokenizer: String,
    pub vocab_size: u32,
    pub draft_tokens: u32,
}

/// Metrics reported by the llama.cpp server.
#[derive(Debug, Clone)]
pub struct ServerMetrics {
    pub loaded: bool,
    pub tps: f64,
    pub prompt_tps: f64,
    pub cpu_usage: f64,
    /// Previous CPU ticks (utime + stime) for delta-based CPU calculation.
    pub cpu_ticks_prev: u64,
    /// System uptime in seconds at last poll, used for wall-time delta in CPU calculation.
    pub system_uptime_prev: f64,
    pub gpu_mem_used: u64,
    pub gpu_mem_total: u64,
    pub ram_used: u64,
    pub ctx_used: u32,
    pub ctx_max: u32,
    /// Sum of gpu_mem_used across all loaded models (for Total VRAM display).
    pub total_vram_used: u64,
}

/// GPU device buffer reported by llama-server during model loading.
#[derive(Debug, Clone)]
pub struct GPUBuffer {
    pub device: String,
    pub buffer_size_mib: f64,
}

/// Progress information during model loading, parsed from llama-server log output.
#[derive(Debug, Clone)]
#[derive(Default)]
pub struct LoadProgress {
    /// Total number of layers in the model.
    pub layers_total: Option<u32>,
    /// Number of layers already offloaded to GPU.
    pub layers_loaded: Option<u32>,
   /// Number of tensors loaded (counted from dot-lines in log).
    pub tensors_loaded: u32,
    /// GPU device buffers with their sizes.
    pub buffers: Vec<GPUBuffer>,
}


impl Default for ServerMetrics {
    fn default() -> Self {
        Self {
            loaded: false,
            tps: 0.0,
            prompt_tps: 0.0,
            cpu_usage: 0.0,
            cpu_ticks_prev: 0,
            system_uptime_prev: 0.0,
            gpu_mem_used: 0,
            gpu_mem_total: 0,
            ram_used: 0,
            ctx_used: 0,
            ctx_max: 0,
            total_vram_used: 0,
        }
    }
}

/// Estimate VRAM usage (in MiB) for a model with the given settings.
///
/// Model file size is the size of the GGUF file in MiB. The model itself
/// takes 1x its size in VRAM (loaded as-is). KV cache is the dominant
/// variable cost — it scales with context_length, batch_size, and layers.
///
/// The KV cache formula accounts for:
/// - Actual GQA ratio from model metadata (n_kv_head / n_head)
/// - FlashAttention: reduces KV cache storage by ~2x
/// - Unified KV cache: shares KV across sequences, dividing by parallel count
/// - KV cache quantization (q4_0, q5_0, q8_0, etc.)
pub fn estimate_vram_mib(
    model_mib: u64,
    settings: &ModelSettings,
    total_layers: u32,
    hidden_size_opt: Option<u32>,
    n_head_opt: Option<u32>,
    n_kv_head_opt: Option<u32>,
    gpu_mem_total_mib: u64,
) -> u64 {
    let model_mib_f = model_mib as f64;

    // Compute how much of the model is loaded into VRAM based on GPU layers.
    let gpu_layers = match settings.gpu_layers_mode {
        GpuLayersMode::Auto => {
            // Heuristic: ~60% of layers when Auto (llama.cpp will decide at runtime)
            if total_layers > 0 {
                (total_layers as f64 * 0.6) as u32
            } else {
                20
            }
        }
        GpuLayersMode::Specific(n) => {
            if total_layers > 0 {
                n.min(total_layers)
            } else {
                n
            }
        }
        GpuLayersMode::All => {
            if total_layers > 0 { total_layers } else { 32 }
        }
    };

    // Model weights loaded into VRAM: proportional to GPU layers.
    let model_vram = if total_layers > 0 && gpu_layers > 0 {
        model_mib_f * (gpu_layers as f64 / total_layers as f64).min(1.0)
    } else if gpu_layers > 0 {
        model_mib_f
    } else {
        0.0
    };

    if matches!(settings.gpu_layers_mode, GpuLayersMode::Specific(0)) {
        return 0; // CPU only
    }

    // Heuristic for hidden_size if not provided:
    // A 7B model (4-bit) is ~4000 MiB and has hidden=4096.
    let hidden_size = match hidden_size_opt {
        Some(h) => h as f64,
        None => {
            let params_est = model_mib_f / 550.0;
            (1024.0 * params_est.sqrt().max(1.0) * 1.5).max(512.0)
        }
    };

    // ── KV cache estimation ─────────────────────────────────────

    // GQA ratio: real KV heads vs query heads.
    // If n_kv_head == n_head, ratio = 1.0 (no reduction).
    // If n_kv_head < n_head, ratio < 1.0 (KV cache is smaller).
    let gqa_ratio = match (n_head_opt, n_kv_head_opt) {
        (Some(n_head), Some(n_kv_head)) if n_head > 0 => {
            n_kv_head as f64 / n_head as f64
        }
        _ => 1.0, // fallback: assume no GQA
    };

    // FlashAttention reduces KV cache storage by ~2x because it doesn't
    // need to keep the full attention matrix in memory.
    let flash_attn_factor = if settings.flash_attn { 0.5 } else { 1.0 };

    // Unified KV cache shares a single KV buffer across all sequences.
    let uniform_cache_factor = if settings.uniform_cache { 1.0 / settings.parallel as f64 } else { 1.0 };

    // KV cache in MiB:
    // Formula: 2 * n_layer * n_ctx * n_embd_kv * sizeof(type)
    // n_embd_kv = hidden_size * gqa_ratio
    let kv_mib = (2.0
        * hidden_size
        * settings.context_length as f64
        * gpu_layers as f64
        * gqa_ratio
        * flash_attn_factor
        * uniform_cache_factor
        * kv_quant_bytes(
            settings.cache_type_k.unwrap_or(CacheTypeK::F16),
            settings.cache_type_v.unwrap_or(CacheTypeV::F16)
        ))
        / (1024.0 * 1024.0);

    // Activation overhead during inference (proportional to batch * hidden).
    // Increased multiplier to 8.0 (from 2.0) to be more pessimistic about scratch buffers.
    let activation_mib = (settings.batch_size as f64 * hidden_size * 8.0)
        / (1024.0 * 1024.0);

    // Fixed overhead for driver, fragmentation, and small meta buffers.
    // Use 3.8% of max VRAM, falling back to 500MiB if unknown.
    let fixed_overhead = if gpu_mem_total_mib > 0 {
        gpu_mem_total_mib as f64 * 0.038
    } else {
        500.0
    };

    let total_mib = model_vram + kv_mib + activation_mib + fixed_overhead + 550.0;

    total_mib.ceil() as u64
}

/// Return the average KV cache element size in bytes for the given K/V types.
///
/// KV cache stores K and V separately, potentially at different precisions.
/// We average the two to get a single per-element size.
fn kv_quant_bytes(k_type: CacheQuantType, v_type: CacheQuantType) -> f64 {
    let get_bytes = |t: CacheQuantType| match t {
        CacheQuantType::F32 => 4.0,
        CacheQuantType::F16 | CacheQuantType::BF16 => 2.0,
        CacheQuantType::Q8_0 => 1.0,
        CacheQuantType::Q5_0 | CacheQuantType::Q5_1 => 0.625, // 5 bits
        CacheQuantType::Q4_0 | CacheQuantType::Q4_1 | CacheQuantType::Iq4Nl => 0.5, // 4 bits
    };
    (get_bytes(k_type) + get_bytes(v_type)) / 2.0
}

pub fn kv_quant_bytes_from_str(k: &str, v: &str) -> f64 {
    let k_type = CacheTypeK::from(k);
    let v_type = CacheTypeV::from(v);
    kv_quant_bytes(k_type, v_type)
}

pub fn format_mib(mib: u64) -> String {
    crate::tui::format_size(mib * 1024 * 1024)
}

// Benchmark Tuning types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BenchTuneConfig {
    pub model_path: PathBuf,
    pub num_iterations: u32,
    pub prompt: String,
    pub params_to_test: Vec<BenchTuneParam>,
    pub test_duration: Duration,
    pub bench_mode: BenchTuneMode,
    pub n_predict: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchTuneParam {
    pub name: String,
    pub min: f64,
    pub max: f64,
    pub step: f64,
    pub enabled: bool,
}

impl PartialEq for BenchTuneParam {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name &&
        self.min.to_bits() == other.min.to_bits() &&
        self.max.to_bits() == other.max.to_bits() &&
        self.step.to_bits() == other.step.to_bits() &&
        self.enabled == other.enabled
    }
}
impl Eq for BenchTuneParam {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchTuneParamValue {
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub top_k: Option<i64>,
    pub repeat_penalty: Option<f64>,
    pub context_length: Option<u32>,
    pub batch_size: Option<u32>,
    pub flash_attn: Option<bool>,
    pub threads: Option<u32>,
    pub expert_count: Option<i32>,
}

impl PartialEq for BenchTuneParamValue {
    fn eq(&self, other: &Self) -> bool {
        self.temperature.map(|v| v.to_bits()) == other.temperature.map(|v| v.to_bits()) &&
        self.top_p.map(|v| v.to_bits()) == other.top_p.map(|v| v.to_bits()) &&
        self.top_k == other.top_k &&
        self.repeat_penalty.map(|v| v.to_bits()) == other.repeat_penalty.map(|v| v.to_bits()) &&
        self.context_length == other.context_length &&
        self.batch_size == other.batch_size &&
        self.flash_attn == other.flash_attn &&
        self.threads == other.threads &&
        self.expert_count == other.expert_count
    }
}
impl Eq for BenchTuneParamValue {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchTuneResult {
    pub params: BenchTuneParamValue,
    pub metrics: BenchTuneMetrics,
    pub outputs: Vec<String>,
    pub per_iteration_metrics: Vec<BenchTuneMetrics>,
    pub base_settings: Option<ModelSettings>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchTuneMetrics {
    pub prompt_tps: f64,
    pub generation_tps: f64,
    pub combined_tps: f64,
    pub latency_per_token: f64,
    pub first_token_time: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BenchTuneStatus {
    Running {
        current: usize,
        total: usize,
        progress: f32,
        current_params: BenchTuneParamValue,
    },
    Completed {
        total_tests: usize,
        successful_tests: usize,
        elapsed: Duration,
    },
    Error {
        error: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BenchTuneMode {
    /// Runtime-only mode: sends all params in /completion request body, no server restarts
    RuntimeOnly,
    /// Full mode: spawns a new server for each parameter combination (tests server-level params)
    Full,
}

impl Default for BenchTuneMode {
    fn default() -> Self {
        Self::Full
    }
}

/// Progress status for benchmark tuning
#[derive(Debug, Clone)]
pub enum BenchTuneProgress {
    /// Tuning is running.
    Running {
        current: usize,
        total: usize,
        progress: f32,
        current_params: BenchTuneParamValue,
    },
    /// Tuning is complete.
    Completed {
        total_tests: usize,
        successful_tests: usize,
        elapsed: Duration,
    },
    /// Tuning failed.
    Error {
        error: String,
    },
}

impl BenchTuneProgress {
    pub fn from_status(status: &BenchTuneStatus) -> Option<Self> {
        match status {
            BenchTuneStatus::Running { current, total, progress, current_params } => Some(BenchTuneProgress::Running {
                current: *current,
                total: *total,
                progress: *progress,
                current_params: current_params.clone(),
            }),
            BenchTuneStatus::Completed { total_tests, successful_tests, elapsed } => Some(BenchTuneProgress::Completed {
                total_tests: *total_tests,
                successful_tests: *successful_tests,
                elapsed: *elapsed,
            }),
            BenchTuneStatus::Error { error } => Some(BenchTuneProgress::Error {
                error: error.clone(),
            }),
        }
    }
}

impl BenchTuneConfig {
    pub fn new(model_path: PathBuf, num_iterations: u32, prompt: String) -> Self {
        Self {
            model_path,
            num_iterations,
            prompt,
            params_to_test: vec![
                BenchTuneParam {
                    name: "temperature".to_string(),
                    min: 0.4,
                    max: 1.0,
                    step: 0.1,
                    enabled: false,
                },
                BenchTuneParam {
                    name: "top_p".to_string(),
                    min: 0.8,
                    max: 1.0,
                    step: 0.1,
                    enabled: false,
                },
                BenchTuneParam {
                    name: "top_k".to_string(),
                    min: 40.0,
                    max: 50.0,
                    step: 10.0,
                    enabled: false,
                },
                BenchTuneParam {
                    name: "repeat_penalty".to_string(),
                    min: 1.0,
                    max: 1.2,
                    step: 0.1,
                    enabled: false,
                },
                BenchTuneParam {
                    name: "flash_attn".to_string(),
                    min: 0.0,
                    max: 1.0,
                    step: 1.0,
                    enabled: false,
                },
                BenchTuneParam {
                    name: "threads".to_string(),
                    min: 4.0,
                    max: 16.0,
                    step: 4.0,
                    enabled: false,
                },
                BenchTuneParam {
                    name: "batch_size".to_string(),
                    min: 512.0,
                    max: 2048.0,
                    step: 512.0,
                    enabled: false,
                },
                BenchTuneParam {
                    name: "expert_count".to_string(),
                    min: 1.0,
                    max: 4.0,
                    step: 1.0,
                    enabled: false,
                },
                ],
                test_duration: Duration::from_secs(30),
                bench_mode: BenchTuneMode::default(),
                n_predict: 2048,
                }
                }


    /// Generate all parameter combinations based on the config
    pub fn generate_combinations(&self) -> Vec<BenchTuneParamValue> {
        let mut temp_values = vec![None];
        let mut top_p_values = vec![None];
        let mut top_k_values = vec![None];
        let mut repeat_penalty_values = vec![None];
        let mut flash_attn_values = vec![None];
        let mut threads_values = vec![None];
        let mut batch_size_values = vec![None];
        let mut expert_count_values = vec![None];

        for p in &self.params_to_test {
            if !p.enabled { continue; }
            
            let vals: Vec<f64> = {
                let step_count = ((p.max - p.min) / p.step).ceil() as usize;
                (0..=step_count).map(|i| (p.min + (i as f64 * p.step)).min(p.max)).collect()
            };

            match p.name.as_str() {
                "temperature" => temp_values = vals.into_iter().map(Some).collect(),
                "top_p" => top_p_values = vals.into_iter().map(Some).collect(),
                "top_k" => top_k_values = vals.into_iter().map(|v| Some(v as i64)).collect(),
                "repeat_penalty" => repeat_penalty_values = vals.into_iter().map(Some).collect(),
                "flash_attn" => flash_attn_values = vals.into_iter().map(|v| Some(v >= 0.5)).collect(),
                "threads" => threads_values = vals.into_iter().map(|v| Some(v as u32)).collect(),
                "batch_size" => batch_size_values = vals.into_iter().map(|v| Some(v as u32)).collect(),
                "expert_count" => expert_count_values = vals.into_iter().map(|v| Some(v as i32)).collect(),
                _ => {}
            }
        }

        let mut combinations = Vec::new();
        for &temp in &temp_values {
            for &top_p in &top_p_values {
                for &top_k in &top_k_values {
                    for &rp in &repeat_penalty_values {
                        for &fa in &flash_attn_values {
                            for &th in &threads_values {
                                for &bs in &batch_size_values {
                                    for &ec in &expert_count_values {
                                        combinations.push(BenchTuneParamValue {
                                            temperature: temp,
                                            top_p,
                                            top_k,
                                            repeat_penalty: rp,
                                            context_length: None,
                                            batch_size: bs,
                                            flash_attn: fa,
                                            threads: th,
                                            expert_count: ec,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        combinations
    }

    /// Get total number of tests to run
    pub fn get_total_tests_count(&self) -> usize {
        self.generate_combinations().len()
    }
}

