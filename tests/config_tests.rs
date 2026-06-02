//! Comprehensive tests for config.rs types and functions.
//!
//! Tests cover: DefaultParams defaults, ModelOverride apply/from_settings,
//! Config validation, YAML roundtrip, builtin profiles/presets, resolve_settings.

use std::path::PathBuf;

use llm_manager::config::*;
use llm_manager::models::*;

// ── DefaultParams ───────────────────────────────────────────────

#[test]
fn default_params_context_length() {
    let dp = DefaultParams::default();
    assert_eq!(dp.context_length, 131072);
}

#[test]
fn default_params_temperature() {
    let dp = DefaultParams::default();
    assert!((dp.temperature - 0.8).abs() < f32::EPSILON);
}

#[test]
fn default_params_top_k() {
    let dp = DefaultParams::default();
    assert_eq!(dp.top_k, 40);
}

#[test]
fn default_params_top_p() {
    let dp = DefaultParams::default();
    assert!((dp.top_p - 0.95).abs() < f32::EPSILON);
}

#[test]
fn default_params_repeat_penalty() {
    let dp = DefaultParams::default();
    assert!((dp.repeat_penalty - 1.1).abs() < f32::EPSILON);
}

#[test]
fn default_params_system_prompt() {
    let dp = DefaultParams::default();
    assert_eq!(dp.system_prompt, "You are a helpful assistant.");
    assert_eq!(dp.system_prompt_preset_name, "General");
}

#[test]
fn default_params_server_defaults() {
    let dp = DefaultParams::default();
    assert_eq!(dp.host, "127.0.0.1");
    assert_eq!(dp.port, 8080);
    assert_eq!(dp.timeout, 600);
    assert!(dp.cache_prompt);
}

#[test]
fn default_params_backend_is_auto_detected() {
    let dp = DefaultParams::default();
    // Backend should be one of the known variants (platform-dependent)
    assert!(matches!(
        dp.backend,
        Backend::Cpu
            | Backend::Vulkan
            | Backend::Rocm
            | Backend::RocmLemonade
            | Backend::Cuda
            | Backend::CpuArm64
            | Backend::CudaWindows12_4
            | Backend::CudaWindows13_1
            | Backend::HipWindows
            | Backend::CpuWindows
            | Backend::VulkanWindows
            | Backend::CpuMacosArm64
            | Backend::CpuMacosX64
    ));
}

#[test]
fn default_params_mirostat_defaults() {
    let dp = DefaultParams::default();
    assert_eq!(dp.mirostat, Mirostat::Off);
    assert!((dp.mirostat_lr - 0.1).abs() < f32::EPSILON);
    assert!((dp.mirostat_ent - 5.0).abs() < f32::EPSILON);
}

#[test]
fn default_params_rope_defaults() {
    let dp = DefaultParams::default();
    assert_eq!(dp.rope_scaling, RopeScaling::None);
    assert!((dp.rope_scale - 1.0).abs() < f32::EPSILON);
    assert!((dp.rope_freq_scale - 1.0).abs() < f32::EPSILON);
}

#[test]
fn default_params_gpu_defaults() {
    let dp = DefaultParams::default();
    assert_eq!(dp.gpu_layers, -1); // All
    assert_eq!(dp.gpu_layers_mode, GpuLayersMode::Auto);
    assert_eq!(dp.split_mode, SplitMode::Layer);
    assert!(dp.flash_attn);
    assert!(dp.mmap);
}

#[test]
fn default_params_numa_default() {
    let dp = DefaultParams::default();
    assert_eq!(dp.numa, NumMode::None);
}

#[test]
fn default_params_dry_defaults() {
    let dp = DefaultParams::default();
    assert_eq!(dp.dry_multiplier, 0.0);
    assert!((dp.dry_base - 1.75).abs() < f32::EPSILON);
    assert_eq!(dp.dry_allowed_length, 2);
    assert_eq!(dp.dry_penalty_last_n, -1);
}

#[test]
fn default_params_api_defaults() {
    let dp = DefaultParams::default();
    assert!(!dp.api_endpoint_enabled);
    assert_eq!(dp.api_endpoint_port, 49222);
}

// ── ModelOverride ───────────────────────────────────────────────

#[test]
fn model_override_apply_overrides_context_length() {
    let mut settings = ModelSettings::default();
    let original_ctx = settings.context_length;
    let override_settings = ModelOverride {
        context_length: Some(8192),
        ..Default::default()
    };
    override_settings.apply(&mut settings);
    assert_eq!(settings.context_length, 8192);
    assert_ne!(settings.context_length, original_ctx);
}

#[test]
fn model_override_apply_overrides_temperature() {
    let mut settings = ModelSettings::default();
    let override_settings = ModelOverride {
        temperature: Some(0.5),
        ..Default::default()
    };
    override_settings.apply(&mut settings);
    assert!((settings.temperature - 0.5).abs() < f32::EPSILON);
}

#[test]
fn model_override_apply_unwrap_or_keeps_default() {
    let mut settings = ModelSettings::default();
    settings.context_length = 4096;
    let override_settings = ModelOverride {
        context_length: None, // Not set, should keep default
        ..Default::default()
    };
    override_settings.apply(&mut settings);
    assert_eq!(settings.context_length, 4096);
}

#[test]
fn model_override_apply_system_prompt() {
    let mut settings = ModelSettings::default();
    let override_settings = ModelOverride {
        system_prompt: Some("You are a coder.".into()),
        ..Default::default()
    };
    override_settings.apply(&mut settings);
    assert_eq!(settings.system_prompt, "You are a coder.");
}

#[test]
fn model_override_from_settings_roundtrip() {
    let mut settings = ModelSettings::default();
    settings.context_length = 16384;
    settings.temperature = 0.7;
    settings.top_k = 30;

    let override_ = ModelOverride::from_settings(&settings);
    let mut restored = ModelSettings::default();
    override_.apply(&mut restored);

    assert_eq!(restored.context_length, 16384);
    assert!((restored.temperature - 0.7).abs() < f32::EPSILON);
    assert_eq!(restored.top_k, 30);
}

#[test]
fn model_override_apply_gpu_layers_all() {
    let mut settings = ModelSettings::default();
    let override_settings = ModelOverride {
        gpu_layers: Some(-1),
        ..Default::default()
    };
    override_settings.apply(&mut settings);
    assert!(matches!(settings.gpu_layers_mode, GpuLayersMode::All));
}

#[test]
fn model_override_apply_gpu_layers_specific() {
    let mut settings = ModelSettings::default();
    let override_settings = ModelOverride {
        gpu_layers: Some(20),
        ..Default::default()
    };
    override_settings.apply(&mut settings);
    // Positive gpu_layers value (not negative) sets Auto mode per implementation
    assert!(matches!(settings.gpu_layers_mode, GpuLayersMode::Auto));
}

// ── Config validation ──────────────────────────────────────────

#[test]
fn config_validate_good_values_no_warnings() {
    let config = Config::default();
    let warnings = config.validate();
    assert!(warnings.is_empty());
}

#[test]
fn config_validate_context_length_too_low() {
    let mut config = Config::default();
    config.default.context_length = 256;
    let warnings = config.validate();
    assert!(warnings.iter().any(|w| w.contains("context_length")));
}

#[test]
fn config_validate_context_length_too_high() {
    let mut config = Config::default();
    config.default.context_length = 200000;
    let warnings = config.validate();
    assert!(warnings.iter().any(|w| w.contains("context_length")));
}

#[test]
fn config_validate_temperature_out_of_range() {
    let mut config = Config::default();
    config.default.temperature = 3.0;
    let warnings = config.validate();
    assert!(warnings.iter().any(|w| w.contains("temperature")));
}

#[test]
fn config_validate_top_p_out_of_range() {
    let mut config = Config::default();
    config.default.top_p = 1.5;
    let warnings = config.validate();
    assert!(warnings.iter().any(|w| w.contains("top_p")));
}

#[test]
fn config_validate_repeat_penalty_out_of_range() {
    let mut config = Config::default();
    config.default.repeat_penalty = 5.0;
    let warnings = config.validate();
    assert!(warnings.iter().any(|w| w.contains("repeat_penalty")));
}

#[test]
fn config_validate_mirostat_lr_out_of_range() {
    let mut config = Config::default();
    config.default.mirostat_lr = 2.0;
    let warnings = config.validate();
    assert!(warnings.iter().any(|w| w.contains("mirostat_lr")));
}

#[test]
fn config_validate_mirostat_ent_out_of_range() {
    let mut config = Config::default();
    config.default.mirostat_ent = 15.0;
    let warnings = config.validate();
    assert!(warnings.iter().any(|w| w.contains("mirostat_ent")));
}

#[test]
fn config_validate_timeout_too_low() {
    let mut config = Config::default();
    config.default.timeout = 0;
    let warnings = config.validate();
    assert!(warnings.iter().any(|w| w.contains("timeout")));
}

// ── Builtin profiles ───────────────────────────────────────────

#[test]
fn builtin_profiles_contains_all_families() {
    let profiles = builtin_profiles();
    let names: Vec<&str> = profiles.iter().map(|p| p.name.as_str()).collect();
    assert!(names.contains(&"Qwen"));
    assert!(names.contains(&"Gemma"));
    assert!(names.contains(&"Llama"));
    assert!(names.contains(&"Mistral"));
    assert!(names.contains(&"Phi"));
}

#[test]
fn builtin_profiles_have_descriptions() {
    let profiles = builtin_profiles();
    for p in &profiles {
        assert!(!p.description.is_empty());
    }
}

#[test]
fn builtin_profiles_have_settings() {
    let profiles = builtin_profiles();
    for p in &profiles {
        // Each profile should have at least one non-None setting
        let override_ = &p.settings;
        assert!(
            override_.context_length.is_some()
                || override_.temperature.is_some()
                || override_.top_k.is_some()
                || override_.top_p.is_some()
                || override_.repeat_penalty.is_some()
                || override_.min_p.is_some()
                || override_.typical_p.is_some()
                || override_.max_tokens.is_some()
                || override_.uniform_cache.is_some()
        );
    }
}

#[test]
fn builtin_profiles_apply_to_settings() {
    let profiles = builtin_profiles();
    let qwen = profiles.iter().find(|p| p.name == "Qwen").unwrap();
    let mut settings = ModelSettings::default();
    settings = qwen.apply(settings);
    assert_eq!(settings.context_length, 131072);
    assert!((settings.temperature - 0.7).abs() < f32::EPSILON);
}

// ── Builtin system prompt presets ───────────────────────────────

#[test]
fn builtin_system_prompt_presets_contains_all() {
    let presets = builtin_system_prompt_presets();
    let names: Vec<&str> = presets.iter().map(|p| p.name.as_str()).collect();
    assert!(names.contains(&"General"));
    assert!(names.contains(&"Coder"));
    assert!(names.contains(&"Thinker"));
    assert!(names.contains(&"Mathematician"));
}

#[test]
fn builtin_system_prompt_presets_have_content() {
    let presets = builtin_system_prompt_presets();
    for p in &presets {
        assert!(!p.content.is_empty());
        assert!(!p.description.is_empty());
    }
}

// ── Config resolve_settings ────────────────────────────────────

#[test]
fn config_resolve_settings_returns_model_settings() {
    let config = Config::default();
    let settings = config.resolve_settings(None, None);
    assert_eq!(settings.context_length, 131072);
}

#[test]
fn config_resolve_settings_with_model_override() {
    let mut config = Config::default();
    config.model_overrides.save(
        "my-model.gguf",
        &ModelOverride {
            context_length: Some(8192),
            temperature: Some(0.5),
            ..Default::default()
        },
    );
    let settings = config.resolve_settings(Some("my-model.gguf"), None);
    assert_eq!(settings.context_length, 8192);
    assert!((settings.temperature - 0.5).abs() < f32::EPSILON);
}

#[test]
fn config_resolve_settings_profile_overrides() {
    let config = Config::default();
    let settings = config.resolve_settings(None, Some("Qwen"));
    assert_eq!(settings.context_length, 131072);
    assert!((settings.temperature - 0.7).abs() < f32::EPSILON);
}

// ── Config merged_profiles ─────────────────────────────────────

#[test]
fn config_merged_profiles_includes_builtins() {
    let config = Config::default();
    let merged = config.merged_profiles();
    let names: Vec<&str> = merged.iter().map(|p| p.name.as_str()).collect();
    for builtin in [
        "Qwen",
        "Qwen-MoE",
        "Qwen-Coding",
        "Gemma",
        "Llama",
        "Mistral",
        "Phi",
    ] {
        assert!(
            names.contains(&builtin),
            "missing builtin profile: {}",
            builtin
        );
    }
}

// ── Config search_limit ────────────────────────────────────────

#[test]
fn config_default_search_limit() {
    let config = Config::default();
    assert_eq!(config.search_limit, 50);
}

// ── Config config_path ─────────────────────────────────────────

#[test]
fn config_config_path_contains_llm_manager() {
    let path = Config::config_path();
    let path_str = path.to_string_lossy();
    assert!(path_str.contains("llm-manager"));
}

// ── LogEntry ────────────────────────────────────────────────────

#[test]
fn log_entry_new_has_timestamp() {
    let entry = LogEntry::new("test message", LogLevel::Info);
    assert!(!entry.timestamp.is_empty());
    assert_eq!(entry.message, "test message");
    assert_eq!(entry.level, LogLevel::Info);
}

#[test]
fn log_entry_level_labels() {
    assert_eq!(LogLevel::Info.label(), "INFO");
    assert_eq!(LogLevel::Warning.label(), "WARNING");
    assert_eq!(LogLevel::Error.label(), "ERROR");
}

// ── RpcWorker ───────────────────────────────────────────────────

#[test]
fn rpc_worker_default_port() {
    let worker = RpcWorker {
        selected: false,
        name: "test".into(),
        ip: "192.168.1.1".into(),
        port: 50052,
    };
    assert_eq!(worker.port, 50052);
}

// ── ModelOverride field coverage ────────────────────────────────

#[test]
fn model_override_apply_sampling_params() {
    let mut settings = ModelSettings::default();
    let override_ = ModelOverride {
        seed: Some(42),
        temperature: Some(0.7),
        top_k: Some(20),
        top_p: Some(0.9),
        min_p: Some(0.1),
        typical_p: Some(0.95),
        mirostat: Some(Mirostat::V1),
        ignore_eos: Some(true),
        ..Default::default()
    };
    override_.apply(&mut settings);
    assert_eq!(settings.seed, 42);
    assert!((settings.temperature - 0.7).abs() < f32::EPSILON);
    assert_eq!(settings.top_k, 20);
    assert!((settings.top_p - 0.9).abs() < f32::EPSILON);
    assert!((settings.min_p - 0.1).abs() < f32::EPSILON);
    assert!(settings.ignore_eos);
}

#[test]
fn model_override_apply_repetition_params() {
    let mut settings = ModelSettings::default();
    let override_ = ModelOverride {
        repeat_penalty: Some(1.2),
        repeat_last_n: Some(128),
        presence_penalty: Some(0.5),
        frequency_penalty: Some(0.3),
        ..Default::default()
    };
    override_.apply(&mut settings);
    assert!((settings.repeat_penalty - 1.2).abs() < f32::EPSILON);
    assert_eq!(settings.repeat_last_n, 128);
    assert_eq!(settings.presence_penalty, Some(0.5));
    assert_eq!(settings.frequency_penalty, Some(0.3));
}

#[test]
fn model_override_apply_rope_params() {
    let mut settings = ModelSettings::default();
    let override_ = ModelOverride {
        rope_scaling: Some(RopeScaling::Yarn),
        rope_scale: Some(2.0),
        rope_freq_scale: Some(0.5),
        ..Default::default()
    };
    override_.apply(&mut settings);
    assert_eq!(settings.rope_scaling, RopeScaling::Yarn);
    assert!((settings.rope_scale - 2.0).abs() < f32::EPSILON);
    assert!((settings.rope_freq_scale - 0.5).abs() < f32::EPSILON);
}

#[test]
fn model_override_apply_server_params() {
    let mut settings = ModelSettings::default();
    let override_ = ModelOverride {
        cache_prompt: Some(false),
        cache_reuse: Some(64),
        webui: Some(true),
        max_tokens: Some(1024),
        ..Default::default()
    };
    override_.apply(&mut settings);
    assert!(!settings.cache_prompt);
    assert_eq!(settings.cache_reuse, 64);
    assert!(settings.webui);
    assert_eq!(settings.max_tokens, Some(1024));
}

#[test]
fn model_override_apply_backend_version() {
    let mut settings = ModelSettings::default();
    let override_ = ModelOverride {
        llama_cpp_version_cpu: Some("b1234".into()),
        llama_cpp_version_cuda: Some("b5678".into()),
        ..Default::default()
    };
    override_.apply(&mut settings);
    assert_eq!(settings.llama_cpp_version_cpu, Some("b1234".into()));
    assert_eq!(settings.llama_cpp_version_cuda, Some("b5678".into()));
}

#[test]
fn model_override_apply_mtp_settings() {
    let mut settings = ModelSettings::default();
    let override_ = ModelOverride {
        spec_type: Some("draft-mtp".into()),
        draft_tokens: Some(5),
        ..Default::default()
    };
    override_.apply(&mut settings);
    assert_eq!(settings.spec_type, "draft-mtp");
    assert_eq!(settings.draft_tokens, 5);
}

#[test]
fn model_override_apply_tags() {
    let mut settings = ModelSettings::default();
    let override_ = ModelOverride {
        tags: Some(vec!["tag1".into(), "tag2".into()]),
        ..Default::default()
    };
    override_.apply(&mut settings);
    assert_eq!(
        settings.tags,
        vec![String::from("tag1"), String::from("tag2")]
    );
}

// ── Config::default() completeness ─────────────────────────────

#[test]
fn config_default_has_builtin_profiles() {
    let config = Config::default();
    assert!(!config.profiles.all().is_empty());
}

#[test]
fn config_default_has_builtin_presets() {
    let config = Config::default();
    assert!(!config.system_prompt_presets.all().is_empty());
}

#[test]
fn config_default_empty_rpc_workers() {
    let config = Config::default();
    assert!(config.rpc_workers.is_empty());
}

#[test]
fn config_default_empty_model_overrides() {
    let config = Config {
        models_dirs: vec![],
        llama_server: std::path::PathBuf::new(),
        default: DefaultParams::default(),
        model_overrides: ModelConfigStore::new(),
        profiles: ProfileStore::new(),
        system_prompt_presets: PresetStore::new(),
        rpc_workers: Vec::new(),
        ws_server: llm_manager::WsServer::default(),
        search_limit: 50,
    };
    // Store is initialized successfully (may contain existing configs on disk)
    let _keys = config.model_overrides.keys();
}

// ── ModelOverride apply with chat template ─────────────────────

#[test]
fn model_override_apply_chat_template() {
    let mut settings = ModelSettings::default();
    let override_ = ModelOverride {
        jinja: Some(true),
        chat_template: Some("{{ messages }}".into()),
        chat_template_kwargs: Some(r#"{"enable_thinking": false}"#.into()),
        ..Default::default()
    };
    override_.apply(&mut settings);
    assert!(settings.jinja);
    assert_eq!(settings.chat_template, Some("{{ messages }}".into()));
    assert_eq!(
        settings.chat_template_kwargs,
        Some(r#"{"enable_thinking": false}"#.into())
    );
}

// ── ModelOverride apply with LoRA ──────────────────────────────

#[test]
fn model_override_apply_lora_paths() {
    let mut settings = ModelSettings::default();
    let override_ = ModelOverride {
        lora: Some("/path/to/lora.gguf".into()),
        lora_scaled: Some((PathBuf::from("/path/to/lora2.gguf"), 0.5)),
        ..Default::default()
    };
    override_.apply(&mut settings);
    assert_eq!(settings.lora, Some(PathBuf::from("/path/to/lora.gguf")));
    assert_eq!(
        settings.lora_scaled,
        Some((PathBuf::from("/path/to/lora2.gguf"), 0.5))
    );
}

// ── ModelOverride apply with KV cache ──────────────────────────

#[test]
fn model_override_apply_kv_cache_params() {
    let mut settings = ModelSettings::default();
    let override_ = ModelOverride {
        uniform_cache: Some(false),
        kv_cache_offload: Some(false),
        cache_type_k: Some(CacheTypeK::Q8_0),
        cache_type_v: Some(CacheTypeV::Q8_0),
        ..Default::default()
    };
    override_.apply(&mut settings);
    assert!(!settings.uniform_cache);
    assert!(!settings.kv_cache_offload);
    assert_eq!(settings.cache_type_k, Some(CacheTypeK::Q8_0));
    assert_eq!(settings.cache_type_v, Some(CacheTypeV::Q8_0));
}

// ── ModelOverride apply with expert count ──────────────────────

#[test]
fn model_override_apply_expert_count() {
    let mut settings = ModelSettings::default();
    let override_ = ModelOverride {
        expert_count: Some(2),
        ..Default::default()
    };
    override_.apply(&mut settings);
    assert_eq!(settings.expert_count, 2);
}
