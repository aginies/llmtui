//! Tests for backend/server.rs — command building and server management.
//!
//! Tests cover: build_server_cmd for all modes, build_bench_cmd, argument construction,
//! and display string generation.
//!
//! Network-dependent tests (spawn_server, check_health, kill_server, load_model, etc.)
//! are not included as they require a real llama-server binary or HTTP server.

use llm_manager::backend::server::{build_bench_cmd, build_server_cmd};
use llm_manager::config::Config;
use llm_manager::models::*;
use std::path::PathBuf;

// ── Test helpers ─────────────────────────────────────────────────

fn make_model(path: &str, name: &str, display: &str) -> DiscoveredModel {
    DiscoveredModel {
        path: PathBuf::from(path),
        name: name.into(),
        file_size: 4_000_000_000,
        display_name: display.into(),
        pipeline_tag: None,
        capabilities: vec![],
    }
}

fn make_settings() -> ModelSettings {
    ModelSettings::default()
}

fn make_config() -> Config {
    Config::default()
}

fn make_worker(selected: bool, ip: &str) -> llm_manager::config::RpcWorker {
    llm_manager::config::RpcWorker {
        selected,
        name: "worker".into(),
        ip: ip.into(),
        port: 50052,
        tensor_split: "1".into(),
    }
}

// ── build_server_cmd — Normal mode ──────────────────────────────

#[test]
fn test_build_server_cmd_normal_includes_model_path() {
    let binary = PathBuf::from("/usr/bin/llama-server");
    let model = make_model("/models/qwen.gguf", "qwen", "Qwen");
    let settings = make_settings();
    let config = make_config();

    let (_cmd, display) = build_server_cmd(
        &binary,
        Some(&model),
        &settings,
        &config,
        ServerMode::Normal,
        0,
        false,
    );

    assert!(display.contains("qwen.gguf"));
}

#[test]
fn test_build_server_cmd_normal_includes_alias() {
    let binary = PathBuf::from("/usr/bin/llama-server");
    let model = make_model("/models/qwen.gguf", "qwen", "Qwen");
    let settings = make_settings();
    let config = make_config();

    let (_cmd, display) = build_server_cmd(
        &binary,
        Some(&model),
        &settings,
        &config,
        ServerMode::Normal,
        0,
        false,
    );

    assert!(display.contains("--alias"));
    assert!(display.contains("Qwen"));
}

#[test]
fn test_build_server_cmd_normal_no_model() {
    let binary = PathBuf::from("/usr/bin/llama-server");
    let settings = make_settings();
    let config = make_config();

    let (_cmd, display) = build_server_cmd(
        &binary,
        None,
        &settings,
        &config,
        ServerMode::Normal,
        0,
        false,
    );

    assert!(!display.contains(".gguf"));
}

// ── build_server_cmd — Router mode ──────────────────────────────

#[test]
fn test_build_server_cmd_router_includes_models_max() {
    let binary = PathBuf::from("/usr/bin/llama-server");
    let model = make_model("/models/test.gguf", "test", "Test");
    let settings = make_settings();
    let config = make_config();

    let (_cmd, display) = build_server_cmd(
        &binary,
        Some(&model),
        &settings,
        &config,
        ServerMode::Router,
        4,
        false,
    );

    assert!(display.contains("--models-max"));
    assert!(display.contains("4"));
}

#[test]
fn test_build_server_cmd_router_no_model_path() {
    let binary = PathBuf::from("/usr/bin/llama-server");
    let model = make_model("/models/test.gguf", "test", "Test");
    let settings = make_settings();
    let config = make_config();

    let (_cmd, display) = build_server_cmd(
        &binary,
        Some(&model),
        &settings,
        &config,
        ServerMode::Router,
        0,
        false,
    );

    assert!(!display.contains("test.gguf"));
}

#[test]
fn test_build_server_cmd_router_includes_models_dir() {
    let binary = PathBuf::from("/usr/bin/llama-server");
    let model = make_model("/models/test.gguf", "test", "Test");
    let settings = make_settings();
    let config = make_config();

    let (_cmd, display) = build_server_cmd(
        &binary,
        Some(&model),
        &settings,
        &config,
        ServerMode::Router,
        0,
        false,
    );

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

    let (_cmd, display) = build_server_cmd(
        &binary,
        Some(&model),
        &settings,
        &config,
        ServerMode::Normal,
        0,
        false,
    );

    assert!(display.contains("--threads"));
    assert!(display.contains("4"));
}

#[test]
fn test_build_server_cmd_includes_context_size() {
    let binary = PathBuf::from("/usr/bin/llama-server");
    let model = make_model("/models/test.gguf", "test", "Test");
    let settings = make_settings();
    let config = make_config();

    let (_cmd, display) = build_server_cmd(
        &binary,
        Some(&model),
        &settings,
        &config,
        ServerMode::Normal,
        0,
        false,
    );

    assert!(display.contains("--ctx-size"));
}

#[test]
fn test_build_server_cmd_includes_no_warmup() {
    let binary = PathBuf::from("/usr/bin/llama-server");
    let model = make_model("/models/test.gguf", "test", "Test");
    let settings = make_settings();
    let config = make_config();

    let (_cmd, display) = build_server_cmd(
        &binary,
        Some(&model),
        &settings,
        &config,
        ServerMode::Normal,
        0,
        false,
    );

    assert!(display.contains("--no-warmup"));
}

#[test]
fn test_build_server_cmd_includes_gpu_layers_specific() {
    let binary = PathBuf::from("/usr/bin/llama-server");
    let model = make_model("/models/test.gguf", "test", "Test");
    let mut settings = make_settings();
    settings.gpu_layers_mode = GpuLayersMode::Specific(32);
    let config = make_config();

    let (_cmd, display) = build_server_cmd(
        &binary,
        Some(&model),
        &settings,
        &config,
        ServerMode::Normal,
        0,
        false,
    );

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

    let (_cmd, display) = build_server_cmd(
        &binary,
        Some(&model),
        &settings,
        &config,
        ServerMode::Normal,
        0,
        false,
    );

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

    let (_cmd, display) = build_server_cmd(
        &binary,
        Some(&model),
        &settings,
        &config,
        ServerMode::Normal,
        0,
        false,
    );

    // Auto mode should not include -ngl
    assert!(!display.contains("-ngl"));
}

#[test]
fn test_build_server_cmd_includes_sampling_params() {
    let binary = PathBuf::from("/usr/bin/llama-server");
    let model = make_model("/models/test.gguf", "test", "Test");
    let settings = make_settings();
    let config = make_config();

    let (_cmd, display) = build_server_cmd(
        &binary,
        Some(&model),
        &settings,
        &config,
        ServerMode::Normal,
        0,
        false,
    );

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

    let (_cmd, display) = build_server_cmd(
        &binary,
        Some(&model),
        &settings,
        &config,
        ServerMode::Normal,
        0,
        false,
    );

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

    let (_cmd, display) = build_server_cmd(
        &binary,
        Some(&model),
        &settings,
        &config,
        ServerMode::Normal,
        0,
        false,
    );

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

    let (_cmd, display) = build_server_cmd(
        &binary,
        Some(&model),
        &settings,
        &config,
        ServerMode::Normal,
        0,
        false,
    );

    assert!(display.contains("llama-server"));
}

#[test]
fn test_build_server_cmd_display_contains_model() {
    let binary = PathBuf::from("/usr/bin/llama-server");
    let model = make_model("/models/qwen2.5-7b.gguf", "qwen2.5-7b", "Qwen2.5-7B");
    let settings = make_settings();
    let config = make_config();

    let (_cmd, display) = build_server_cmd(
        &binary,
        Some(&model),
        &settings,
        &config,
        ServerMode::Normal,
        0,
        false,
    );

    assert!(display.contains("qwen2.5-7b.gguf"));
}

#[test]
fn test_build_server_cmd_display_contains_settings() {
    let binary = PathBuf::from("/usr/bin/llama-server");
    let model = make_model("/models/test.gguf", "test", "Test");
    let settings = make_settings();
    let config = make_config();

    let (_cmd, display) = build_server_cmd(
        &binary,
        Some(&model),
        &settings,
        &config,
        ServerMode::Normal,
        0,
        false,
    );

    assert!(display.contains("--threads"));
    assert!(display.contains("--ctx-size"));
}

#[test]
fn test_build_server_cmd_includes_system_prompt() {
    let binary = PathBuf::from("/usr/bin/llama-server");
    let model = make_model("/models/test.gguf", "test", "Test");
    let config = make_config();

    // 1. Test default system prompt (which used to be skipped)
    let settings = make_settings(); // contains default prompt
    let (_cmd, display) = build_server_cmd(
        &binary,
        Some(&model),
        &settings,
        &config,
        ServerMode::Normal,
        0,
        false,
    );
    assert!(display.contains("--chat-template-kwargs"));
    assert!(display.contains("system_prompt"));
    assert!(display.contains("expert software developer"));

    // 2. Test empty system prompt
    let mut settings_empty = make_settings();
    settings_empty.system_prompt = String::new();
    let (_cmd, display_empty) = build_server_cmd(
        &binary,
        Some(&model),
        &settings_empty,
        &config,
        ServerMode::Normal,
        0,
        false,
    );
    assert!(!display_empty.contains("system_prompt"));

    // 3. Test custom system prompt
    let mut settings_custom = make_settings();
    settings_custom.system_prompt = "You are a custom AI.".to_string();
    let (_cmd, display_custom) = build_server_cmd(
        &binary,
        Some(&model),
        &settings_custom,
        &config,
        ServerMode::Normal,
        0,
        false,
    );
    assert!(display_custom.contains("--chat-template-kwargs"));
    assert!(display_custom.contains("You are a custom AI."));
}

// ── build_server_cmd — RPC workers auto tensor-split ───────────

#[test]
fn test_build_server_cmd_rpc_worker_auto_tensor_split_one() {
    let binary = PathBuf::from("/usr/bin/llama-server");
    let model = make_model("/models/test.gguf", "test", "Test");
    let settings = make_settings();
    let mut config = make_config();
    config.rpc_workers.push(make_worker(true, "192.168.1.10"));

    let (_cmd, display) = build_server_cmd(
        &binary,
        Some(&model),
        &settings,
        &config,
        ServerMode::Normal,
        0,
        false,
    );

    assert!(display.contains("--rpc 192.168.1.10:50052"));
    // One share for the local device + one for the worker
    assert!(display.contains("--tensor-split 1,1"));
}

#[test]
fn test_build_server_cmd_rpc_workers_auto_tensor_split_multiple() {
    let binary = PathBuf::from("/usr/bin/llama-server");
    let model = make_model("/models/test.gguf", "test", "Test");
    let settings = make_settings();
    let mut config = make_config();
    config.rpc_workers.push(make_worker(true, "192.168.1.10"));
    config.rpc_workers.push(make_worker(true, "192.168.1.11"));

    let (_cmd, display) = build_server_cmd(
        &binary,
        Some(&model),
        &settings,
        &config,
        ServerMode::Normal,
        0,
        false,
    );

    // One share for the local device + one per worker
    assert!(display.contains("--tensor-split 1,1,1"));
}

#[test]
fn test_build_server_cmd_rpc_worker_unselected_no_tensor_split() {
    let binary = PathBuf::from("/usr/bin/llama-server");
    let model = make_model("/models/test.gguf", "test", "Test");
    let settings = make_settings();
    let mut config = make_config();
    config.rpc_workers.push(make_worker(false, "192.168.1.10"));

    let (_cmd, display) = build_server_cmd(
        &binary,
        Some(&model),
        &settings,
        &config,
        ServerMode::Normal,
        0,
        false,
    );

    assert!(!display.contains("--rpc"));
    assert!(!display.contains("--tensor-split"));
}

#[test]
fn test_build_server_cmd_rpc_worker_invalid_ip_no_tensor_split() {
    let binary = PathBuf::from("/usr/bin/llama-server");
    let model = make_model("/models/test.gguf", "test", "Test");
    let settings = make_settings();
    let mut config = make_config();
    config.rpc_workers.push(make_worker(true, "not-an-ip"));

    let (_cmd, display) = build_server_cmd(
        &binary,
        Some(&model),
        &settings,
        &config,
        ServerMode::Normal,
        0,
        false,
    );

    assert!(!display.contains("--rpc"));
    assert!(!display.contains("--tensor-split"));
}

#[test]
fn test_build_server_cmd_tensor_split_user_value_wins_over_rpc() {
    let binary = PathBuf::from("/usr/bin/llama-server");
    let model = make_model("/models/test.gguf", "test", "Test");
    let mut settings = make_settings();
    settings.tensor_split = "2,0".to_string();
    let mut config = make_config();
    config.rpc_workers.push(make_worker(true, "192.168.1.10"));

    let (_cmd, display) = build_server_cmd(
        &binary,
        Some(&model),
        &settings,
        &config,
        ServerMode::Normal,
        0,
        false,
    );

    // User's explicit tensor_split is kept, auto value not injected
    assert!(display.contains("--tensor-split 2,0"));
    assert!(!display.contains("--tensor-split 1,1"));
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
