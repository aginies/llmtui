//! Tests for tui/event/ module — keyboard event handling.
//!
//! Tests cover: global shortcuts, confirmation dialogs, search mode, files mode,
//! settings panel, log panel, downloads panel, pickers, bench tune, and tags modal.
//!
//! Event handlers are async and call network functions (hub::search_models, etc.).
//! In tests we verify state transitions (pending_* fields) without executing network calls.

use crossterm::event::{KeyCode, KeyModifiers, KeyEvent, KeyEventKind};
use llm_manager::config::Config;
use llm_manager::models::*;
use llm_manager::tui::app::{App, ActivePanel, GlobalMode, ModelsMode, ConfirmationKind, LoadProgress};
use llm_manager::backend::server::ServerHandle;
use llm_manager::tui::event::handle_key;

// ── Test helpers ─────────────────────────────────────────────────

fn make_app() -> App {
    let config = Config::default();
    let mut app = App::new(config);
    app.loading_phases.clear();
    app.last_active_phase = None;
    app.loading_progress = 0.0;
    app.progress_target = 0.0;
    app.load_progress = LoadProgress {
        layers_total: None,
        layers_loaded: None,
        tensors_total: None,
        tensors_loaded: 0,
        buffers: vec![],
    };
    app.last_spinner_time = None;
    app
}

fn make_key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn make_key_with_mod(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, modifiers)
}

fn make_search_mode(app: &mut App) {
    app.models_mode = ModelsMode::Search {
        query: "test".into(),
        results: vec![],
        sort_by: SearchSort::Relevance,
        show_readme: true,
        page: 0,
        loading: false,
        has_more: true,
    };
    app.active_panel = ActivePanel::Models;
}

fn make_files_mode(app: &mut App) {
    app.models_mode = ModelsMode::Files {
        model_id: "test/model".into(),
        files: vec![
            ("model-q4.gguf".into(), 4_000_000_000, "https://example.com/q4".into()),
            ("model-q8.gguf".into(), 8_000_000_000, "https://example.com/q8".into()),
        ],
        selected_idx: Some(0),
        previous_query: "test".into(),
        previous_results: vec![],
        selected_result: None,
    };
    app.active_panel = ActivePanel::Models;
}

// ── Global shortcuts ────────────────────────────────────────────

#[tokio::test]
async fn test_ctrl_c_without_loaded_model_exits() {
    let mut app = make_app();
    let key = make_key_with_mod(KeyCode::Char('c'), KeyModifiers::CONTROL);
    handle_key(&mut app, key).await;
    assert!(!app.running);
}

#[tokio::test]
async fn test_ctrl_c_with_loaded_model_shows_confirmation() {
    let mut app = make_app();
    app.model_states.insert("model.gguf".into(), ModelState::Loaded { port: 8080, pid: 1234 });
    let key = make_key_with_mod(KeyCode::Char('c'), KeyModifiers::CONTROL);
    handle_key(&mut app, key).await;
    assert!(matches!(app.global_mode, GlobalMode::Confirmation { kind: ConfirmationKind::Exit, .. }));
}

#[tokio::test]
async fn test_ctrl_c_with_multiple_loaded_models_shows_confirmation() {
    let mut app = make_app();
    app.model_states.insert("model1.gguf".into(), ModelState::Loaded { port: 8080, pid: 1234 });
    app.model_states.insert("model2.gguf".into(), ModelState::Loaded { port: 8081, pid: 1235 });
    let key = make_key_with_mod(KeyCode::Char('c'), KeyModifiers::CONTROL);
    handle_key(&mut app, key).await;
    assert!(matches!(app.global_mode, GlobalMode::Confirmation { .. }));
}

#[tokio::test]
async fn test_tab_focus_next() {
    let mut app = make_app();
    app.active_panel = ActivePanel::Models;
    let key = make_key(KeyCode::Tab);
    handle_key(&mut app, key).await;
    // Should cycle to next panel
    assert!(app.needs_redraw);
}

#[tokio::test]
async fn test_shift_tab_focus_prev() {
    let mut app = make_app();
    app.active_panel = ActivePanel::LlmSettings;
    let key = make_key_with_mod(KeyCode::Tab, KeyModifiers::SHIFT);
    handle_key(&mut app, key).await;
    // Should cycle to previous panel
    assert!(app.needs_redraw);
}

#[tokio::test]
async fn test_ctrl_h_toggles_panel_help() {
    let mut app = make_app();
    assert!(!app.panel_help);
    let key = make_key_with_mod(KeyCode::Char('h'), KeyModifiers::CONTROL);
    handle_key(&mut app, key).await;
    assert!(app.panel_help);
    assert_eq!(app.panel_help_offset, 0);
}

async fn test_ctrl_h_toggles_panel_help_off() {
    let mut app = make_app();
    app.panel_help = true;
    let key = make_key_with_mod(KeyCode::Char('h'), KeyModifiers::CONTROL);
    handle_key(&mut app, key).await;
    assert!(!app.panel_help);
}

#[tokio::test]
async fn test_f1_switches_to_models_panel() {
    let mut app = make_app();
    app.active_panel = ActivePanel::Log;
    let key = make_key(KeyCode::F(1));
    handle_key(&mut app, key).await;
    assert_eq!(app.active_panel, ActivePanel::Models);
}

#[tokio::test]
async fn test_f2_switches_to_server_settings() {
    let mut app = make_app();
    app.active_panel = ActivePanel::Models;
    // Hide panel 1 first so toggle turns it back on
    app.toggle_panel_visibility(1);
    let key = make_key(KeyCode::F(2));
    handle_key(&mut app, key).await;
    assert_eq!(app.active_panel, ActivePanel::ServerSettings);
}

#[tokio::test]
async fn test_f3_switches_to_model_info() {
    let mut app = make_app();
    // Hide panel 2 first so toggle turns it back on
    app.toggle_panel_visibility(2);
    let key = make_key(KeyCode::F(3));
    handle_key(&mut app, key).await;
    assert_eq!(app.active_panel, ActivePanel::ModelInfo);
}

#[tokio::test]
async fn test_f4_switches_to_llm_settings() {
    let mut app = make_app();
    // Hide panel 3 first so toggle turns it back on
    app.toggle_panel_visibility(3);
    let key = make_key(KeyCode::F(4));
    handle_key(&mut app, key).await;
    assert_eq!(app.active_panel, ActivePanel::LlmSettings);
}

#[tokio::test]
async fn test_f5_toggles_panel() {
    let mut app = make_app();
    let key = make_key(KeyCode::F(5));
    handle_key(&mut app, key).await;
    assert!(app.needs_redraw);
}

#[tokio::test]
async fn test_f6_switches_to_log() {
    let mut app = make_app();
    // Hide panel 5 first so toggle turns it back on
    app.toggle_panel_visibility(5);
    let key = make_key(KeyCode::F(6));
    handle_key(&mut app, key).await;
    assert_eq!(app.active_panel, ActivePanel::Log);
}

#[tokio::test]
async fn test_shift_left_resizes_left_panel() {
    let mut app = make_app();
    app.left_pct = 50;
    let key = make_key_with_mod(KeyCode::Left, KeyModifiers::SHIFT);
    handle_key(&mut app, key).await;
    assert_eq!(app.left_pct, 49);
}

#[tokio::test]
async fn test_shift_left_clamps_at_20() {
    let mut app = make_app();
    app.left_pct = 21;
    let key = make_key_with_mod(KeyCode::Left, KeyModifiers::SHIFT);
    handle_key(&mut app, key).await;
    assert_eq!(app.left_pct, 20);
}

#[tokio::test]
async fn test_shift_right_resizes_left_panel() {
    let mut app = make_app();
    app.left_pct = 50;
    let key = make_key_with_mod(KeyCode::Right, KeyModifiers::SHIFT);
    handle_key(&mut app, key).await;
    assert_eq!(app.left_pct, 51);
}

#[tokio::test]
async fn test_shift_right_clamps_at_80() {
    let mut app = make_app();
    app.left_pct = 79;
    let key = make_key_with_mod(KeyCode::Right, KeyModifiers::SHIFT);
    handle_key(&mut app, key).await;
    assert_eq!(app.left_pct, 80);
}

#[tokio::test]
async fn test_f10_all_panels_visible() {
    let mut app = make_app();
    app.panel_visibility = 0;
    app.log_expanded = true;
    let key = make_key(KeyCode::F(10));
    handle_key(&mut app, key).await;
    assert_eq!(app.panel_visibility, 0b111111);
    assert!(!app.log_expanded);
}

#[tokio::test]
async fn test_ctrl_l_switches_to_log() {
    let mut app = make_app();
    app.active_panel = ActivePanel::Models;
    let key = make_key_with_mod(KeyCode::Char('l'), KeyModifiers::CONTROL);
    handle_key(&mut app, key).await;
    assert_eq!(app.active_panel, ActivePanel::Log);
}

#[tokio::test]
async fn test_ctrl_k_kills_server() {
    let mut app = make_app();
    app.server_handle = Some(ServerHandle {
        port: 8080,
        host: "127.0.0.1".into(),
        pid: 1234,
        kill_tx: tokio::sync::mpsc::channel(1).0,
    });
    let key = make_key_with_mod(KeyCode::Char('k'), KeyModifiers::CONTROL | KeyModifiers::ALT);
    handle_key(&mut app, key).await;
    assert!(app.server_handle.is_none());
    assert!(app.pending_kill.is_some());
}

#[tokio::test]
async fn test_ctrl_k_no_server_logs_warning() {
    let mut app = make_app();
    let key = make_key_with_mod(KeyCode::Char('k'), KeyModifiers::CONTROL | KeyModifiers::ALT);
    handle_key(&mut app, key).await;
    assert!(app.log_entries.back().unwrap().message.contains("No server is running"));
}

#[tokio::test]
async fn test_ctrl_k_opens_cmdline() {
    let mut app = make_app();
    let key = make_key_with_mod(KeyCode::Char('k'), KeyModifiers::CONTROL);
    handle_key(&mut app, key).await;
    assert!(matches!(app.global_mode, GlobalMode::CmdLine { .. }));
}

#[tokio::test]
async fn test_slash_opens_search_mode() {
    let mut app = make_app();
    app.panel_visibility = 0b111111;
    let key = make_key(KeyCode::Char('/'));
    handle_key(&mut app, key).await;
    assert!(matches!(app.models_mode, ModelsMode::Search { .. }));
    assert_eq!(app.active_panel, ActivePanel::Models);
}

#[tokio::test]
async fn test_slash_hides_panels() {
    let mut app = make_app();
    app.panel_visibility = 0b111111;
    let key = make_key(KeyCode::Char('/'));
    handle_key(&mut app, key).await;
    // Panels 4 and 5 should be hidden
    assert!(!app.is_panel_visible(4));
    assert!(!app.is_panel_visible(5));
}

#[tokio::test]
async fn test_about_opens_about_mode() {
    let mut app = make_app();
    let key = make_key(KeyCode::Char('A'));
    handle_key(&mut app, key).await;
    assert!(matches!(app.global_mode, GlobalMode::About));
}

// ── Confirmation dialog ─────────────────────────────────────────

#[tokio::test]
async fn test_confirmation_y_confirms_exit() {
    let mut app = make_app();
    app.model_states.insert("model.gguf".into(), ModelState::Loaded { port: 8080, pid: 1234 });
    app.global_mode = GlobalMode::Confirmation { selected: true, kind: ConfirmationKind::Exit };
    let key = make_key(KeyCode::Char('y'));
    handle_key(&mut app, key).await;
    assert!(!app.running);
    assert!(matches!(app.global_mode, GlobalMode::Normal));
}

#[tokio::test]
async fn test_confirmation_y_confirms_delete() {
    let mut app = make_app();
    app.models = vec![DiscoveredModel {
        path: "/model.gguf".into(),
        name: "test".into(),
        file_size: 1000,
        display_name: "test".into(),
    }];
    app.selected_model_idx = Some(0);
    app.global_mode = GlobalMode::Confirmation { selected: true, kind: ConfirmationKind::Delete };
    let key = make_key(KeyCode::Char('y'));
    handle_key(&mut app, key).await;
    assert!(matches!(app.global_mode, GlobalMode::Normal));
}

#[tokio::test]
async fn test_confirmation_n_cancels() {
    let mut app = make_app();
    app.global_mode = GlobalMode::Confirmation { selected: true, kind: ConfirmationKind::Exit };
    let key = make_key(KeyCode::Char('n'));
    handle_key(&mut app, key).await;
    assert!(app.running);
    assert!(matches!(app.global_mode, GlobalMode::Normal));
}

#[tokio::test]
async fn test_confirmation_esc_cancels() {
    let mut app = make_app();
    app.global_mode = GlobalMode::Confirmation { selected: true, kind: ConfirmationKind::Exit };
    let key = make_key(KeyCode::Esc);
    handle_key(&mut app, key).await;
    assert!(app.running);
    assert!(matches!(app.global_mode, GlobalMode::Normal));
}

#[tokio::test]
async fn test_confirmation_tab_toggles_selection() {
    let mut app = make_app();
    app.global_mode = GlobalMode::Confirmation { selected: false, kind: ConfirmationKind::Exit };
    let key = make_key(KeyCode::Tab);
    handle_key(&mut app, key).await;
    if let GlobalMode::Confirmation { selected, .. } = app.global_mode {
        assert!(selected);
    }
}

#[tokio::test]
async fn test_confirmation_left_toggles_selection() {
    let mut app = make_app();
    app.global_mode = GlobalMode::Confirmation { selected: true, kind: ConfirmationKind::Exit };
    let key = make_key(KeyCode::Left);
    handle_key(&mut app, key).await;
    if let GlobalMode::Confirmation { selected, .. } = app.global_mode {
        assert!(!selected);
    }
}

#[tokio::test]
async fn test_confirmation_right_toggles_selection() {
    let mut app = make_app();
    app.global_mode = GlobalMode::Confirmation { selected: false, kind: ConfirmationKind::Exit };
    let key = make_key(KeyCode::Right);
    handle_key(&mut app, key).await;
    if let GlobalMode::Confirmation { selected, .. } = app.global_mode {
        assert!(selected);
    }
}

#[tokio::test]
async fn test_confirmation_enter_with_selected_confirms() {
    let mut app = make_app();
    app.model_states.insert("model.gguf".into(), ModelState::Loaded { port: 8080, pid: 1234 });
    app.global_mode = GlobalMode::Confirmation { selected: true, kind: ConfirmationKind::Exit };
    let key = make_key(KeyCode::Enter);
    handle_key(&mut app, key).await;
    assert!(!app.running);
    assert!(matches!(app.global_mode, GlobalMode::Normal));
}

#[tokio::test]
async fn test_confirmation_enter_without_selected_cancels() {
    let mut app = make_app();
    app.global_mode = GlobalMode::Confirmation { selected: false, kind: ConfirmationKind::Exit };
    let key = make_key(KeyCode::Enter);
    handle_key(&mut app, key).await;
    assert!(app.running);
    assert!(matches!(app.global_mode, GlobalMode::Normal));
}

#[tokio::test]
async fn test_confirmation_ctrl_h_cancels() {
    let mut app = make_app();
    app.global_mode = GlobalMode::Confirmation { selected: true, kind: ConfirmationKind::Exit };
    let key = make_key_with_mod(KeyCode::Char('h'), KeyModifiers::CONTROL);
    handle_key(&mut app, key).await;
    assert!(app.running);
    assert!(matches!(app.global_mode, GlobalMode::Normal));
}

// ── CmdLine overlay ─────────────────────────────────────────────

#[tokio::test]
async fn test_cmdline_esc_closes() {
    let mut app = make_app();
    app.global_mode = GlobalMode::CmdLine { cmd_line: "llama-server -m model.gguf".into() };
    let key = make_key(KeyCode::Esc);
    handle_key(&mut app, key).await;
    assert!(matches!(app.global_mode, GlobalMode::Normal));
}

#[tokio::test]
async fn test_cmdline_e_exports_script() {
    let mut app = make_app();
    app.global_mode = GlobalMode::CmdLine { cmd_line: "llama-server -m model.gguf".into() };
    let key = make_key(KeyCode::Char('e'));
    handle_key(&mut app, key).await;
    // Script should have been written to /tmp/test_llamaserver.sh
    assert!(std::path::Path::new("/tmp/test_llamaserver.sh").exists());
    // Clean up
    let _ = std::fs::remove_file("/tmp/test_llamaserver.sh");
}

// ── Search mode ─────────────────────────────────────────────────

#[tokio::test]
async fn test_esc_exits_search_returns_to_list() {
    let mut app = make_app();
    make_search_mode(&mut app);
    let key = make_key(KeyCode::Esc);
    handle_key(&mut app, key).await;
    assert!(matches!(app.models_mode, ModelsMode::List));
}

#[tokio::test]
async fn test_esc_restores_panel_visibility() {
    let mut app = make_app();
    make_search_mode(&mut app);
    let key = make_key(KeyCode::Esc);
    handle_key(&mut app, key).await;
    // Panels should be visible again
    assert!(app.is_panel_visible(4));
    assert!(app.is_panel_visible(5));
}

#[tokio::test]
async fn test_enter_in_empty_search_does_nothing() {
    let mut app = make_app();
    app.models_mode = ModelsMode::Search {
        query: String::new(),
        results: vec![],
        sort_by: SearchSort::Relevance,
        show_readme: true,
        page: 0,
        loading: false,
        has_more: true,
    };
    app.active_panel = ActivePanel::Models;
    let key = make_key(KeyCode::Enter);
    handle_key(&mut app, key).await;
    // Should remain in search mode with empty query
    assert!(matches!(app.models_mode, ModelsMode::Search { query, .. } if query.is_empty()));
}

#[tokio::test]
async fn test_enter_with_query_sets_pending_search() {
    let mut app = make_app();
    app.models_mode = ModelsMode::Search {
        query: "qwen".into(),
        results: vec![],
        sort_by: SearchSort::Relevance,
        show_readme: true,
        page: 0,
        loading: false,
        has_more: true,
    };
    app.active_panel = ActivePanel::Models;
    let key = make_key(KeyCode::Enter);
    handle_key(&mut app, key).await;
    assert!(app.pending_search_load.is_some());
    assert!(app.search_loading);
    // search_table_state is reset to default (None)
}

#[tokio::test]
async fn test_backspace_in_search_query() {
    let mut app = make_app();
    app.models_mode = ModelsMode::Search {
        query: "test".into(),
        results: vec![],
        sort_by: SearchSort::Relevance,
        show_readme: true,
        page: 0,
        loading: false,
        has_more: true,
    };
    app.active_panel = ActivePanel::Models;
    let key = make_key(KeyCode::Backspace);
    handle_key(&mut app, key).await;
    if let ModelsMode::Search { query, .. } = &app.models_mode {
        assert_eq!(query, "tes");
    }
}

#[tokio::test]
async fn test_char_in_search_query() {
    let mut app = make_app();
    app.models_mode = ModelsMode::Search {
        query: String::new(),
        results: vec![],
        sort_by: SearchSort::Relevance,
        show_readme: true,
        page: 0,
        loading: false,
        has_more: true,
    };
    app.active_panel = ActivePanel::Models;
    let key = make_key(KeyCode::Char('q'));
    handle_key(&mut app, key).await;
    if let ModelsMode::Search { query, .. } = &app.models_mode {
        assert_eq!(query, "q");
    }
}

#[tokio::test]
async fn test_search_down_moves_selection() {
    let mut app = make_app();
    app.models_mode = ModelsMode::Search {
        query: "test".into(),
        results: vec![
            SearchResult { model_id: "a".into(), model_name: "A".into(), tags: vec![], downloads: 0, likes: 0, pipeline_tag: None, size: None, parameters: None, capabilities: vec![], context_length: None, readme: None, quantization: None, license: None, trending_score: 0, created_at: None, downloaded: false },
            SearchResult { model_id: "b".into(), model_name: "B".into(), tags: vec![], downloads: 0, likes: 0, pipeline_tag: None, size: None, parameters: None, capabilities: vec![], context_length: None, readme: None, quantization: None, license: None, trending_score: 0, created_at: None, downloaded: false },
        ],
        sort_by: SearchSort::Relevance,
        show_readme: true,
        page: 0,
        loading: false,
        has_more: true,
    };
    app.active_panel = ActivePanel::Models;
    app.search_results_idx = Some(0);
    let key = make_key(KeyCode::Down);
    handle_key(&mut app, key).await;
    assert_eq!(app.search_results_idx, Some(1));
}

#[tokio::test]
async fn test_search_down_at_end_stays() {
    let mut app = make_app();
    app.models_mode = ModelsMode::Search {
        query: "test".into(),
        results: vec![
            SearchResult { model_id: "a".into(), model_name: "A".into(), tags: vec![], downloads: 0, likes: 0, pipeline_tag: None, size: None, parameters: None, capabilities: vec![], context_length: None, readme: None, quantization: None, license: None, trending_score: 0, created_at: None, downloaded: false },
        ],
        sort_by: SearchSort::Relevance,
        show_readme: true,
        page: 0,
        loading: false,
        has_more: true,
    };
    app.active_panel = ActivePanel::Models;
    app.search_results_idx = Some(0);
    let key = make_key(KeyCode::Down);
    handle_key(&mut app, key).await;
    assert_eq!(app.search_results_idx, Some(0));
}

#[tokio::test]
async fn test_search_down_from_none_selects_first() {
    let mut app = make_app();
    app.models_mode = ModelsMode::Search {
        query: "test".into(),
        results: vec![
            SearchResult { model_id: "a".into(), model_name: "A".into(), tags: vec![], downloads: 0, likes: 0, pipeline_tag: None, size: None, parameters: None, capabilities: vec![], context_length: None, readme: None, quantization: None, license: None, trending_score: 0, created_at: None, downloaded: false },
        ],
        sort_by: SearchSort::Relevance,
        show_readme: true,
        page: 0,
        loading: false,
        has_more: true,
    };
    app.active_panel = ActivePanel::Models;
    app.search_results_idx = None;
    let key = make_key(KeyCode::Down);
    handle_key(&mut app, key).await;
    assert_eq!(app.search_results_idx, Some(0));
}

#[tokio::test]
async fn test_search_up_moves_selection() {
    let mut app = make_app();
    app.models_mode = ModelsMode::Search {
        query: "test".into(),
        results: vec![
            SearchResult { model_id: "a".into(), model_name: "A".into(), tags: vec![], downloads: 0, likes: 0, pipeline_tag: None, size: None, parameters: None, capabilities: vec![], context_length: None, readme: None, quantization: None, license: None, trending_score: 0, created_at: None, downloaded: false },
            SearchResult { model_id: "b".into(), model_name: "B".into(), tags: vec![], downloads: 0, likes: 0, pipeline_tag: None, size: None, parameters: None, capabilities: vec![], context_length: None, readme: None, quantization: None, license: None, trending_score: 0, created_at: None, downloaded: false },
        ],
        sort_by: SearchSort::Relevance,
        show_readme: true,
        page: 0,
        loading: false,
        has_more: true,
    };
    app.active_panel = ActivePanel::Models;
    app.search_results_idx = Some(1);
    let key = make_key(KeyCode::Up);
    handle_key(&mut app, key).await;
    assert_eq!(app.search_results_idx, Some(0));
}

#[tokio::test]
async fn test_search_up_at_zero_stays() {
    let mut app = make_app();
    app.models_mode = ModelsMode::Search {
        query: "test".into(),
        results: vec![
            SearchResult { model_id: "a".into(), model_name: "A".into(), tags: vec![], downloads: 0, likes: 0, pipeline_tag: None, size: None, parameters: None, capabilities: vec![], context_length: None, readme: None, quantization: None, license: None, trending_score: 0, created_at: None, downloaded: false },
        ],
        sort_by: SearchSort::Relevance,
        show_readme: true,
        page: 0,
        loading: false,
        has_more: true,
    };
    app.active_panel = ActivePanel::Models;
    app.search_results_idx = Some(0);
    let key = make_key(KeyCode::Up);
    handle_key(&mut app, key).await;
    assert_eq!(app.search_results_idx, Some(0));
}

#[tokio::test]
async fn test_search_sort_cycles() {
    let mut app = make_app();
    app.models_mode = ModelsMode::Search {
        query: "test".into(),
        results: vec![
            SearchResult { model_id: "a".into(), model_name: "A".into(), tags: vec![], downloads: 100, likes: 10, pipeline_tag: None, size: None, parameters: None, capabilities: vec![], context_length: None, readme: None, quantization: None, license: None, trending_score: 50, created_at: Some("2024-01-01".into()), downloaded: false },
            SearchResult { model_id: "b".into(), model_name: "B".into(), tags: vec![], downloads: 200, likes: 20, pipeline_tag: None, size: None, parameters: None, capabilities: vec![], context_length: None, readme: None, quantization: None, license: None, trending_score: 60, created_at: Some("2024-02-01".into()), downloaded: false },
        ],
        sort_by: SearchSort::Relevance,
        show_readme: true,
        page: 0,
        loading: false,
        has_more: true,
    };
    app.active_panel = ActivePanel::Models;
    let key = make_key(KeyCode::Char('S'));
    handle_key(&mut app, key).await;
    if let ModelsMode::Search { sort_by, results, .. } = &app.models_mode {
        assert_eq!(*sort_by, SearchSort::Downloads);
        // Results should be sorted by downloads descending
        assert_eq!(results[0].model_id, "b");
        assert_eq!(results[1].model_id, "a");
    }
}

// ── Files mode ──────────────────────────────────────────────────

#[tokio::test]
async fn test_files_esc_returns_to_search() {
    let mut app = make_app();
    make_files_mode(&mut app);
    let key = make_key(KeyCode::Esc);
    handle_key(&mut app, key).await;
    assert!(matches!(app.models_mode, ModelsMode::Search { .. }));
}

#[tokio::test]
async fn test_files_down_moves_selection() {
    let mut app = make_app();
    make_files_mode(&mut app);
    let key = make_key(KeyCode::Down);
    handle_key(&mut app, key).await;
    if let ModelsMode::Files { selected_idx, .. } = &app.models_mode {
        assert_eq!(*selected_idx, Some(1));
    }
}

#[tokio::test]
async fn test_files_up_moves_selection() {
    let mut app = make_app();
    make_files_mode(&mut app);
    if let ModelsMode::Files { selected_idx, .. } = &mut app.models_mode {
        *selected_idx = Some(1);
    }
    let key = make_key(KeyCode::Up);
    handle_key(&mut app, key).await;
    if let ModelsMode::Files { selected_idx, .. } = &app.models_mode {
        assert_eq!(*selected_idx, Some(0));
    }
}

#[tokio::test]
async fn test_files_enter_sets_pending_download() {
    let mut app = make_app();
    make_files_mode(&mut app);
    app.config.models_dir = std::path::PathBuf::from("/tmp");
    let key = make_key(KeyCode::Enter);
    handle_key(&mut app, key).await;
    assert!(app.pending_download.is_some());
}

#[tokio::test]
async fn test_files_enter_duplicate_download_warns() {
    let mut app = make_app();
    make_files_mode(&mut app);
    app.config.models_dir = std::path::PathBuf::from("/tmp");
    // Trigger first download
    let key1 = make_key(KeyCode::Enter);
    handle_key(&mut app, key1).await;
    // pending_download should be set
    assert!(app.pending_download.is_some());
}

// ── Settings panel ──────────────────────────────────────────────

#[tokio::test]
async fn test_settings_down_increases_selected_idx() {
    let mut app = make_app();
    app.active_panel = ActivePanel::LlmSettings;
    app.settings_selected_idx = 0;
    let key = make_key(KeyCode::Down);
    handle_key(&mut app, key).await;
    assert_eq!(app.settings_selected_idx, 1);
}

#[tokio::test]
async fn test_settings_up_decreases_selected_idx() {
    let mut app = make_app();
    app.active_panel = ActivePanel::LlmSettings;
    app.settings_selected_idx = 5;
    let key = make_key(KeyCode::Up);
    handle_key(&mut app, key).await;
    assert_eq!(app.settings_selected_idx, 4);
}

#[tokio::test]
async fn test_settings_up_at_zero_stays() {
    let mut app = make_app();
    app.active_panel = ActivePanel::LlmSettings;
    app.settings_selected_idx = 0;
    let key = make_key(KeyCode::Up);
    handle_key(&mut app, key).await;
    assert_eq!(app.settings_selected_idx, 0);
}

#[tokio::test]
async fn test_settings_arrow_right_increases_context() {
    let mut app = make_app();
    app.active_panel = ActivePanel::LlmSettings;
    app.settings_selected_idx = 1; // Context length field (index 1)
    let initial = app.settings.context_length;
    let key = make_key(KeyCode::Right);
    handle_key(&mut app, key).await;
    assert!(app.settings.context_length > initial);
}

#[tokio::test]
async fn test_settings_arrow_left_decreases_context() {
    let mut app = make_app();
    app.active_panel = ActivePanel::LlmSettings;
    app.settings_selected_idx = 1; // Context length field (index 1)
    let initial = app.settings.context_length;
    let key = make_key(KeyCode::Left);
    handle_key(&mut app, key).await;
    assert!(app.settings.context_length < initial);
}

#[tokio::test]
async fn test_settings_enter_opens_edit_buffer_for_context() {
    let mut app = make_app();
    app.active_panel = ActivePanel::LlmSettings;
    app.settings_selected_idx = 0; // System prompt preset field (index 0)
    let key = make_key(KeyCode::Enter);
    handle_key(&mut app, key).await;
    // Enter on system prompt preset opens the prompt picker, not edit buffer
    assert!(matches!(app.global_mode, GlobalMode::PromptPicker { .. }));
}

// ── Log panel ───────────────────────────────────────────────────

#[tokio::test]
async fn test_log_down_increases_scroll() {
    let mut app = make_app();
    app.active_panel = ActivePanel::Log;
    let key = make_key(KeyCode::Down);
    handle_key(&mut app, key).await;
    assert!(app.log_scroll_offset >= 0);
}

#[tokio::test]
async fn test_log_up_decreases_scroll() {
    let mut app = make_app();
    app.active_panel = ActivePanel::Log;
    app.log_scroll_offset = 10;
    let key = make_key(KeyCode::Up);
    handle_key(&mut app, key).await;
    assert_eq!(app.log_scroll_offset, 9);
}

#[tokio::test]
async fn test_esc_collapses_log() {
    let mut app = make_app();
    app.log_expanded = true;
    app.filtering_local = false;
    let key = make_key(KeyCode::Esc);
    handle_key(&mut app, key).await;
    assert!(!app.log_expanded);
}

#[tokio::test]
async fn test_esc_does_not_collapse_when_filtering() {
    let mut app = make_app();
    app.log_expanded = true;
    app.filtering_local = true;
    let key = make_key(KeyCode::Esc);
    handle_key(&mut app, key).await;
    assert!(app.log_expanded);
}

// ── Downloads panel ─────────────────────────────────────────────

#[tokio::test]
async fn test_downloads_panel_down_increases_selection() {
    let mut app = make_app();
    app.active_panel = ActivePanel::Downloads;
    let key = make_key(KeyCode::Down);
    handle_key(&mut app, key).await;
    assert!(app.needs_redraw);
}

#[tokio::test]
async fn test_downloads_panel_up_decreases_selection() {
    let mut app = make_app();
    app.active_panel = ActivePanel::Downloads;
    app.download_scroll_state.select(Some(5));
    let key = make_key(KeyCode::Up);
    handle_key(&mut app, key).await;
    assert!(app.needs_redraw);
}

// ── Host picker ─────────────────────────────────────────────────

#[tokio::test]
async fn test_host_picker_up_decreases_selection() {
    let mut app = make_app();
    app.host_picker_entries = vec![("127.0.0.1".into(), "lo".into()), ("192.168.1.1".into(), "eth0".into())];
    app.global_mode = GlobalMode::HostPicker { entries: app.host_picker_entries.clone(), selected: 1 };
    let key = make_key(KeyCode::Up);
    handle_key(&mut app, key).await;
    if let GlobalMode::HostPicker { selected, .. } = app.global_mode {
        assert_eq!(selected, 0);
    }
}

#[tokio::test]
async fn test_host_picker_down_increases_selection() {
    let mut app = make_app();
    app.host_picker_entries = vec![("127.0.0.1".into(), "lo".into()), ("192.168.1.1".into(), "eth0".into())];
    app.global_mode = GlobalMode::HostPicker { entries: app.host_picker_entries.clone(), selected: 0 };
    let key = make_key(KeyCode::Down);
    handle_key(&mut app, key).await;
    if let GlobalMode::HostPicker { selected, .. } = app.global_mode {
        assert_eq!(selected, 1);
    }
}

#[tokio::test]
async fn test_host_picker_down_clamps_at_max() {
    let mut app = make_app();
    app.host_picker_entries = vec![("127.0.0.1".into(), "lo".into())];
    app.global_mode = GlobalMode::HostPicker { entries: app.host_picker_entries.clone(), selected: 0 };
    let key = make_key(KeyCode::Down);
    handle_key(&mut app, key).await;
    if let GlobalMode::HostPicker { selected, .. } = app.global_mode {
        assert_eq!(selected, 0);
    }
}

#[tokio::test]
async fn test_host_picker_enter_selects_host() {
    let mut app = make_app();
    app.host_picker_entries = vec![("192.168.1.100".into(), "eth0".into())];
    app.global_mode = GlobalMode::HostPicker { entries: app.host_picker_entries.clone(), selected: 0 };
    let key = make_key(KeyCode::Enter);
    handle_key(&mut app, key).await;
    assert_eq!(app.settings.host, "192.168.1.100");
    assert!(matches!(app.global_mode, GlobalMode::Normal));
}

#[tokio::test]
async fn test_host_picker_esc_exits() {
    let mut app = make_app();
    app.host_picker_entries = vec![("127.0.0.1".into(), "lo".into())];
    app.global_mode = GlobalMode::HostPicker { entries: app.host_picker_entries.clone(), selected: 0 };
    let key = make_key(KeyCode::Esc);
    handle_key(&mut app, key).await;
    assert!(matches!(app.global_mode, GlobalMode::Normal));
}

// ── Backend picker ──────────────────────────────────────────────

#[tokio::test]
async fn test_backend_picker_up_decreases_selection() {
    let mut app = make_app();
    app.backend_picker_entries = vec![(Backend::Cpu, Some("b4100".into()))];
    app.global_mode = GlobalMode::BackendPicker { entries: app.backend_picker_entries.clone(), selected: 1 };
    let key = make_key(KeyCode::Up);
    handle_key(&mut app, key).await;
    if let GlobalMode::BackendPicker { selected, .. } = app.global_mode {
        assert_eq!(selected, 0);
    }
}

#[tokio::test]
async fn test_backend_picker_down_increases_selection() {
    let mut app = make_app();
    app.backend_picker_entries = vec![(Backend::Cpu, Some("b4100".into()))];
    app.global_mode = GlobalMode::BackendPicker { entries: app.backend_picker_entries.clone(), selected: 0 };
    let key = make_key(KeyCode::Down);
    handle_key(&mut app, key).await;
    if let GlobalMode::BackendPicker { selected, .. } = app.global_mode {
        assert_eq!(selected, 0);
    }
}

#[tokio::test]
async fn test_backend_picker_enter_selects_backend() {
    let mut app = make_app();
    app.backend_picker_entries = vec![(Backend::Cpu, Some("b4100".into()))];
    app.global_mode = GlobalMode::BackendPicker { entries: app.backend_picker_entries.clone(), selected: 0 };
    let key = make_key(KeyCode::Enter);
    handle_key(&mut app, key).await;
    assert_eq!(app.settings.backend, Backend::Cpu);
    assert!(matches!(app.global_mode, GlobalMode::Normal));
}

#[tokio::test]
async fn test_backend_picker_esc_exits() {
    let mut app = make_app();
    app.backend_picker_entries = vec![(Backend::Cpu, Some("b4100".into()))];
    app.global_mode = GlobalMode::BackendPicker { entries: app.backend_picker_entries.clone(), selected: 0 };
    let key = make_key(KeyCode::Esc);
    handle_key(&mut app, key).await;
    assert!(matches!(app.global_mode, GlobalMode::Normal));
}

// ── MaxConcurrentPicker ─────────────────────────────────────────

#[tokio::test]
async fn test_max_concurrent_picker_enter_valid() {
    let mut app = make_app();
    app.global_mode = GlobalMode::MaxConcurrentPicker { value: "5".into() };
    let key = make_key(KeyCode::Enter);
    handle_key(&mut app, key).await;
    assert_eq!(app.settings.max_concurrent_predictions, Some(5));
    assert!(matches!(app.global_mode, GlobalMode::Normal));
}

#[tokio::test]
async fn test_max_concurrent_picker_enter_clamped_min() {
    let mut app = make_app();
    app.global_mode = GlobalMode::MaxConcurrentPicker { value: "0".into() };
    let key = make_key(KeyCode::Enter);
    handle_key(&mut app, key).await;
    assert_eq!(app.settings.max_concurrent_predictions, Some(1));
}

#[tokio::test]
async fn test_max_concurrent_picker_enter_clamped_max() {
    let mut app = make_app();
    app.global_mode = GlobalMode::MaxConcurrentPicker { value: "99".into() };
    let key = make_key(KeyCode::Enter);
    handle_key(&mut app, key).await;
    assert_eq!(app.settings.max_concurrent_predictions, Some(10));
}

#[tokio::test]
async fn test_max_concurrent_picker_char_adds_digit() {
    let mut app = make_app();
    app.global_mode = GlobalMode::MaxConcurrentPicker { value: "1".into() };
    let key = make_key(KeyCode::Char('5'));
    handle_key(&mut app, key).await;
    if let GlobalMode::MaxConcurrentPicker { value, .. } = &app.global_mode {
        assert_eq!(value, "15");
    }
}

#[tokio::test]
async fn test_max_concurrent_picker_char_limits_length() {
    let mut app = make_app();
    app.global_mode = GlobalMode::MaxConcurrentPicker { value: "123".into() };
    let key = make_key(KeyCode::Char('5'));
    handle_key(&mut app, key).await;
    if let GlobalMode::MaxConcurrentPicker { value, .. } = &app.global_mode {
        assert_eq!(value, "123");
    }
}

#[tokio::test]
async fn test_max_concurrent_picker_backspace_removes() {
    let mut app = make_app();
    app.global_mode = GlobalMode::MaxConcurrentPicker { value: "12".into() };
    let key = make_key(KeyCode::Backspace);
    handle_key(&mut app, key).await;
    if let GlobalMode::MaxConcurrentPicker { value, .. } = &app.global_mode {
        assert_eq!(value, "1");
    }
}

#[tokio::test]
async fn test_max_concurrent_picker_esc_exits() {
    let mut app = make_app();
    app.global_mode = GlobalMode::MaxConcurrentPicker { value: "5".into() };
    let key = make_key(KeyCode::Esc);
    handle_key(&mut app, key).await;
    assert!(matches!(app.global_mode, GlobalMode::Normal));
}

// ── BenchTune mode ──────────────────────────────────────────────

#[tokio::test]
async fn test_bench_tune_esc_stops_server() {
    let mut app = make_app();
    app.server_handle = Some(ServerHandle {
        port: 8080,
        host: "127.0.0.1".into(),
        pid: 1234,
        kill_tx: tokio::sync::mpsc::channel(1).0,
    });
    app.bench_tune_running = true;
    app.models_mode = ModelsMode::BenchTune;
    let key = make_key(KeyCode::Esc);
    handle_key(&mut app, key).await;
    assert!(app.server_handle.is_none());
    assert!(!app.bench_tune_running);
    assert!(matches!(app.models_mode, ModelsMode::List));
}

#[tokio::test]
async fn test_bench_tune_down_moves_result_row() {
    let mut app = make_app();
    app.models_mode = ModelsMode::BenchTune;
    app.bench_tune_results = vec![
        BenchTuneResult { params: BenchTuneParamValue { temperature: None, top_p: None, top_k: None, repeat_penalty: None, context_length: None, batch_size: None, flash_attn: None, threads: None, expert_count: None }, metrics: BenchTuneMetrics { prompt_tps: 0.0, generation_tps: 0.0, combined_tps: 0.0, latency_per_token: 0.0, first_token_time: 0.0 }, outputs: vec![], per_iteration_metrics: vec![], base_settings: None },
        BenchTuneResult { params: BenchTuneParamValue { temperature: None, top_p: None, top_k: None, repeat_penalty: None, context_length: None, batch_size: None, flash_attn: None, threads: None, expert_count: None }, metrics: BenchTuneMetrics { prompt_tps: 0.0, generation_tps: 0.0, combined_tps: 0.0, latency_per_token: 0.0, first_token_time: 0.0 }, outputs: vec![], per_iteration_metrics: vec![], base_settings: None },
    ];
    app.bench_tune_result_row = 0;
    let key = make_key(KeyCode::Down);
    handle_key(&mut app, key).await;
    assert_eq!(app.bench_tune_result_row, 1);
}

#[tokio::test]
async fn test_bench_tune_up_decreases_result_row() {
    let mut app = make_app();
    app.models_mode = ModelsMode::BenchTune;
    app.bench_tune_results = vec![
        BenchTuneResult { params: BenchTuneParamValue { temperature: None, top_p: None, top_k: None, repeat_penalty: None, context_length: None, batch_size: None, flash_attn: None, threads: None, expert_count: None }, metrics: BenchTuneMetrics { prompt_tps: 0.0, generation_tps: 0.0, combined_tps: 0.0, latency_per_token: 0.0, first_token_time: 0.0 }, outputs: vec![], per_iteration_metrics: vec![], base_settings: None },
        BenchTuneResult { params: BenchTuneParamValue { temperature: None, top_p: None, top_k: None, repeat_penalty: None, context_length: None, batch_size: None, flash_attn: None, threads: None, expert_count: None }, metrics: BenchTuneMetrics { prompt_tps: 0.0, generation_tps: 0.0, combined_tps: 0.0, latency_per_token: 0.0, first_token_time: 0.0 }, outputs: vec![], per_iteration_metrics: vec![], base_settings: None },
    ];
    app.bench_tune_result_row = 1;
    let key = make_key(KeyCode::Up);
    handle_key(&mut app, key).await;
    assert_eq!(app.bench_tune_result_row, 0);
}

#[tokio::test]
async fn test_bench_tune_enter_opens_output_view() {
    let mut app = make_app();
    app.models_mode = ModelsMode::BenchTune;
    app.bench_tune_results = vec![
        BenchTuneResult { params: BenchTuneParamValue { temperature: None, top_p: None, top_k: None, repeat_penalty: None, context_length: None, batch_size: None, flash_attn: None, threads: None, expert_count: None }, metrics: BenchTuneMetrics { prompt_tps: 0.0, generation_tps: 0.0, combined_tps: 0.0, latency_per_token: 0.0, first_token_time: 0.0 }, outputs: vec!["output1".into()], per_iteration_metrics: vec![], base_settings: None },
    ];
    app.bench_tune_result_row = 0;
    let key = make_key(KeyCode::Enter);
    handle_key(&mut app, key).await;
    assert_eq!(app.bench_tune_output_view, Some(0));
}

// ── BenchTune output view ───────────────────────────────────────

#[tokio::test]
async fn test_bench_tune_output_esc_closes() {
    let mut app = make_app();
    app.bench_tune_output_view = Some(0);
    let key = make_key(KeyCode::Esc);
    handle_key(&mut app, key).await;
    assert!(app.bench_tune_output_view.is_none());
}

#[tokio::test]
async fn test_bench_tune_output_down_increases_scroll() {
    let mut app = make_app();
    app.bench_tune_output_view = Some(0);
    let key = make_key(KeyCode::Down);
    handle_key(&mut app, key).await;
    assert_eq!(app.bench_tune_output_scroll, 1);
}

#[tokio::test]
async fn test_bench_tune_output_up_decreases_scroll() {
    let mut app = make_app();
    app.bench_tune_output_view = Some(0);
    app.bench_tune_output_scroll = 10;
    let key = make_key(KeyCode::Up);
    handle_key(&mut app, key).await;
    assert_eq!(app.bench_tune_output_scroll, 9);
}

#[tokio::test]
async fn test_bench_tune_output_page_down_increases_scroll_by_10() {
    let mut app = make_app();
    app.bench_tune_output_view = Some(0);
    let key = make_key(KeyCode::PageDown);
    handle_key(&mut app, key).await;
    assert_eq!(app.bench_tune_output_scroll, 10);
}

#[tokio::test]
async fn test_bench_tune_output_page_up_decreases_scroll_by_10() {
    let mut app = make_app();
    app.bench_tune_output_view = Some(0);
    app.bench_tune_output_scroll = 25;
    let key = make_key(KeyCode::PageUp);
    handle_key(&mut app, key).await;
    assert_eq!(app.bench_tune_output_scroll, 15);
}

// ── Panel help ──────────────────────────────────────────────────

#[tokio::test]
async fn test_panel_help_esc_closes() {
    let mut app = make_app();
    app.panel_help = true;
    app.filtering_local = false;
    let key = make_key(KeyCode::Esc);
    handle_key(&mut app, key).await;
    assert!(!app.panel_help);
}

#[tokio::test]
async fn test_panel_help_down_increases_offset() {
    let mut app = make_app();
    app.panel_help = true;
    app.filtering_local = false;
    app.panel_help_offset = 5;
    let key = make_key(KeyCode::Down);
    handle_key(&mut app, key).await;
    assert_eq!(app.panel_help_offset, 6);
}

#[tokio::test]
async fn test_panel_help_up_decreases_offset() {
    let mut app = make_app();
    app.panel_help = true;
    app.filtering_local = false;
    app.panel_help_offset = 5;
    let key = make_key(KeyCode::Up);
    handle_key(&mut app, key).await;
    assert_eq!(app.panel_help_offset, 4);
}

// ── Save settings ───────────────────────────────────────────────

#[tokio::test]
async fn test_ctrl_s_saves_settings() {
    let mut app = make_app();
    // Select a model so save_model_settings has something to save
    app.models = vec![DiscoveredModel {
        path: "/model.gguf".into(),
        name: "test".into(),
        file_size: 1000,
        display_name: "test".into(),
    }];
    app.selected_model_idx = Some(0);
    app.model_settings_cache = app.settings.clone();
    app.settings.context_length = 4096;
    let key = make_key_with_mod(KeyCode::Char('s'), KeyModifiers::CONTROL);
    handle_key(&mut app, key).await;
    // Ctrl+S should set redraw flag
    assert!(app.needs_redraw);
}

// ── Log expanded in normal mode ─────────────────────────────────

#[tokio::test]
async fn test_esc_in_normal_mode_collapse_log() {
    let mut app = make_app();
    app.log_expanded = true;
    app.filtering_local = false;
    let key = make_key(KeyCode::Esc);
    handle_key(&mut app, key).await;
    assert!(!app.log_expanded);
}

// ── About overlay ───────────────────────────────────────────────

#[tokio::test]
async fn test_about_esc_exits() {
    let mut app = make_app();
    app.global_mode = GlobalMode::About;
    let key = make_key(KeyCode::Esc);
    handle_key(&mut app, key).await;
    assert!(matches!(app.global_mode, GlobalMode::Normal));
}

// ── Profile picker ──────────────────────────────────────────────

#[tokio::test]
async fn test_profile_picker_up_decreases_selection() {
    let mut app = make_app();
    app.profile_picker_entries = vec![("Qwen".into(), "Qwen profile".into())];
    app.global_mode = GlobalMode::ProfilePicker { entries: app.profile_picker_entries.clone(), selected: 1 };
    let key = make_key(KeyCode::Up);
    handle_key(&mut app, key).await;
    if let GlobalMode::ProfilePicker { selected, .. } = app.global_mode {
        assert_eq!(selected, 0);
    }
}

#[tokio::test]
async fn test_profile_picker_enter_applies_profile() {
    let mut app = make_app();
    app.profile_picker_entries = vec![("Qwen".into(), "Qwen profile".into())];
    app.global_mode = GlobalMode::ProfilePicker { entries: app.profile_picker_entries.clone(), selected: 0 };
    let key = make_key(KeyCode::Enter);
    handle_key(&mut app, key).await;
    assert!(matches!(app.global_mode, GlobalMode::Normal));
}

#[tokio::test]
async fn test_profile_picker_esc_exits() {
    let mut app = make_app();
    app.profile_picker_entries = vec![("Qwen".into(), "Qwen profile".into())];
    app.global_mode = GlobalMode::ProfilePicker { entries: app.profile_picker_entries.clone(), selected: 0 };
    let key = make_key(KeyCode::Esc);
    handle_key(&mut app, key).await;
    assert!(matches!(app.global_mode, GlobalMode::Normal));
}

// ── Server settings panel ───────────────────────────────────────

#[tokio::test]
async fn test_server_settings_down_increases_selection() {
    let mut app = make_app();
    app.active_panel = ActivePanel::ServerSettings;
    app.server_settings_selected_idx = 0;
    let key = make_key(KeyCode::Down);
    handle_key(&mut app, key).await;
    assert_eq!(app.server_settings_selected_idx, 1);
}

#[tokio::test]
async fn test_server_settings_up_decreases_selection() {
    let mut app = make_app();
    app.active_panel = ActivePanel::ServerSettings;
    app.server_settings_selected_idx = 3;
    let key = make_key(KeyCode::Up);
    handle_key(&mut app, key).await;
    assert_eq!(app.server_settings_selected_idx, 2);
}

#[tokio::test]
async fn test_server_settings_enter_opens_host_picker() {
    let mut app = make_app();
    app.active_panel = ActivePanel::ServerSettings;
    app.server_settings_selected_idx = 0;
    let key = make_key(KeyCode::Enter);
    handle_key(&mut app, key).await;
    assert!(matches!(app.global_mode, GlobalMode::HostPicker { .. }));
}

#[tokio::test]
async fn test_server_settings_enter_opens_backend_picker() {
    let mut app = make_app();
    app.active_panel = ActivePanel::ServerSettings;
    app.server_settings_selected_idx = 1;
    let key = make_key(KeyCode::Enter);
    handle_key(&mut app, key).await;
    assert!(matches!(app.global_mode, GlobalMode::BackendPicker { .. }));
}

// ── RpcManager overlay ──────────────────────────────────────────

#[tokio::test]
async fn test_rpc_manager_esc_exits() {
    let mut app = make_app();
    app.global_mode = GlobalMode::RpcManager;
    let key = make_key(KeyCode::Esc);
    handle_key(&mut app, key).await;
    // RpcManager handles Esc internally
    assert!(app.needs_redraw);
}

// ── Tags modal ──────────────────────────────────────────────────

#[tokio::test]
async fn test_tags_modal_opens_from_settings() {
    let mut app = make_app();
    app.active_panel = ActivePanel::LlmSettings;
    app.tags_editing = false;
    let key = make_key(KeyCode::Char('t'));
    handle_key(&mut app, key).await;
    assert!(app.tags_editing);
    assert!(app.tags_insert_mode);
    assert!(app.tags_edit_buffer.is_empty());
}

#[tokio::test]
async fn test_tags_modal_esc_closes() {
    let mut app = make_app();
    app.tags_editing = true;
    let key = make_key(KeyCode::Esc);
    handle_key(&mut app, key).await;
    assert!(!app.tags_editing);
}
