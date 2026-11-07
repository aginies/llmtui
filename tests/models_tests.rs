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
    assert_eq!(
        llm_manager::tui::search_sort_label(SearchSort::Relevance),
        "Relevance"
    );
    assert_eq!(
        llm_manager::tui::search_sort_label(SearchSort::Downloads),
        "Downloads"
    );
    assert_eq!(
        llm_manager::tui::search_sort_label(SearchSort::Likes),
        "Likes"
    );
    assert_eq!(
        llm_manager::tui::search_sort_label(SearchSort::Trending),
        "Trending"
    );
    assert_eq!(
        llm_manager::tui::search_sort_label(SearchSort::CreatedAt),
        "Created"
    );
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
        CacheQuantType::Turbo2,
        CacheQuantType::Turbo3,
        CacheQuantType::Turbo4,
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
        CacheQuantType::Turbo4,
        CacheQuantType::Turbo3,
        CacheQuantType::Turbo2,
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
    assert_eq!(CacheQuantType::from_u8(9), CacheQuantType::Turbo2);
    assert_eq!(CacheQuantType::from_u8(10), CacheQuantType::Turbo3);
    assert_eq!(CacheQuantType::from_u8(11), CacheQuantType::Turbo4);
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
    assert_eq!(CacheQuantType::from("Turbo2"), CacheQuantType::Turbo2);
    assert_eq!(CacheQuantType::from("turbo2"), CacheQuantType::Turbo2);
    assert_eq!(CacheQuantType::from("Turbo3"), CacheQuantType::Turbo3);
    assert_eq!(CacheQuantType::from("turbo3"), CacheQuantType::Turbo3);
    assert_eq!(CacheQuantType::from("Turbo4"), CacheQuantType::Turbo4);
    assert_eq!(CacheQuantType::from("turbo4"), CacheQuantType::Turbo4);
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
    assert_eq!(format!("{}", CacheQuantType::Turbo2), "turbo2");
    assert_eq!(format!("{}", CacheQuantType::Turbo3), "turbo3");
    assert_eq!(format!("{}", CacheQuantType::Turbo4), "turbo4");
}

#[test]
fn cache_quant_type_default_is_f16() {
    assert_eq!(CacheQuantType::default(), CacheQuantType::F16);
}

// ── CacheQuantType context-aware turbo cycling ────────────────

#[test]
fn cache_quant_type_next_with_turbo_enabled_includes_turbo() {
    let mut t = CacheQuantType::Iq4Nl;
    assert_eq!(t.next_with_turbo(true), CacheQuantType::Turbo2);
    t = CacheQuantType::Turbo4;
    assert_eq!(t.next_with_turbo(true), CacheQuantType::F32);
}

#[test]
fn cache_quant_type_next_with_turbo_disabled_skips_turbo() {
    let t = CacheQuantType::Iq4Nl;
    assert_eq!(t.next_with_turbo(false), CacheQuantType::F32);
    // Any stored turbo value behaves as adjacent to the Iq4Nl/F32 boundary.
    for turbo in [
        CacheQuantType::Turbo2,
        CacheQuantType::Turbo3,
        CacheQuantType::Turbo4,
    ] {
        assert_eq!(turbo.next_with_turbo(false), CacheQuantType::F32);
    }
    // Non-turbo values cycle normally.
    assert_eq!(
        CacheQuantType::Q4_0.next_with_turbo(false),
        CacheQuantType::Iq4Nl
    );
}

#[test]
fn cache_quant_type_prev_with_turbo_disabled_skips_turbo() {
    let t = CacheQuantType::F32;
    assert_eq!(t.prev_with_turbo(false), CacheQuantType::Iq4Nl);
    for turbo in [
        CacheQuantType::Turbo2,
        CacheQuantType::Turbo3,
        CacheQuantType::Turbo4,
    ] {
        assert_eq!(turbo.prev_with_turbo(false), CacheQuantType::Iq4Nl);
    }
    assert_eq!(
        CacheQuantType::Iq4Nl.prev_with_turbo(false),
        CacheQuantType::Q4_0
    );
}

#[test]
fn cache_quant_type_with_turbo_matches_base_when_enabled() {
    for t in [
        CacheQuantType::F32,
        CacheQuantType::F16,
        CacheQuantType::BF16,
        CacheQuantType::Q8_0,
        CacheQuantType::Q5_1,
        CacheQuantType::Q5_0,
        CacheQuantType::Q4_1,
        CacheQuantType::Q4_0,
        CacheQuantType::Iq4Nl,
        CacheQuantType::Turbo2,
        CacheQuantType::Turbo3,
        CacheQuantType::Turbo4,
    ] {
        assert_eq!(t.next_with_turbo(true), t.next());
        assert_eq!(t.prev_with_turbo(true), t.prev());
    }
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
fn split_mode_default_is_none() {
    assert_eq!(SplitMode::default(), SplitMode::None);
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
    assert!(parts.contains(&"top_k"));
    assert!(parts.contains(&"top_p"));
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
    assert_eq!(Backend::parse_backend("cpu"), Backend::Cpu);
    assert_eq!(Backend::parse_backend("CPU"), Backend::Cpu);
    assert_eq!(Backend::parse_backend("vulkan"), Backend::Vulkan);
    assert_eq!(Backend::parse_backend("vk"), Backend::Vulkan);
    assert_eq!(Backend::parse_backend("rocm"), Backend::Rocm);
    assert_eq!(Backend::parse_backend("ro"), Backend::Rocm);
    assert_eq!(
        Backend::parse_backend("rocm-lemonade"),
        Backend::RocmLemonade
    );
    assert_eq!(Backend::parse_backend("cuda"), Backend::Cuda);
    assert_eq!(Backend::parse_backend("cu"), Backend::Cuda);
    // Unknown defaults to Cpu
    assert_eq!(Backend::parse_backend("unknown"), Backend::Cpu);
}

#[test]
fn backend_slug_from_slug_roundtrip() {
    // Every variant must round-trip through slug() -> from_slug().
    let all = [
        Backend::Cpu,
        Backend::Vulkan,
        Backend::Rocm,
        Backend::RocmLemonade,
        Backend::Cuda,
        Backend::CpuArm64,
        Backend::CpuWindows,
        Backend::VulkanWindows,
        Backend::CudaWindows12_4,
        Backend::CudaWindows13_1,
        Backend::HipWindows,
        Backend::CpuMacosArm64,
        Backend::CpuMacosX64,
    ];
    for b in all {
        assert_eq!(
            Backend::from_slug(b.slug()),
            Some(b),
            "roundtrip failed for slug {}",
            b.slug()
        );
    }
    assert_eq!(Backend::from_slug("nonexistent"), None);
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

// ── estimate_vram_mib ──────────────────────────────────────────

#[test]
fn estimate_vram_cpu_only_returns_zero() {
    let mut settings = ModelSettings::default();
    settings.gpu_layers_mode = GpuLayersMode::Specific(0);
    let result = estimate_vram_mib(
        4000,
        &settings,
        32,
        Some(4096),
        Some(32),
        Some(8),
        8192,
        &ArchVramInfo::default(),
    );
    assert_eq!(result, 0);
}

#[test]
fn estimate_vram_all_layers_uses_all() {
    let mut settings = ModelSettings::default();
    settings.gpu_layers_mode = GpuLayersMode::All;
    let result = estimate_vram_mib(
        4000,
        &settings,
        32,
        Some(4096),
        Some(32),
        Some(8),
        8192,
        &ArchVramInfo::default(),
    );
    // Should be significantly higher than auto since all layers are in VRAM
    assert!(result > 0);
}

#[test]
fn estimate_vram_flash_attn_no_effect() {
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
        &ArchVramInfo::default(),
    );
    let flash = estimate_vram_mib(
        4000,
        &settings_flash,
        32,
        Some(4096),
        Some(32),
        Some(8),
        8192,
        &ArchVramInfo::default(),
    );
    // Flash attention is a compute optimization; it does not change KV cache size.
    assert_eq!(flash, no_flash);
}

#[test]
fn estimate_vram_unified_cache_reduces_vram() {
    let mut settings_normal = ModelSettings::default();
    settings_normal.gpu_layers_mode = GpuLayersMode::All;
    settings_normal.context_length = 8192;
    settings_normal.max_concurrent_predictions = Some(4);
    let mut settings_unified = ModelSettings::default();
    settings_unified.gpu_layers_mode = GpuLayersMode::All;
    settings_unified.context_length = 8192;
    settings_unified.max_concurrent_predictions = Some(4);
    settings_unified.uniform_cache = true;

    let normal = estimate_vram_mib(
        4000,
        &settings_normal,
        32,
        Some(4096),
        Some(32),
        Some(8),
        8192,
        &ArchVramInfo::default(),
    );
    let unified = estimate_vram_mib(
        4000,
        &settings_unified,
        32,
        Some(4096),
        Some(32),
        Some(8),
        8192,
        &ArchVramInfo::default(),
    );
    // Unified cache shares one buffer across all slots instead of one per slot.
    assert!(unified < normal);
}

#[test]
fn estimate_vram_gqa_reduces_kv_cache() {
    // Model with GQA: 32 query heads, 8 KV heads (ratio 0.25)
    let settings = ModelSettings::default();
    let with_gqa = estimate_vram_mib(
        4000,
        &settings,
        32,
        Some(4096),
        Some(32),
        Some(8),
        8192,
        &ArchVramInfo::default(),
    );
    // Model without GQA: 32 query heads, 32 KV heads (ratio 1.0)
    let without_gqa = estimate_vram_mib(
        4000,
        &settings,
        32,
        Some(4096),
        Some(32),
        Some(32),
        8192,
        &ArchVramInfo::default(),
    );
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

    let f32_vram = estimate_vram_mib(
        4000,
        &settings_f32,
        32,
        Some(4096),
        Some(32),
        Some(8),
        8192,
        &ArchVramInfo::default(),
    );
    let q4_vram = estimate_vram_mib(
        4000,
        &settings_q4,
        32,
        Some(4096),
        Some(32),
        Some(8),
        8192,
        &ArchVramInfo::default(),
    );
    assert!(q4_vram < f32_vram);
}

#[test]
fn estimate_vram_zero_total_layers() {
    let settings = ModelSettings::default();
    // No layer metadata: the estimate can't be computed and returns 0.
    let result = estimate_vram_mib(
        4000,
        &settings,
        0,
        None,
        None,
        None,
        0,
        &ArchVramInfo::default(),
    );
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
        &ArchVramInfo::default(),
    );
    let large = estimate_vram_mib(
        4000,
        &settings_large,
        32,
        Some(4096),
        Some(32),
        Some(8),
        8192,
        &ArchVramInfo::default(),
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
        &ArchVramInfo::default(),
    );
    let large = estimate_vram_mib(
        4000,
        &settings_large,
        32,
        Some(4096),
        Some(32),
        Some(8),
        8192,
        &ArchVramInfo::default(),
    );
    assert!(large > small);
}

#[test]
fn estimate_vram_auto_uses_fit_heuristic() {
    let mut settings = ModelSettings::default();
    settings.gpu_layers_mode = GpuLayersMode::Auto;
    let result = estimate_vram_mib(
        4000,
        &settings,
        32,
        Some(4096),
        Some(32),
        Some(8),
        8192,
        &ArchVramInfo::default(),
    );
    // Auto solves the layer count that fits in the GPU; result must be positive.
    assert!(result > 0);
}

#[test]
fn estimate_vram_auto_fits_all_layers() {
    let mut settings = ModelSettings::default();
    settings.gpu_layers_mode = GpuLayersMode::Auto;
    settings.context_length = 4096;
    // Pin batch size so the estimate is independent of the global default.
    settings.batch_size = 512;
    // Small model on a big GPU: everything fits, so all 32 layers are offloaded.
    // Weights 1000 + KV 32*4 + activation 4 + overhead 300 = 1432 MiB.
    let result = estimate_vram_mib(
        1000,
        &settings,
        32,
        Some(1024),
        Some(32),
        Some(8),
        8192,
        &ArchVramInfo::default(),
    );
    assert_eq!(result, 1432);
}

#[test]
fn estimate_vram_auto_partial_fit() {
    let mut settings_auto = ModelSettings::default();
    settings_auto.gpu_layers_mode = GpuLayersMode::Auto;
    settings_auto.context_length = 8192;
    let mut settings_all = ModelSettings::default();
    settings_all.context_length = 8192;
    settings_all.gpu_layers_mode = GpuLayersMode::All;

    // 8 GiB model on an 8 GiB GPU with 1 GiB KV: not all layers fit.
    let auto = estimate_vram_mib(
        8000,
        &settings_auto,
        32,
        Some(4096),
        Some(32),
        Some(8),
        8192,
        &ArchVramInfo::default(),
    );
    let all = estimate_vram_mib(
        8000,
        &settings_all,
        32,
        Some(4096),
        Some(32),
        Some(8),
        8192,
        &ArchVramInfo::default(),
    );
    assert!(auto < all);
}

#[test]
fn estimate_vram_kv_offload_full_cache() {
    let mut settings_off = ModelSettings::default();
    settings_off.kv_cache_offload = false;
    settings_off.gpu_layers_mode = GpuLayersMode::Specific(16);
    settings_off.context_length = 8192;
    let mut settings_on = ModelSettings::default();
    settings_on.kv_cache_offload = true;
    settings_on.gpu_layers_mode = GpuLayersMode::Specific(16);
    settings_on.context_length = 8192;

    let off = estimate_vram_mib(
        4000,
        &settings_off,
        32,
        Some(4096),
        Some(32),
        Some(8),
        8192,
        &ArchVramInfo::default(),
    );
    let on = estimate_vram_mib(
        4000,
        &settings_on,
        32,
        Some(4096),
        Some(32),
        Some(8),
        8192,
        &ArchVramInfo::default(),
    );
    // Offload puts the KV of CPU-resident layers on the GPU as well.
    assert!(on > off);
    // Difference is exactly the KV of the 16 non-offloaded layers: 16 * 32 MiB.
    assert_eq!(on - off, 16 * 32);
}

#[test]
fn estimate_vram_parallel_multiplier() {
    let mut settings_single = ModelSettings::default();
    settings_single.gpu_layers_mode = GpuLayersMode::All;
    settings_single.context_length = 8192;
    let mut settings_parallel = ModelSettings::default();
    settings_parallel.gpu_layers_mode = GpuLayersMode::All;
    settings_parallel.context_length = 8192;
    settings_parallel.max_concurrent_predictions = Some(4);

    let single = estimate_vram_mib(
        4000,
        &settings_single,
        32,
        Some(4096),
        Some(32),
        Some(8),
        8192,
        &ArchVramInfo::default(),
    );
    let parallel = estimate_vram_mib(
        4000,
        &settings_parallel,
        32,
        Some(4096),
        Some(32),
        Some(8),
        8192,
        &ArchVramInfo::default(),
    );
    // Each slot gets its own full context: KV scales with the parallel count.
    assert_eq!(parallel - single, 3 * 32 * 32);
}

#[test]
fn estimate_vram_specific_layers() {
    let mut settings = ModelSettings::default();
    settings.gpu_layers_mode = GpuLayersMode::Specific(16);
    let result = estimate_vram_mib(
        4000,
        &settings,
        32,
        Some(4096),
        Some(32),
        Some(8),
        8192,
        &ArchVramInfo::default(),
    );
    assert!(result > 0);
}

#[test]
fn estimate_vram_specific_zero_returns_zero() {
    let mut settings = ModelSettings::default();
    settings.gpu_layers_mode = GpuLayersMode::Specific(0);
    let result = estimate_vram_mib(
        4000,
        &settings,
        32,
        Some(4096),
        Some(32),
        Some(8),
        8192,
        &ArchVramInfo::default(),
    );
    assert_eq!(result, 0);
}

#[test]
fn estimate_vram_no_gpu_memory_total() {
    let mut settings = ModelSettings::default();
    settings.gpu_layers_mode = GpuLayersMode::Auto;
    // gpu_mem_total_mib = 0: Auto falls back to the 60% layer heuristic
    let result = estimate_vram_mib(
        4000,
        &settings,
        32,
        Some(4096),
        Some(32),
        Some(8),
        0,
        &ArchVramInfo::default(),
    );
    assert!(result > 0);
}

// ── estimate_vram_mib: hybrid (linear attention) models ─────────

fn hybrid_settings() -> ModelSettings {
    let mut settings = ModelSettings::default();
    settings.gpu_layers_mode = GpuLayersMode::All;
    settings.context_length = 8192;
    settings
}

#[test]
fn estimate_vram_hybrid_kv_only_full_attention_layers() {
    // 64 layers, full attention every 4th layer -> KV cache for 16 layers.
    let settings = hybrid_settings();
    let dense = estimate_vram_mib(
        4000,
        &settings,
        64,
        Some(4096),
        Some(32),
        Some(8),
        8192,
        &ArchVramInfo::default(),
    );
    let hybrid = estimate_vram_mib(
        4000,
        &settings,
        64,
        Some(4096),
        Some(32),
        Some(8),
        8192,
        &ArchVramInfo {
            full_attention_interval: 4,
            ..Default::default()
        },
    );
    // KV per layer is 32 MiB; 48 layers fewer in the hybrid model.
    assert_eq!(dense - hybrid, 48 * 32);
}

#[test]
fn estimate_vram_hybrid_explicit_head_dim() {
    // Expanded heads: head_dim (256) != hidden / n_head (5120 / 24 ~= 213).
    let settings = hybrid_settings();
    let derived = estimate_vram_mib(
        4000,
        &settings,
        32,
        Some(5120),
        Some(24),
        Some(4),
        8192,
        &ArchVramInfo::default(),
    );
    let explicit = estimate_vram_mib(
        4000,
        &settings,
        32,
        Some(5120),
        Some(24),
        Some(4),
        8192,
        &ArchVramInfo {
            head_dim: 256,
            ..Default::default()
        },
    );
    // n_embd_kv: 4 * 256 = 1024 > 5120 * 4 / 24 ~= 853
    assert!(explicit > derived);
}

#[test]
fn estimate_vram_hybrid_ssm_state() {
    // SSM recurrent state is fixed-size per layer and per slot.
    let mut settings = hybrid_settings();
    // Unified cache so the KV term stays constant while slots change.
    settings.uniform_cache = true;
    let arch = ArchVramInfo {
        full_attention_interval: 4,
        ssm_inner: 6144,
        ssm_state: 128,
        ssm_conv: 4,
        ..Default::default()
    };
    let single = estimate_vram_mib(
        4000,
        &settings,
        64,
        Some(4096),
        Some(32),
        Some(8),
        8192,
        &arch,
    );

    settings.max_concurrent_predictions = Some(4);
    let multi = estimate_vram_mib(
        4000,
        &settings,
        64,
        Some(4096),
        Some(32),
        Some(8),
        8192,
        &arch,
    );

    // 48 SSM layers * 6144 * 132 * 4 bytes ~= 148.5 MiB per slot.
    assert!(multi > single);
    assert!((444..448).contains(&(multi - single)));
}

#[test]
fn estimate_vram_hybrid_mtp_draft_context() {
    // MTP speculative decoding adds the nextn-layer KV cache plus a compute
    // buffer that scales with the draft-token count.
    let mut settings = hybrid_settings();
    let arch = ArchVramInfo {
        full_attention_interval: 4,
        nextn_layers: 1,
        ..Default::default()
    };
    let without_spec = estimate_vram_mib(
        4000,
        &settings,
        64,
        Some(4096),
        Some(32),
        Some(8),
        8192,
        &arch,
    );

    settings.spec_type = "draft-mtp".to_string();
    settings.draft_tokens = 2;
    let with_spec = estimate_vram_mib(
        4000,
        &settings,
        64,
        Some(4096),
        Some(32),
        Some(8),
        8192,
        &arch,
    );

    // MTP cost at draft=2 (calibrated per hidden unit, hidden=4096, nextn=1):
    // 4096 * (0.2 base + 0.12*2 per-draft) = 1802 MiB.
    let delta = with_spec - without_spec;
    assert!((1801..=1804).contains(&delta), "unexpected MTP delta {delta}");

    // Each additional draft token costs more VRAM.
    settings.draft_tokens = 4;
    let with_spec4 = estimate_vram_mib(
        4000,
        &settings,
        64,
        Some(4096),
        Some(32),
        Some(8),
        8192,
        &arch,
    );
    assert!(
        with_spec4 > with_spec,
        "draft=4 ({with_spec4}) must cost more than draft=2 ({with_spec})"
    );
}

#[test]
fn estimate_vram_hybrid_qwen35_scenario() {
    // Regression: Qwen3.8-27B (qwen35 arch) on a 32 GB GPU.
    // 65 layers (64 + 1 MTP), full attention every 4th layer, Gated
    // DeltaNet SSM layers, expanded heads (key_length 256), 192K ctx,
    // MTP speculative decoding. Real measured usage: ~30.9 GB.
    let mut settings = ModelSettings::default();
    settings.gpu_layers_mode = GpuLayersMode::All;
    settings.context_length = 196560;
    settings.batch_size = 512;
    settings.uniform_cache = true;
    settings.kv_cache_offload = true;
    settings.spec_type = "draft-mtp".to_string();
    settings.draft_tokens = 2;

    let arch = ArchVramInfo {
        full_attention_interval: 4,
        head_dim: 256,
        ssm_inner: 6144,
        ssm_state: 128,
        ssm_conv: 4,
        nextn_layers: 1,
    };

    // 16.34 GiB file, 32607 MiB GPU.
    let result = estimate_vram_mib(
        16732,
        &settings,
        65,
        Some(5120),
        Some(24),
        Some(4),
        32607,
        &arch,
    );

    // Weights 16.3 GB + KV 12.3 GB (16 layers) + SSM 0.4 GB + MTP 1.0 GB
    // + overhead ~= 30.8 GB. Must be far below the old dense estimate
    // of ~57.3 GB.
    assert!((29_000..33_000).contains(&result), "got {result} MiB");
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
        context_length: None,
        batch_size: None,
        flash_attn: Some(true),
        threads: None,
        expert_count: None,
        spec_type: None,
        draft_tokens: None,
    };
    let b = BenchTuneParamValue {
        context_length: None,
        batch_size: None,
        flash_attn: Some(true),
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
        context_length: None,
        batch_size: None,
        flash_attn: None,
        threads: Some(8),
        expert_count: None,
        spec_type: None,
        draft_tokens: None,
    };
    let b = BenchTuneParamValue {
        context_length: None,
        batch_size: None,
        flash_attn: None,
        threads: Some(16),
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
        variants: vec![],
    };
    let b = BenchTuneParam {
        name: "temperature".into(),
        min: 0.4,
        max: 1.0,
        step: 0.1,
        enabled: true,
        variants: vec![],
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
        variants: vec![],
    };
    let b = BenchTuneParam {
        name: "top_p".into(),
        min: 0.4,
        max: 1.0,
        step: 0.1,
        enabled: true,
        variants: vec![],
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
    assert_eq!(config.params_to_test.len(), 5);
    assert_eq!(config.test_duration, std::time::Duration::from_secs(30));
}

// ── WsMetrics from_metrics ──────────────────────────────────────

#[test]
fn test_ws_metrics_from_metrics_prompt_tokens() {
    let metrics = ServerMetrics {
        loaded: true,
        tps: 25.5,
        prompt_tps: 100.0,
        cpu_usage: 45.0,
        gpu_mem_used: 8_000,
        gpu_mem_total: 16_000,
        ram_used: 16_000,
        ctx_used: 128,
        ctx_max: 32768,
        total_vram_used: 8_000,
        decoded_tokens: 0,
        gen_tps: 0.0,
        latency_per_token_ms: 0.0,
        prompt_latency_ms: 0.0,
        prompt_tokens: 512,   // Actual prompt tokens
        prompt_progress: 0.5, // 50% evaluated
        prompt_elapsed_ms: 120.0,
        prompt_tps_eval: 200.0,
        busy: false,
    };
    let settings = ModelSettings::default();
    let ws_metrics = WsMetrics::from_metrics(&metrics, "test-model", "loaded", &settings, None);

    // Verify that the actual prompt_tokens (512) is preserved and not overridden by 0 (ctx_used * progress fallback).
    assert_eq!(ws_metrics.prompt_tokens, 512);
    assert_eq!(ws_metrics.prompt_progress, 0.5);
    assert_eq!(ws_metrics.prompt_elapsed_ms, 120.0);
    assert_eq!(ws_metrics.prompt_tps_eval, 200.0);
}

// ── GgufMetadata from_path tests ────────────────────────────────

#[test]
fn test_gguf_metadata_nonexistent_file() {
    let result = GgufMetadata::from_path(std::path::Path::new("nonexistent_file_12345.gguf"));
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("cannot open"));
}

#[test]
fn test_gguf_metadata_invalid_file_no_panic() {
    // Create a temporary invalid file to test parsing
    let temp_dir = std::env::temp_dir();
    let temp_file = temp_dir.join("invalid_model_test.gguf");
    std::fs::write(&temp_file, b"INVALID_GGUF_MAGIC_AND_DATA_1234567890").unwrap();

    let result = GgufMetadata::from_path(&temp_file);
    let _ = std::fs::remove_file(temp_file);

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    // It should fail gracefully with a descriptive error, never panic
    assert!(err_msg.contains("not a GGUF file"));
}

// ── file_type mapping (regression: must match llama.cpp llama_ftype) ──

#[test]
fn file_type_name_matches_llama_ftype_enum() {
    // Values verified against the `llama_ftype` enum in llama.cpp include/llama.h
    assert_eq!(GgufMetadata::file_type_name(0), "F32");
    assert_eq!(GgufMetadata::file_type_name(1), "F16");
    assert_eq!(GgufMetadata::file_type_name(2), "Q4_0");
    assert_eq!(GgufMetadata::file_type_name(3), "Q4_1");
    assert_eq!(GgufMetadata::file_type_name(7), "Q8_0");
    assert_eq!(GgufMetadata::file_type_name(8), "Q5_0");
    assert_eq!(GgufMetadata::file_type_name(9), "Q5_1");
    assert_eq!(GgufMetadata::file_type_name(10), "Q2_K");
    assert_eq!(GgufMetadata::file_type_name(11), "Q3_K_S");
    assert_eq!(GgufMetadata::file_type_name(12), "Q3_K_M");
    assert_eq!(GgufMetadata::file_type_name(13), "Q3_K_L");
    assert_eq!(GgufMetadata::file_type_name(14), "Q4_K_S");
    assert_eq!(GgufMetadata::file_type_name(15), "Q4_K_M");
    assert_eq!(GgufMetadata::file_type_name(16), "Q5_K_S");
    assert_eq!(GgufMetadata::file_type_name(17), "Q5_K_M");
    assert_eq!(GgufMetadata::file_type_name(18), "Q6_K");
    assert_eq!(GgufMetadata::file_type_name(19), "IQ2_XXS");
    assert_eq!(GgufMetadata::file_type_name(20), "IQ2_XS");
    assert_eq!(GgufMetadata::file_type_name(21), "Q2_K_S");
    assert_eq!(GgufMetadata::file_type_name(22), "IQ3_XS");
    assert_eq!(GgufMetadata::file_type_name(23), "IQ3_XXS");
    assert_eq!(GgufMetadata::file_type_name(24), "IQ1_S");
    assert_eq!(GgufMetadata::file_type_name(25), "IQ4_NL");
    assert_eq!(GgufMetadata::file_type_name(26), "IQ3_S");
    assert_eq!(GgufMetadata::file_type_name(27), "IQ3_M");
    assert_eq!(GgufMetadata::file_type_name(28), "IQ2_S");
    assert_eq!(GgufMetadata::file_type_name(29), "IQ2_M");
    assert_eq!(GgufMetadata::file_type_name(30), "IQ4_XS");
    assert_eq!(GgufMetadata::file_type_name(31), "IQ1_M");
    assert_eq!(GgufMetadata::file_type_name(32), "BF16");
    assert_eq!(GgufMetadata::file_type_name(36), "TQ1_0");
    assert_eq!(GgufMetadata::file_type_name(37), "TQ2_0");
    assert_eq!(GgufMetadata::file_type_name(38), "MXFP4_MOE");
    assert_eq!(GgufMetadata::file_type_name(39), "NVFP4");
    assert_eq!(GgufMetadata::file_type_name(40), "Q1_0");
    assert_eq!(GgufMetadata::file_type_name(41), "Q2_0");
    assert_eq!(GgufMetadata::file_type_name(1024), "Guessed");
    assert_eq!(GgufMetadata::file_type_name(999), "Unknown (999)");
}

#[test]
fn file_type_quality_rank_ordering() {
    // Higher bits/quality => higher rank
    let f32 = GgufMetadata::file_type_quality_rank(0);
    let f16 = GgufMetadata::file_type_quality_rank(1);
    let q8_0 = GgufMetadata::file_type_quality_rank(7);
    let q5_1 = GgufMetadata::file_type_quality_rank(9);
    let bf16 = GgufMetadata::file_type_quality_rank(32);
    assert_eq!(f32, 4);
    assert_eq!(f16, 4);
    assert_eq!(q8_0, 4);
    assert_eq!(q5_1, 4);
    assert_eq!(bf16, 4);

    let q6_k = GgufMetadata::file_type_quality_rank(18);
    let q5_k_m = GgufMetadata::file_type_quality_rank(17);
    let q5_0 = GgufMetadata::file_type_quality_rank(8);
    assert_eq!(q6_k, 3);
    assert_eq!(q5_k_m, 3);
    assert_eq!(q5_0, 3);

    let q4_k_m = GgufMetadata::file_type_quality_rank(15);
    let q4_0 = GgufMetadata::file_type_quality_rank(2);
    let iq4_nl = GgufMetadata::file_type_quality_rank(25);
    let iq3_s = GgufMetadata::file_type_quality_rank(26);
    assert_eq!(q4_k_m, 2);
    assert_eq!(q4_0, 2);
    assert_eq!(iq4_nl, 2);
    assert_eq!(iq3_s, 2);

    let q3_k_m = GgufMetadata::file_type_quality_rank(12);
    let q2_k = GgufMetadata::file_type_quality_rank(10);
    let iq2_s = GgufMetadata::file_type_quality_rank(28);
    assert_eq!(q3_k_m, 1);
    assert_eq!(q2_k, 1);
    assert_eq!(iq2_s, 1);

    let iq2_xxs = GgufMetadata::file_type_quality_rank(19);
    let iq1_s = GgufMetadata::file_type_quality_rank(24);
    let q1_0 = GgufMetadata::file_type_quality_rank(40);
    assert_eq!(iq2_xxs, 0);
    assert_eq!(iq1_s, 0);
    assert_eq!(q1_0, 0);

    // Monotonic: F32 >= Q8_0 >= Q5_1 >= Q6_K >= Q4_K_M >= Q3_K_M >= IQ2_S >= IQ1_S
    assert!(f32 >= q8_0 && q8_0 >= q5_1 && q5_1 >= q6_k && q6_k >= q4_k_m);
    assert!(q4_k_m >= q3_k_m && q3_k_m >= iq2_s && iq2_s >= iq1_s);
}

#[test]
fn chat_template_picker_structure() {
    let templates = get_available_chat_templates();
    assert_eq!(templates.first().map(String::as_str), Some("Auto (detect)"));
    assert_eq!(templates.last().map(String::as_str), Some("None"));
    assert_eq!(
        templates[templates.len() - 2].as_str(),
        "Select a Template file..."
    );
    let middle: Vec<&str> = templates[1..templates.len() - 2]
        .iter()
        .map(String::as_str)
        .collect();
    assert_eq!(middle, BUILTIN_CHAT_TEMPLATES.to_vec());
    assert!(templates.len() > 50);
}

#[test]
fn builtin_chat_templates_sorted_and_unique() {
    let mut sorted = BUILTIN_CHAT_TEMPLATES.to_vec();
    sorted.sort();
    assert_eq!(BUILTIN_CHAT_TEMPLATES, &sorted[..]);
    let unique: std::collections::HashSet<&str> = BUILTIN_CHAT_TEMPLATES.iter().copied().collect();
    assert_eq!(unique.len(), BUILTIN_CHAT_TEMPLATES.len());
    for name in BUILTIN_CHAT_TEMPLATES {
        assert!(!name.is_empty());
    }
}

#[test]
fn arch_to_chat_template_maps_to_builtin_names() {
    let archs = [
        "llama",
        "llama-moe",
        "llama4",
        "mistral",
        "mistral3",
        "mistral4",
        "qwen",
        "qwen2",
        "qwen2moe",
        "qwen3",
        "qwen3moe",
        "qwen3next",
        "qwen35",
        "qwen35moe",
        "qwen2vl",
        "qwen3vl",
        "qwen3vlmoe",
        "gemma",
        "gemma2",
        "gemma3",
        "gemma3n",
        "gemma4",
        "gemma4-assistant",
        "phi2",
        "phi3",
        "phimoe",
        "phi4",
        "cohere",
        "cohere2",
        "deepseek",
        "deepseek2",
        "deepseek2-ocr",
        "deepseek32",
        "internlm2",
        "glm4",
        "glm4moe",
        "chatglm",
        "exaone",
        "exaone4",
        "exaone-moe",
        "minicpm",
        "minicpm3",
        "minicpmo",
        "falcon",
        "falcon-h1",
        "falcon3",
        "rwkv6",
        "rwkv6qwen2",
        "rwkv7",
        "arwkv7",
        "granite",
        "granitehybrid",
        "granitemoe",
        "hunyuan-dense",
        "hunyuan-moe",
        "hunyuan_vl",
        "olmo",
        "olmo2",
        "olmoe",
        "sonar",
        "mamba",
        "mamba2",
        "mamba_ssm",
        "dbrx",
        "starcoder",
        "starcoder2",
        "baichuan",
        "gpt-neox",
        "gptj",
        "mpt",
        "jais",
        "jais2",
        "stablelm",
        "chameleon",
        "nemo",
        "nemotron",
        "nemotron_h",
        "nemotron_h_moe",
        "plamo",
        "plamo2",
        "plamo3",
        "ernie4_5",
        "ernie4_5-moe",
        "mini-max-m2",
        "talkie",
        "apertus",
        "arcee",
        "arctic",
        "jamba",
        "lfm2",
        "lfm2moe",
        "llada",
        "llada-moe",
        "maincoder",
        "mellum",
        "mimo2",
        "refact",
        "rnd1",
        "smallthinker",
        "xverse",
        "gpt2",
        "codeshell",
        "cogvlm",
        "deci",
        "dots1",
        "dream",
        "app",
        "step35",
        "smollm3",
        "megrez",
        "yandex",
        "bailing",
        "bailingmoe",
        "bailingmoe2",
        "bailing2",
        "bailing-think",
        "kimi-k2",
        "seed_oss",
        "grok",
        "solar-open",
        "gpt-oss",
        "pangu-embedded",
        "gigachat",
    ];
    for arch in &archs {
        if let Some(template) = arch_to_chat_template(arch) {
            assert!(
                BUILTIN_CHAT_TEMPLATES.contains(&template),
                "arch '{}' maps to '{}' which is not in BUILTIN_CHAT_TEMPLATES",
                arch,
                template
            );
        }
    }
}

// ── generate_ctx_proposals ─────────────────────────────────────

fn proposal_base_settings() -> ModelSettings {
    let mut s = ModelSettings::default();
    s.gpu_layers_mode = GpuLayersMode::All;
    s
}

/// Proposals for a small (8B) model — no big-model rules apply.
fn proposals(
    model_mib: u64,
    gpu_mib: u64,
    n_ctx_train: u32,
    arch: &ArchVramInfo,
    mtp: Option<u32>,
) -> Vec<CtxProposal> {
    proposals_sized(model_mib, gpu_mib, n_ctx_train, arch, 8_000_000_000, mtp)
}

#[allow(clippy::too_many_arguments)]
fn proposals_sized(
    model_mib: u64,
    gpu_mib: u64,
    n_ctx_train: u32,
    arch: &ArchVramInfo,
    model_params: u64,
    mtp: Option<u32>,
) -> Vec<CtxProposal> {
    proposals_quant(model_mib, gpu_mib, n_ctx_train, arch, model_params, 0, mtp)
}

#[allow(clippy::too_many_arguments)]
fn proposals_quant(
    model_mib: u64,
    gpu_mib: u64,
    n_ctx_train: u32,
    arch: &ArchVramInfo,
    model_params: u64,
    quant_bits: u32,
    mtp: Option<u32>,
) -> Vec<CtxProposal> {
    generate_ctx_proposals(
        model_mib,
        &proposal_base_settings(),
        32,
        4096,
        32,
        8,
        gpu_mib,
        arch,
        n_ctx_train,
        model_params,
        quant_bits,
        mtp,
    )
}

#[test]
fn ctx_proposals_empty_without_gpu_metadata_or_with_cpu_only() {
    // No GPU info.
    assert!(proposals(4000, 0, 131072, &ArchVramInfo::default(), None).is_empty());
    // CPU-only settings.
    let mut s = proposal_base_settings();
    s.gpu_layers_mode = GpuLayersMode::Specific(0);
    let p = generate_ctx_proposals(
        4000,
        &s,
        32,
        4096,
        32,
        8,
        8192,
        &ArchVramInfo::default(),
        131072,
        8_000_000_000,
        0,
        None,
    );
    assert!(p.is_empty());
}

#[test]
fn ctx_proposals_skip_infeasible_model() {
    // 30 GiB model on an 8 GiB card: no rung fits.
    assert!(proposals(30000, 8192, 131072, &ArchVramInfo::default(), None).is_empty());
}

#[test]
fn ctx_proposals_ladder_order_and_constraints() {
    let p = proposals(4000, 6144, 131072, &ArchVramInfo::default(), None);
    assert_eq!(p.len(), 10, "full ladder feasible on 6 GiB with 4 GiB model");
    // First rung: cache disabled (llama.cpp default f16) = best quality.
    assert_eq!(p[0].cache_k, None);
    assert_eq!(p[0].cache_v, None);
    // Context never decreases as quality degrades.
    for w in p.windows(2) {
        assert!(w[1].ctx >= w[0].ctx, "ctx must be non-decreasing down the ladder");
    }
    // K and V stay within one ladder step: allowed pairs only.
    use CacheQuantType::*;
    let allowed: [(Option<CacheQuantType>, Option<CacheQuantType>); 10] = [
        (None, None),
        (Some(Q8_0), Some(Q8_0)),
        (Some(Q8_0), Some(Q5_1)),
        (Some(Q5_1), Some(Q5_1)),
        (Some(Q5_1), Some(Q5_0)),
        (Some(Q5_0), Some(Q5_0)),
        (Some(Q5_0), Some(Q4_1)),
        (Some(Q4_1), Some(Q4_1)),
        (Some(Q4_0), Some(Q4_0)),
        (Some(Iq4Nl), Some(Q4_0)),
    ];
    for pr in &p {
        assert!(allowed.contains(&(pr.cache_k, pr.cache_v)), "unexpected K/V pair");
    }
    // Every proposal fits in VRAM.
    for pr in &p {
        assert!(pr.vram_mib <= 6144);
    }
    // None reaches the 100k target on this small budget.
    assert!(!p.iter().any(|pr| pr.recommended));
}

#[test]
fn ctx_proposals_cache_floor_matches_model_quant() {
    // Q8 model: only disabled and q8_0 rungs are acceptable — a q4/q5 cache
    // for a Q8 LLM is nonsense.
    let p = proposals_quant(4000, 6144, 131072, &ArchVramInfo::default(), 8_000_000_000, 8, None);
    assert_eq!(p.len(), 2);
    assert_eq!((p[0].cache_k, p[0].cache_v), (None, None));
    assert_eq!(
        (p[1].cache_k, p[1].cache_v),
        (Some(CacheQuantType::Q8_0), Some(CacheQuantType::Q8_0))
    );

    // Q6 model: q5 rungs are below 6 bits — same floor as Q8.
    let p = proposals_quant(4000, 6144, 131072, &ArchVramInfo::default(), 8_000_000_000, 6, None);
    for pr in &p {
        for c in [pr.cache_k, pr.cache_v].into_iter().flatten() {
            assert_eq!(c, CacheQuantType::Q8_0, "Q6 model got sub-q8 cache {:?}", c);
        }
    }

    // Q4 model: full ladder down to the q4 rungs.
    let p = proposals_quant(4000, 6144, 131072, &ArchVramInfo::default(), 8_000_000_000, 4, None);
    assert_eq!(p.len(), 10);
    assert_eq!(p.last().unwrap().cache_k, Some(CacheQuantType::Iq4Nl));

    // F16 model: only the disabled rung (no quantized cache below f16).
    let p = proposals_quant(4000, 6144, 131072, &ArchVramInfo::default(), 8_000_000_000, 16, None);
    assert_eq!(p.len(), 1);
    assert_eq!((p[0].cache_k, p[0].cache_v), (None, None));
}

#[test]
fn gguf_metadata_quant_bits_parses_file_types() {
    let mk = |ft: &str| GgufMetadata {
        file_type: ft.to_string(),
        ..Default::default()
    };
    assert_eq!(mk("Q8_0").quant_bits(), 8);
    assert_eq!(mk("Q4_K_M").quant_bits(), 4);
    assert_eq!(mk("Q4_K_XL").quant_bits(), 4);
    assert_eq!(mk("IQ4_NL").quant_bits(), 4);
    assert_eq!(mk("Q6_K").quant_bits(), 6);
    assert_eq!(mk("Q2_K").quant_bits(), 2);
    assert_eq!(mk("F16").quant_bits(), 16);
    assert_eq!(mk("BF16").quant_bits(), 16);
    assert_eq!(mk("F32").quant_bits(), 32);
    assert_eq!(mk("Q1_0").quant_bits(), 1);
    assert_eq!(mk("").quant_bits(), 0);
}

#[test]
fn ctx_proposals_capped_at_native_ctx() {
    // Huge budget but native ctx of 8k: every rung is capped at 8k, so the
    // rows differ only by quality/perf. Best quality is first and recommended.
    let p = proposals(4000, 24576, 8192, &ArchVramInfo::default(), None);
    assert!(!p.is_empty());
    assert!(p.iter().all(|pr| pr.ctx == 8192), "all rows capped at native ctx");
    assert!(p[0].recommended, "native cap is below target so first rung reaches it");
    assert_eq!(p[0].cache_k, None);
    assert_eq!(p[0].cache_v, None);
}

#[test]
fn ctx_proposals_recommended_is_first_to_reach_target() {
    // 16 GiB card, 4 GiB model: f16/f16 falls short of 100k, degraded rungs reach it.
    let p = proposals(4000, 16384, 131072, &ArchVramInfo::default(), None);
    assert!(!p.is_empty());
    assert!(p.len() <= 10);
    let rec = p.iter().filter(|pr| pr.recommended).count();
    assert_eq!(rec, 1, "exactly one recommended proposal");
    let idx = p.iter().position(|pr| pr.recommended).unwrap();
    assert!(p[idx].ctx >= 100_000);
    assert!(p[..idx].iter().all(|pr| pr.ctx < 100_000));
    // ctx is rounded down to a 1024-token step.
    for pr in &p {
        assert_eq!(pr.ctx % 1024, 0);
    }
}

#[test]
fn ctx_proposals_mtp_only_for_nextn_models() {
    // No MTP layers → no MTP in proposals even if a draft cap is provided.
    let p = proposals(4000, 16384, 131072, &ArchVramInfo::default(), Some(5));
    assert!(p.iter().all(|pr| pr.mtp_draft_tokens.is_none()));

    // MTP model → the top tiers get both an MTP-on (perf) and MTP-off (ctx)
    // variant, so the set mixes draft=5 and no-MTP rows.
    let arch = ArchVramInfo {
        nextn_layers: 1,
        ..Default::default()
    };
    let p = proposals(4000, 16384, 131072, &arch, Some(5));
    assert!(!p.is_empty());
    assert!(p.iter().any(|pr| pr.mtp_draft_tokens == Some(5)), "perf variant present");
    assert!(p.iter().any(|pr| pr.mtp_draft_tokens.is_none()), "ctx variant present");
    // No draft ever exceeds the 1..=5 bound.
    assert!(p.iter().all(|pr| pr.mtp_draft_tokens.is_none_or(|d| (1..=5).contains(&d))));
}

#[test]
fn ctx_proposals_mtp_cap_respected_and_clamped() {
    let arch = ArchVramInfo {
        nextn_layers: 1,
        ..Default::default()
    };
    // Cap of 3 → on-variants use draft ≤ 3.
    let p = proposals(4000, 16384, 131072, &arch, Some(3));
    assert_eq!(p.iter().filter_map(|pr| pr.mtp_draft_tokens).max(), Some(3));
    // Cap above 6 is clamped to 6.
    let p = proposals(4000, 16384, 131072, &arch, Some(9));
    assert_eq!(p.iter().filter_map(|pr| pr.mtp_draft_tokens).max(), Some(6));
}

#[test]
fn ctx_proposals_notes_assigned() {
    let p = proposals(4000, 6144, 131072, &ArchVramInfo::default(), None);
    assert!(p.len() >= 2);
    // Highest-precision rung is labelled best quality.
    assert_eq!(p[0].note, ProposalNote::BestQuality);
    // Largest-ctx rung (last) is best ctx or a ctx/perf balance.
    let last = p.last().unwrap().note;
    assert!(matches!(
        last,
        ProposalNote::BestCtx | ProposalNote::BalancedCtxPerf | ProposalNote::BestPerf
    ));
}

#[test]
fn ctx_proposals_mtp_perf_label_appears() {
    // An MTP model with a roomy budget should surface a performance-leaning
    // label somewhere in the set (MTP-on variants), proving perf is separable.
    let arch = ArchVramInfo {
        nextn_layers: 1,
        ..Default::default()
    };
    let p = proposals(4000, 24576, 131072, &arch, Some(5));
    assert!(!p.is_empty());
    assert!(
        p.iter().any(|pr| matches!(
            pr.note,
            ProposalNote::BestPerf | ProposalNote::BalancedQualityPerf | ProposalNote::BalancedCtxPerf
        )),
        "expected at least one perf-related note, got {:?}",
        p.iter().map(|pr| pr.note).collect::<Vec<_>>()
    );
}

#[test]
fn estimate_vram_mtp_scales_with_draft_tokens() {
    let arch = ArchVramInfo {
        nextn_layers: 1,
        ..Default::default()
    };
    let mut s = ModelSettings::default();
    s.gpu_layers_mode = GpuLayersMode::All;
    s.spec_type = "draft-mtp".to_string();
    s.cache_type_k = Some(CacheQuantType::F16);
    s.cache_type_v = Some(CacheQuantType::F16);
    s.context_length = 32768;
    s.draft_tokens = 1;
    let lo = estimate_vram_mib(4000, &s, 32, Some(4096), Some(32), Some(8), 24576, &arch);
    s.draft_tokens = 5;
    let hi = estimate_vram_mib(4000, &s, 32, Some(4096), Some(32), Some(8), 24576, &arch);
    assert!(hi > lo, "more draft tokens must cost more VRAM (lo={lo} hi={hi})");

    // MTP off must be cheaper than MTP on at draft=1.
    s.spec_type = String::new();
    s.draft_tokens = 0;
    let off = estimate_vram_mib(4000, &s, 32, Some(4096), Some(32), Some(8), 24576, &arch);
    assert!(off < lo, "MTP off ({off}) must cost less than MTP on ({lo})");
}

#[test]
fn optimal_ctx_shrinks_with_more_draft_tokens() {
    let arch = ArchVramInfo {
        nextn_layers: 1,
        ..Default::default()
    };
    let mut s = ModelSettings::default();
    s.gpu_layers_mode = GpuLayersMode::All;
    s.spec_type = "draft-mtp".to_string();
    s.draft_tokens = 1;
    let ctx_lo = optimal_ctx_size(4000, &s, 32, 4096, 32, 8, 24576, &arch, 131072);
    s.draft_tokens = 5;
    let ctx_hi = optimal_ctx_size(4000, &s, 32, 4096, 32, 8, 24576, &arch, 131072);
    assert!(
        ctx_hi <= ctx_lo,
        "higher draft count leaves less room for ctx (draft1={ctx_lo}, draft5={ctx_hi})"
    );
}

#[test]
fn ctx_proposals_offer_varied_mtp_levels() {
    // The set must span several draft values (0, 1, 2, mid, cap), not just
    // the maximum, and stay within the 15-row budget.
    let arch = ArchVramInfo {
        nextn_layers: 1,
        ..Default::default()
    };
    let p = proposals(4000, 16384, 131072, &arch, Some(6));
    assert!(p.len() <= 15, "at most 15 proposals, got {}", p.len());
    let mut drafts: Vec<u32> = p.iter().filter_map(|pr| pr.mtp_draft_tokens).collect();
    drafts.sort_unstable();
    drafts.dedup();
    assert!(drafts.contains(&1), "draft 1 missing: {drafts:?}");
    assert!(drafts.contains(&2), "draft 2 missing: {drafts:?}");
    assert!(drafts.contains(&6), "draft 6 (cap) missing: {drafts:?}");
    assert!(
        p.iter().any(|pr| pr.mtp_draft_tokens.is_none()),
        "MTP-disabled (0) variant missing"
    );
}

#[test]
fn gguf_metadata_active_parameters_moe() {
    // MoE: 35B total with 8 of 128 experts active ≈ 2.2B effective.
    let moe = GgufMetadata {
        parameters: 35_000_000_000,
        expert_count: 128,
        expert_used_count: 8,
        ..Default::default()
    };
    let active = moe.active_parameters();
    assert!(active < 27_000_000_000, "MoE must classify as small: {active}");
    assert!((2_000_000_000..2_500_000_000).contains(&active), "got {active}");

    // Dense model: active == total.
    let dense = GgufMetadata {
        parameters: 27_500_000_000,
        ..Default::default()
    };
    assert_eq!(dense.active_parameters(), 27_500_000_000);
}

#[test]
fn ctx_proposals_moe_model_uses_small_rules() {
    // A 35B-A3B MoE (≈2.2B active) gets the small-model treatment: draft up
    // to 6 and no 64k minimum-ctx filter.
    let arch = ArchVramInfo {
        nextn_layers: 1,
        ..Default::default()
    };
    let p = proposals_sized(4000, 16384, 131072, &arch, 2_187_500_000, Some(6));
    assert!(!p.is_empty());
    assert_eq!(p.iter().filter_map(|pr| pr.mtp_draft_tokens).max(), Some(6));
}

#[test]
fn ctx_proposals_big_model_mtp_capped_at_3() {
    // Big model (≥ 27B): prefer small draft counts (0..=3) to protect ctx.
    let arch = ArchVramInfo {
        nextn_layers: 1,
        ..Default::default()
    };
    let p = proposals_sized(4000, 16384, 131072, &arch, 27_500_000_000, Some(5));
    assert!(!p.is_empty());
    assert_eq!(
        p.iter().filter_map(|pr| pr.mtp_draft_tokens).max(),
        Some(3),
        "big models must cap MTP draft at 3"
    );
    // The disabled (0) variant is still offered.
    assert!(p.iter().any(|pr| pr.mtp_draft_tokens.is_none()));
}

#[test]
fn ctx_proposals_big_model_drops_ctx_under_64k() {
    // Tight budget: every tier solves to < 64k ctx. For a big model those are
    // unusable and get dropped; a small model still gets the proposals.
    let big = proposals_sized(
        22500,
        24576,
        131072,
        &ArchVramInfo::default(),
        30_000_000_000,
        None,
    );
    assert!(big.is_empty(), "big model must not propose ctx < 64k");

    let small = proposals_sized(
        22500,
        24576,
        131072,
        &ArchVramInfo::default(),
        8_000_000_000,
        None,
    );
    assert!(!small.is_empty());
    assert!(small.iter().all(|pr| pr.ctx < 65536));

    // Roomy budget: a big model keeps only rows ≥ 64k.
    let big_ok = proposals_sized(
        4000,
        24576,
        131072,
        &ArchVramInfo::default(),
        30_000_000_000,
        None,
    );
    assert!(!big_ok.is_empty());
    assert!(big_ok.iter().all(|pr| pr.ctx >= 65536));
}

#[test]
fn ctx_proposals_respect_vram_safety_margin() {
    let total = 8192u64;
    let p = proposals(4000, total, 131072, &ArchVramInfo::default(), None);
    assert!(!p.is_empty());
    let budget = ((total as f64) * 0.97) as u64;
    for pr in &p {
        assert!(
            pr.vram_mib <= budget,
            "proposal VRAM {} exceeds 97% budget {}",
            pr.vram_mib,
            budget
        );
    }
}
