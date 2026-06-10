//! Integration tests for log-based parsing of llama.cpp output.
//!
//! Tests validate regex patterns against real llama.cpp log output samples.
//! Covers phase detection, tensor parsing, layer offloading, buffer sizes, and error detection.

use llm_manager::config::Config;
use llm_manager::models::{LoadProgress, ModelState};
use llm_manager::tui::app::App;
use llm_manager::tui::app::state::parsing::*;
use llm_manager::tui::app::types::LoadingPhase;
use std::path::PathBuf;

fn make_app() -> App {
    let config = Config {
        models_dirs: vec![],
        llama_server: PathBuf::new(),
        default: llm_manager::config::DefaultParams::default(),
        model_overrides: llm_manager::config::ModelConfigStore::new(),
        profiles: llm_manager::config::ProfileStore::new(),
        system_prompt_presets: llm_manager::config::PresetStore::new(),
        rpc_workers: Vec::new(),
        search_limit: 50,
        active_panel: llm_manager::tui::app::types::ActivePanel::Models,
        left_pct: 55,
        language: "en".to_string(),
        onboarding_complete: false,
    };
    let mut app = App::new(config);
    app.loading.loading_phases.clear();
    app.loading.last_active_phase = None;
    app.loading.loading_progress = 0.0;
    app.loading.progress_target = 0.0;
    app.loading.load_progress = LoadProgress::default();
    app.loading.last_spinner_time = None;
    app
}

// ── Phase detection tests ────────────────────────────────────────

#[test]
fn test_phase_server_starting_llama_server() {
    let mut app = make_app();
    let msg = "llama server starting on port 49222";
    app.add_log(msg, llm_manager::config::LogLevel::Info);
    assert!(
        app.loading
            .loading_phases
            .contains(&LoadingPhase::ServerStarting)
    );
}

#[test]
fn test_phase_server_starting_ggml() {
    let mut app = make_app();
    let msg = "ggml version: 1234";
    app.add_log(msg, llm_manager::config::LogLevel::Info);
    assert!(
        app.loading
            .loading_phases
            .contains(&LoadingPhase::ServerStarting)
    );
}

#[test]
fn test_phase_loading_model() {
    let mut app = make_app();
    let msg = "llama_model_loader: - arch: llama  vocab: tokenizer";
    app.add_log(msg, llm_manager::config::LogLevel::Info);
    assert!(
        app.loading
            .loading_phases
            .contains(&LoadingPhase::LoadingModel)
    );
}

#[test]
fn test_phase_loaded_meta() {
    let mut app = make_app();
    let msg = "llama_model_loader: loaded 423 meta data";
    app.add_log(msg, llm_manager::config::LogLevel::Info);
    assert!(
        app.loading
            .loading_phases
            .contains(&LoadingPhase::LoadingMeta)
    );
}

#[test]
fn test_phase_loaded_meta_alt_format() {
    let mut app = make_app();
    let msg = "llama_model_loader: Loaded meta data with 423 key-value pairs and 629 tensors from";
    app.add_log(msg, llm_manager::config::LogLevel::Info);
    assert!(
        app.loading
            .loading_phases
            .contains(&LoadingPhase::LoadingMeta)
    );
}

#[test]
fn test_phase_load_tensors() {
    let mut app = make_app();
    let msg = "llama_model_loader: load tensors:";
    app.add_log(msg, llm_manager::config::LogLevel::Info);
    assert!(
        app.loading
            .loading_phases
            .contains(&LoadingPhase::LoadingTensors)
    );
}

#[test]
fn test_phase_server_listening() {
    let mut app = make_app();
    let msg = "server listening = true";
    app.add_log(msg, llm_manager::config::LogLevel::Info);
    assert!(
        app.loading
            .loading_phases
            .contains(&LoadingPhase::ServerListening)
    );
}

#[test]
fn test_phase_server_http_listening() {
    let mut app = make_app();
    let msg = "http server listening on port 49222";
    app.add_log(msg, llm_manager::config::LogLevel::Info);
    assert!(
        app.loading
            .loading_phases
            .contains(&LoadingPhase::ServerListening)
    );
}

#[test]
fn test_phase_server_initializing_slots() {
    let mut app = make_app();
    let msg = "load_model: initializing slots, n_slots = 4";
    app.add_log(msg, llm_manager::config::LogLevel::Info);
    assert!(
        app.loading
            .loading_phases
            .contains(&LoadingPhase::ServerListening)
    );
}

// ── Tensor parsing tests ─────────────────────────────────────────

#[test]
fn test_parse_tensor_count_with_of() {
    let mut app = make_app();
    app.loading
        .loading_phases
        .insert(LoadingPhase::LoadingTensors);
    app.loading.last_active_phase = Some(LoadingPhase::LoadingTensors);
    let msg = "llama_model_loader: loading tensor  1 of  640, n_loaded 1";
    app.add_log(msg, llm_manager::config::LogLevel::Info);
    assert_eq!(app.loading.load_progress.tensors_loaded, 1);
    assert_eq!(app.loading.load_progress.tensors_total, Some(640));
}

#[test]
fn test_parse_tensor_count_with_out_of() {
    let mut app = make_app();
    app.loading
        .loading_phases
        .insert(LoadingPhase::LoadingTensors);
    app.loading.last_active_phase = Some(LoadingPhase::LoadingTensors);
    let msg = "llama_model_loader: loading tensor  1 out of  640, n_loaded 1";
    app.add_log(msg, llm_manager::config::LogLevel::Info);
    assert_eq!(app.loading.load_progress.tensors_loaded, 1);
    assert_eq!(app.loading.load_progress.tensors_total, Some(640));
}

#[test]
fn test_parse_tensor_count_updates() {
    let mut app = make_app();
    app.loading
        .loading_phases
        .insert(LoadingPhase::LoadingTensors);
    app.loading.last_active_phase = Some(LoadingPhase::LoadingTensors);

    app.add_log(
        "llama_model_loader: loading tensor  1 of  640, n_loaded 1",
        llm_manager::config::LogLevel::Info,
    );
    assert_eq!(app.loading.load_progress.tensors_loaded, 1);
    assert_eq!(app.loading.load_progress.tensors_total, Some(640));

    app.add_log(
        "llama_model_loader: loading tensor  320 of  640, n_loaded 320",
        llm_manager::config::LogLevel::Info,
    );
    assert_eq!(app.loading.load_progress.tensors_loaded, 320);
    assert_eq!(app.loading.load_progress.tensors_total, Some(640));
}

// ── Layer offloading tests ───────────────────────────────────────

#[test]
fn test_parse_offloading_layers() {
    let mut app = make_app();
    app.loading
        .loading_phases
        .insert(LoadingPhase::LoadingTensors);
    app.loading.last_active_phase = Some(LoadingPhase::LoadingTensors);
    let msg = "offloading 32 repeating layers to GPU";
    app.add_log(msg, llm_manager::config::LogLevel::Info);
    assert_eq!(app.loading.load_progress.layers_total, Some(32));
}

#[test]
fn test_parse_offloaded_layers_with_slash() {
    let mut app = make_app();
    app.loading
        .loading_phases
        .insert(LoadingPhase::LoadingTensors);
    app.loading.last_active_phase = Some(LoadingPhase::LoadingTensors);
    let msg = "offloaded 16/32 layers to GPU (3285.54 MiB)";
    app.add_log(msg, llm_manager::config::LogLevel::Info);
    assert_eq!(app.loading.load_progress.layers_loaded, Some(16));
    assert_eq!(app.loading.load_progress.layers_total, Some(32));
}

#[test]
fn test_parse_offloaded_layers_with_out_of() {
    let mut app = make_app();
    app.loading
        .loading_phases
        .insert(LoadingPhase::LoadingTensors);
    app.loading.last_active_phase = Some(LoadingPhase::LoadingTensors);
    let msg = "offloaded 32 out of 32 layers to GPU (3285.54 MiB)";
    app.add_log(msg, llm_manager::config::LogLevel::Info);
    assert_eq!(app.loading.load_progress.layers_loaded, Some(32));
    assert_eq!(app.loading.load_progress.layers_total, Some(32));
}

// ── Buffer size tests ────────────────────────────────────────────

#[test]
fn test_parse_model_buffer_size_vulkan() {
    let mut app = make_app();
    app.loading
        .loading_phases
        .insert(LoadingPhase::LoadingTensors);
    app.loading.last_active_phase = Some(LoadingPhase::LoadingTensors);
    let msg = "Vulkan0 model buffer size =  3285.54 MiB";
    app.add_log(msg, llm_manager::config::LogLevel::Info);
    assert_eq!(app.loading.load_progress.buffers.len(), 1);
    assert_eq!(app.loading.load_progress.buffers[0].device, "Vulkan0");
    assert!((app.loading.load_progress.buffers[0].buffer_size_mib - 3285.54).abs() < 0.01);
}

#[test]
fn test_parse_model_buffer_size_cpu() {
    let mut app = make_app();
    app.loading
        .loading_phases
        .insert(LoadingPhase::LoadingTensors);
    app.loading.last_active_phase = Some(LoadingPhase::LoadingTensors);
    let msg = "CPU model buffer size =  6571.09 MiB";
    app.add_log(msg, llm_manager::config::LogLevel::Info);
    assert_eq!(app.loading.load_progress.buffers.len(), 1);
    assert_eq!(app.loading.load_progress.buffers[0].device, "CPU");
    assert!((app.loading.load_progress.buffers[0].buffer_size_mib - 6571.09).abs() < 0.01);
}

#[test]
fn test_parse_kv_buffer_size() {
    let mut app = make_app();
    app.loading
        .loading_phases
        .insert(LoadingPhase::LoadingTensors);
    app.loading.last_active_phase = Some(LoadingPhase::LoadingTensors);
    let msg = "kv buffer size =  4194.30 MiB";
    app.add_log(msg, llm_manager::config::LogLevel::Info);
    assert_eq!(app.loading.load_progress.buffers.len(), 1);
    assert_eq!(app.loading.load_progress.buffers[0].device, "kv");
    assert!((app.loading.load_progress.buffers[0].buffer_size_mib - 4194.30).abs() < 0.01);
}

#[test]
fn test_parse_multiple_buffer_sizes() {
    let mut app = make_app();
    app.loading
        .loading_phases
        .insert(LoadingPhase::LoadingTensors);
    app.loading.last_active_phase = Some(LoadingPhase::LoadingTensors);
    app.add_log(
        "Vulkan0 model buffer size =  3285.54 MiB",
        llm_manager::config::LogLevel::Info,
    );
    app.add_log(
        "CPU model buffer size =  6571.09 MiB",
        llm_manager::config::LogLevel::Info,
    );
    app.add_log(
        "kv buffer size =  4194.30 MiB",
        llm_manager::config::LogLevel::Info,
    );
    assert_eq!(app.loading.load_progress.buffers.len(), 3);
}

// ── Error detection tests ────────────────────────────────────────

#[test]
fn test_error_oom_detection() {
    let msg = "ERROR: out of memory allocated_id=0 reserved_id=0 total=8589934592";
    assert!(is_loading_error(msg));
    assert!(is_oom_error(msg));
}

#[test]
fn test_error_outofdevicememory() {
    let msg = "vk::ERROR_OUT_OF_DEVICE_MEMORY";
    assert!(is_loading_error(msg));
    assert!(is_oom_error(msg));
}

#[test]
fn test_error_outofmemory_vk() {
    let msg = "VK_OUT_OF_MEMORY";
    assert!(is_loading_error(msg));
    assert!(is_oom_error(msg));
}

#[test]
fn test_error_generic_error() {
    let msg = "ERROR: failed to initialize backend";
    assert!(is_loading_error(msg));
    assert!(!is_oom_error(msg));
}

#[test]
fn test_error_failed_to_load() {
    let msg = "failed to load model";
    assert!(is_loading_error(msg));
    assert!(!is_oom_error(msg));
}

#[test]
fn test_error_exception() {
    let msg = "Exception: CUDA error occurred";
    assert!(is_loading_error(msg));
    assert!(!is_oom_error(msg));
}

#[test]
fn test_error_vk_systemerror() {
    let msg = "vk::SYSTEMERROR: something went wrong";
    assert!(is_loading_error(msg));
    assert!(!is_oom_error(msg));
}

#[test]
fn test_no_error_on_normal_line() {
    let msg = "llama_model_loader: loaded 423 meta data";
    assert!(!is_loading_error(msg));
}

#[test]
fn test_no_error_on_tensor_loading() {
    let msg = "loading tensor  1 of  640, n_loaded 1";
    assert!(!is_loading_error(msg));
}

// ── Full loading sequence tests ──────────────────────────────────

#[test]
fn test_full_loading_sequence_phases() {
    let mut app = make_app();

    app.add_log("ggml version: 1234", llm_manager::config::LogLevel::Info);
    assert!(
        app.loading
            .loading_phases
            .contains(&LoadingPhase::ServerStarting)
    );

    app.add_log(
        "llama_model_loader: - arch: llama  vocab: tokenizer",
        llm_manager::config::LogLevel::Info,
    );
    assert!(
        app.loading
            .loading_phases
            .contains(&LoadingPhase::LoadingModel)
    );

    app.add_log(
        "llama_model_loader: loaded 423 meta data",
        llm_manager::config::LogLevel::Info,
    );
    assert!(
        app.loading
            .loading_phases
            .contains(&LoadingPhase::LoadingMeta)
    );

    app.add_log(
        "llama_model_loader: load tensors:",
        llm_manager::config::LogLevel::Info,
    );
    assert!(
        app.loading
            .loading_phases
            .contains(&LoadingPhase::LoadingTensors)
    );

    app.add_log(
        "http server listening on port 49222",
        llm_manager::config::LogLevel::Info,
    );
    assert!(
        app.loading
            .loading_phases
            .contains(&LoadingPhase::ServerListening)
    );
}

#[test]
fn test_full_loading_sequence_details() {
    let mut app = make_app();

    // Trigger tensor loading phase
    app.add_log(
        "llama_model_loader: load tensors:",
        llm_manager::config::LogLevel::Info,
    );

    // Parse offloading info
    app.add_log(
        "offloading 32 repeating layers to GPU",
        llm_manager::config::LogLevel::Info,
    );
    assert_eq!(app.loading.load_progress.layers_total, Some(32));

    // Parse tensor loading
    app.add_log(
        "llama_model_loader: loading tensor  1 of  640, n_loaded 1",
        llm_manager::config::LogLevel::Info,
    );
    assert_eq!(app.loading.load_progress.tensors_loaded, 1);
    assert_eq!(app.loading.load_progress.tensors_total, Some(640));

    // Parse layer progress
    app.add_log(
        "offloaded 16/32 layers to GPU (3285.54 MiB)",
        llm_manager::config::LogLevel::Info,
    );
    assert_eq!(app.loading.load_progress.layers_loaded, Some(16));

    // Parse buffer sizes
    app.add_log(
        "Vulkan0 model buffer size =  3285.54 MiB",
        llm_manager::config::LogLevel::Info,
    );
    assert_eq!(app.loading.load_progress.buffers.len(), 1);
}

#[test]
fn test_error_triggers_reset() {
    let mut app = make_app();

    // Set a model to Loading state
    app.model_states
        .insert("test-model".to_string(), ModelState::Loading);

    // Send an error
    app.add_log(
        "ERROR: out of memory allocated_id=0",
        llm_manager::config::LogLevel::Error,
    );

    // Model should now be Failed
    match app.model_states.get("test-model") {
        Some(ModelState::Failed { .. }) => {}
        other => panic!("Expected Failed state, got {:?}", other),
    }
    assert!(app.ui.last_error_message.is_some());
}

#[test]
fn test_error_not_loading_model_unchanged() {
    let mut app = make_app();

    // Set a model to Available state (not loading)
    app.model_states
        .insert("test-model".to_string(), ModelState::Available);

    // Send an error
    app.add_log(
        "ERROR: something went wrong",
        llm_manager::config::LogLevel::Error,
    );

    // Model should still be Available (error detection only triggers during loading)
    assert!(matches!(
        app.model_states.get("test-model"),
        Some(ModelState::Available)
    ));
}

// ── Edge cases ───────────────────────────────────────────────────

#[test]
fn test_dot_fallback_progress() {
    let mut app = make_app();
    app.loading
        .loading_phases
        .insert(LoadingPhase::LoadingTensors);
    app.loading.last_active_phase = Some(LoadingPhase::LoadingTensors);

    // No explicit tensor count yet, dots should be counted as fallback
    let dots = ".".repeat(10);
    app.add_log(&dots, llm_manager::config::LogLevel::Info);
    assert_eq!(app.loading.load_progress.tensors_loaded, 10);
    assert_eq!(app.loading.load_progress.tensors_total, None);
}

#[test]
fn test_no_dot_fallback_when_tensor_count_set() {
    let mut app = make_app();
    app.loading
        .loading_phases
        .insert(LoadingPhase::LoadingTensors);
    app.loading.last_active_phase = Some(LoadingPhase::LoadingTensors);

    // Set explicit tensor count first
    app.add_log(
        "llama_model_loader: loading tensor  1 of  640, n_loaded 1",
        llm_manager::config::LogLevel::Info,
    );
    assert_eq!(app.loading.load_progress.tensors_total, Some(640));

    // Dots should NOT be counted as fallback now
    let dots = ".".repeat(10);
    app.add_log(&dots, llm_manager::config::LogLevel::Info);
    // tensors_loaded should still be 1 (from the explicit count), not 11
    assert_eq!(app.loading.load_progress.tensors_loaded, 1);
}

#[test]
fn test_case_insensitive_matching() {
    let mut app = make_app();

    // Lowercase variants should still match
    app.add_log(
        "llama_model_loader: load tensors:",
        llm_manager::config::LogLevel::Info,
    );
    assert!(
        app.loading
            .loading_phases
            .contains(&LoadingPhase::LoadingTensors)
    );

    app.add_log(
        "offloading 32 repeating layers to GPU",
        llm_manager::config::LogLevel::Info,
    );
    assert_eq!(app.loading.load_progress.layers_total, Some(32));
}

#[test]
fn test_phases_not_reset_on_new_lines() {
    let mut app = make_app();

    // Add multiple lines - phases should accumulate
    app.add_log("llama server starting", llm_manager::config::LogLevel::Info);
    app.add_log(
        "llama_model_loader: loaded 423 meta data",
        llm_manager::config::LogLevel::Info,
    );
    app.add_log(
        "llama_model_loader: load tensors:",
        llm_manager::config::LogLevel::Info,
    );

    // All phases should be present
    assert!(
        app.loading
            .loading_phases
            .contains(&LoadingPhase::ServerStarting)
    );
    assert!(
        app.loading
            .loading_phases
            .contains(&LoadingPhase::LoadingMeta)
    );
    assert!(
        app.loading
            .loading_phases
            .contains(&LoadingPhase::LoadingTensors)
    );
}
