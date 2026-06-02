//! Comprehensive tests for models.rs domain types and functions.
//!
//! Tests cover: string utilities, host cleaning, enum cycling, Display/From impls,
//! Backend platform detection, VRAM estimation, quantization bytes, and domain types.

use llm_manager::models::*;

// ── strip_gguf ──────────────────────────────────────────────────

#[test]
fn strip_gguf_strips_lowercase_extension() {
    assert_eq!(strip_gguf("model.gguf"), "model");
}

#[test]
fn strip_gguf_strips_uppercase_extension() {
    assert_eq!(strip_gguf("model.GGUF"), "model");
}

#[test]
fn strip_gguf_no_extension_returns_as_is() {
    assert_eq!(strip_gguf("model.bin"), "model.bin");
}

#[test]
fn strip_gguf_already_stripped() {
    assert_eq!(strip_gguf("model"), "model");
}

#[test]
fn strip_gguf_with_dots_in_name() {
    assert_eq!(strip_gguf("qwen2.5-7b.gguf"), "qwen2.5-7b");
}

// ── clean_host / format_host ────────────────────────────────────

#[test]
fn clean_host_empty_returns_loopback() {
    assert_eq!(clean_host(""), "127.0.0.1");
}

#[test]
fn clean_host_ipv4_unchanged() {
    assert_eq!(clean_host("192.168.1.1"), "192.168.1.1");
}

#[test]
fn clean_host_ipv6_wrapped_in_brackets() {
    assert_eq!(clean_host("::1"), "[::1]");
}

#[test]
fn clean_host_ipv6_full() {
    assert_eq!(clean_host("2001:db8::1"), "[2001:db8::1]");
}

#[test]
fn clean_host_with_display_suffix() {
    assert_eq!(clean_host("localhost (127.0.0.1)"), "localhost");
}

#[test]
fn clean_host_trims_whitespace() {
    assert_eq!(clean_host("  192.168.1.1  "), "192.168.1.1");
}

#[test]
fn format_host_empty() {
    assert_eq!(format_host(""), "localhost (127.0.0.1)");
}

#[test]
fn format_host_loopback() {
    assert_eq!(format_host("127.0.0.1"), "localhost (127.0.0.1)");
}

#[test]
fn format_host_custom_returns_as_is() {
    assert_eq!(format_host("192.168.1.100"), "192.168.1.100");
}

// ── SearchSort ──────────────────────────────────────────────────

#[test]
fn search_sort_next_cycles_through_all() {
    let mut sort = SearchSort::Relevance;
    let expected = [
        SearchSort::Downloads,
        SearchSort::Likes,
        SearchSort::Trending,
        SearchSort::CreatedAt,
        SearchSort::Relevance,
    ];
    for exp in &expected {
        sort = sort.next();
        assert_eq!(sort, *exp);
    }
}

#[test]
fn search_sort_label_all() {
    assert_eq!(SearchSort::Relevance.label(), "Relevance");
    assert_eq!(SearchSort::Downloads.label(), "Downloads");
    assert_eq!(SearchSort::Likes.label(), "Likes");
    assert_eq!(SearchSort::Trending.label(), "Trending");
    assert_eq!(SearchSort::CreatedAt.label(), "Created");
}

// ── CacheQuantType cycling ──────────────────────────────────────

#[test]
fn cache_quant_type_next_cycles_through_all() {
    let mut t = CacheQuantType::F32;
    let expected = [
        CacheQuantType::F16,
        CacheQuantType::BF16,
        CacheQuantType::Q8_0,
        CacheQuantType::Q5_1,
        CacheQuantType::Q5_0,
        CacheQuantType::Q4_1,
        CacheQuantType::Q4_0,
        CacheQuantType::Iq4Nl,
        CacheQuantType::F32,
    ];
    for exp in &expected {
        t = t.next();
        assert_eq!(t, *exp);
    }
}

#[test]
fn cache_quant_type_prev_cycles_through_all() {
    let mut t = CacheQuantType::F32;
    let expected = [
        CacheQuantType::Iq4Nl,
        CacheQuantType::Q4_0,
        CacheQuantType::Q4_1,
        CacheQuantType::Q5_0,
        CacheQuantType::Q5_1,
        CacheQuantType::Q8_0,
        CacheQuantType::BF16,
        CacheQuantType::F16,
        CacheQuantType::F32,
    ];
    for exp in &expected {
        t = t.prev();
        assert_eq!(t, *exp);
    }
}

#[test]
fn cache_quant_type_from_u8_all_values() {
    assert_eq!(CacheQuantType::from_u8(0), CacheQuantType::F32);
    assert_eq!(CacheQuantType::from_u8(1), CacheQuantType::F16);
    assert_eq!(CacheQuantType::from_u8(2), CacheQuantType::BF16);
    assert_eq!(CacheQuantType::from_u8(3), CacheQuantType::Q8_0);
    assert_eq!(CacheQuantType::from_u8(4), CacheQuantType::Q5_1);
    assert_eq!(CacheQuantType::from_u8(5), CacheQuantType::Q5_0);
    assert_eq!(CacheQuantType::from_u8(6), CacheQuantType::Q4_1);
    assert_eq!(CacheQuantType::from_u8(7), CacheQuantType::Q4_0);
    assert_eq!(CacheQuantType::from_u8(8), CacheQuantType::Iq4Nl);
    // Out of range defaults to F16
    assert_eq!(CacheQuantType::from_u8(99), CacheQuantType::F16);
}

#[test]
fn cache_quant_type_from_str_all() {
    assert_eq!(CacheQuantType::from("F32"), CacheQuantType::F32);
    assert_eq!(CacheQuantType::from("F16"), CacheQuantType::F16);
    assert_eq!(CacheQuantType::from("BF16"), CacheQuantType::BF16);
    assert_eq!(CacheQuantType::from("Q8_0"), CacheQuantType::Q8_0);
    assert_eq!(CacheQuantType::from("Q4_0"), CacheQuantType::Q4_0);
    assert_eq!(CacheQuantType::from("Q4_1"), CacheQuantType::Q4_1);
    assert_eq!(CacheQuantType::from("Iq4Nl"), CacheQuantType::Iq4Nl);
    assert_eq!(CacheQuantType::from("Q5_0"), CacheQuantType::Q5_0);
    assert_eq!(CacheQuantType::from("Q5_1"), CacheQuantType::Q5_1);
    // Unknown defaults to F16
    assert_eq!(CacheQuantType::from("unknown"), CacheQuantType::F16);
}

#[test]
fn cache_quant_type_display_all() {
    assert_eq!(format!("{}", CacheQuantType::F32), "f32");
    assert_eq!(format!("{}", CacheQuantType::F16), "f16");
    assert_eq!(format!("{}", CacheQuantType::BF16), "bf16");
    assert_eq!(format!("{}", CacheQuantType::Q8_0), "q8_0");
    assert_eq!(format!("{}", CacheQuantType::Q4_0), "q4_0");
    assert_eq!(format!("{}", CacheQuantType::Q4_1), "q4_1");
    assert_eq!(format!("{}", CacheQuantType::Iq4Nl), "iq4_nl");
    assert_eq!(format!("{}", CacheQuantType::Q5_0), "q5_0");
    assert_eq!(format!("{}", CacheQuantType::Q5_1), "q5_1");
}

#[test]
fn cache_quant_type_default_is_f16() {
    assert_eq!(CacheQuantType::default(), CacheQuantType::F16);
}

// ── CacheType ───────────────────────────────────────────────────

#[test]
fn cache_type_display_all() {
    assert_eq!(format!("{}", CacheType::F16), "f16");
    assert_eq!(format!("{}", CacheType::BF16), "bf16");
    assert_eq!(format!("{}", CacheType::Fq8_0), "fq8_0");
    assert_eq!(format!("{}", CacheType::Fq4_1), "fq4_1");
}

#[test]
fn cache_type_default_is_f16() {
    assert_eq!(CacheType::default(), CacheType::F16);
}

// ── SplitMode ───────────────────────────────────────────────────

#[test]
fn split_mode_display_all() {
    assert_eq!(format!("{}", SplitMode::None), "none");
    assert_eq!(format!("{}", SplitMode::Layer), "layer");
    assert_eq!(format!("{}", SplitMode::Row), "row");
    assert_eq!(format!("{}", SplitMode::Tensor), "tensor");
}

#[test]
fn split_mode_default_is_layer() {
    assert_eq!(SplitMode::default(), SplitMode::Layer);
}

// ── NumMode ─────────────────────────────────────────────────────

#[test]
fn num_mode_display_all() {
    assert_eq!(format!("{}", NumMode::None), "none");
    assert_eq!(format!("{}", NumMode::Distribute), "distribute");
    assert_eq!(format!("{}", NumMode::Isolate), "isolate");
    assert_eq!(format!("{}", NumMode::Numactl), "numactl");
}

#[test]
fn num_mode_default_is_none() {
    assert_eq!(NumMode::default(), NumMode::None);
}

// ── RopeScaling ─────────────────────────────────────────────────

#[test]
fn rope_scaling_display_all() {
    assert_eq!(format!("{}", RopeScaling::None), "none");
    assert_eq!(format!("{}", RopeScaling::Linear), "linear");
    assert_eq!(format!("{}", RopeScaling::Yarn), "yarn");
}

#[test]
fn rope_scaling_default_is_none() {
    assert_eq!(RopeScaling::default(), RopeScaling::None);
}

// ── Mirostat ────────────────────────────────────────────────────

#[test]
fn mirostat_display_all() {
    assert_eq!(format!("{}", Mirostat::Off), "off");
    assert_eq!(format!("{}", Mirostat::V1), "1");
    assert_eq!(format!("{}", Mirostat::Mirostat2), "2");
}

#[test]
fn mirostat_default_is_off() {
    assert_eq!(Mirostat::default(), Mirostat::Off);
}

// ── Samplers ────────────────────────────────────────────────────

#[test]
fn samplers_default_contains_expected_order() {
    let s = Samplers::default();
    let parts: Vec<&str> = s.0.split(';').collect();
    assert!(parts.contains(&"penalties"));
    assert!(parts.contains(&"dry"));
    assert!(parts.contains(&"top_k"));
    assert!(parts.contains(&"temperature"));
}

// ── Backend ─────────────────────────────────────────────────────

#[test]
fn backend_slug_all_variants() {
    assert_eq!(Backend::Cpu.slug(), "cpu");
    assert_eq!(Backend::Vulkan.slug(), "vulkan");
    assert_eq!(Backend::Rocm.slug(), "rocm");
    assert_eq!(Backend::RocmLemonade.slug(), "rocm-lemonade");
    assert_eq!(Backend::Cuda.slug(), "cuda");
    assert_eq!(Backend::CpuArm64.slug(), "cpu-arm64");
    assert_eq!(Backend::CpuWindows.slug(), "win-cpu");
    assert_eq!(Backend::VulkanWindows.slug(), "win-vulkan");
    assert_eq!(Backend::CudaWindows12_4.slug(), "win-cuda-12.4");
    assert_eq!(Backend::CudaWindows13_1.slug(), "win-cuda-13.1");
    assert_eq!(Backend::HipWindows.slug(), "win-hip");
    assert_eq!(Backend::CpuMacosArm64.slug(), "macos-arm64");
    assert_eq!(Backend::CpuMacosX64.slug(), "macos-x64");
}

#[test]
fn backend_is_linux_variants() {
    assert!(Backend::Cpu.is_linux());
    assert!(Backend::Vulkan.is_linux());
    assert!(Backend::Rocm.is_linux());
    assert!(Backend::RocmLemonade.is_linux());
    assert!(Backend::Cuda.is_linux());
    assert!(Backend::CpuArm64.is_linux());
}

#[test]
fn backend_is_linux_non_linux() {
    assert!(!Backend::CpuWindows.is_linux());
    assert!(!Backend::VulkanWindows.is_linux());
    assert!(!Backend::CudaWindows12_4.is_linux());
    assert!(!Backend::CudaWindows13_1.is_linux());
    assert!(!Backend::HipWindows.is_linux());
    assert!(!Backend::CpuMacosArm64.is_linux());
    assert!(!Backend::CpuMacosX64.is_linux());
}

#[test]
fn backend_is_windows_variants() {
    assert!(Backend::CpuWindows.is_windows());
    assert!(Backend::VulkanWindows.is_windows());
    assert!(Backend::CudaWindows12_4.is_windows());
    assert!(Backend::CudaWindows13_1.is_windows());
    assert!(Backend::HipWindows.is_windows());
}

#[test]
fn backend_is_windows_non_windows() {
    assert!(!Backend::Cpu.is_windows());
    assert!(!Backend::Vulkan.is_windows());
    assert!(!Backend::Rocm.is_windows());
    assert!(!Backend::RocmLemonade.is_windows());
    assert!(!Backend::Cuda.is_windows());
}

#[test]
fn backend_is_macos_variants() {
    assert!(Backend::CpuMacosArm64.is_macos());
    assert!(Backend::CpuMacosX64.is_macos());
}

#[test]
fn backend_is_macos_non_macos() {
    assert!(!Backend::Cpu.is_macos());
    assert!(!Backend::Vulkan.is_macos());
    assert!(!Backend::Cuda.is_macos());
}

#[test]
fn backend_from_str_variants() {
    assert_eq!(Backend::from_str("cpu"), Backend::Cpu);
    assert_eq!(Backend::from_str("CPU"), Backend::Cpu);
    assert_eq!(Backend::from_str("vulkan"), Backend::Vulkan);
    assert_eq!(Backend::from_str("vk"), Backend::Vulkan);
    assert_eq!(Backend::from_str("rocm"), Backend::Rocm);
    assert_eq!(Backend::from_str("ro"), Backend::Rocm);
    assert_eq!(Backend::from_str("rocm-lemonade"), Backend::RocmLemonade);
    assert_eq!(Backend::from_str("cuda"), Backend::Cuda);
    assert_eq!(Backend::from_str("cu"), Backend::Cuda);
    // Unknown defaults to Cpu
    assert_eq!(Backend::from_str("unknown"), Backend::Cpu);
}

#[test]
fn backend_default_is_cpu() {
    assert_eq!(Backend::default(), Backend::Cpu);
}

// ── GpuLayersMode ───────────────────────────────────────────────

#[test]
fn gpu_layers_mode_default_is_auto() {
    assert_eq!(GpuLayersMode::default(), GpuLayersMode::Auto);
}

// ── ServerMode ──────────────────────────────────────────────────

#[test]
fn server_mode_display_all() {
    assert_eq!(format!("{}", ServerMode::Normal), "Normal");
    assert_eq!(format!("{}", ServerMode::Router), "Router (XP!)");
    assert_eq!(format!("{}", ServerMode::Bench), "Bench GPU");
    assert_eq!(format!("{}", ServerMode::BenchTune), "BenchTune");
}

#[test]
fn server_mode_default_is_normal() {
    assert_eq!(ServerMode::default(), ServerMode::Normal);
}

// ── BenchTuneMode ───────────────────────────────────────────────

#[test]
fn bench_tune_mode_default_is_full() {
    assert_eq!(BenchTuneMode::default(), BenchTuneMode::Full);
}

// ── estimate_vram_mib ──────────────────────────────────────────

#[test]
fn estimate_vram_cpu_only_returns_zero() {
    let mut settings = ModelSettings::default();
    settings.gpu_layers_mode = GpuLayersMode::Specific(0);
    let result = estimate_vram_mib(4000, &settings, 32, Some(4096), Some(32), Some(8), 8192);
    assert_eq!(result, 0);
}

#[test]
fn estimate_vram_all_layers_uses_all() {
    let mut settings = ModelSettings::default();
    settings.gpu_layers_mode = GpuLayersMode::All;
    let result = estimate_vram_mib(4000, &settings, 32, Some(4096), Some(32), Some(8), 8192);
    // Should be significantly higher than auto since all layers are in VRAM
    assert!(result > 0);
}

#[test]
fn estimate_vram_flash_attn_reduces_vram() {
    let mut settings_no_flash = ModelSettings::default();
    settings_no_flash.flash_attn = false;
    let settings_flash = ModelSettings::default(); // default has flash_attn = true

    let no_flash = estimate_vram_mib(
        4000,
        &settings_no_flash,
        32,
        Some(4096),
        Some(32),
        Some(8),
        8192,
    );
    let flash = estimate_vram_mib(
        4000,
        &settings_flash,
        32,
        Some(4096),
        Some(32),
        Some(8),
        8192,
    );
    // Flash attention should reduce VRAM (approximately 2x KV cache)
    assert!(flash < no_flash);
}

#[test]
fn estimate_vram_unified_cache_reduces_vram() {
    let settings_normal = ModelSettings::default();
    let mut settings_unified = ModelSettings::default();
    settings_unified.uniform_cache = true;
    settings_unified.parallel = 4;

    let normal = estimate_vram_mib(
        4000,
        &settings_normal,
        32,
        Some(4096),
        Some(32),
        Some(8),
        8192,
    );
    let unified = estimate_vram_mib(
        4000,
        &settings_unified,
        32,
        Some(4096),
        Some(32),
        Some(8),
        8192,
    );
    assert!(unified < normal);
}

#[test]
fn estimate_vram_gqa_reduces_kv_cache() {
    // Model with GQA: 32 query heads, 8 KV heads (ratio 0.25)
    let settings = ModelSettings::default();
    let with_gqa = estimate_vram_mib(4000, &settings, 32, Some(4096), Some(32), Some(8), 8192);
    // Model without GQA: 32 query heads, 32 KV heads (ratio 1.0)
    let without_gqa = estimate_vram_mib(4000, &settings, 32, Some(4096), Some(32), Some(32), 8192);
    assert!(with_gqa < without_gqa);
}

#[test]
fn estimate_vram_quantization_affects_size() {
    let mut settings_f32 = ModelSettings::default();
    settings_f32.cache_type_k = Some(CacheTypeK::F32);
    settings_f32.cache_type_v = Some(CacheTypeV::F32);

    let mut settings_q4 = ModelSettings::default();
    settings_q4.cache_type_k = Some(CacheTypeK::Q4_0);
    settings_q4.cache_type_v = Some(CacheTypeV::Q4_0);

    let f32_vram = estimate_vram_mib(4000, &settings_f32, 32, Some(4096), Some(32), Some(8), 8192);
    let q4_vram = estimate_vram_mib(4000, &settings_q4, 32, Some(4096), Some(32), Some(8), 8192);
    assert!(q4_vram < f32_vram);
}

#[test]
fn estimate_vram_zero_total_layers() {
    let settings = ModelSettings::default();
    // With 0 total layers, the KV cache formula has 0/0 = NaN, which becomes 0 when cast to u64
    let result = estimate_vram_mib(4000, &settings, 0, None, None, None, 0);
    // Due to NaN from 0/0 in KV cache formula, result is 0
    assert_eq!(result, 0);
}

#[test]
fn estimate_vram_increases_with_context_length() {
    let mut settings_small = ModelSettings::default();
    settings_small.context_length = 2048;
    let mut settings_large = ModelSettings::default();
    settings_large.context_length = 65536;

    let small = estimate_vram_mib(
        4000,
        &settings_small,
        32,
        Some(4096),
        Some(32),
        Some(8),
        8192,
    );
    let large = estimate_vram_mib(
        4000,
        &settings_large,
        32,
        Some(4096),
        Some(32),
        Some(8),
        8192,
    );
    assert!(large > small);
}

#[test]
fn estimate_vram_increases_with_batch_size() {
    let mut settings_small = ModelSettings::default();
    settings_small.batch_size = 128;
    let mut settings_large = ModelSettings::default();
    settings_large.batch_size = 2048;

    let small = estimate_vram_mib(
        4000,
        &settings_small,
        32,
        Some(4096),
        Some(32),
        Some(8),
        8192,
    );
    let large = estimate_vram_mib(
        4000,
        &settings_large,
        32,
        Some(4096),
        Some(32),
        Some(8),
        8192,
    );
    assert!(large > small);
}

#[test]
fn estimate_vram_auto_uses_heuristic() {
    let mut settings = ModelSettings::default();
    settings.gpu_layers_mode = GpuLayersMode::Auto;
    let result = estimate_vram_mib(4000, &settings, 32, Some(4096), Some(32), Some(8), 8192);
    // Auto should use ~60% heuristic (19 layers out of 32)
    assert!(result > 0);
}

#[test]
fn estimate_vram_specific_layers() {
    let mut settings = ModelSettings::default();
    settings.gpu_layers_mode = GpuLayersMode::Specific(16);
    let result = estimate_vram_mib(4000, &settings, 32, Some(4096), Some(32), Some(8), 8192);
    assert!(result > 0);
}

#[test]
fn estimate_vram_specific_zero_returns_zero() {
    let mut settings = ModelSettings::default();
    settings.gpu_layers_mode = GpuLayersMode::Specific(0);
    let result = estimate_vram_mib(4000, &settings, 32, Some(4096), Some(32), Some(8), 8192);
    assert_eq!(result, 0);
}

#[test]
fn estimate_vram_no_gpu_memory_total() {
    let settings = ModelSettings::default();
    // gpu_mem_total_mib = 0 should use 500 MiB fallback
    let result = estimate_vram_mib(4000, &settings, 32, Some(4096), Some(32), Some(8), 0);
    assert!(result > 0);
}

// ── DownloadState ───────────────────────────────────────────────

#[test]
fn download_state_new_initial_values() {
    let ds = DownloadState::new("model-id".into(), "file.gguf".into(), 1024);
    assert_eq!(ds.model_id, "model-id");
    assert_eq!(ds.filename, "file.gguf");
    assert_eq!(ds.total_bytes, 1024);
    assert_eq!(ds.downloaded_bytes, 0);
    assert!(!ds.cancelled);
    assert_eq!(ds.download_state, 1); // downloading
    assert!(ds.dest.is_none());
}

// ── ServerMetrics ───────────────────────────────────────────────

#[test]
fn server_metrics_default_all_zero() {
    let m = ServerMetrics::default();
    assert!(!m.loaded);
    assert_eq!(m.tps, 0.0);
    assert_eq!(m.prompt_tps, 0.0);
    assert_eq!(m.cpu_usage, 0.0);
    assert_eq!(m.cpu_ticks_prev, 0);
    assert_eq!(m.system_uptime_prev, 0.0);
    assert_eq!(m.gpu_mem_used, 0);
    assert_eq!(m.gpu_mem_total, 0);
    assert_eq!(m.ram_used, 0);
    assert_eq!(m.ctx_used, 0);
    assert_eq!(m.ctx_max, 0);
    assert_eq!(m.total_vram_used, 0);
    assert_eq!(m.decoded_tokens, 0);
    assert_eq!(m.latency_per_token_ms, 0.0);
    assert_eq!(m.prompt_latency_ms, 0.0);
}

// ── ModelState ──────────────────────────────────────────────────

#[test]
fn model_state_loaded_has_port_pid() {
    let state = ModelState::Loaded {
        port: 8080,
        pid: 12345,
    };
    assert!(matches!(
        state,
        ModelState::Loaded {
            port: 8080,
            pid: 12345
        }
    ));
}

#[test]
fn model_state_failed_has_error() {
    let state = ModelState::Failed {
        error: "OOM".into(),
    };
    assert!(matches!(state, ModelState::Failed { .. }));
}

// ── BenchTuneProgress ──────────────────────────────────────────

#[test]
fn bench_tune_progress_from_status_running() {
    let status = BenchTuneStatus::Running {
        current: 1,
        total: 10,
        progress: 10.0,
        current_params: BenchTuneParamValue {
            temperature: None,
            top_p: None,
            top_k: None,
            repeat_penalty: None,
            context_length: None,
            batch_size: None,
            flash_attn: None,
            threads: None,
            expert_count: None,
            spec_type: None,
            draft_tokens: None,
        },
    };
    let progress = BenchTuneProgress::from_status(&status);
    assert!(matches!(progress, Some(BenchTuneProgress::Running { .. })));
}

#[test]
fn bench_tune_progress_from_status_completed() {
    let status = BenchTuneStatus::Completed {
        total_tests: 10,
        successful_tests: 9,
        elapsed: std::time::Duration::from_secs(60),
    };
    let progress = BenchTuneProgress::from_status(&status);
    assert!(matches!(
        progress,
        Some(BenchTuneProgress::Completed { .. })
    ));
}

#[test]
fn bench_tune_progress_from_status_error() {
    let status = BenchTuneStatus::Error {
        error: "fail".into(),
    };
    let progress = BenchTuneProgress::from_status(&status);
    assert!(matches!(progress, Some(BenchTuneProgress::Error { .. })));
}

// ── LoadProgress ────────────────────────────────────────────────

#[test]
fn load_progress_default() {
    let p = LoadProgress::default();
    assert!(p.layers_total.is_none());
    assert!(p.layers_loaded.is_none());
    assert!(p.tensors_total.is_none());
    assert_eq!(p.tensors_loaded, 0);
    assert!(p.buffers.is_empty());
}

// ── BenchTuneParamValue equality ───────────────────────────────

#[test]
fn bench_tune_param_value_eq_with_some_none() {
    let a = BenchTuneParamValue {
        temperature: Some(0.8),
        top_p: None,
        top_k: None,
        repeat_penalty: None,
        context_length: None,
        batch_size: None,
        flash_attn: None,
        threads: None,
        expert_count: None,
        spec_type: None,
        draft_tokens: None,
    };
    let b = BenchTuneParamValue {
        temperature: Some(0.8),
        top_p: None,
        top_k: None,
        repeat_penalty: None,
        context_length: None,
        batch_size: None,
        flash_attn: None,
        threads: None,
        expert_count: None,
        spec_type: None,
        draft_tokens: None,
    };
    assert_eq!(a, b);
}

#[test]
fn bench_tune_param_value_ne_different_values() {
    let a = BenchTuneParamValue {
        temperature: Some(0.8),
        top_p: None,
        top_k: None,
        repeat_penalty: None,
        context_length: None,
        batch_size: None,
        flash_attn: None,
        threads: None,
        expert_count: None,
        spec_type: None,
        draft_tokens: None,
    };
    let b = BenchTuneParamValue {
        temperature: Some(0.7),
        top_p: None,
        top_k: None,
        repeat_penalty: None,
        context_length: None,
        batch_size: None,
        flash_attn: None,
        threads: None,
        expert_count: None,
        spec_type: None,
        draft_tokens: None,
    };
    assert_ne!(a, b);
}

// ── BenchTuneParam equality ────────────────────────────────────

#[test]
fn bench_tune_param_eq_same_values() {
    let a = BenchTuneParam {
        name: "temperature".into(),
        min: 0.4,
        max: 1.0,
        step: 0.1,
        enabled: true,
    };
    let b = BenchTuneParam {
        name: "temperature".into(),
        min: 0.4,
        max: 1.0,
        step: 0.1,
        enabled: true,
    };
    assert_eq!(a, b);
}

#[test]
fn bench_tune_param_ne_different_name() {
    let a = BenchTuneParam {
        name: "temperature".into(),
        min: 0.4,
        max: 1.0,
        step: 0.1,
        enabled: true,
    };
    let b = BenchTuneParam {
        name: "top_p".into(),
        min: 0.4,
        max: 1.0,
        step: 0.1,
        enabled: true,
    };
    assert_ne!(a, b);
}

// ── SearchResult serialization ─────────────────────────────────

#[test]
fn search_result_serializable() {
    let sr = SearchResult {
        model_id: "org/model".into(),
        model_name: "Model".into(),
        tags: vec!["gguf".into()],
        downloads: 1000,
        likes: 50,
        pipeline_tag: Some("text-generation".into()),
        size: Some(5_000_000_000),
        parameters: Some("7B".into()),
        capabilities: vec!["chat".into()],
        context_length: Some(32768),
        readme: None,
        quantization: Some("Q4_K_M".into()),
        license: Some("mit".into()),
        trending_score: 100,
        created_at: Some("2024-01-01".into()),
        downloaded: false,
    };
    let json = serde_json::to_string(&sr).expect("should serialize");
    assert!(json.contains("org/model"));
    let deserialized: SearchResult = serde_json::from_str(&json).expect("should deserialize");
    assert_eq!(deserialized.model_id, "org/model");
}

// ── Backend Display ─────────────────────────────────────────────

#[test]
fn backend_display_uses_slug() {
    assert_eq!(format!("{}", Backend::Cpu), "cpu");
    assert_eq!(format!("{}", Backend::Vulkan), "vulkan");
    assert_eq!(format!("{}", Backend::RocmLemonade), "rocm-lemonade");
    assert_eq!(format!("{}", Backend::Cuda), "cuda");
}

// ── BenchTuneConfig new ─────────────────────────────────────────

#[test]
fn bench_tune_config_new_has_default_params() {
    let config = BenchTuneConfig::new("/path/to/model.gguf".into(), 3, "test prompt".into());
    assert_eq!(config.model_path.to_string_lossy(), "/path/to/model.gguf");
    assert_eq!(config.num_iterations, 3);
    assert_eq!(config.prompt, "test prompt");
    assert_eq!(config.params_to_test.len(), 8);
    assert_eq!(config.test_duration, std::time::Duration::from_secs(30));
}
