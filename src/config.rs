use std::collections::HashSet;
use std::path::PathBuf;

use chrono::Local;
use serde::{Deserialize, Serialize};

use crate::models::{Backend, CacheType, CacheTypeK, CacheTypeV, Mirostat, NumMode, RopeScaling, Samplers, SplitMode};

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

/// Global configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub models_dir: PathBuf,
    pub llama_server: PathBuf,
    pub default: DefaultParams,
    /// Per-model overrides (keyed by model file name).
    #[serde(default)]
    pub model_overrides: std::collections::HashMap<String, ModelOverride>,
    /// Named profiles of settings presets.
    #[serde(default)]
    pub profiles: Vec<Profile>,
    /// System prompt presets.
    #[serde(default)]
    pub system_prompt_presets: Vec<SystemPromptPreset>,
}

/// A named profile of settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
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

impl DefaultParams {}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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

    // Server
    pub cache_prompt: Option<bool>,
    pub cache_reuse: Option<u32>,

    // Other
    pub max_tokens: Option<u32>,
    pub cache_type: Option<CacheType>,
    pub reasoning_mode: Option<crate::models::ReasoningMode>,
}

impl ModelOverride {
    pub fn from_settings(s: &crate::models::ModelSettings) -> Self {
        Self {
            context_length: Some(s.context_length),
            batch_size: Some(s.batch_size),
            ubatch_size: Some(s.ubatch_size),
            cache_type_k: Some(s.cache_type_k.clone()),
            cache_type_v: Some(s.cache_type_v.clone()),
            keep: Some(s.keep),
            swa_full: Some(s.swa_full),
            mlock: Some(s.mlock),
            mmap: Some(s.mmap),
            numa: Some(s.numa.clone()),
            uniform_cache: Some(s.uniform_cache),
            system_prompt: Some(s.system_prompt.clone()),
            system_prompt_preset_name: Some(s.system_prompt_preset_name.clone()),
            gpu_layers: Some(s.gpu_layers),
            split_mode: Some(s.split_mode.clone()),
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
            seed: Some(s.seed),
            temperature: Some(s.temperature),
            top_k: Some(s.top_k),
            top_p: Some(s.top_p),
            min_p: Some(s.min_p),
            typical_p: Some(s.typical_p),
            mirostat: Some(s.mirostat.clone()),
            mirostat_lr: Some(s.mirostat_lr),
            mirostat_ent: Some(s.mirostat_ent),
            ignore_eos: Some(s.ignore_eos),
            samplers: Some(s.samplers.clone()),
            repeat_penalty: Some(s.repeat_penalty),
            repeat_last_n: Some(s.repeat_last_n),
            presence_penalty: Some(s.presence_penalty),
            frequency_penalty: Some(s.frequency_penalty),
            dry_multiplier: Some(s.dry_multiplier),
            dry_base: Some(s.dry_base),
            dry_allowed_length: Some(s.dry_allowed_length),
            dry_penalty_last_n: Some(s.dry_penalty_last_n),
            rope_scaling: Some(s.rope_scaling.clone()),
            rope_scale: Some(s.rope_scale),
            rope_freq_base: Some(s.rope_freq_base),
            rope_freq_scale: Some(s.rope_freq_scale),
            cache_prompt: Some(s.cache_prompt),
            cache_reuse: Some(s.cache_reuse),
            max_tokens: Some(s.max_tokens),
            cache_type: Some(s.cache_type.clone()),
            reasoning_mode: Some(s.reasoning_mode),
        }
    }

    /// Merge override into a base ModelSettings (in-place).
    pub fn apply(&self, base: &mut crate::models::ModelSettings) {
        if let Some(v) = self.context_length { base.context_length = v; }
        if let Some(v) = self.batch_size { base.batch_size = v; }
        if let Some(v) = self.ubatch_size { base.ubatch_size = v; }
        if let Some(v) = self.cache_type_k { base.cache_type_k = v; }
        if let Some(v) = self.cache_type_v { base.cache_type_v = v; }
        if let Some(v) = self.keep { base.keep = v; }
        if let Some(v) = self.swa_full { base.swa_full = v; }
        if let Some(v) = self.mlock { base.mlock = v; }
        if let Some(v) = self.mmap { base.mmap = v; }
        if let Some(v) = self.numa { base.numa = v; }
        if let Some(v) = self.uniform_cache { base.uniform_cache = v; }
        if let Some(v) = self.kv_cache_offload { base.kv_cache_offload = v; }
        if let Some(v) = &self.system_prompt { base.system_prompt = v.clone(); }
        if let Some(v) = &self.system_prompt_preset_name { base.system_prompt_preset_name = v.clone(); }
        if let Some(v) = self.gpu_layers { base.gpu_layers = v; }
        if let Some(v) = &self.split_mode { base.split_mode = v.clone(); }
        if let Some(v) = &self.tensor_split { base.tensor_split = v.clone(); }
        if let Some(v) = self.main_gpu { base.main_gpu = v; }
        if let Some(v) = self.fit { base.fit = v; }
        if let Some(v) = &self.lora { base.lora = Some(v.clone()); }
        if let Some(v) = &self.lora_scaled { base.lora_scaled = Some(v.clone()); }
        if let Some(v) = &self.rpc { base.rpc = v.clone(); }
        if let Some(v) = self.embedding { base.embedding = v; }
        if let Some(v) = self.flash_attn { base.flash_attn = v; }
        if let Some(v) = self.jinja { base.jinja = v; }
        if let Some(v) = &self.chat_template { base.chat_template = Some(v.clone()); }
        if let Some(v) = self.reasoning_mode { base.reasoning_mode = v; }
        if let Some(v) = self.seed { base.seed = v; }
        if let Some(v) = self.temperature { base.temperature = v; }
        if let Some(v) = self.top_k { base.top_k = v; }
        if let Some(v) = self.top_p { base.top_p = v; }
        if let Some(v) = self.min_p { base.min_p = v; }
        if let Some(v) = self.typical_p { base.typical_p = v; }
        if let Some(v) = self.mirostat { base.mirostat = v; }
        if let Some(v) = self.mirostat_lr { base.mirostat_lr = v; }
        if let Some(v) = self.mirostat_ent { base.mirostat_ent = v; }
        if let Some(v) = self.ignore_eos { base.ignore_eos = v; }
        if let Some(v) = &self.samplers { base.samplers = v.clone(); }
        if let Some(v) = self.repeat_penalty { base.repeat_penalty = v; }
        if let Some(v) = self.repeat_last_n { base.repeat_last_n = v; }
        if let Some(v) = self.presence_penalty { base.presence_penalty = v; }
        if let Some(v) = self.frequency_penalty { base.frequency_penalty = v; }
        if let Some(v) = self.dry_multiplier { base.dry_multiplier = v; }
        if let Some(v) = self.dry_base { base.dry_base = v; }
        if let Some(v) = self.dry_allowed_length { base.dry_allowed_length = v; }
        if let Some(v) = self.dry_penalty_last_n { base.dry_penalty_last_n = v; }
        if let Some(v) = &self.rope_scaling { base.rope_scaling = v.clone(); }
        if let Some(v) = self.rope_scale { base.rope_scale = v; }
        if let Some(v) = self.rope_freq_base { base.rope_freq_base = v; }
        if let Some(v) = self.rope_freq_scale { base.rope_freq_scale = v; }
        if let Some(v) = self.cache_prompt { base.cache_prompt = v; }
        if let Some(v) = self.cache_reuse { base.cache_reuse = v; }
        if let Some(v) = self.max_tokens { base.max_tokens = v; }
        if let Some(v) = &self.cache_type { base.cache_type = v.clone(); }
    }
}

/// Built-in profiles with sensible defaults for popular model families.
pub fn builtin_profiles() -> Vec<Profile> {
    vec![
        Profile {
            name: "Qwen".into(),
            description: "Optimized for Qwen models".into(),
            settings: ModelOverride {
                context_length: Some(32768),
                temperature: Some(0.6),
                top_k: Some(20),
                top_p: Some(0.9),
                max_tokens: Some(2048),
                repeat_penalty: Some(1.2),
                uniform_cache: Some(true),
                ..Default::default()
            },
        },
        Profile {
            name: "Gemma".into(),
            description: "Optimized for Gemma models".into(),
            settings: ModelOverride {
                min_p: Some(0.1),
                typical_p: Some(0.9),
                temperature: Some(0.8),
                top_p: Some(0.95),
                uniform_cache: Some(true),
                reasoning_mode: Some(crate::models::ReasoningMode::Gemma),
                ..Default::default()
            },
        },
        Profile {
            name: "Llama".into(),
            description: "Optimized for Llama models".into(),
            settings: ModelOverride {
                temperature: Some(0.7),
                top_p: Some(0.9),
                repeat_penalty: Some(1.1),
                uniform_cache: Some(true),
                ..Default::default()
            },
        },
        Profile {
            name: "Mistral".into(),
            description: "Optimized for Mistral models".into(),
            settings: ModelOverride {
                temperature: Some(0.7),
                top_k: Some(50),
                top_p: Some(0.9),
                uniform_cache: Some(true),
                ..Default::default()
            },
        },
        Profile {
            name: "Phi".into(),
            description: "Optimized for Phi models".into(),
            settings: ModelOverride {
                temperature: Some(0.7),
                top_k: Some(50),
                top_p: Some(0.9),
                repeat_penalty: Some(1.1),
                uniform_cache: Some(true),
                ..Default::default()
            },
        },
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    #[serde(default)]
    pub cache_type_k: CacheTypeK,
    #[serde(default)]
    pub cache_type_v: CacheTypeV,
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
    pub system_prompt: String,
    #[serde(default = "default_system_prompt_preset_name")]
    pub system_prompt_preset_name: String,
    #[serde(default)]
    pub reasoning_mode: crate::models::ReasoningMode,

    // GPU
    #[serde(default)]
    pub gpu_layers: i32,
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
    #[serde(default)]
    pub presence_penalty: f32,
    #[serde(default)]
    pub frequency_penalty: f32,
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

    // Server
    #[serde(default)]
    pub host: String,
    #[serde(default)]
    pub port: u16,
    #[serde(default)]
    pub timeout: u32,
    #[serde(default)]
    pub cache_prompt: bool,
    #[serde(default)]
    pub cache_reuse: u32,
    #[serde(default)]
    pub webui: bool,
    #[serde(default)]
    pub router_max_models: u32,

    // Other
    #[serde(default)]
    pub max_tokens: u32,
    #[serde(default)]
    pub cache_type: CacheType,
    #[serde(default)]
    pub backend: Backend,
}

fn default_system_prompt_preset_name() -> String {
    "General".to_string()
}

impl Default for DefaultParams {
    fn default() -> Self {
        Self {
            // Loading
            context_length: 32096,
            threads: physical_cores(),
            threads_batch: 8,
            batch_size: 512,
            ubatch_size: 512,
            cache_type_k: CacheTypeK::F16,
            cache_type_v: CacheTypeV::F16,
            keep: 0,
            swa_full: false,
            mlock: false,
            mmap: true,
            numa: NumMode::None,
            uniform_cache: true,
            kv_cache_offload: true,
            parallel: 1,
            system_prompt: "You are a helpful assistant.".to_string(),
            system_prompt_preset_name: "General".to_string(),
            reasoning_mode: crate::models::ReasoningMode::Default,

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
            presence_penalty: 0.0,
            frequency_penalty: 0.0,
            dry_multiplier: 0.0,
            dry_base: 1.75,
            dry_allowed_length: 2,
            dry_penalty_last_n: -1,

            // RoPE
            rope_scaling: RopeScaling::None,
            rope_scale: 1.0,
            rope_freq_base: 0.0,
            rope_freq_scale: 1.0,

            // Server
            host: "127.0.0.1".to_string(),
            port: 8080,
            timeout: 600,
            cache_prompt: true,
            cache_reuse: 0,
            webui: false,
            router_max_models: 4,

            // Other
            max_tokens: 2048,
            cache_type: CacheType::F16,
            backend: Backend::Vulkan,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            models_dir: dirs::config_dir().unwrap_or_default().join("llm-manager").join("models"),
            llama_server: "llama-server".into(),
            default: DefaultParams::default(),
            model_overrides: Default::default(),
            profiles: builtin_profiles(),
            system_prompt_presets: builtin_system_prompt_presets(),
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

    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let path = Self::config_path();
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            let mut config: Config = serde_yaml::from_str(&content)?;
            // normalize models_dir
            let path_str = config.models_dir.to_string_lossy();
            if path_str.starts_with("~/") {
                let home = dirs::home_dir().unwrap_or_default();
                config.models_dir = home.join(&path_str[2..]);
            } else if !config.models_dir.is_absolute() {
                config.models_dir = dirs::home_dir()
                    .unwrap_or_default()
                    .join(&config.models_dir);
            }
            // Merge built-in profiles (add any missing ones)
            let builtin = builtin_profiles();
            let mut builtin_names: std::collections::HashSet<&str> = builtin.iter().map(|p| p.name.as_str()).collect();
            config.profiles.retain(|p| {
                if builtin_names.contains(p.name.as_str()) {
                    builtin_names.remove(p.name.as_str());
                    true
                } else {
                    true
                }
            });
            // Add any built-in profiles that weren't in the config
            for p in builtin {
                if config.profiles.iter().all(|u| u.name != p.name) {
                    config.profiles.push(p);
                }
            }
            // Merge built-in system prompt presets (add any missing ones)
            let builtin_presets = builtin_system_prompt_presets();
            let mut builtin_preset_names: std::collections::HashSet<&str> = builtin_presets.iter().map(|p| p.name.as_str()).collect();
            config.system_prompt_presets.retain(|p| {
                if builtin_preset_names.contains(p.name.as_str()) {
                    builtin_preset_names.remove(p.name.as_str());
                    true
                } else {
                    true
                }
            });
            for p in builtin_presets {
                if config.system_prompt_presets.iter().all(|u| u.name != p.name) {
                    config.system_prompt_presets.push(p);
                }
            }
            Ok(config)
        } else {
            let config = Config::default();
            config.save()?;
            Ok(config)
        }
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_yaml::to_string(self)?;
        std::fs::write(&path, content)?;
        Ok(())
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
    let input = if input.len() > max_len {
        &input[..max_len]
    } else {
        input
    };

    let mut output = String::with_capacity(input.len());
    for c in input.chars() {
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
    if input.len() >= max_len {
        result.push_str("... (truncated)");
    }
    result
}
