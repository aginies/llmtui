mod model_config;
mod presets;
mod profiles;
mod store;

use std::collections::HashSet;
use std::path::PathBuf;

use chrono::Local;
use serde::{Deserialize, Serialize};

pub use model_config::ModelConfigStore;

pub use profiles::ProfileStore;

use crate::models::{
    Backend, CacheType, CacheTypeK, CacheTypeV, Mirostat, NumMode, RopeScaling, Samplers, SplitMode,
};
pub use presets::PresetStore;

/// Count physical CPU cores on Linux (ignores hyperthreading).
/// Falls back to 1 if the file can't be read or parsing fails.
pub fn physical_cores() -> u32 {
    let content = match std::fs::read_to_string("/proc/cpuinfo") {
        Ok(c) => c,
        Err(_) => return 1,
    };
    let mut seen = HashSet::new();
    let mut cur_phys: Option<&str> = None;
    let mut cur_core: Option<&str> = None;
    for line in content.lines() {
        if let Some((key, val)) = line.split_once(':') {
            let key = key.trim();
            let val = val.trim();
            match key {
                "physical id" => cur_phys = Some(val),
                "core id" => cur_core = Some(val),
                _ => {}
            }
            if let (Some(phys), Some(core)) = (cur_phys, cur_core) {
                seen.insert((phys, core));
            }
        }
    }
    seen.len() as u32
}

/// A remote RPC worker for distributed inference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcWorker {
    #[serde(default)]
    pub selected: bool,
    #[serde(default)]
    pub name: String,
    pub ip: String,
    #[serde(default = "default_rpc_port")]
    pub port: u16,
}

fn default_rpc_port() -> u16 {
    50052
}

/// WebSocket dashboard server configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WsServer {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_ws_port")]
    pub port: u16,
    #[serde(default)]
    pub auth_key: Option<String>,
    #[serde(default = "default_ws_host")]
    pub host: String,
    #[serde(default)]
    pub tls_enabled: bool,
    #[serde(default)]
    pub tls_cert: Option<String>,
    #[serde(default)]
    pub tls_key: Option<String>,
}

fn default_ws_host() -> String {
    "0.0.0.0".to_string()
}

fn default_ws_port() -> u16 {
    49223
}

/// Global configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub models_dirs: Vec<PathBuf>,
    pub llama_server: PathBuf,
    pub default: DefaultParams,
    /// Per-model overrides (keyed by model file name, stored as YAML in models/).
    #[serde(default, skip)]
    pub model_overrides: ModelConfigStore,
    /// Named profiles of settings presets (stored as YAML in profiles/).
    #[serde(default, skip)]
    pub profiles: ProfileStore,
    /// System prompt presets (stored as YAML in presets/).
    #[serde(default, skip)]
    pub system_prompt_presets: PresetStore,
    /// RPC Workers for distributed inference.
    #[serde(default)]
    pub rpc_workers: Vec<RpcWorker>,
    /// WebSocket dashboard server for live metrics.
    #[serde(default)]
    pub ws_server: WsServer,
    /// Number of results per HuggingFace search query.
    #[serde(default = "default_search_limit")]
    pub search_limit: u32,
}

fn default_search_limit() -> u32 {
    50
}

/// A named profile of settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Profile {
    pub name: String,
    /// Brief description shown in the profile list.
    pub description: String,
    /// The settings for this profile.
    #[serde(default)]
    pub settings: ModelOverride,
}

impl Profile {
    /// Apply this profile's settings to a base ModelSettings.
    pub fn apply(&self, mut base: crate::models::ModelSettings) -> crate::models::ModelSettings {
        self.settings.apply(&mut base);
        base
    }
}

/// A named system prompt preset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemPromptPreset {
    pub name: String,
    pub description: String,
    pub content: String,
}

/// Built-in system prompt presets.
pub fn builtin_system_prompt_presets() -> Vec<SystemPromptPreset> {
    vec![
        SystemPromptPreset {
            name: "General".into(),
            description: "General-purpose assistant".into(),
            content: "You are a helpful assistant.".into(),
        },
        SystemPromptPreset {
            name: "Coder".into(),
            description: "Expert software developer".into(),
            content: "You are an expert software developer. Write clean, well-documented code. Explain your reasoning and suggest improvements.".into(),
        },
        SystemPromptPreset {
            name: "Thinker".into(),
            description: "Analytical and thoughtful".into(),
            content: "You are a thoughtful and analytical AI assistant. Think carefully before answering. Provide well-reasoned responses with clear explanations.".into(),
        },
        SystemPromptPreset {
            name: "Mathematician".into(),
            description: "Expert in mathematics".into(),
            content: "You are an expert in mathematics. Provide clear, step-by-step solutions to mathematical problems. Show your reasoning and explain key concepts.".into(),
        },
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ModelOverride {
    // Loading
    pub context_length: Option<u32>,
    pub batch_size: Option<u32>,
    pub ubatch_size: Option<u32>,
    pub cache_type_k: Option<CacheTypeK>,
    pub cache_type_v: Option<CacheTypeV>,
    pub keep: Option<i32>,
    pub swa_full: Option<bool>,
    pub mlock: Option<bool>,
    pub mmap: Option<bool>,
    pub numa: Option<NumMode>,
    pub uniform_cache: Option<bool>,
    pub system_prompt: Option<String>,
    pub system_prompt_preset_name: Option<String>,
    pub max_concurrent_predictions: Option<u32>,
    pub threads: Option<u32>,
    pub threads_batch: Option<u32>,
    pub parallel: Option<u32>,

    // GPU
    pub gpu_layers: Option<i32>,
    pub split_mode: Option<SplitMode>,
    pub tensor_split: Option<String>,
    pub main_gpu: Option<i32>,
    pub fit: Option<bool>,
    pub lora: Option<PathBuf>,
    pub lora_scaled: Option<(PathBuf, f32)>,
    pub rpc: Option<String>,
    pub embedding: Option<bool>,
    pub kv_cache_offload: Option<bool>,
    pub flash_attn: Option<bool>,
    pub jinja: Option<bool>,
    pub chat_template: Option<String>,
    pub chat_template_kwargs: Option<String>,
    pub expert_count: Option<i32>,
    pub gpu_layers_mode: Option<crate::models::GpuLayersMode>,

    // Sampling
    pub seed: Option<i32>,
    pub temperature: Option<f32>,
    pub top_k: Option<i32>,
    pub top_p: Option<f32>,
    pub min_p: Option<f32>,
    pub typical_p: Option<f32>,
    pub mirostat: Option<Mirostat>,
    pub mirostat_lr: Option<f32>,
    pub mirostat_ent: Option<f32>,
    pub ignore_eos: Option<bool>,
    pub samplers: Option<Samplers>,

    // Repetition
    pub repeat_penalty: Option<f32>,
    pub repeat_last_n: Option<i32>,
    pub presence_penalty: Option<f32>,
    pub frequency_penalty: Option<f32>,
    pub dry_multiplier: Option<f32>,
    pub dry_base: Option<f32>,
    pub dry_allowed_length: Option<i32>,
    pub dry_penalty_last_n: Option<i32>,

    // RoPE
    pub rope_scaling: Option<RopeScaling>,
    pub rope_scale: Option<f32>,
    pub rope_freq_base: Option<f32>,
    pub rope_freq_scale: Option<f32>,
    pub rope_yarn_enabled: Option<bool>,

    // Server
    pub cache_prompt: Option<bool>,
    pub cache_reuse: Option<u32>,
    pub webui: Option<bool>,
    pub ws_server_enabled: Option<bool>,
    pub ws_server_port: Option<u16>,
    pub ws_server_auth_key: Option<String>,
    pub ws_server_tls_enabled: Option<bool>,
    pub ws_server_tls_cert: Option<String>,
    pub ws_server_tls_key: Option<String>,

    // Other
    pub max_tokens: Option<u32>,
    pub cache_type: Option<CacheType>,
    pub llama_cpp_version_cpu: Option<String>,
    pub llama_cpp_version_vulkan: Option<String>,
    pub llama_cpp_version_rocm: Option<String>,
    pub llama_cpp_version_rocm_lemonade: Option<String>,
    pub llama_cpp_version_cuda: Option<String>,
    pub spec_type: Option<String>,
    pub draft_tokens: Option<u32>,
    pub tags: Option<Vec<String>>,
}

/// Apply a scalar Copy field from override: `base.f = self.f.unwrap_or(base.f)`.
macro_rules! apply_scalar {
    ($self:ident, $base:ident, $($field:ident),+ $(,)?) => {
        $(
            $base.$field = $self.$field.unwrap_or($base.$field);
        )+
    };
}

/// Apply a Clone field from override: `if let Some(v) = &self.f { base.f = v.clone(); }`.
macro_rules! apply_clone {
    ($self:ident, $base:ident, $($field:ident),+ $(,)?) => {
        $(
            if let Some(v) = &$self.$field {
                $base.$field = v.clone();
            }
        )+
    };
}

/// Apply an Option<T> field from override: `if let Some(v) = &self.f { base.f = Some(v.clone()); }`.
macro_rules! apply_option {
    ($self:ident, $base:ident, $($field:ident),+ $(,)?) => {
        $(
            if let Some(v) = &$self.$field {
                $base.$field = Some(v.clone());
            }
        )+
    };
}

impl ModelOverride {
    pub fn from_settings(s: &crate::models::ModelSettings) -> Self {
        Self {
            context_length: Some(s.context_length),
            batch_size: Some(s.batch_size),
            ubatch_size: Some(s.ubatch_size),
            cache_type_k: s.cache_type_k,
            cache_type_v: s.cache_type_v,
            keep: Some(s.keep),
            swa_full: Some(s.swa_full),
            mlock: Some(s.mlock),
            mmap: Some(s.mmap),
            numa: Some(s.numa),
            uniform_cache: Some(s.uniform_cache),
            system_prompt: Some(s.system_prompt.clone()),
            system_prompt_preset_name: Some(s.system_prompt_preset_name.clone()),
            max_concurrent_predictions: s.max_concurrent_predictions,
            threads: Some(s.threads),
            threads_batch: Some(s.threads_batch),
            parallel: Some(s.parallel),
            gpu_layers: Some(match s.gpu_layers_mode {
                crate::models::GpuLayersMode::Auto => 0,
                crate::models::GpuLayersMode::Specific(n) => n as i32,
                crate::models::GpuLayersMode::All => -1,
            }),
            gpu_layers_mode: Some(s.gpu_layers_mode),
            split_mode: Some(s.split_mode),
            tensor_split: Some(s.tensor_split.clone()),
            main_gpu: Some(s.main_gpu),
            fit: Some(s.fit),
            lora: s.lora.clone(),
            lora_scaled: s.lora_scaled.clone(),
            rpc: Some(s.rpc.clone()),
            embedding: Some(s.embedding),
            kv_cache_offload: Some(s.kv_cache_offload),
            flash_attn: Some(s.flash_attn),
            jinja: Some(s.jinja),
            chat_template: s.chat_template.clone(),
            chat_template_kwargs: s.chat_template_kwargs.clone(),
            expert_count: Some(s.expert_count),
            seed: Some(s.seed),
            temperature: Some(s.temperature),
            top_k: Some(s.top_k),
            top_p: Some(s.top_p),
            min_p: Some(s.min_p),
            typical_p: Some(s.typical_p),
            mirostat: Some(s.mirostat),
            mirostat_lr: Some(s.mirostat_lr),
            mirostat_ent: Some(s.mirostat_ent),
            ignore_eos: Some(s.ignore_eos),
            samplers: Some(s.samplers.clone()),
            repeat_penalty: Some(s.repeat_penalty),
            repeat_last_n: Some(s.repeat_last_n),
            presence_penalty: s.presence_penalty,
            frequency_penalty: s.frequency_penalty,
            dry_multiplier: Some(s.dry_multiplier),
            dry_base: Some(s.dry_base),
            dry_allowed_length: Some(s.dry_allowed_length),
            dry_penalty_last_n: Some(s.dry_penalty_last_n),
            rope_scaling: Some(s.rope_scaling),
            rope_scale: Some(s.rope_scale),
            rope_freq_base: Some(s.rope_freq_base),
            rope_freq_scale: Some(s.rope_freq_scale),
            rope_yarn_enabled: Some(s.rope_yarn_enabled),
            cache_prompt: Some(s.cache_prompt),
            cache_reuse: Some(s.cache_reuse),
            webui: Some(s.webui),
            max_tokens: s.max_tokens,
            cache_type: Some(s.cache_type),
            llama_cpp_version_cpu: s.llama_cpp_version_cpu.clone(),
            llama_cpp_version_vulkan: s.llama_cpp_version_vulkan.clone(),
            llama_cpp_version_rocm: s.llama_cpp_version_rocm.clone(),
            llama_cpp_version_rocm_lemonade: s.llama_cpp_version_rocm_lemonade.clone(),
            llama_cpp_version_cuda: s.llama_cpp_version_cuda.clone(),
            spec_type: Some(s.spec_type.clone()),
            draft_tokens: Some(s.draft_tokens),
            tags: Some(s.tags.clone()),
            ws_server_enabled: Some(s.ws_server_enabled),
            ws_server_port: Some(s.ws_server_port),
            ws_server_auth_key: s.ws_server_auth_key.clone(),
            ws_server_tls_enabled: Some(s.ws_server_tls_enabled),
            ws_server_tls_cert: s.ws_server_tls_cert.clone(),
            ws_server_tls_key: s.ws_server_tls_key.clone(),
        }
    }

    /// Merge override into a base ModelSettings (in-place).
    pub fn apply(&self, base: &mut crate::models::ModelSettings) {
        // Override values always take precedence. For Option<T> fields,
        // the override value (even None) is explicitly set by the user.

        // Scalar Copy fields: base.f = self.f.unwrap_or(base.f)
        apply_scalar!(self, base,
            context_length, batch_size, ubatch_size, keep, swa_full, mlock, mmap,
            numa, uniform_cache, kv_cache_offload, threads, threads_batch, parallel,
            split_mode, main_gpu, fit, embedding, flash_attn, jinja, expert_count,
            seed, temperature, top_k, top_p, min_p, typical_p,
            mirostat, mirostat_lr, mirostat_ent, ignore_eos,
            repeat_penalty, repeat_last_n,
            dry_multiplier, dry_base, dry_allowed_length, dry_penalty_last_n,
            rope_scaling, rope_scale, rope_freq_base, rope_freq_scale, rope_yarn_enabled,
            cache_prompt, cache_reuse, webui, cache_type,
            ws_server_enabled, ws_server_port, ws_server_tls_enabled,
            draft_tokens,
        );

        // Cloneable fields: if let Some(v) = &self.f { base.f = v.clone(); }
        apply_clone!(self, base,
            system_prompt, system_prompt_preset_name, tensor_split, rpc,
            samplers, spec_type, tags,
        );

        // Option<T> fields: if let Some(v) = &self.f { base.f = Some(v.clone()); }
        apply_option!(self, base,
            lora, lora_scaled, chat_template, chat_template_kwargs,
            llama_cpp_version_cpu, llama_cpp_version_vulkan,
            llama_cpp_version_rocm, llama_cpp_version_rocm_lemonade,
            llama_cpp_version_cuda,
            ws_server_auth_key, ws_server_tls_cert, ws_server_tls_key,
        );

        // Direct Option<T> assignment (same type in both structs)
        base.cache_type_k = self.cache_type_k;
        base.cache_type_v = self.cache_type_v;
        base.presence_penalty = self.presence_penalty;
        base.frequency_penalty = self.frequency_penalty;
        base.max_tokens = self.max_tokens;

        // Special: max_concurrent_predictions uses or() for Option chaining
        base.max_concurrent_predictions = self
            .max_concurrent_predictions
            .or(base.max_concurrent_predictions);

        // Special: gpu_layers converts i32 legacy field to GpuLayersMode enum
        base.gpu_layers_mode = match self.gpu_layers.unwrap_or(-1) {
            n if n < 0 => crate::models::GpuLayersMode::All,
            _ => crate::models::GpuLayersMode::Auto,
        };

        // FIELD ACCOUNTING (ModelOverride: 92 fields):
        // - apply_scalar: 65 fields
        // - apply_clone: 7 fields
        // - apply_option: 13 fields
        // - direct Option assign: 5 fields (cache_type_k, cache_type_v, presence_penalty,
        //   frequency_penalty, max_tokens)
        // - special: 2 fields (max_concurrent_predictions, gpu_layers->gpu_layers_mode)
        // - NOT in ModelSettings: 0 (all ModelOverride fields mapped above)
        //
        // ModelSettings fields NOT in ModelOverride (not overridable):
        // host, port, timeout, backend, platform, router_max_models, server_mode,
        // api_endpoint_enabled, api_endpoint_port
        //
        // When adding a field: ensure it appears in exactly one category above.
    }
}

/// Built-in profiles with sensible defaults for popular model families.
pub fn builtin_profiles() -> Vec<Profile> {
    vec![
        Profile {
            name: "Qwen".into(),
            description: "Optimized for Qwen models (dense)".into(),
            settings: ModelOverride {
                context_length: Some(131072),
                temperature: Some(0.7),
                top_k: Some(20),
                top_p: Some(0.95),
                max_tokens: Some(4096),
                presence_penalty: Some(0.0),
                uniform_cache: Some(true),
                jinja: Some(true),
                ..Default::default()
            },
        },
        Profile {
            name: "Qwen-MoE".into(),
            description: "Optimized for Qwen MoE models (35B-A3B)".into(),
            settings: ModelOverride {
                context_length: Some(131072),
                temperature: Some(0.8),
                top_k: Some(20),
                top_p: Some(0.95),
                max_tokens: Some(4096),
                presence_penalty: Some(1.5),
                uniform_cache: Some(true),
                jinja: Some(true),
                ..Default::default()
            },
        },
        Profile {
            name: "Qwen-Coding".into(),
            description: "Optimized for Qwen models in coding mode".into(),
            settings: ModelOverride {
                context_length: Some(131072),
                temperature: Some(0.6),
                top_k: Some(20),
                top_p: Some(0.95),
                max_tokens: Some(4096),
                presence_penalty: Some(0.0),
                uniform_cache: Some(true),
                jinja: Some(true),
                ..Default::default()
            },
        },
        Profile {
            name: "Gemma".into(),
            description: "Optimized for Gemma 2/4 models".into(),
            settings: ModelOverride {
                context_length: Some(131072),
                min_p: Some(0.1),
                temperature: Some(1.0),
                top_k: Some(65),
                top_p: Some(0.95),
                max_tokens: Some(4096),
                uniform_cache: Some(true),
                jinja: Some(true),
                ..Default::default()
            },
        },
        Profile {
            name: "Llama".into(),
            description: "Optimized for Llama 3.1/3.3 models".into(),
            settings: ModelOverride {
                context_length: Some(131072),
                temperature: Some(0.7),
                top_p: Some(0.9),
                repeat_penalty: Some(1.1),
                max_tokens: Some(4096),
                uniform_cache: Some(true),
                jinja: Some(true),
                ..Default::default()
            },
        },
        Profile {
            name: "Mistral".into(),
            description: "Optimized for Mistral 7B/NeMo models".into(),
            settings: ModelOverride {
                context_length: Some(131072),
                temperature: Some(0.7),
                top_k: Some(50),
                top_p: Some(0.9),
                max_tokens: Some(4096),
                uniform_cache: Some(true),
                jinja: Some(true),
                ..Default::default()
            },
        },
        Profile {
            name: "Phi".into(),
            description: "Optimized for Phi 3.5 Mini models".into(),
            settings: ModelOverride {
                context_length: Some(131072),
                temperature: Some(0.7),
                top_k: Some(50),
                top_p: Some(0.9),
                repeat_penalty: Some(1.1),
                max_tokens: Some(4096),
                uniform_cache: Some(true),
                ..Default::default()
            },
        },
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct DefaultParams {
    // Loading
    #[serde(default)]
    pub context_length: u32,
    #[serde(default)]
    pub threads: u32,
    #[serde(default)]
    pub threads_batch: u32,
    #[serde(default)]
    pub batch_size: u32,
    #[serde(default)]
    pub ubatch_size: u32,
    #[serde(default = "default_cache_type_k")]
    pub cache_type_k: Option<CacheTypeK>,
    #[serde(default = "default_cache_type_v")]
    pub cache_type_v: Option<CacheTypeV>,
    #[serde(default)]
    pub keep: i32,
    #[serde(default)]
    pub swa_full: bool,
    #[serde(default)]
    pub mlock: bool,
    #[serde(default)]
    pub mmap: bool,
    #[serde(default)]
    pub numa: NumMode,
    #[serde(default)]
    pub uniform_cache: bool,
    #[serde(default)]
    pub kv_cache_offload: bool,
    #[serde(default)]
    pub parallel: u32,
    #[serde(default)]
    pub max_concurrent_predictions: Option<u32>,
    #[serde(default)]
    pub system_prompt: String,
    #[serde(default = "default_system_prompt_preset_name")]
    pub system_prompt_preset_name: String,
    // GPU
    #[serde(default)]
    pub gpu_layers: i32,
    #[serde(default = "default_gpu_layers_mode")]
    pub gpu_layers_mode: crate::models::GpuLayersMode,
    #[serde(default)]
    pub split_mode: SplitMode,
    #[serde(default)]
    pub tensor_split: String,
    #[serde(default)]
    pub main_gpu: i32,
    #[serde(default)]
    pub fit: bool,
    #[serde(default)]
    pub lora: Option<PathBuf>,
    #[serde(default)]
    pub lora_scaled: Option<(PathBuf, f32)>,
    #[serde(default)]
    pub rpc: String,
    #[serde(default)]
    pub embedding: bool,
    #[serde(default)]
    pub flash_attn: bool,
    #[serde(default)]
    pub jinja: bool,
    #[serde(default)]
    pub chat_template: Option<String>,
    #[serde(default)]
    pub chat_template_kwargs: Option<String>,
    #[serde(default)]
    pub expert_count: i32,

    // Sampling
    #[serde(default)]
    pub seed: i32,
    #[serde(default)]
    pub temperature: f32,
    #[serde(default)]
    pub top_k: i32,
    #[serde(default)]
    pub top_p: f32,
    #[serde(default)]
    pub min_p: f32,
    #[serde(default)]
    pub typical_p: f32,
    #[serde(default)]
    pub mirostat: Mirostat,
    #[serde(default)]
    pub mirostat_lr: f32,
    #[serde(default)]
    pub mirostat_ent: f32,
    #[serde(default)]
    pub ignore_eos: bool,
    #[serde(default)]
    pub samplers: Samplers,

    // Repetition
    #[serde(default)]
    pub repeat_penalty: f32,
    #[serde(default)]
    pub repeat_last_n: i32,
    #[serde(default = "default_presence_penalty")]
    pub presence_penalty: Option<f32>,
    #[serde(default = "default_frequency_penalty")]
    pub frequency_penalty: Option<f32>,
    #[serde(default)]
    pub dry_multiplier: f32,
    #[serde(default)]
    pub dry_base: f32,
    #[serde(default)]
    pub dry_allowed_length: i32,
    #[serde(default)]
    pub dry_penalty_last_n: i32,

    // RoPE
    #[serde(default)]
    pub rope_scaling: RopeScaling,
    #[serde(default)]
    pub rope_scale: f32,
    #[serde(default)]
    pub rope_freq_base: f32,
    #[serde(default)]
    pub rope_freq_scale: f32,
    #[serde(default)]
    pub rope_yarn_enabled: bool,

    // Server
    #[serde(default)]
    pub host: String,
    #[serde(default)]
    pub port: u16,
    #[serde(default)]
    pub timeout: u32,
    #[serde(default = "default_cache_prompt")]
    pub cache_prompt: bool,
    #[serde(default)]
    pub cache_reuse: u32,
    #[serde(default)]
    pub webui: bool,
    #[serde(default)]
    pub ws_server_enabled: bool,
    #[serde(default = "default_ws_server_port")]
    pub ws_server_port: u16,
    #[serde(default)]
    pub ws_server_auth_key: Option<String>,
    #[serde(default)]
    pub ws_server_tls_enabled: bool,
    #[serde(default)]
    pub ws_server_tls_cert: Option<String>,
    #[serde(default)]
    pub ws_server_tls_key: Option<String>,
    #[serde(default)]
    pub router_max_models: u32,
    #[serde(default)]
    pub server_mode: crate::models::ServerMode,

    // Other
    #[serde(default = "default_max_tokens")]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub cache_type: CacheType,
    #[serde(default)]
    pub backend: Backend,
    /// Platform override: "linux", "windows", or "macos". If None, auto-detected.
    #[serde(default)]
    pub platform: Option<String>,
    #[serde(default)]
    pub llama_cpp_version_cpu: Option<String>,
    #[serde(default)]
    pub llama_cpp_version_vulkan: Option<String>,
    #[serde(default)]
    pub llama_cpp_version_rocm: Option<String>,
    #[serde(default)]
    pub llama_cpp_version_rocm_lemonade: Option<String>,
    #[serde(default)]
    pub llama_cpp_version_cuda: Option<String>,

    // API
    #[serde(default)]
    pub api_endpoint_enabled: bool,
    #[serde(default = "default_api_endpoint_port")]
    pub api_endpoint_port: u16,
    #[serde(default)]
    pub spec_type: String,
    #[serde(default)]
    pub draft_tokens: u32,
    #[serde(default)]
    pub tags: Vec<String>,
}

fn default_api_endpoint_port() -> u16 {
    49222
}

fn default_system_prompt_preset_name() -> String {
    "General".to_string()
}

fn default_cache_type_k() -> Option<CacheTypeK> {
    None
}
fn default_cache_type_v() -> Option<CacheTypeV> {
    None
}
fn default_presence_penalty() -> Option<f32> {
    None
}
fn default_frequency_penalty() -> Option<f32> {
    None
}
fn default_max_tokens() -> Option<u32> {
    None
}
fn default_cache_prompt() -> bool {
    true
}
fn default_ws_server_port() -> u16 {
    49223
}
fn default_gpu_layers_mode() -> crate::models::GpuLayersMode {
    crate::models::GpuLayersMode::Auto
}

impl Default for DefaultParams {
    fn default() -> Self {
        Self {
            // Loading
            context_length: 131072,
            threads: physical_cores(),
            threads_batch: 8,
            batch_size: 512,
            ubatch_size: 512,
            cache_type_k: None,
            cache_type_v: None,
            keep: 0,
            swa_full: false,
            mlock: false,
            mmap: true,
            numa: NumMode::None,
            uniform_cache: true,
            kv_cache_offload: true,
            parallel: 1,
            max_concurrent_predictions: None,
            system_prompt: "You are a helpful assistant.".to_string(),
            system_prompt_preset_name: "General".to_string(),

            // GPU
            gpu_layers: -1,
            gpu_layers_mode: crate::models::GpuLayersMode::Auto,
            split_mode: SplitMode::Layer,
            tensor_split: String::new(),
            main_gpu: 0,
            fit: true,
            lora: None,
            lora_scaled: None,
            rpc: String::new(),
            embedding: false,
            flash_attn: true,
            jinja: true,
            chat_template: None,
            chat_template_kwargs: None,
            expert_count: -1,

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
            presence_penalty: None,
            frequency_penalty: None,
            dry_multiplier: 0.0,
            dry_base: 1.75,
            dry_allowed_length: 2,
            dry_penalty_last_n: -1,

            // RoPE
            rope_scaling: RopeScaling::None,
            rope_scale: 1.0,
            rope_freq_base: 0.0,
            rope_freq_scale: 1.0,
            rope_yarn_enabled: false,

            // Server
            host: "127.0.0.1".to_string(),
            port: 8080,
            timeout: 600,
            cache_prompt: true,
            cache_reuse: 0,
            webui: false,
            ws_server_enabled: false,
            ws_server_port: 49223,
            ws_server_auth_key: None,
            ws_server_tls_enabled: false,
            ws_server_tls_cert: None,
            ws_server_tls_key: None,
            router_max_models: 4,
            server_mode: crate::models::ServerMode::Normal,

            // Other
            max_tokens: None,
            cache_type: CacheType::F16,
            backend: {
                use crate::backend::hardware::{GpuVendor, detect_gpu_vendors};
                let vendors = detect_gpu_vendors();
                let mut result = Backend::Cpu;
                for v in &vendors {
                    if matches!(v, GpuVendor::Nvidia) {
                        result = Backend::Cuda;
                        break;
                    }
                    if matches!(v, GpuVendor::Amd) {
                        result = Backend::Rocm;
                        break;
                    }
                    if matches!(v, GpuVendor::Intel) {
                        result = Backend::Vulkan;
                        break;
                    }
                }
                result
            },
            platform: None,
            llama_cpp_version_cpu: None,
            llama_cpp_version_vulkan: None,
            llama_cpp_version_rocm: None,
            llama_cpp_version_rocm_lemonade: None,
            llama_cpp_version_cuda: None,
            api_endpoint_enabled: false,
            api_endpoint_port: 49222,
            spec_type: String::new(),
            draft_tokens: 0,
            tags: Vec::new(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            models_dirs: vec![
                dirs::data_dir()
                    .unwrap_or_default()
                    .join("llm-manager")
                    .join("models"),
            ],
            llama_server: "llama-server".into(),
            default: DefaultParams::default(),
            model_overrides: Default::default(),
            profiles: Default::default(),
            system_prompt_presets: Default::default(),
            rpc_workers: Vec::new(),
            ws_server: WsServer {
                enabled: false,
                port: 49223,
                auth_key: None,
                host: "0.0.0.0".to_string(),
                tls_enabled: false,
                tls_cert: None,
                tls_key: None,
            },
            search_limit: default_search_limit(),
        }
    }
}

impl Config {
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_default()
            .join("llm-manager")
            .join("config.yaml")
    }

    /// Validate config values and return a list of warnings for invalid entries.
    pub fn validate(&self) -> Vec<String> {
        let mut warnings = Vec::new();
        let default = &self.default;

        // Numeric range checks
        if default.context_length < 512 || default.context_length > 131072 {
            warnings.push(format!(
                "context_length {} is outside recommended range 512-131072",
                default.context_length
            ));
        }
        if default.temperature < 0.0 || default.temperature > 2.0 {
            warnings.push(format!(
                "temperature {} is outside recommended range 0.0-2.0",
                default.temperature
            ));
        }
        if (default.top_p < 0.0 || default.top_p > 1.0) && default.top_p != 0.0 {
            warnings.push(format!(
                "top_p {} is outside recommended range 0.0-1.0",
                default.top_p
            ));
        }
        if (default.repeat_penalty < 0.0 || default.repeat_penalty > 3.0)
            && default.repeat_penalty != 1.0
        {
            warnings.push(format!(
                "repeat_penalty {} is outside recommended range 0.0-3.0",
                default.repeat_penalty
            ));
        }
        if default.mirostat_lr < 0.0 || default.mirostat_lr > 1.0 {
            warnings.push(format!(
                "mirostat_lr {} is outside recommended range 0.0-1.0",
                default.mirostat_lr
            ));
        }
        if default.mirostat_ent < 0.0 || default.mirostat_ent > 10.0 {
            warnings.push(format!(
                "mirostat_ent {} is outside recommended range 0.0-10.0",
                default.mirostat_ent
            ));
        }

        if default.timeout < 1 {
            warnings.push(format!(
                "timeout {} must be at least 1 second",
                default.timeout
            ));
        }

        // Path validation
        if let Some(lora) = &default.lora
            && !lora.exists() {
                warnings.push(format!("lora path {} does not exist", lora.display()));
            }
        if let Some((lora, _)) = &default.lora_scaled
            && !lora.exists() {
                warnings.push(format!("lora path {} does not exist", lora.display()));
            }

        // Model override validation
        for model_name in self.model_overrides.keys() {
            if let Some(override_settings) = self.model_overrides.get(model_name.as_str()) {
                if let Some(lora) = &override_settings.lora
                    && !lora.exists() {
                        warnings.push(format!(
                            "model '{}' lora path {} does not exist",
                            model_name,
                            lora.display()
                        ));
                    }
                if let Some((lora, _)) = &override_settings.lora_scaled
                    && !lora.exists() {
                        warnings.push(format!(
                            "model '{}' lora path {} does not exist",
                            model_name,
                            lora.display()
                        ));
                    }
            }
        }

        warnings
    }

    /// Resolve settings for a specific model and profile.
    pub fn resolve_settings(
        &self,
        model_name: Option<&str>,
        profile_name: Option<&str>,
    ) -> crate::models::ModelSettings {
        let mut settings = crate::models::ModelSettings::from_config(self);

        // Apply model-specific override
        if let Some(name) = model_name
            && let Some(override_settings) = self.model_overrides.get(name)
        {
            override_settings.apply(&mut settings);
        }

        // Apply profile override if specified
        if let Some(p_name) = profile_name {
            if let Some(profile) = self.profiles.get(p_name) {
                profile.settings.apply(&mut settings);
            } else if let Some(profile) = builtin_profiles().iter().find(|p| p.name == p_name) {
                profile.settings.apply(&mut settings);
            }
        }

        settings
    }

    /// Get a system prompt preset content by name.
    pub fn get_preset_content(&self, name: &str) -> Option<String> {
        self.system_prompt_presets
            .get(name)
            .map(|p| p.content.clone())
    }

    fn normalize_config(mut config: Config) -> Config {
        // normalize models_dirs
        for path in &mut config.models_dirs {
            let path_str = path.to_string_lossy();
            if let Some(stripped) = path_str.strip_prefix("~/") {
                let home = dirs::home_dir().unwrap_or_default();
                *path = home.join(stripped);
            } else if !path.is_absolute() {
                let home = dirs::home_dir().unwrap_or_default();
                *path = home.join(path_str.as_ref());
            }
        }

        // Merge built-in profiles (add any missing ones)
        for p in builtin_profiles() {
            if config.profiles.get(&p.name).is_none() {
                config.profiles.save(&p);
            }
        }

        // Merge built-in system prompt presets (add any missing ones)
        for p in builtin_system_prompt_presets() {
            if config.system_prompt_presets.get(&p.name).is_none() {
                config.system_prompt_presets.save(&p);
            }
        }
        config
    }

    fn load_impl(path: &PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = serde_yaml::from_str(&content)
            .map_err(|e| format!("Failed to parse config file {}: {}", path.display(), e))?;
        let config = Self::normalize_config(config);
        let config = config.auto_detect_platform();
        let warnings = config.validate();
        if !warnings.is_empty() {
            eprintln!("Config validation warnings:");
            for warning in &warnings {
                eprintln!("  - {}", warning);
            }
        }
        Ok(config)
    }

    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let path = Self::config_path();
        if path.exists() {
            Self::load_impl(&path)
        } else {
            let mut config = Config::default();
            config.save()?;
            Ok(config)
        }
    }

    pub fn load_from(path: PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        if path.exists() {
            Self::load_impl(&path)
        } else {
            Err(format!("Config file not found: {}", path.display()).into())
        }
    }

    /// Auto-detect the platform if not explicitly set in config.
    fn auto_detect_platform(mut self) -> Self {
        if self.default.platform.is_none() {
            self.default.platform =
                Some(
                    crate::backend::hardware::platform_name(
                        crate::backend::hardware::detect_platform(),
                    )
                    .to_string(),
                );
        }
        self
    }

    pub fn save(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_yaml::to_string(self)?;
        std::fs::write(&path, content)?;
        // Persist model configs to individual YAML files
        let entries: Vec<(String, ModelOverride)> = self
            .model_overrides
            .keys()
            .iter()
            .filter_map(|k| self.model_overrides.get(k).map(|v| (k.clone(), v.clone())))
            .collect();
        for (name, cfg) in entries {
            self.model_overrides.save(&name, &cfg);
        }
        // Persist profiles to individual YAML files
        for profile in self.profiles.all() {
            self.profiles.save(&profile);
        }
        // Persist presets to individual YAML files
        for preset in self.system_prompt_presets.all() {
            self.system_prompt_presets.save(&preset);
        }
        Ok(())
    }

    pub fn merged_profiles(&self) -> Vec<Profile> {
        self.profiles.all()
    }

    pub fn merged_presets(&self) -> Vec<SystemPromptPreset> {
        self.system_prompt_presets.all()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Warning,
    Error,
}

impl LogLevel {
    pub fn label(&self) -> &'static str {
        match self {
            LogLevel::Info => "INFO",
            LogLevel::Warning => "WARNING",
            LogLevel::Error => "ERROR",
        }
    }
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: LogLevel,
    pub message: String,
}

impl LogEntry {
    pub fn new(message: impl Into<String>, level: LogLevel) -> Self {
        let timestamp = Local::now().format("%H:%M:%S").to_string();
        let message = sanitize_log(&message.into());
        Self {
            timestamp,
            level,
            message,
        }
    }
}

/// Sanitize log messages to prevent TUI layout breakages.
/// Strips non-printable characters and control sequences, and limits length.
fn sanitize_log(input: &str) -> String {
    // Limit length to avoid layout/perf issues with massive lines
    let max_len = 2000;
    let chars: Vec<char> = input.chars().collect();
    let truncated = chars.len() > max_len;
    let chars = if truncated {
        chars[..max_len].to_vec()
    } else {
        chars
    };

    let mut output = String::with_capacity(chars.len());
    for c in chars {
        // Strip ALL control characters except newline and tab.
        // Critically: strip \r (carriage return) as it breaks TUI rendering.
        if c.is_control() && c != '\n' && c != '\t' {
            continue;
        }
        output.push(c);
    }

    // Replace tabs with spaces for consistent rendering
    let output = output.replace('\t', "    ");

    // Final trim to remove trailing junk
    let mut result = output.trim_end().to_string();
    if truncated {
        result.push_str("... (truncated)");
    }
    result
}
