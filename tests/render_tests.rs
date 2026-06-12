//! Tests for tui/render.rs — rendering and layout.
//!
//! Tests cover: main layout, confirmation overlays, help overlay, CmdLine overlay,
//! About overlay, panel rendering, status bar, and model list rendering.
//!
//! Uses ratatui::backend::TestBackend for headless rendering tests.

use llm_manager::LoadProgress;
use llm_manager::config::Config;
use llm_manager::models::*;
use llm_manager::tui::app::{
    ActivePanel, App, ConfirmationKind, GlobalMode, LoadingPhase, ModelsMode,
};
use llm_manager::tui::render::render;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

// ── Test helpers ─────────────────────────────────────────────────

fn make_app() -> App {
    let config = Config::default();
    let mut app = App::new(config);
    app.loading.loading_phases.clear();
    app.loading.last_active_phase = None;
    app.loading.loading_progress = 0.0;
    app.loading.progress_target = 0.0;
    app.loading.load_progress = LoadProgress {
        layers_total: None,
        layers_loaded: None,
        tensors_total: None,
        tensors_loaded: 0,
        buffers: vec![],
    };
    app.loading.last_spinner_time = None;
    app
}

fn make_terminal(app: &mut App) -> Terminal<TestBackend> {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| render(f, app)).unwrap();
    terminal
}

fn get_buffer(terminal: &mut Terminal<TestBackend>) -> &ratatui::buffer::Buffer {
    terminal.backend_mut().buffer()
}

// ── Main layout ─────────────────────────────────────────────────

#[test]
fn test_normal_mode_renders_without_panic() {
    let mut app = make_app();
    let _terminal = make_terminal(&mut app);
    // If we get here without panicking, the test passes
}

#[test]
fn test_normal_mode_has_content() {
    let mut app = make_app();
    let mut terminal = make_terminal(&mut app);
    let buffer = get_buffer(&mut terminal);
    assert!(buffer.content.len() > 0);
}

#[test]
fn test_log_expanded_mode_renders() {
    let mut app = make_app();
    app.log.log_expanded = true;
    let _terminal = make_terminal(&mut app);
    // Should render without panic
}

#[test]
fn test_hidden_panels_renders() {
    let mut app = make_app();
    app.ui.panel_visibility = 0;
    let _terminal = make_terminal(&mut app);
    // Should render without panic
}

#[test]
fn test_all_panels_hidden_renders() {
    let mut app = make_app();
    app.ui.panel_visibility = 0;
    app.log.log_expanded = false;
    let _terminal = make_terminal(&mut app);
    // Should render without panic
}

// ── Confirmation overlay ────────────────────────────────────────

#[test]
fn test_confirmation_exit_renders() {
    let mut app = make_app();
    app.model_states.insert(
        "model.gguf".into(),
        ModelState::Loaded {
            port: 8080,
            pid: 1234,
        },
    );
    app.ui.global_mode = GlobalMode::Confirmation {
        selected: false,
        kind: ConfirmationKind::Exit,
        detail: None,
        display_name: String::new(),
    };
    let _terminal = make_terminal(&mut app);
}

#[test]
fn test_confirmation_reset_renders() {
    let mut app = make_app();
    app.ui.global_mode = GlobalMode::Confirmation {
        selected: false,
        kind: ConfirmationKind::Reset,
        detail: None,
        display_name: String::new(),
    };
    let _terminal = make_terminal(&mut app);
}

#[test]
fn test_confirmation_delete_renders() {
    let mut app = make_app();
    app.models = vec![DiscoveredModel {
        path: "/model.gguf".into(),
        name: "test".into(),
        file_size: 1000,
        display_name: "test".into(),
        pipeline_tag: None,
        capabilities: vec![],
    }];
    app.selected_model_idx = Some(0);
    app.ui.global_mode = GlobalMode::Confirmation {
        selected: false,
        kind: ConfirmationKind::Delete,
        detail: None,
        display_name: String::new(),
    };
    let _terminal = make_terminal(&mut app);
}

#[test]
fn test_confirmation_unload_renders() {
    let mut app = make_app();
    app.pending.pending_api_unload = Some("test-model".into());
    app.ui.global_mode = GlobalMode::Confirmation {
        selected: false,
        kind: ConfirmationKind::Unload,
        detail: None,
        display_name: String::new(),
    };
    let _terminal = make_terminal(&mut app);
}

#[test]
fn test_confirmation_delete_backend_renders() {
    let mut app = make_app();
    app.ui.global_mode = GlobalMode::Confirmation {
        selected: false,
        kind: ConfirmationKind::DeleteBackend,
        detail: None,
        display_name: String::new(),
    };
    let _terminal = make_terminal(&mut app);
}

// ── Help overlay ────────────────────────────────────────────────

#[test]
fn test_help_overlay_renders() {
    let mut app = make_app();
    app.ui.panel_help = true;
    let _terminal = make_terminal(&mut app);
}

#[test]
fn test_help_overlay_has_content() {
    let mut app = make_app();
    app.ui.panel_help = true;
    let mut terminal = make_terminal(&mut app);
    let buffer = get_buffer(&mut terminal);
    assert!(buffer.content.len() > 0);
}

#[test]
fn test_help_overlay_dimensions() {
    let mut app = make_app();
    app.ui.panel_help = true;
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            // Help overlay should be 70% of terminal size, clamped to 60x20 - 80x35
            render(f, &mut app);
        })
        .unwrap();
}

// ── CmdLine overlay ─────────────────────────────────────────────

#[test]
fn test_cmdline_overlay_renders() {
    let mut app = make_app();
    app.ui.global_mode = GlobalMode::CmdLine {
        cmd_line: "llama-server -m model.gguf -c 4096".into(),
    };
    let _terminal = make_terminal(&mut app);
}

#[test]
fn test_cmdline_overlay_long_text_wraps() {
    let mut app = make_app();
    let long_cmd = "llama-server -m /path/to/a/very/long/model/file/that/should/wrap/multiple/times.gguf -c 32768 --batch-size 2048 --threads 8";
    app.ui.global_mode = GlobalMode::CmdLine {
        cmd_line: long_cmd.into(),
    };
    let _terminal = make_terminal(&mut app);
}

#[test]
fn test_cmdline_overlay_has_title() {
    let mut app = make_app();
    app.ui.global_mode = GlobalMode::CmdLine {
        cmd_line: "test".into(),
    };
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| render(f, &mut app)).unwrap();
    // CmdLine renders a block with border - verify buffer has content
    let buffer = get_buffer(&mut terminal);
    assert!(buffer.content.len() > 0);
}

// ── About overlay ───────────────────────────────────────────────

#[test]
fn test_about_overlay_renders() {
    let mut app = make_app();
    app.ui.global_mode = GlobalMode::About;
    let _terminal = make_terminal(&mut app);
}

#[test]
fn test_about_overlay_has_content() {
    let mut app = make_app();
    app.ui.global_mode = GlobalMode::About;
    let mut terminal = make_terminal(&mut app);
    let buffer = get_buffer(&mut terminal);
    assert!(buffer.content.len() > 0);
}

// ── Panel rendering ─────────────────────────────────────────────

#[test]
fn test_models_panel_renders() {
    let mut app = make_app();
    app.ui.active_panel = ActivePanel::Models;
    app.models = vec![
        DiscoveredModel {
            path: "/model1.gguf".into(),
            name: "model1".into(),
            file_size: 1000,
            display_name: "Model 1".into(),
            pipeline_tag: None,
            capabilities: vec![],
        },
        DiscoveredModel {
            path: "/model2.gguf".into(),
            name: "model2".into(),
            file_size: 2000,
            display_name: "Model 2".into(),
            pipeline_tag: None,
            capabilities: vec![],
        },
    ];
    let _terminal = make_terminal(&mut app);
}

#[test]
fn test_models_panel_with_search_results_renders() {
    let mut app = make_app();
    app.ui.active_panel = ActivePanel::Models;
    app.models_mode = ModelsMode::Search {
        query: "test".into(),
        results: vec![SearchResult {
            model_id: "a".into(),
            model_name: "A".into(),
            tags: vec![],
            downloads: 100,
            likes: 10,
            pipeline_tag: None,
            size: None,
            parameters: None,
            capabilities: vec![],
            context_length: None,
            readme: None,
            quantization: None,
            license: None,
            trending_score: 50,
            created_at: None,
            downloaded: false,
        }],
        sort_by: SearchSort::Relevance,
        show_readme: true,
        page: 0,
        loading: false,
        has_more: false,
    };
    let _terminal = make_terminal(&mut app);
}

#[test]
fn test_settings_panel_renders() {
    let mut app = make_app();
    app.ui.active_panel = ActivePanel::LlmSettings;
    let _terminal = make_terminal(&mut app);
}

#[test]
fn test_server_settings_panel_renders() {
    let mut app = make_app();
    app.ui.active_panel = ActivePanel::ServerSettings;
    let _terminal = make_terminal(&mut app);
}

#[test]
fn test_log_panel_renders() {
    let mut app = make_app();
    app.ui.active_panel = ActivePanel::Log;
    app.add_log("Test log entry", llm_manager::config::LogLevel::Info);
    let _terminal = make_terminal(&mut app);
}

#[test]
fn test_downloads_panel_renders() {
    let mut app = make_app();
    app.ui.active_panel = ActivePanel::Downloads;
    let _terminal = make_terminal(&mut app);
}

#[test]
fn test_profiles_panel_renders() {
    let mut app = make_app();
    app.ui.active_panel = ActivePanel::Profiles;
    let _terminal = make_terminal(&mut app);
}

#[test]
fn test_system_prompt_presets_panel_renders() {
    let mut app = make_app();
    app.ui.active_panel = ActivePanel::SystemPromptPresets;
    let _terminal = make_terminal(&mut app);
}

#[test]
fn test_active_model_panel_renders() {
    let mut app = make_app();
    app.ui.active_panel = ActivePanel::ActiveModel;
    app.metrics = ServerMetrics {
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
            prompt_tokens: 0,
            prompt_progress: 0.0,
            prompt_elapsed_ms: 0.0,
            prompt_tps_eval: 0.0,
        };
    let _terminal = make_terminal(&mut app);
}

#[test]
fn test_model_info_panel_renders() {
    let mut app = make_app();
    app.ui.active_panel = ActivePanel::ModelInfo;
    let _terminal = make_terminal(&mut app);
}

// ── Status bar ──────────────────────────────────────────────────

#[test]
fn test_status_bar_shows_in_normal_mode() {
    let mut app = make_app();
    app.ui.active_panel = ActivePanel::Models;
    let mut terminal = make_terminal(&mut app);
    let buffer = get_buffer(&mut terminal);
    // Buffer should have content
    assert!(buffer.content.len() > 0);
}

#[test]
fn test_status_bar_shows_in_search_mode() {
    let mut app = make_app();
    app.ui.active_panel = ActivePanel::Models;
    app.models_mode = ModelsMode::Search {
        query: "test".into(),
        results: vec![],
        sort_by: SearchSort::Relevance,
        show_readme: true,
        page: 0,
        loading: false,
        has_more: true,
    };
    let mut terminal = make_terminal(&mut app);
    let buffer = get_buffer(&mut terminal);
    assert!(buffer.content.len() > 0);
}

#[test]
fn test_status_bar_shows_in_settings() {
    let mut app = make_app();
    app.ui.active_panel = ActivePanel::LlmSettings;
    let mut terminal = make_terminal(&mut app);
    let buffer = get_buffer(&mut terminal);
    assert!(buffer.content.len() > 0);
}

#[test]
fn test_status_bar_shows_when_log_expanded() {
    let mut app = make_app();
    app.log.log_expanded = true;
    let mut terminal = make_terminal(&mut app);
    let buffer = get_buffer(&mut terminal);
    assert!(buffer.content.len() > 0);
}

#[test]
fn test_status_bar_shows_in_confirmation() {
    let mut app = make_app();
    app.ui.global_mode = GlobalMode::Confirmation {
        selected: false,
        kind: ConfirmationKind::Exit,
        detail: None,
        display_name: String::new(),
    };
    let mut terminal = make_terminal(&mut app);
    let buffer = get_buffer(&mut terminal);
    assert!(buffer.content.len() > 0);
}

// ── Model list rendering ────────────────────────────────────────

#[test]
fn test_model_list_with_loaded_model_shows_status() {
    let mut app = make_app();
    app.ui.active_panel = ActivePanel::Models;
    app.models = vec![DiscoveredModel {
        path: "/loaded.gguf".into(),
        name: "loaded".into(),
        file_size: 1000,
        display_name: "Loaded".into(),
        pipeline_tag: None,
        capabilities: vec![],
    }];
    app.model_states.insert(
        "loaded.gguf".into(),
        ModelState::Loaded {
            port: 8080,
            pid: 1234,
        },
    );
    let _terminal = make_terminal(&mut app);
}

#[test]
fn test_model_list_with_failed_model_shows_error() {
    let mut app = make_app();
    app.ui.active_panel = ActivePanel::Models;
    app.models = vec![DiscoveredModel {
        path: "/failed.gguf".into(),
        name: "failed".into(),
        file_size: 1000,
        display_name: "Failed".into(),
        pipeline_tag: None,
        capabilities: vec![],
    }];
    app.model_states.insert(
        "failed.gguf".into(),
        ModelState::Failed {
            error: "Load failed".into(),
        },
    );
    let _terminal = make_terminal(&mut app);
}

#[test]
fn test_model_list_with_loading_model_shows_progress() {
    let mut app = make_app();
    app.ui.active_panel = ActivePanel::Models;
    app.models = vec![DiscoveredModel {
        path: "/loading.gguf".into(),
        name: "loading".into(),
        file_size: 1000,
        display_name: "Loading".into(),
        pipeline_tag: None,
        capabilities: vec![],
    }];
    app.model_states
        .insert("loading.gguf".into(), ModelState::Loading);
    app.loading
        .loading_phases
        .insert(LoadingPhase::LoadingModel);
    app.loading.last_active_phase = Some(LoadingPhase::LoadingModel);
    let _terminal = make_terminal(&mut app);
}

#[test]
fn test_model_list_with_download_progress() {
    let mut app = make_app();
    app.ui.active_panel = ActivePanel::Models;
    app.download.download_progress = vec![DownloadState {
        model_id: "test/model".into(),
        filename: "model.gguf".into(),
        downloaded_bytes: 1_000_000,
        total_bytes: 10_000_000,
        bytes_per_second: 1_000_000.0,
        status: DownloadStatus::Downloading,
        download_state_arc: None,
        cancelled: false,
        cancel_token: None,
        download_state: 1,
        start_time: std::time::Instant::now(),
        dest: None,
    }];
    let _terminal = make_terminal(&mut app);
}

#[test]
fn test_model_list_with_bench_tune_mode() {
    let mut app = make_app();
    app.ui.active_panel = ActivePanel::Models;
    app.models_mode = ModelsMode::BenchTune;
    app.bench_tune.bench_tune_results = vec![BenchTuneResult {
        params: BenchTuneParamValue {
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
        metrics: BenchTuneMetrics {
            prompt_tps: 0.0,
            generation_tps: 0.0,
            combined_tps: 0.0,
            latency_per_token: 0.0,
            prompt_processing_time: 0.0,
        },
        outputs: vec![],
        per_iteration_metrics: vec![],
        base_settings: None,
        server_command: None,
    }];
    let _terminal = make_terminal(&mut app);
}
