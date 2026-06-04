pub mod async_ops;
pub mod help;
pub mod metadata;
pub mod panels;
pub mod pickers;
pub mod profiles;
pub mod state;
pub mod sync_ops;
pub mod types;

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
        let active_panel = config.active_panel;
        let left_pct = config.left_pct;
        let mut log = VecDeque::new();
        log.push_back(LogEntry::new(
            "Starting llm-manager...",
            crate::config::LogLevel::Info,
        ));
        let settings: crate::models::ModelSettings =
            crate::models::ModelSettings::from_config(&config);
        let settings_clone = settings.clone();
        let server_mode = config.default.server_mode;
        let router_max_models = config.default.router_max_models;
        Self {
            running: true,
            config,
            models: Vec::new(),
            selected_model_idx: None,
            models_mode: types::ModelsMode::List,
            settings: settings_clone,
            model_settings_cache: settings.clone(),
            model_states: Default::default(),
            metrics: Default::default(),
            max_threads: physical_cores(),
            cancelled: None,
            server_mode,
            router_max_models,
            ws_server_handle: None,
            background_tasks: Default::default(),

            settings_state: SettingsState {
                settings_selected_idx: 0,
                server_settings_selected_idx: 0,
                server_settings_scroll_offset: 0,
                settings_edit_buffer: String::new(),
                settings_scroll_offset: 0,
                settings_render_cache: None,
                expert_mode: false,
            },
            picker: PickerState {
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
                rpc_workers_selected_idx: 0,
                editing_rpc_worker: None,
                rpc_workers_scroll_offset: 0,
                readme_scroll_offset: 0,
            },
            download: DownloadState {
                download_progress: Vec::new(),
                download_tx: None,
                download_rx: None,
                download_scroll_state: Default::default(),
                downloading: false,
                last_progress_update: std::time::Instant::now(),
            },
            server: ServerState {
                server_handle: None,
                metrics_task_handle: None,
                sync_task_handle: None,
                spawn_task_handle: None,
                bench_tune_task_handle: None,
                server_log_rx: None,
                metrics_rx: None,
                sync_rx: None,
                spawn_log_tx: None,
                metrics_model_name: std::sync::Arc::new(std::sync::Mutex::new(None)),
                loaded_model_names: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
                api_proxy_handle: None,
                metrics_tx: None,
                running_ws_port: None,
                running_ws_auth: None,
                running_ws_tls: None,
                running_api_port: None,
                running_api_server_port: None,
                running_api_model: None,
                running_ws_tls_cfg: None,
                running_ws_tls_cert_path: None,
                running_ws_tls_key_path: None,
                cmd_display: None,
                spawned_settings: None,
                spawned_model_name: None,
                spawned_model_state: None,
                spawned_context_length: 0,
                server_exit_rx: None,
                server_exit_tx: None,
                api_shutdown_tx: None,
            },
            bench_tune: BenchTuneState {
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
                bench_tune_table_state: Default::default(),
                bench_tune_output_index: 0,
            },
            log: LogState {
                log_entries: log,
                log_expanded: false,
                log_scroll_offset: 0,
                log_follow: true,
                log_total_lines: 0,
            },
            loading: LoadingState {
                loading_phases: Default::default(),
                last_active_phase: None,
                loading_progress: 0.0,
                progress_target: 0.0,
                load_progress: Default::default(),
                last_spinner_time: None,
                loading_spinner: 0,
                model_total_layers: 0,
                model_hidden_size: 0,
                model_n_ctx_train: 0,
                model_n_head: 0,
                model_n_kv_head: 0,
                vram_estimate: 0,
                health_poll_handle: None,
                loading_completion_rx: None,
            },
            pending: PendingOperations {
                pending_download: None,
                pending_deletion: None,
                pending_backend_deletion: None,
                pending_spawn: None,
                pending_api_load: None,
                pending_api_unload: None,
                pending_kill: None,
                backend_resolving: false,
                backend_resolve_handle: None,
            },
            search: SearchState {
                local_filter: String::new(),
                filtering_local: false,
                search_results_idx: None,
                search_table_state: Default::default(),
                files_table_state: Default::default(),
                readme_cache: None,
                gguf_metadata_cache: Default::default(),
                pending_search_load: None,
                search_loading: false,
                search_input: None,
                gguf_naming_cache: Default::default(),
            },
            ui: UIState {
                active_panel,
                global_mode: types::GlobalMode::Normal,
                panel_visibility: 0b111111,
                panel_help: false,
                panel_help_offset: 0,
                last_error_message: None,
                list_state: Default::default(),
                resize_state: None,
                left_pct,
                needs_full_redraw: false,
                needs_redraw: true,
                text_scrolls: Default::default(),
            },
            edit: EditState {
                edit_cursor_pos: 0,
                editing_n_predict: false,
                n_predict_edit_buffer: String::new(),
                editing_iters: false,
                iters_edit_buffer: String::new(),
                tags_editing: false,
                tags_edit_buffer: String::new(),
                tags_selected_idx: None,
                tags_insert_mode: false,
                editing_preset: None,
            },
        }
    }
}

impl App {
    const SCROLL_TICK_MS: u64 = 870;
    const SCROLL_HOLD_FRAMES: u8 = 5;

    pub fn tick_text_scrolls(&mut self) {
        let now = std::time::Instant::now();
        let mut changed = false;

        for (_, state) in self.ui.text_scrolls.iter_mut() {
            // Skip invisible entries entirely
            if !state.visible {
                continue;
            }

            if state.max_offset == 0 {
                if state.offset != 0 {
                    state.offset = 0;
                    changed = true;
                }
                continue;
            }

            if now.duration_since(state.last_tick) >= std::time::Duration::from_millis(Self::SCROLL_TICK_MS) {
                // Handle window resize where the new visible width is smaller (max_offset shrinks)
                if state.offset > state.max_offset {
                    state.offset = state.max_offset;
                    state.direction = -1;
                    state.hold_count = Self::SCROLL_HOLD_FRAMES;
                }

                let prev_offset = state.offset;

                if state.offset == 0 && state.direction == -1 {
                    state.direction = 1;
                    state.hold_count = Self::SCROLL_HOLD_FRAMES;
                } else if state.offset == state.max_offset && state.direction == 1 {
                    state.direction = -1;
                    state.hold_count = Self::SCROLL_HOLD_FRAMES;
                }

                if state.hold_count > 0 {
                    state.hold_count -= 1;
                } else {
                    state.offset = if state.direction > 0 {
                        state.offset.saturating_add(1)
                    } else {
                        state.offset.saturating_sub(1)
                    };
                }

                state.last_tick = now;
                if state.offset != prev_offset {
                    changed = true;
                }
            }
        }

        if changed {
            self.ui.needs_redraw = true;
        }
    }

    pub fn init_scrolls_for_models(&mut self) {
        use std::time::Instant;
        for model in &self.models {
            let key = model.display_name.clone();
            let max_offset = model.display_name.chars().count().saturating_sub(20);
            self.ui.text_scrolls.insert(key, TextScrollState {
                offset: 0,
                last_tick: Instant::now(),
                direction: 1,
                hold_count: 0,
                max_offset,
                visible: false,
            });
        }
    }

    #[allow(dead_code)]
    pub fn get_scroll_state(&self, key: &str) -> Option<&TextScrollState> {
        self.ui.text_scrolls.get(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::LoadProgress;

    fn make_app() -> App {
        let config = crate::config::Config {
            models_dirs: vec![],
            llama_server: std::path::PathBuf::new(),
            default: crate::config::DefaultParams::default(),
            model_overrides: crate::config::ModelConfigStore::new(),
            profiles: crate::config::ProfileStore::new(),
            system_prompt_presets: crate::config::PresetStore::new(),
            rpc_workers: Vec::new(),
            search_limit: 50,
            active_panel: types::ActivePanel::Models,
            left_pct: 55,
        };
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

    #[test]
    fn test_progress_server_starting() {
        let mut app = make_app();
        app.loading
            .loading_phases
            .insert(LoadingPhase::ServerStarting);
        app.loading.last_active_phase = Some(LoadingPhase::ServerStarting);
        app.compute_progress();
        assert!((app.loading.loading_progress - 0.08).abs() < 0.001);
    }

    #[test]
    fn test_progress_with_layers() {
        let mut app = make_app();
        app.loading
            .loading_phases
            .insert(LoadingPhase::ServerStarting);
        app.loading
            .loading_phases
            .insert(LoadingPhase::LoadingModel);
        app.loading.loading_phases.insert(LoadingPhase::LoadingMeta);
        app.loading
            .loading_phases
            .insert(LoadingPhase::LoadingTensors);
        app.loading.last_active_phase = Some(LoadingPhase::LoadingTensors);
        app.loading.load_progress.layers_loaded = Some(16);
        app.loading.load_progress.layers_total = Some(32);
        app.compute_progress();
        assert!((app.loading.loading_progress - 0.57).abs() < 0.01);
    }

    #[test]
    fn test_progress_complete() {
        let mut app = make_app();
        app.loading.loading_phases.insert(LoadingPhase::Complete);
        app.loading.last_active_phase = Some(LoadingPhase::Complete);
        app.compute_progress();
        assert!((app.loading.loading_progress - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_progress_all_phases() {
        let mut app = make_app();
        app.loading
            .loading_phases
            .insert(LoadingPhase::ServerStarting);
        app.loading
            .loading_phases
            .insert(LoadingPhase::LoadingModel);
        app.loading.loading_phases.insert(LoadingPhase::LoadingMeta);
        app.loading
            .loading_phases
            .insert(LoadingPhase::LoadingTensors);
        app.loading
            .loading_phases
            .insert(LoadingPhase::ServerListening);
        app.loading.last_active_phase = Some(LoadingPhase::ServerListening);
        app.compute_progress();
        assert!((app.loading.loading_progress - 0.98).abs() < 0.01);
    }
}
