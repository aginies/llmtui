pub mod types;
pub mod state;
pub mod metadata;
pub mod panels;
pub mod profiles;
pub mod help;
pub mod pickers;
pub mod async_ops;
pub mod sync_ops;

// Re-export all types for backward compatibility
pub use types::*;

use crate::config::Config;
use crate::config::LogEntry;
use crate::config::physical_cores;
use std::collections::VecDeque;

// Import App from types submodule for impl block
pub use types::App;

impl App {
    pub fn new(config: Config) -> Self {
        let mut log = VecDeque::new();
        log.push_back(LogEntry::new("Starting llm-manager...", crate::config::LogLevel::Info));
        let default_params = config.default.clone();
        let settings: crate::models::ModelSettings = default_params.into();
        let server_mode = config.default.server_mode.clone();
        let router_max_models = config.default.router_max_models;
        Self {
            running: true,
            config,
            models: Vec::new(),
            selected_model_idx: None,
            models_mode: types::ModelsMode::List,
            local_filter: String::new(),
            filtering_local: false,
            search_results_idx: None,
            model_settings_cache: settings.clone(),
            settings,
            readme_cache: None,
            model_states: Default::default(),
            metrics: Default::default(),
            download_progress: Vec::new(),
            download_tx: None,
            download_rx: None,
            download_scroll_state: Default::default(),
            search_table_state: Default::default(),
            files_table_state: Default::default(),
            log_entries: log,
            active_panel: ActivePanel::Models,
            log_expanded: false,
            log_scroll_offset: 0,
            log_follow: true,
            log_total_lines: 0,
            settings_selected_idx: 0,
            server_settings_selected_idx: 0,
            server_settings_scroll_offset: 0,
            settings_edit_buffer: String::new(),
            settings_scroll_offset: 0,
            host_picker_entries: Vec::new(),
            host_picker_selected: 0,
            backend_picker_entries: Vec::new(),
            backend_picker_selected: 0,
            prompt_picker_entries: Vec::new(),
            prompt_picker_selected: 0,
            profile_picker_entries: Vec::new(),
            profile_picker_selected: 0,
            profiles_scroll_offset: 0,
            system_prompt_presets_scroll_offset: 0,
            readme_scroll_offset: 0,
            editing_preset: None,
            rpc_workers_selected_idx: 0,
            editing_rpc_worker: None,
            rpc_workers_scroll_offset: 0,
            edit_cursor_pos: 0,
            gguf_metadata_cache: Default::default(),
            vram_estimate: 0,
            backend_resolving: false,
            backend_resolve_handle: None,
            model_total_layers: 0,
            model_hidden_size: 0,
            model_n_ctx_train: 0,
            model_n_head: 0,
            model_n_kv_head: 0,
            max_threads: physical_cores(),
            pending_download: None,
            pending_deletion: None,
            pending_backend_deletion: None,
            pending_spawn: None,
            pending_api_load: None,
            pending_api_unload: None,
            pending_kill: None,
            downloading: false,
            server_log_rx: None,
            metrics_rx: None,
            global_mode: types::GlobalMode::Normal,
            loading_phases: Default::default(),
            last_active_phase: None,
            loading_progress: 0.0,
            progress_target: 0.0,
            load_progress: Default::default(),
            last_spinner_time: None,
            loading_spinner: 0,
            cancelled: None,
            server_handle: None,
            metrics_task_handle: None,
            sync_task_handle: None,
            sync_rx: None,
            spawn_task_handle: None,
            bench_tune_task_handle: None,
            spawn_log_tx: None,
            metrics_model_name: std::sync::Arc::new(std::sync::Mutex::new(None)),
            loaded_model_names: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            api_proxy_handle: None,
            background_tasks: Default::default(),
            needs_redraw: true,
            panel_visibility: 0b111111,
            panel_help: false,
            panel_help_offset: 0,
            last_error_message: None,
            last_metadata_parse: (std::path::PathBuf::new(), std::time::SystemTime::now()),
            pending_search_load: None,
            search_loading: false,
            server_mode,
            router_max_models,
            settings_render_cache: None,
            bench_tune_progress: None,
            bench_tune_results: Vec::new(),
            bench_tune_running: false,
            bench_tune_config: None,
            bench_tune_rx: None,
            bench_tune_tx: None,
            bench_tune_output_view: None,
            bench_tune_cancel_tx: None,
            bench_tune_output_scroll: 0,
            bench_tune_output_h_scroll: 0,
            bench_tune_result_row: 0,
            list_state: Default::default(),
            bench_tune_table_state: Default::default(),
            bench_tune_output_index: 0,
            editing_n_predict: false,
            n_predict_edit_buffer: String::new(),
            editing_iters: false,
            iters_edit_buffer: String::new(),
            tags_editing: false,
            tags_edit_buffer: String::new(),
            tags_selected_idx: None,
            tags_insert_mode: false,
            left_pct: 55,
            resize_state: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_app() -> App {
        let config = crate::config::Config {
            models_dir: std::path::PathBuf::new(),
            llama_server: std::path::PathBuf::new(),
            default: crate::config::DefaultParams::default(),
            model_overrides: std::collections::HashMap::new(),
            profiles: Vec::new(),
            system_prompt_presets: Vec::new(),
            rpc_workers: Vec::new(),
            search_limit: 50,
        };
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

    #[test]
    fn test_progress_server_starting() {
        let mut app = make_app();
        app.loading_phases.insert(LoadingPhase::ServerStarting);
        app.last_active_phase = Some(LoadingPhase::ServerStarting);
        app.compute_progress();
        // With only one phase, we get the full weight
        assert!((app.loading_progress - 0.08).abs() < 0.001);
    }

    #[test]
    fn test_progress_with_layers() {
        let mut app = make_app();
        app.loading_phases.insert(LoadingPhase::ServerStarting);
        app.loading_phases.insert(LoadingPhase::LoadingModel);
        app.loading_phases.insert(LoadingPhase::LoadingMeta);
        app.loading_phases.insert(LoadingPhase::LoadingTensors);
        app.last_active_phase = Some(LoadingPhase::LoadingTensors);
        app.load_progress.layers_loaded = Some(16);
        app.load_progress.layers_total = Some(32);
        app.compute_progress();
        // Should be ~0.22 + 0.5 * 0.70 = 0.57
        assert!((app.loading_progress - 0.57).abs() < 0.01);
    }

    #[test]
    fn test_progress_complete() {
        let mut app = make_app();
        app.loading_phases.insert(LoadingPhase::Complete);
        app.last_active_phase = Some(LoadingPhase::Complete);
        app.compute_progress();
        assert!((app.loading_progress - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_progress_all_phases() {
        let mut app = make_app();
        app.loading_phases.insert(LoadingPhase::ServerStarting);
        app.loading_phases.insert(LoadingPhase::LoadingModel);
        app.loading_phases.insert(LoadingPhase::LoadingMeta);
        app.loading_phases.insert(LoadingPhase::LoadingTensors);
        app.loading_phases.insert(LoadingPhase::ServerListening);
        app.last_active_phase = Some(LoadingPhase::ServerListening);
        app.compute_progress();
        // All phases complete = 1.0 (ServerListening interpolates at 0.8)
        assert!((app.loading_progress - 0.98).abs() < 0.01);
    }
}
