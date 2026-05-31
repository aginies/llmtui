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
    /// Whether a matching GGUF file is already downloaded locally.
    #[serde(default)]
    pub downloaded: bool,
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
    /// Filesystem path where the download is being saved.
    pub dest: Option<std::path::PathBuf>,
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
            dest: None,
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
            _ => None,
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
            _ => {}
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
            chat_template_kwargs: dp.chat_template_kwargs,
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
            rope_yarn_enabled: dp.rope_yarn_enabled,
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
            ws_server_enabled: dp.ws_server_enabled,
            ws_server_port: dp.ws_server_port,
            ws_server_auth_key: dp.ws_server_auth_key,
            ws_server_tls_enabled: dp.ws_server_tls_enabled,
            ws_server_tls_cert: dp.ws_server_tls_cert,
            ws_server_tls_key: dp.ws_server_tls_key,
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
    Cancelled,
}

// ── Cache type enums ──────────────────────────────────────────

/// Main KV cache data type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    #[serde(rename = "cpu_arm64")]
    CpuArm64,
    #[serde(rename = "win_cpu")]
    CpuWindows,
    #[serde(rename = "win_vulkan")]
    VulkanWindows,
    #[serde(rename = "win_cuda_12_4")]
    CudaWindows12_4,
    #[serde(rename = "win_cuda_13_1")]
    CudaWindows13_1,
    #[serde(rename = "win_hip")]
    HipWindows,
    #[serde(rename = "macos_arm64")]
    CpuMacosArm64,
    #[serde(rename = "macos_x64")]
    CpuMacosX64,
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
            Backend::CpuArm64 => "cpu-arm64",
            Backend::CpuWindows => "win-cpu",
            Backend::VulkanWindows => "win-vulkan",
            Backend::CudaWindows12_4 => "win-cuda-12.4",
            Backend::CudaWindows13_1 => "win-cuda-13.1",
            Backend::HipWindows => "win-hip",
            Backend::CpuMacosArm64 => "macos-arm64",
            Backend::CpuMacosX64 => "macos-x64",
        }
    }

    /// Returns true if this backend is for Linux.
    pub fn is_linux(self) -> bool {
        matches!(self, Backend::Cpu | Backend::Vulkan | Backend::Rocm | Backend::RocmLemonade | Backend::Cuda | Backend::CpuArm64)
    }

    /// Returns true if this backend is for Windows.
    pub fn is_windows(self) -> bool {
        matches!(self, Backend::CpuWindows | Backend::VulkanWindows | Backend::CudaWindows12_4 | Backend::CudaWindows13_1 | Backend::HipWindows)
    }

    /// Returns true if this backend is for macOS.
    pub fn is_macos(self) -> bool {
        matches!(self, Backend::CpuMacosArm64 | Backend::CpuMacosX64)
    }

    /// Parse backend from string representation.
    pub fn from_str(s: &str) -> Self {
        let s = s.to_lowercase();
        if s.starts_with("vulkan") || s.starts_with("vk") {
            Backend::Vulkan
        } else if s.starts_with("rocm") || s.starts_with("ro") {
            if s.contains("lemonade") {
                Backend::RocmLemonade
            } else {
                Backend::Rocm
            }
        } else if s.starts_with("cuda") || s.starts_with("cu") {
            Backend::Cuda
        } else if s.starts_with("cpu") || s.starts_with("cp") {
            Backend::Cpu
        } else {
            Backend::Cpu // Default
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
    /// JSON string for --chat-template-kwargs (e.g. {"enable_thinking": false}).
    pub chat_template_kwargs: Option<String>,

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
    /// Enable Yarn RoPE scaling mode.
    pub rope_yarn_enabled: bool,

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
    /// Whether to enable the WebSocket dashboard server.
   pub ws_server_enabled: bool,
    pub ws_server_port: u16,
    pub ws_server_auth_key: Option<String>,
    pub ws_server_tls_enabled: bool,
    pub ws_server_tls_cert: Option<String>,
    pub ws_server_tls_key: Option<String>,
}

impl Default for ModelSettings {
    fn default() -> Self {
        let mut s: Self = crate::config::DefaultParams::default().into();
        // Override fields that differ from DefaultParams defaults
        s.uniform_cache = false;
        s.cache_type_k = Some(CacheTypeK::F16);
        s.cache_type_v = Some(CacheTypeV::F16);
        s.cache_type = CacheType::default();
        s.backend = Backend::Cpu;
        s.presence_penalty = Some(0.0);
        s.frequency_penalty = Some(0.0);
        s
    }
}

impl ModelSettings {
    /// Create ModelSettings from config defaults, applying model-specific overrides.
    pub fn from_config(config: &crate::config::Config) -> Self {
        let mut settings: ModelSettings = config.default.clone().into();
        settings.ws_server_enabled = config.ws_server.enabled;
        settings.ws_server_port = config.ws_server.port;
        settings.ws_server_auth_key = config.ws_server.auth_key.clone();
        settings.ws_server_tls_enabled = config.ws_server.tls_enabled;
        settings.ws_server_tls_cert = config.ws_server.tls_cert.clone();
        settings.ws_server_tls_key = config.ws_server.tls_key.clone();
        settings
    }
}

/// Default benchmark prompt used when starting a tuning session.
pub const BENCHMARK_PROMPT: &str =
    "Create Mona Lisa image in ascii art using text, number, symbol, everything possible. this should be the perfect painting.";

/// A discovered model file.
#[derive(Debug, Clone)]
pub struct DiscoveredModel {
    pub path: PathBuf,
    pub name: String,
    pub file_size: u64,
    pub display_name: String, // path relative to model_dir for display
}

/// Parsed GGUF metadata for a model, cached to avoid re-parsing the file.
#[derive(Debug, Clone, Default)]
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

impl GgufMetadata {
    pub fn from_path(path: &std::path::Path) -> anyhow::Result<Self> {
        let path_str = path.to_string_lossy();
        let mut container = gguf_rs::get_gguf_container(&path_str)
            .map_err(|e| anyhow::anyhow!("Failed to get GGUF container: {}", e))?;
        let model_data = container.decode()
            .map_err(|e| anyhow::anyhow!("Failed to decode GGUF: {}", e))?;
        
        let mut meta = Self::default();

        let extract_str = |key: &str| -> String {
            model_data.metadata().get(key).and_then(|v| v.as_str().map(|s| s.to_string())).unwrap_or_default()
        };

        let extract_num = |key: &str| -> Option<u64> {
            model_data.metadata().get(key).and_then(|v| {
                v.as_u64()
                    .or_else(|| v.as_i64().map(|x| x as u64))
                    .or_else(|| v.as_f64().map(|x| x as u64))
            })
        };

        meta.arch = extract_str("general.architecture");
        let prefix = if meta.arch.is_empty() { "llama" } else { &meta.arch };

        let get_num_with_fallback = |suffix: &str| -> u32 {
            extract_num(&format!("{}.{}", prefix, suffix))
                .or_else(|| {
                    if prefix != "llama" {
                        extract_num(&format!("llama.{}", suffix))
                    } else {
                        None
                    }
                })
                .unwrap_or(0) as u32
        };

        meta.layers = get_num_with_fallback("block_count");
        meta.hidden_size = get_num_with_fallback("embedding_length");
        meta.n_ctx_train = get_num_with_fallback("context_length");
        meta.n_head = get_num_with_fallback("attention.head_count");
        meta.n_kv_head = get_num_with_fallback("attention.head_count_kv");
        
        if let Some(value) = model_data.metadata().get("tokenizer.ggml.tokens")
            && let Some(arr) = value.as_array() {
                meta.vocab_size = arr.len() as u32;
            }

        if meta.arch == "mtp" {
            meta.draft_tokens = extract_num("mtp.draft_tokens").unwrap_or(0) as u32;
        }

        if let Some(v) = extract_num("general.file_type") {
            meta.file_type = match v {
                0 => "F32".to_string(),
                1 => "F16".to_string(),
                2 => "Q4_0".to_string(),
                3 => "Q4_1".to_string(),
                7 => "Q8_0".to_string(),
                8 => "Q5_0".to_string(),
                9 => "Q5_1".to_string(),
                10 => "Q2_K".to_string(),
                11 => "Q3_K_S".to_string(),
                12 => "Q3_K_M".to_string(),
                13 => "Q3_K_L".to_string(),
                14 => "Q4_K_S".to_string(),
                15 => "Q4_K_M".to_string(),
                16 => "Q5_K_S".to_string(),
                17 => "Q5_K_M".to_string(),
                18 => "Q6_K".to_string(),
                19 => "IQ2_XXS".to_string(),
                20 => "IQ2_XS".to_string(),
                21 => "IQ3_XXS".to_string(),
                22 => "IQ1_S".to_string(),
                23 => "IQ4_NL".to_string(),
                24 => "IQ3_S".to_string(),
                25 => "IQ2_S".to_string(),
                26 => "IQ4_XS".to_string(),
                _ => format!("Unknown ({})", v),
            };
        }

        if let Some(value) = model_data.metadata().get("general.capabilities")
            && let Some(arr) = value.as_array() {
                for v in arr {
                    if let Some(s) = v.as_str() {
                        meta.capabilities.push(s.to_string());
                    }
                }
            }
        
        if model_data.metadata().contains_key("tokenizer.chat_template") {
            meta.capabilities.push("chat".to_string());
        }

        meta.tokenizer = extract_str("tokenizer.ggml.model");
        meta.domain = extract_str("general.domain");
        meta.model_parameters = model_data.model_parameters();

        Ok(meta)
    }
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
    /// Number of decoded tokens from print_timing logs.
    pub decoded_tokens: u64,
    /// Estimated latency per generated token in milliseconds.
    pub latency_per_token_ms: f64,
    /// Estimated prompt processing latency in milliseconds (1000 / prompt_tps).
    pub prompt_latency_ms: f64,
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
    /// Total number of tensors in the model (from "Loading tensor X of Y" log).
    pub tensors_total: Option<u32>,
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
            decoded_tokens: 0,
            latency_per_token_ms: 0.0,
            prompt_latency_ms: 0.0,
        }
    }
}

/// WebSocket-friendly metrics snapshot (serializable, no internal state).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WsMetrics {
    pub model_name: String,
    pub loaded: bool,
    pub state: String,
    pub tps: f64,
    pub prompt_tps: f64,
    pub ctx_used: u32,
    pub ctx_max: u32,
    pub cpu_usage: f64,
    pub gpu_mem_used: u64,
    pub gpu_mem_total: u64,
    pub ram_used: u64,
    pub latency_per_token_ms: f64,
    pub decoded_tokens: u64,
    pub timestamp: u64,
    // Server command
    pub cmd_display: Option<String>,
    // LLM settings
    pub threads: u32,
    pub threads_batch: u32,
    pub context_length: u32,
    pub ubatch_size: u32,
    pub batch_size: u32,
    pub temperature: f32,
    pub top_k: u32,
    pub top_p: f32,
    pub min_p: f32,
    pub typical_p: f32,
    pub seed: i32,
    pub repeat_penalty: f32,
    pub repeat_last_n: i32,
    pub presence_penalty: Option<f32>,
    pub frequency_penalty: Option<f32>,
    pub mirostat: Option<u32>,
    pub mirostat_lr: Option<f32>,
    pub mirostat_ent: Option<f32>,
    pub max_tokens: Option<u32>,
    pub flash_attn: bool,
    pub kv_cache_offload: bool,
    pub cache_type_k: Option<String>,
    pub cache_type_v: Option<String>,
    pub uniform_cache: bool,
    pub mlock: bool,
    pub mmap: bool,
    pub embedding: bool,
    pub jinja: bool,
    pub ignore_eos: bool,
    pub samplers: String,
    pub expert_count: u32,
    pub gpu_layers: String,
    pub backend: String,
    pub llama_cpp_version: String,
    pub is_mtp: bool,
    pub draft_tokens: u32,
}

impl WsMetrics {
    pub fn from_metrics(metrics: &ServerMetrics, model_name: &str, state: &str, settings: &crate::models::ModelSettings, cmd_display: Option<&str>) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let gpu_layers = match settings.gpu_layers_mode {
            crate::models::GpuLayersMode::Auto => "Auto".to_string(),
            crate::models::GpuLayersMode::Specific(n) => n.to_string(),
            crate::models::GpuLayersMode::All => "All".to_string(),
        };
        Self {
            model_name: model_name.to_string(),
            loaded: metrics.loaded,
            state: state.to_string(),
            tps: metrics.tps,
            prompt_tps: metrics.prompt_tps,
            ctx_used: metrics.ctx_used,
            ctx_max: metrics.ctx_max,
            cpu_usage: metrics.cpu_usage,
            gpu_mem_used: metrics.gpu_mem_used,
            gpu_mem_total: metrics.gpu_mem_total,
            ram_used: metrics.ram_used,
            latency_per_token_ms: metrics.latency_per_token_ms,
            decoded_tokens: metrics.decoded_tokens,
            timestamp,
            cmd_display: cmd_display.map(String::from),
            threads: settings.threads,
            threads_batch: settings.threads_batch,
            context_length: settings.context_length,
            ubatch_size: settings.ubatch_size,
            batch_size: settings.batch_size,
            temperature: settings.temperature,
            top_k: settings.top_k as u32,
            top_p: settings.top_p,
            min_p: settings.min_p,
            typical_p: settings.typical_p,
            seed: settings.seed,
            repeat_penalty: settings.repeat_penalty,
            repeat_last_n: settings.repeat_last_n as i32,
            presence_penalty: settings.presence_penalty,
            frequency_penalty: settings.frequency_penalty,
            mirostat: Some(match settings.mirostat {
                crate::models::Mirostat::Off => 0,
                crate::models::Mirostat::Mirostat => 1,
                crate::models::Mirostat::Mirostat2 => 2,
            }),
            mirostat_lr: Some(settings.mirostat_lr),
            mirostat_ent: Some(settings.mirostat_ent),
            max_tokens: settings.max_tokens,
            flash_attn: settings.flash_attn,
            kv_cache_offload: settings.kv_cache_offload,
            cache_type_k: settings.cache_type_k.map(|k| k.to_string()),
            cache_type_v: settings.cache_type_v.map(|k| k.to_string()),
            uniform_cache: settings.uniform_cache,
            mlock: settings.mlock,
            mmap: settings.mmap,
            embedding: settings.embedding,
            jinja: settings.jinja,
            ignore_eos: settings.ignore_eos,
            samplers: settings.samplers.to_string(),
            expert_count: settings.expert_count as u32,
            gpu_layers,
            backend: settings.backend.to_string(),
            llama_cpp_version: settings.get_active_backend_version_display().to_string(),
            is_mtp: settings.is_mtp,
            draft_tokens: settings.draft_tokens,
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
    // The KV cache is allocated for the total number of model layers,
    // not just the number layers loaded into the GPU (gpu_layers).
    // However only gpu_layers * sizeof(type) contributes to the VRAM cost.
    let kv_mib = (2.0
        * hidden_size
        * settings.context_length as f64
        * total_layers as f64
        * gqa_ratio
        * gpu_layers as f64
        / total_layers as f64  // VRAM cost: only GPU-loaded portion of KV cache
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

impl ModelSettings {
    /// Check if this settings differs from `other` in any field.
    pub fn is_dirty(&self, other: &Self) -> bool {
        let f32_dirty = |a: Option<f32>, b: Option<f32>| match (a, b) {
            (Some(v1), Some(v2)) => (v1 - v2).abs() > 0.001,
            (None, None) => false,
            _ => true,
        };

        self.context_length != other.context_length
            || self.threads != other.threads
            || self.threads_batch != other.threads_batch
            || self.mlock != other.mlock
            || self.system_prompt_preset_name != other.system_prompt_preset_name
            || self.gpu_layers_mode != other.gpu_layers_mode
            || self.flash_attn != other.flash_attn
            || self.kv_cache_offload != other.kv_cache_offload
            || self.cache_type_k != other.cache_type_k
            || self.cache_type_v != other.cache_type_v
            || self.batch_size != other.batch_size
            || self.ubatch_size != other.ubatch_size
            || self.uniform_cache != other.uniform_cache
            || self.max_concurrent_predictions != other.max_concurrent_predictions
            || self.seed != other.seed
            || (self.temperature - other.temperature).abs() > 0.001
            || self.top_k != other.top_k
            || (self.top_p - other.top_p).abs() > 0.001
            || (self.min_p - other.min_p).abs() > 0.001
            || self.max_tokens != other.max_tokens
            || (self.repeat_penalty - other.repeat_penalty).abs() > 0.001
            || self.repeat_last_n != other.repeat_last_n
            || f32_dirty(self.presence_penalty, other.presence_penalty)
            || f32_dirty(self.frequency_penalty, other.frequency_penalty)
            || self.keep != other.keep
            || self.mmap != other.mmap
            || self.numa != other.numa
            || self.expert_count != other.expert_count
            || self.tags != other.tags
            || self.get_active_backend_version() != other.get_active_backend_version()
            || self.ws_server_enabled != other.ws_server_enabled
            || self.ws_server_port != other.ws_server_port
            || self.ws_server_auth_key != other.ws_server_auth_key
            || self.ws_server_tls_enabled != other.ws_server_tls_enabled
            || self.ws_server_tls_cert != other.ws_server_tls_cert
            || self.ws_server_tls_key != other.ws_server_tls_key
            || self.rope_yarn_enabled != other.rope_yarn_enabled
            || self.rope_scale != other.rope_scale
            || self.rope_freq_base != other.rope_freq_base
            || self.rope_freq_scale != other.rope_freq_scale
    }
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
    pub chat_template_kwargs: Option<String>,
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
    PartiallyCompleted {
        total_tests: usize,
        successful_tests: usize,
        failed_tests: usize,
        elapsed: Duration,
    },
    Cancelled {
        total_tests: usize,
        successful_tests: usize,
        failed_tests: usize,
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
    /// Tuning completed with some failures.
    PartiallyCompleted {
        total_tests: usize,
        successful_tests: usize,
        failed_tests: usize,
        elapsed: Duration,
    },
    /// Tuning was cancelled by the user.
    Cancelled {
        total_tests: usize,
        successful_tests: usize,
        failed_tests: usize,
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
            BenchTuneStatus::PartiallyCompleted { total_tests, successful_tests, failed_tests, elapsed } => {
                Some(BenchTuneProgress::PartiallyCompleted {
                    total_tests: *total_tests,
                    successful_tests: *successful_tests,
                    failed_tests: *failed_tests,
                    elapsed: *elapsed,
                })
            }
            BenchTuneStatus::Cancelled { total_tests, successful_tests, failed_tests, elapsed } => {
                Some(BenchTuneProgress::Cancelled {
                    total_tests: *total_tests,
                    successful_tests: *successful_tests,
                    failed_tests: *failed_tests,
                    elapsed: *elapsed,
                })
            }
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
                    min: 10.0,
                    max: 40.0,
                    step: 5.0,
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
                n_predict: 512,
chat_template_kwargs: Some(r#"{"enable_thinking": false}"#.to_string()),
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

