//! Tests for tui/app.rs App state machine and helpers.
//!
//! Tests cover: App initialization, model filtering, panel visibility,
//  model selection changes, and state transitions.

use llm_manager::config::Config;
use llm_manager::models::*;
use llm_manager::tui::app::{ActivePanel, App, GlobalMode, ModelsMode};

// ── App initialization ─────────────────────────────────────────

#[test]
fn app_new_is_running() {
    let config = Config::default();
    let app = App::new(config);
    assert!(app.running);
}

#[test]
fn app_new_initial_state() {
    let config = Config::default();
    let app = App::new(config);
    assert!(app.models.is_empty());
    assert!(app.selected_model_idx.is_none());
    assert!(matches!(app.models_mode, ModelsMode::List));
    assert!(app.search.local_filter.is_empty());
    assert!(!app.search.filtering_local);
    assert!(!app.log.log_expanded);
    assert!(matches!(app.ui.global_mode, GlobalMode::Normal));
    assert_eq!(app.ui.active_panel, ActivePanel::Models);
}

#[test]
fn app_new_has_settings() {
    let config = Config::default();
    let app = App::new(config);
    // Settings should be initialized from config defaults
    assert!(app.settings.context_length > 0);
    assert!(app.settings.threads > 0);
}

#[test]
fn app_new_empty_log() {
    let config = Config::default();
    let app = App::new(config);
    // Should have at least the startup log entry
    assert!(!app.log.log_entries.is_empty());
}

#[test]
fn app_new_no_server_handle() {
    let config = Config::default();
    let app = App::new(config);
    assert!(app.server.server_handle.is_none());
}

#[test]
fn app_new_no_download_progress() {
    let config = Config::default();
    let app = App::new(config);
    assert!(app.download.download_progress.is_empty());
}

#[test]
fn app_new_metrics_default() {
    let config = Config::default();
    let app = App::new(config);
    assert!(!app.metrics.loaded);
    assert_eq!(app.metrics.tps, 0.0);
}

#[test]
fn app_new_default_left_pct() {
    let config = Config::default();
    let app = App::new(config);
    assert_eq!(app.ui.left_pct, 55);
}

// ── Model filtering ────────────────────────────────────────────

#[test]
fn app_get_filtered_model_indices_empty() {
    let config = Config::default();
    let mut app = App::new(config);
    app.models = vec![];
    let indices = app.get_filtered_model_indices();
    assert!(indices.is_empty());
}

#[test]
fn app_get_filtered_model_indices_no_filter() {
    let config = Config::default();
    let mut app = App::new(config);
    app.models = vec![
        DiscoveredModel {
            path: "/a.gguf".into(),
            name: "model-a".into(),
            file_size: 1000,
            display_name: "a".into(),
        },
        DiscoveredModel {
            path: "/b.gguf".into(),
            name: "model-b".into(),
            file_size: 2000,
            display_name: "b".into(),
        },
    ];
    let indices = app.get_filtered_model_indices();
    assert_eq!(indices, vec![0, 1]);
}

#[test]
fn app_get_filtered_model_indices_case_insensitive() {
    let config = Config::default();
    let mut app = App::new(config);
    app.models = vec![
        DiscoveredModel {
            path: "/a.gguf".into(),
            name: "Qwen2.5-7B".into(),
            file_size: 1000,
            display_name: "Qwen2.5-7B".into(),
        },
        DiscoveredModel {
            path: "/b.gguf".into(),
            name: "Llama3-8B".into(),
            file_size: 2000,
            display_name: "Llama3-8B".into(),
        },
    ];
    app.search.local_filter = "qwen".into();
    let indices = app.get_filtered_model_indices();
    assert_eq!(indices, vec![0]);
}

#[test]
fn app_get_filtered_model_indices_no_match() {
    let config = Config::default();
    let mut app = App::new(config);
    app.models = vec![DiscoveredModel {
        path: "/a.gguf".into(),
        name: "model-a".into(),
        file_size: 1000,
        display_name: "a".into(),
    }];
    app.search.local_filter = "nonexistent".into();
    let indices = app.get_filtered_model_indices();
    assert!(indices.is_empty());
}

// ── Panel visibility ───────────────────────────────────────────

#[test]
fn app_is_panel_visible_default_all_hidden() {
    let config = Config::default();
    let app = App::new(config);
    // panel_visibility = 0 (all bits cleared), so all panels 0-5 should be visible
    for i in 0..6 {
        assert!(app.is_panel_visible(i));
    }
}

#[test]
fn app_toggle_panel_visibility_hides() {
    let config = Config::default();
    let mut app = App::new(config);
    app.toggle_panel_visibility(0);
    assert!(!app.is_panel_visible(0));
}

#[test]
fn app_toggle_panel_visibility_shows() {
    let config = Config::default();
    let mut app = App::new(config);
    // Hide panel 0
    app.toggle_panel_visibility(0);
    assert!(!app.is_panel_visible(0));
    // Show it again
    app.toggle_panel_visibility(0);
    assert!(app.is_panel_visible(0));
}

#[test]
fn app_toggle_panel_visibility_log_collapses() {
    let config = Config::default();
    let mut app = App::new(config);
    // Expand log first
    app.log.log_expanded = true;
    // Hide log panel (index 5)
    app.toggle_panel_visibility(5);
    assert!(!app.is_panel_visible(5));
    assert!(!app.log.log_expanded); // Should collapse log when hiding it
}

// ── Model selection ────────────────────────────────────────────

#[test]
fn app_selected_model_none_when_empty() {
    let config = Config::default();
    let app = App::new(config);
    assert!(app.selected_model().is_none());
}

#[test]
fn app_selected_model_returns_some() {
    let config = Config::default();
    let mut app = App::new(config);
    app.models = vec![DiscoveredModel {
        path: "/model.gguf".into(),
        name: "test".into(),
        file_size: 1000,
        display_name: "test".into(),
    }];
    app.selected_model_idx = Some(0);
    assert!(app.selected_model().is_some());
}

#[test]
fn app_is_model_loaded_false_when_not_loaded() {
    let config = Config::default();
    let app = App::new(config);
    assert!(!app.is_model_loaded("nonexistent"));
}

#[test]
fn app_is_model_loaded_true_when_loaded() {
    let config = Config::default();
    let mut app = App::new(config);
    app.model_states.insert(
        "model.gguf".into(),
        ModelState::Loaded {
            port: 8080,
            pid: 1234,
        },
    );
    assert!(app.is_model_loaded("model.gguf"));
}

#[test]
fn app_is_model_loaded_false_when_available() {
    let config = Config::default();
    let mut app = App::new(config);
    app.model_states
        .insert("model.gguf".into(), ModelState::Available);
    assert!(!app.is_model_loaded("model.gguf"));
}

// ── Search results ─────────────────────────────────────────────

#[test]
fn app_search_results_len_zero_not_in_search_mode() {
    let config = Config::default();
    let app = App::new(config);
    assert_eq!(app.search_results_len(), 0);
}

#[test]
fn app_search_results_len_in_search_mode() {
    let config = Config::default();
    let mut app = App::new(config);
    app.models_mode = ModelsMode::Search {
        query: "test".into(),
        results: vec![
            SearchResult {
                model_id: "a".into(),
                model_name: "A".into(),
                tags: vec![],
                downloads: 0,
                likes: 0,
                pipeline_tag: None,
                size: None,
                parameters: None,
                capabilities: vec![],
                context_length: None,
                readme: None,
                quantization: None,
                license: None,
                trending_score: 0,
                created_at: None,
                downloaded: false,
            },
            SearchResult {
                model_id: "b".into(),
                model_name: "B".into(),
                tags: vec![],
                downloads: 0,
                likes: 0,
                pipeline_tag: None,
                size: None,
                parameters: None,
                capabilities: vec![],
                context_length: None,
                readme: None,
                quantization: None,
                license: None,
                trending_score: 0,
                created_at: None,
                downloaded: false,
            },
        ],
        sort_by: SearchSort::Relevance,
        show_readme: false,
        page: 0,
        loading: false,
        has_more: false,
    };
    assert_eq!(app.search_results_len(), 2);
}

// ── API port string ────────────────────────────────────────────

#[test]
fn app_get_api_port_str_returns_string() {
    let config = Config::default();
    let app = App::new(config);
    let port_str = app.get_api_port_str();
    assert!(!port_str.is_empty());
}

// ── Log management ─────────────────────────────────────────────

#[test]
fn app_add_log_adds_entry() {
    let config = Config::default();
    let mut app = App::new(config);
    let initial_len = app.log.log_entries.len();
    app.add_log("test message", llm_manager::config::LogLevel::Info);
    assert_eq!(app.log.log_entries.len(), initial_len + 1);
}

#[test]
fn app_add_log_sets_redraw() {
    let config = Config::default();
    let mut app = App::new(config);
    app.add_log("test", llm_manager::config::LogLevel::Info);
}

// ── Settings tracking ──────────────────────────────────────────

#[test]
fn app_new_settings_match_config() {
    let config = Config::default();
    let app = App::new(config);
    // Settings should be initialized from config default
    assert_eq!(app.settings.context_length, 32768);
}

#[test]
fn app_new_model_settings_cache_initialized() {
    let config = Config::default();
    let app = App::new(config);
    // model_settings_cache should be a clone of settings
    assert_eq!(
        app.settings.context_length,
        app.model_settings_cache.context_length
    );
}

// ── Loading state ──────────────────────────────────────────────

#[test]
fn app_new_loading_progress_zero() {
    let config = Config::default();
    let app = App::new(config);
    assert_eq!(app.loading.loading_progress, 0.0);
}

#[test]
fn app_new_no_loading_phases() {
    let config = Config::default();
    let app = App::new(config);
    assert!(app.loading.loading_phases.is_empty());
}

#[test]
fn app_new_no_bench_tune_state() {
    let config = Config::default();
    let app = App::new(config);
    assert!(app.bench_tune.bench_tune_progress.is_none());
    assert!(app.bench_tune.bench_tune_results.is_empty());
    assert!(!app.bench_tune.bench_tune_running);
}

// ── GGUF metadata cache ────────────────────────────────────────

#[test]
fn app_new_empty_metadata_cache() {
    let config = Config::default();
    let app = App::new(config);
    assert!(app.search.gguf_metadata_cache.is_empty());
}

#[test]
fn app_new_vram_estimate_zero() {
    let config = Config::default();
    let app = App::new(config);
    assert_eq!(app.loading.vram_estimate, 0);
}

// ── Readme cache ───────────────────────────────────────────────

#[test]
fn app_new_no_readme_cache() {
    let config = Config::default();
    let app = App::new(config);
    assert!(app.search.readme_cache.is_none());
}

// ── Backend resolution ─────────────────────────────────────────

#[test]
fn app_new_no_backend_resolving() {
    let config = Config::default();
    let app = App::new(config);
    assert!(!app.pending.backend_resolving);
    assert!(app.pending.backend_resolve_handle.is_none());
}

// ── Model metadata fields ──────────────────────────────────────

#[test]
fn app_new_model_metadata_zero() {
    let config = Config::default();
    let app = App::new(config);
    assert_eq!(app.loading.model_total_layers, 0);
    assert_eq!(app.loading.model_hidden_size, 0);
    assert_eq!(app.loading.model_n_ctx_train, 0);
    assert_eq!(app.loading.model_n_head, 0);
    assert_eq!(app.loading.model_n_kv_head, 0);
}

// ── Max threads ────────────────────────────────────────────────

#[test]
fn app_new_max_threads_positive() {
    let config = Config::default();
    let app = App::new(config);
    assert!(app.max_threads > 0);
}

// ── Pending operations ─────────────────────────────────────────

#[test]
fn app_new_no_pending_operations() {
    let config = Config::default();
    let app = App::new(config);
    assert!(app.pending.pending_download.is_none());
    assert!(app.pending.pending_deletion.is_none());
    assert!(app.pending.pending_spawn.is_none());
    assert!(app.pending.pending_api_load.is_none());
    assert!(app.pending.pending_api_unload.is_none());
    assert!(app.pending.pending_kill.is_none());
    assert!(!app.download.downloading);
}

// ── Server mode ────────────────────────────────────────────────

#[test]
fn app_new_server_mode_normal() {
    let config = Config::default();
    let app = App::new(config);
    assert_eq!(app.server_mode, ServerMode::Normal);
}

// ── Panel help ─────────────────────────────────────────────────

#[test]
fn app_new_panel_help_hidden() {
    let config = Config::default();
    let app = App::new(config);
    assert!(!app.ui.panel_help);
}
