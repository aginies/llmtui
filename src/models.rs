use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// The state of a model in the manager.
#[derive(Debug, Clone, PartialEq)]
pub enum ModelState {
    Available,
    Loading,
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
}

impl SearchSort {
    pub fn next(self) -> Self {
        match self {
            SearchSort::Relevance => SearchSort::Downloads,
            SearchSort::Downloads => SearchSort::Likes,
            SearchSort::Likes => SearchSort::Relevance,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SearchSort::Relevance => "Relevance",
            SearchSort::Downloads => "Downloads",
            SearchSort::Likes => "Likes",
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
    pub readme: Option<String>,
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
            start_time: std::time::Instant::now(),
            bytes_per_second: 0.0,
        }
    }
}

impl ModelSettings {}

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
            gpu_layers: dp.gpu_layers,
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
            router_max_models: dp.router_max_models,
            server_mode: dp.server_mode,
            max_tokens: dp.max_tokens,
            cache_type: dp.cache_type,
            backend: dp.backend,
            llama_cpp_version_cpu: dp.llama_cpp_version_cpu,
            llama_cpp_version_vulkan: dp.llama_cpp_version_vulkan,
            llama_cpp_version_rocm: dp.llama_cpp_version_rocm,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadStatus {
    Downloading,
    Complete,
    Error(String),
}

impl DownloadState {
    pub fn progress(&self) -> f64 {
        if self.total_bytes == 0 {
            0.0
        } else {
            self.downloaded_bytes as f64 / self.total_bytes as f64
        }
    }

    pub fn formatted_progress(&self) -> String {
        let pct = self.progress() * 100.0;
        format!("{pct:.1}%")
    }

    pub fn formatted_speed(&self) -> String {
        if self.bytes_per_second < 1024.0 {
            format!("{:.1} B/s", self.bytes_per_second)
        } else if self.bytes_per_second < 1024.0 * 1024.0 {
            format!("{:.1} KB/s", self.bytes_per_second / 1024.0)
        } else {
            format!("{:.1} MB/s", self.bytes_per_second / (1024.0 * 1024.0))
        }
    }
}

// ── Cache type enums ──────────────────────────────────────────

/// Main KV cache data type.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum CacheType {
    #[serde(rename = "f16")]
    F16,
    #[serde(rename = "bf16")]
    BF16,
    #[serde(rename = "fq8_0")]
    Fq8_0,
    #[serde(rename = "fq4_1")]
    Fq4_1,
}

impl Default for CacheType {
    fn default() -> Self {
        Self::F16
    }
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

/// KV cache data type for K.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum CacheTypeK {
    #[serde(rename = "f32")]
    F32,
    #[serde(rename = "f16")]
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

impl CacheTypeK {
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

impl Default for CacheTypeK {
    fn default() -> Self {
        Self::F16
    }
}

impl std::fmt::Display for CacheTypeK {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CacheTypeK::F32 => write!(f, "f32"),
            CacheTypeK::F16 => write!(f, "f16"),
            CacheTypeK::BF16 => write!(f, "bf16"),
            CacheTypeK::Q8_0 => write!(f, "q8_0"),
            CacheTypeK::Q4_0 => write!(f, "q4_0"),
            CacheTypeK::Q4_1 => write!(f, "q4_1"),
            CacheTypeK::Iq4Nl => write!(f, "iq4_nl"),
            CacheTypeK::Q5_0 => write!(f, "q5_0"),
            CacheTypeK::Q5_1 => write!(f, "q5_1"),
        }
    }
}

/// KV cache data type for V.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum CacheTypeV {
    #[serde(rename = "f32")]
    F32,
    #[serde(rename = "f16")]
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

impl CacheTypeV {
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

impl Default for CacheTypeV {
    fn default() -> Self {
        Self::F16
    }
}

impl std::fmt::Display for CacheTypeV {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CacheTypeV::F32 => write!(f, "f32"),
            CacheTypeV::F16 => write!(f, "f16"),
            CacheTypeV::BF16 => write!(f, "bf16"),
            CacheTypeV::Q8_0 => write!(f, "q8_0"),
            CacheTypeV::Q4_0 => write!(f, "q4_0"),
            CacheTypeV::Q4_1 => write!(f, "q4_1"),
            CacheTypeV::Iq4Nl => write!(f, "iq4_nl"),
            CacheTypeV::Q5_0 => write!(f, "q5_0"),
            CacheTypeV::Q5_1 => write!(f, "q5_1"),
        }
    }
}

/// Split mode for multi-GPU.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum SplitMode {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "layer")]
    Layer,
    #[serde(rename = "row")]
    Row,
    #[serde(rename = "tensor")]
    Tensor,
}

impl Default for SplitMode {
    fn default() -> Self {
        Self::Layer
    }
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
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum NumMode {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "distribute")]
    Distribute,
    #[serde(rename = "isolate")]
    Isolate,
    #[serde(rename = "numactl")]
    Numactl,
}

impl Default for NumMode {
    fn default() -> Self {
        Self::None
    }
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
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum RopeScaling {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "linear")]
    Linear,
    #[serde(rename = "yarn")]
    Yarn,
}

impl Default for RopeScaling {
    fn default() -> Self {
        Self::None
    }
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
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Mirostat {
    #[serde(rename = "0")]
    Off,
    #[serde(rename = "1")]
    Mirostat,
    #[serde(rename = "2")]
    Mirostat2,
}

impl Default for Mirostat {
    fn default() -> Self {
        Self::Off
    }
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
pub enum Backend {
    #[serde(rename = "cpu")]
    Cpu,
    #[serde(rename = "vulkan")]
    Vulkan,
    #[serde(rename = "rocm")]
    Rocrm,
}

impl Default for Backend {
    fn default() -> Self {
        Self::Cpu
    }
}

impl std::fmt::Display for Backend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Backend::Cpu => write!(f, "cpu"),
            Backend::Vulkan => write!(f, "vulkan"),
            Backend::Rocrm => write!(f, "rocm"),
        }
    }
}

/// Server mode: normal (single model) or router (multiple models).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServerMode {
    #[serde(rename = "normal")]
    Normal,
    #[serde(rename = "router")]
    Router,
}

impl Default for ServerMode {
    fn default() -> Self {
        Self::Normal
    }
}

impl std::fmt::Display for ServerMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServerMode::Normal => write!(f, "Normal"),
            ServerMode::Router => write!(f, "Router"),
        }
    }
}

/// Mode for parsing reasoning tags from model responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReasoningMode {
    #[serde(rename = "default")]
    Default, // DeepSeek/OpenAI style: <think> ... </think>
    #[serde(rename = "gemma")]
    Gemma,   // Gemma style: <|channel>thought <channel|>
}

impl Default for ReasoningMode {
    fn default() -> Self {
        Self::Default
    }
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
    /// Max concurrent predictions (requests in flight).
    pub max_concurrent_predictions: u32,
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

    /// Max number of layers to store in VRAM.
    pub gpu_layers: i32,
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
    /// Maximum number of models to load in router mode.
    pub router_max_models: u32,
    /// Server mode: normal (single model) or router (multiple models).
    pub server_mode: ServerMode,

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
            max_concurrent_predictions: 1,
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
            gpu_layers: -1,
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
            router_max_models: 4,
            server_mode: ServerMode::Normal,

            // Other
            max_tokens: Some(2048),
            cache_type: CacheType::F16,
            backend: Backend::Vulkan,
            llama_cpp_version_cpu: None,
            llama_cpp_version_vulkan: None,
            llama_cpp_version_rocm: None,
        }
    }
}

impl ModelSettings {
    /// Create ModelSettings from config defaults, applying model-specific overrides.
    pub fn from_config(config: &crate::config::Config) -> Self {
        let mut settings = Self::default();
        settings.threads = config.default.threads;
        settings.threads_batch = config.default.threads_batch;
        settings.batch_size = config.default.batch_size;
        settings.ubatch_size = config.default.ubatch_size;
        settings.cache_type_k = config.default.cache_type_k.clone();
        settings.cache_type_v = config.default.cache_type_v.clone();
        settings.keep = config.default.keep;
        settings.swa_full = config.default.swa_full;
        settings.mlock = config.default.mlock;
        settings.mmap = config.default.mmap;
        settings.numa = config.default.numa.clone();
        settings.uniform_cache = config.default.uniform_cache;
        settings.kv_cache_offload = config.default.kv_cache_offload;
        settings.system_prompt = config.default.system_prompt.clone();
        settings.system_prompt_preset_name = config.default.system_prompt_preset_name.clone();
        settings.reasoning_mode = config.default.reasoning_mode;
        settings.gpu_layers = config.default.gpu_layers;
        settings.split_mode = config.default.split_mode.clone();
        settings.tensor_split = config.default.tensor_split.clone();
        settings.main_gpu = config.default.main_gpu;
        settings.fit = config.default.fit;
        settings.lora = config.default.lora.clone();
        settings.lora_scaled = config.default.lora_scaled.clone();
        settings.rpc = config.default.rpc.clone();
        settings.embedding = config.default.embedding;
        settings.flash_attn = config.default.flash_attn;
        settings.jinja = config.default.jinja;
        settings.chat_template = config.default.chat_template.clone();
        settings.expert_count = config.default.expert_count;
        settings.seed = config.default.seed;
        settings.temperature = config.default.temperature;
        settings.top_k = config.default.top_k;
        settings.top_p = config.default.top_p;
        settings.min_p = config.default.min_p;
        settings.typical_p = config.default.typical_p;
        settings.mirostat = config.default.mirostat.clone();
        settings.mirostat_lr = config.default.mirostat_lr;
        settings.mirostat_ent = config.default.mirostat_ent;
        settings.ignore_eos = config.default.ignore_eos;
        settings.samplers = config.default.samplers.clone();
        settings.repeat_penalty = config.default.repeat_penalty;
        settings.repeat_last_n = config.default.repeat_last_n;
        settings.presence_penalty = config.default.presence_penalty;
        settings.frequency_penalty = config.default.frequency_penalty;
        settings.dry_multiplier = config.default.dry_multiplier;
        settings.dry_base = config.default.dry_base;
        settings.dry_allowed_length = config.default.dry_allowed_length;
        settings.dry_penalty_last_n = config.default.dry_penalty_last_n;
        settings.rope_scaling = config.default.rope_scaling.clone();
        settings.rope_scale = config.default.rope_scale;
        settings.rope_freq_base = config.default.rope_freq_base;
        settings.rope_freq_scale = config.default.rope_freq_scale;
        settings.host = config.default.host.clone();
        settings.port = config.default.port;
        settings.timeout = config.default.timeout;
        settings.cache_prompt = config.default.cache_prompt;
        settings.cache_reuse = config.default.cache_reuse;
        settings.webui = config.default.webui;
        settings.router_max_models = config.default.router_max_models;
        settings.server_mode = config.default.server_mode.clone();
        settings.max_tokens = config.default.max_tokens;
        settings.cache_type = config.default.cache_type.clone();
        settings.backend = config.default.backend.clone();
        settings.llama_cpp_version_cpu = config.default.llama_cpp_version_cpu.clone();
        settings.llama_cpp_version_vulkan = config.default.llama_cpp_version_vulkan.clone();
        settings.llama_cpp_version_rocm = config.default.llama_cpp_version_rocm.clone();
        settings
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
}

/// Metrics reported by the llama.cpp server.
#[derive(Debug, Clone)]
pub struct ServerMetrics {
    pub loaded: bool,
    pub tps: f64,
    pub prompt_tps: f64,
    pub cpu_usage: f64,
    pub gpu_mem_used: u64,
    pub gpu_mem_total: u64,
    pub ram_used: u64,
    pub ctx_used: u32,
    pub ctx_max: u32,
    /// Sum of gpu_mem_used across all loaded models (for Total VRAM display).
    pub total_vram_used: u64,
}

/// A llama.cpp release from GitHub.
#[derive(Debug, Clone)]
pub struct LlamaCppRelease {
    pub tag: String,
    pub name: String,
    pub is_prerelease: bool,
    pub size: Option<u64>,
}

impl Default for ServerMetrics {
    fn default() -> Self {
        Self {
            loaded: false,
            tps: 0.0,
            prompt_tps: 0.0,
            cpu_usage: 0.0,
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
    // gpu_layers < 0 means "all layers".
    // gpu_layers == 0 means "no layers on GPU" (CPU only).
    let gpu_layers = if settings.gpu_layers < 0 {
        if total_layers > 0 { total_layers } else { 32 } // fallback if total_layers unknown
    } else {
        let requested = settings.gpu_layers.unsigned_abs() as u32;
        if total_layers > 0 {
            requested.min(total_layers)
        } else {
            requested
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

    if gpu_layers == 0 {
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
    let uniform_cache_factor = if settings.uniform_cache { 1.0 } else { 1.0 };

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
fn kv_quant_bytes(k_type: CacheTypeK, v_type: CacheTypeV) -> f64 {
    let k_bytes = match k_type {
        CacheTypeK::F32 => 4.0,
        CacheTypeK::F16 | CacheTypeK::BF16 => 2.0,
        CacheTypeK::Q8_0 => 1.0,
        CacheTypeK::Q5_0 | CacheTypeK::Q5_1 => 0.625, // 5 bits
        CacheTypeK::Q4_0 | CacheTypeK::Q4_1 => 0.5,   // 4 bits
        CacheTypeK::Iq4Nl => 0.5,                      // 4 bits
    };
    let v_bytes = match v_type {
        CacheTypeV::F32 => 4.0,
        CacheTypeV::F16 | CacheTypeV::BF16 => 2.0,
        CacheTypeV::Q8_0 => 1.0,
        CacheTypeV::Q5_0 | CacheTypeV::Q5_1 => 0.625,
        CacheTypeV::Q4_0 | CacheTypeV::Q4_1 => 0.5,
        CacheTypeV::Iq4Nl => 0.5,
    };
    (k_bytes + v_bytes) / 2.0
}

pub fn format_mib(mib: u64) -> String {
    if mib >= 1024 {
        format!("{:.1} GB", mib as f64 / 1024.0)
    } else {
        format!("{} MB", mib)
    }
}
