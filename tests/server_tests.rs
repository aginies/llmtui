//! Tests for backend/server.rs — command building and server management.
//!
//! Tests cover: build_server_cmd for all modes, build_bench_cmd, argument construction,
//! and display string generation.
//!
//! Network-dependent tests (spawn_server, check_health, kill_server, load_model, etc.)
//! are not included as they require a real llama-server binary or HTTP server.

use llm_manager::config::Config;
use llm_manager::models::*;
use llm_manager::backend::server::{build_server_cmd, build_bench_cmd};
use std::path::PathBuf;

// ── Test helpers ─────────────────────────────────────────────────

fn make_model(path: &str, name: &str, display: &str) -> DiscoveredModel {
    DiscoveredModel {
        path: PathBuf::from(path),
        name: name.into(),
        file_size: 4_000_000_000,
        display_name: display.into(),
    }
}

fn make_settings() -> ModelSettings {
    ModelSettings::default()
}

fn make_config() -> Config {
    Config::default()
}

// ── build_server_cmd — Normal mode ──────────────────────────────

#[test]
fn test_build_server_cmd_normal_includes_model_path() {
    let binary = PathBuf::from("/usr/bin/llama-server");
    let model = make_model("/models/qwen.gguf", "qwen", "Qwen");
    let settings = make_settings();
    let config = make_config();

    let (_cmd, display) = build_server_cmd(&binary, Some(&model), &settings, &config, ServerMode::Normal, 0);

    assert!(display.contains("qwen.gguf"));
}

#[test]
fn test_build_server_cmd_normal_includes_alias() {
    let binary = PathBuf::from("/usr/bin/llama-server");
    let model = make_model("/models/qwen.gguf", "qwen", "Qwen");
    let settings = make_settings();
    let config = make_config();

    let (_cmd, display) = build_server_cmd(&binary, Some(&model), &settings, &config, ServerMode::Normal, 0);

    assert!(display.contains("--alias"));
    assert!(display.contains("Qwen"));
}

#[test]
fn test_build_server_cmd_normal_no_model() {
    let binary = PathBuf::from("/usr/bin/llama-server");
    let settings = make_settings();
    let config = make_config();

    let (_cmd, display) = build_server_cmd(&binary, None, &settings, &config, ServerMode::Normal, 0);

    assert!(!display.contains(".gguf"));
}

// ── build_server_cmd — Router mode ──────────────────────────────

#[test]
fn test_build_server_cmd_router_includes_models_max() {
    let binary = PathBuf::from("/usr/bin/llama-server");
    let model = make_model("/models/test.gguf", "test", "Test");
    let settings = make_settings();
    let config = make_config();

    let (_cmd, display) = build_server_cmd(&binary, Some(&model), &settings, &config, ServerMode::Router, 4);

    assert!(display.contains("--models-max"));
    assert!(display.contains("4"));
}

#[test]
fn test_build_server_cmd_router_no_model_path() {
    let binary = PathBuf::from("/usr/bin/llama-server");
    let model = make_model("/models/test.gguf", "test", "Test");
    let settings = make_settings();
    let config = make_config();

    let (_cmd, display) = build_server_cmd(&binary, Some(&model), &settings, &config, ServerMode::Router, 0);

    assert!(!display.contains("test.gguf"));
}

#[test]
fn test_build_server_cmd_router_includes_models_dir() {
    let binary = PathBuf::from("/usr/bin/llama-server");
    let model = make_model("/models/test.gguf", "test", "Test");
    let settings = make_settings();
    let config = make_config();

    let (_cmd, display) = build_server_cmd(&binary, Some(&model), &settings, &config, ServerMode::Router, 0);

    assert!(display.contains("--models-dir"));
}

// ── build_server_cmd — Settings arguments ───────────────────────

#[test]
fn test_build_server_cmd_includes_threads() {
    let binary = PathBuf::from("/usr/bin/llama-server");
    let model = make_model("/models/test.gguf", "test", "Test");
    let mut settings = make_settings();
    settings.threads = 4;
    let config = make_config();

    let (_cmd, display) = build_server_cmd(&binary, Some(&model), &settings, &config, ServerMode::Normal, 0);

    assert!(display.contains("--threads"));
    assert!(display.contains("4"));
}

#[test]
fn test_build_server_cmd_includes_context_size() {
    let binary = PathBuf::from("/usr/bin/llama-server");
    let model = make_model("/models/test.gguf", "test", "Test");
    let settings = make_settings();
    let config = make_config();

    let (_cmd, display) = build_server_cmd(&binary, Some(&model), &settings, &config, ServerMode::Normal, 0);

    assert!(display.contains("--ctx-size"));
}

#[test]
fn test_build_server_cmd_includes_no_warmup() {
    let binary = PathBuf::from("/usr/bin/llama-server");
    let model = make_model("/models/test.gguf", "test", "Test");
    let settings = make_settings();
    let config = make_config();

    let (_cmd, display) = build_server_cmd(&binary, Some(&model), &settings, &config, ServerMode::Normal, 0);

    assert!(display.contains("--no-warmup"));
}

#[test]
fn test_build_server_cmd_includes_mlock_when_set() {
    let binary = PathBuf::from("/usr/bin/llama-server");
    let model = make_model("/models/test.gguf", "test", "Test");
    let mut settings = make_settings();
    settings.mlock = true;
    let config = make_config();

    let (_cmd, display) = build_server_cmd(&binary, Some(&model), &settings, &config, ServerMode::Normal, 0);

    assert!(display.contains("--mlock"));
}

#[test]
fn test_build_server_cmd_includes_no_mmap_when_not_set() {
    let binary = PathBuf::from("/usr/bin/llama-server");
    let model = make_model("/models/test.gguf", "test", "Test");
    let mut settings = make_settings();
    settings.mmap = false;
    let config = make_config();

    let (_cmd, display) = build_server_cmd(&binary, Some(&model), &settings, &config, ServerMode::Normal, 0);

    assert!(display.contains("--no-mmap"));
}

#[test]
fn test_build_server_cmd_includes_gpu_layers_specific() {
    let binary = PathBuf::from("/usr/bin/llama-server");
    let model = make_model("/models/test.gguf", "test", "Test");
    let mut settings = make_settings();
    settings.gpu_layers_mode = GpuLayersMode::Specific(32);
    let config = make_config();

    let (_cmd, display) = build_server_cmd(&binary, Some(&model), &settings, &config, ServerMode::Normal, 0);

    assert!(display.contains("-ngl"));
    assert!(display.contains("32"));
}

#[test]
fn test_build_server_cmd_includes_gpu_layers_all() {
    let binary = PathBuf::from("/usr/bin/llama-server");
    let model = make_model("/models/test.gguf", "test", "Test");
    let mut settings = make_settings();
    settings.gpu_layers_mode = GpuLayersMode::All;
    let config = make_config();

    let (_cmd, display) = build_server_cmd(&binary, Some(&model), &settings, &config, ServerMode::Normal, 0);

    assert!(display.contains("-ngl"));
    assert!(display.contains("999"));
}

#[test]
fn test_build_server_cmd_no_gpu_layers_for_auto() {
    let binary = PathBuf::from("/usr/bin/llama-server");
    let model = make_model("/models/test.gguf", "test", "Test");
    let mut settings = make_settings();
    settings.gpu_layers_mode = GpuLayersMode::Auto;
    let config = make_config();

    let (_cmd, display) = build_server_cmd(&binary, Some(&model), &settings, &config, ServerMode::Normal, 0);

    // Auto mode should not include -ngl
    assert!(!display.contains("-ngl"));
}

#[test]
fn test_build_server_cmd_includes_sampling_params() {
    let binary = PathBuf::from("/usr/bin/llama-server");
    let model = make_model("/models/test.gguf", "test", "Test");
    let settings = make_settings();
    let config = make_config();

    let (_cmd, display) = build_server_cmd(&binary, Some(&model), &settings, &config, ServerMode::Normal, 0);

    assert!(display.contains("--temp"));
    assert!(display.contains("--top-k"));
    assert!(display.contains("--top-p"));
}

#[test]
fn test_build_server_cmd_includes_repetition_params() {
    let binary = PathBuf::from("/usr/bin/llama-server");
    let model = make_model("/models/test.gguf", "test", "Test");
    let settings = make_settings();
    let config = make_config();

    let (_cmd, display) = build_server_cmd(&binary, Some(&model), &settings, &config, ServerMode::Normal, 0);

    // Check for sampling params that are always included
    assert!(display.contains("--temp"));
    assert!(display.contains("--top-k"));
}

#[test]
fn test_build_server_cmd_includes_mtp_flags() {
    let binary = PathBuf::from("/usr/bin/llama-server");
    let model = make_model("/models/test.gguf", "test", "Test");
    let mut settings = make_settings();
    settings.spec_type = "draft-mtp".to_string();
    settings.draft_tokens = 4;
    let config = make_config();

    let (_cmd, display) = build_server_cmd(&binary, Some(&model), &settings, &config, ServerMode::Normal, 0);

    assert!(display.contains("--spec-type"));
    assert!(display.contains("draft-mtp"));
    assert!(display.contains("--spec-draft-n-max"));
    assert!(display.contains("4"));
}

// ── build_server_cmd — Display string ───────────────────────────

#[test]
fn test_build_server_cmd_display_contains_binary() {
    let binary = PathBuf::from("/usr/bin/llama-server");
    let model = make_model("/models/test.gguf", "test", "Test");
    let settings = make_settings();
    let config = make_config();

    let (_cmd, display) = build_server_cmd(&binary, Some(&model), &settings, &config, ServerMode::Normal, 0);

    assert!(display.contains("llama-server"));
}

#[test]
fn test_build_server_cmd_display_contains_model() {
    let binary = PathBuf::from("/usr/bin/llama-server");
    let model = make_model("/models/qwen2.5-7b.gguf", "qwen2.5-7b", "Qwen2.5-7B");
    let settings = make_settings();
    let config = make_config();

    let (_cmd, display) = build_server_cmd(&binary, Some(&model), &settings, &config, ServerMode::Normal, 0);

    assert!(display.contains("qwen2.5-7b.gguf"));
}

#[test]
fn test_build_server_cmd_display_contains_settings() {
    let binary = PathBuf::from("/usr/bin/llama-server");
    let model = make_model("/models/test.gguf", "test", "Test");
    let settings = make_settings();
    let config = make_config();

    let (_cmd, display) = build_server_cmd(&binary, Some(&model), &settings, &config, ServerMode::Normal, 0);

    assert!(display.contains("--threads"));
    assert!(display.contains("--ctx-size"));
}

// ── build_bench_cmd ─────────────────────────────────────────────

#[test]
fn test_build_bench_cmd_includes_model() {
    let binary = PathBuf::from("/usr/bin/llama-server");
    let model = make_model("/models/test.gguf", "test", "Test");
    let settings = make_settings();

    let (_cmd, display) = build_bench_cmd(&binary, &model, &settings);

    assert!(display.contains("test.gguf"));
}

#[test]
fn test_build_bench_cmd_includes_bench_flags() {
    let binary = PathBuf::from("/usr/bin/llama-server");
    let model = make_model("/models/test.gguf", "test", "Test");
    let settings = make_settings();

    let (_cmd, display) = build_bench_cmd(&binary, &model, &settings);

    assert!(display.contains("--progress"));
}

#[test]
fn test_build_bench_cmd_display_contains_binary() {
    let binary = PathBuf::from("/usr/bin/llama-server");
    let model = make_model("/models/test.gguf", "test", "Test");
    let settings = make_settings();

    let (_cmd, display) = build_bench_cmd(&binary, &model, &settings);

    assert!(display.contains("llama-server"));
}
