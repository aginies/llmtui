use crossterm::event::KeyCode;

use crate::models::ListSort;
use crate::tui::app::pending_events::PendingEvent;
use crate::tui::app::{App, GlobalMode, LoadingPhase, ModelsMode};

pub async fn handle_models_key(app: &mut App, key: crossterm::event::KeyEvent) {
    if app.search.filtering_local {
        match key.code {
            KeyCode::Esc => {
                app.search.filtering_local = false;
                app.search.local_filter.clear();
                app.invalidate_list_caches();
                app.on_model_selection_change();
            }
            KeyCode::Enter => {
                app.search.filtering_local = false;
            }
            KeyCode::Char(c) => {
                app.search.local_filter.push(c);
                app.invalidate_list_caches();
                app.on_model_selection_change();
            }
            KeyCode::Backspace => {
                app.search.local_filter.pop();
                app.invalidate_list_caches();
                app.on_model_selection_change();
            }
            _ => {}
        }
        return;
    }

    match key.code {
        KeyCode::Char('f') => {
            if matches!(app.models_mode, ModelsMode::List { .. }) {
                app.search.filtering_local = true;
                if app.selected_model_idx.is_none() {
                    let filtered = app.get_filtered_model_indices();
                    if !filtered.is_empty() {
                        app.selected_model_idx = Some(filtered[0]);
                        app.on_model_selection_change();
                    }
                }
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            let sorted = if let ModelsMode::List { .. } = &app.models_mode {
                app.get_sorted_model_indices().to_vec()
            } else {
                let filtered = app.get_filtered_model_indices();
                get_sorted_indices(app, &filtered)
            };
            if let Some(idx) = app.selected_model_idx {
                if let Some(pos) = sorted.iter().position(|&i| i == idx) {
                    if pos > 0 {
                        app.selected_model_idx = Some(sorted[pos - 1]);
                        app.on_model_selection_change();
                    }
                } else if !sorted.is_empty() {
                    app.selected_model_idx = Some(sorted[0]);
                    app.on_model_selection_change();
                }
            } else if !sorted.is_empty() {
                app.selected_model_idx = Some(sorted[0]);
                app.on_model_selection_change();
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let sorted = if let ModelsMode::List { .. } = &app.models_mode {
                app.get_sorted_model_indices().to_vec()
            } else {
                let filtered = app.get_filtered_model_indices();
                get_sorted_indices(app, &filtered)
            };
            if let Some(idx) = app.selected_model_idx {
                if let Some(pos) = sorted.iter().position(|&i| i == idx) {
                    if pos + 1 < sorted.len() {
                        app.selected_model_idx = Some(sorted[pos + 1]);
                        app.on_model_selection_change();
                    }
                } else if !sorted.is_empty() {
                    app.selected_model_idx = Some(sorted[0]);
                    app.on_model_selection_change();
                }
            } else if !sorted.is_empty() {
                app.selected_model_idx = Some(sorted[0]);
                app.on_model_selection_change();
            }
        }
        KeyCode::Enter | KeyCode::Char('l')
            if !matches!(app.models_mode, ModelsMode::Files { .. }) =>
        {
            if app.pending.backend_resolving {
                app.add_log(
                    "Wait for backend installation to finish...",
                    crate::config::LogLevel::Info,
                );
                return;
            }
            if let Some(idx) = app.selected_model_idx {
                let model = app.models[idx].clone();
                let already_loaded = matches!(
                    app.model_states.get(&model.display_name),
                    Some(crate::models::ModelState::Loaded { .. })
                );
                if already_loaded {
                    app.add_log(
                        format!("{} is already loaded", model.display_name),
                        crate::config::LogLevel::Info,
                    );
                } else {
                    app.update_model_metadata();
                    let settings = app.selected_model_settings();

                    if let Some(handle) = &app.server.server_handle
                        && !crate::backend::server::check_health(&handle.host, handle.port).await
                    {
                        app.add_log(
                            "Router unresponsive, restarting...",
                            crate::config::LogLevel::Info,
                        );
                        if let Some(h) = app.server.server_handle.take() {
                            let _ = app
                                .pending_tx
                                .send(PendingEvent::KillHandle { handle: h })
                                .await;
                        }
                    }

                    if app.server.server_handle.is_none() {
                        // Start server (with model in CLI for normal mode, without model for router mode)
                        app.ui.last_error_message = None;

                        if app.server_mode == crate::models::ServerMode::BenchTune {
                            let bench_tune_config = crate::models::BenchTuneConfig::new(
                                model.path.clone(),
                                3, // Default iterations
                                crate::models::BENCHMARK_PROMPT.to_string(),
                            );
                            app.ui.global_mode = crate::tui::app::GlobalMode::BenchTuneSetup {
                                config: bench_tune_config,
                                selected_idx: 0,
                                editing_param: false,
                                editing_param_field: 0,
                                param_edit_buffer: String::new(),
                                param_edit_cursor_pos: 0,
                                bench_mode_selection: 0,
                                editing_prompt: false,
                                editing_kwargs: false,
                            };
                            return;
                        }
                        if app.server_mode == crate::models::ServerMode::Router {
                            // Router mode: start server without a model, then load via /load API
                            let _ = app
                                .pending_tx
                                .send(PendingEvent::Spawn {
                                    model: None,
                                    settings: settings.clone(),
                                })
                                .await;
                            // Queue the load so it triggers once server is ready
                            app.pending.pending_api_load = Some(model.display_name.clone());
                            app.loading.loading_phases =
                                std::iter::once(LoadingPhase::ServerStarting).collect();
                            app.loading.last_active_phase = Some(LoadingPhase::ServerStarting);
                            app.loading.loading_progress = 0.25;
                            app.add_log(
                                "Starting router server...".to_string(),
                                crate::config::LogLevel::Info,
                            );
                        } else {
                            // Normal mode: start server WITH the specific model directly
                            let _ = app
                                .pending_tx
                                .send(PendingEvent::Spawn {
                                    model: Some(model.clone()),
                                    settings,
                                })
                                .await;
                            app.loading.loading_phases =
                                std::iter::once(LoadingPhase::ServerStarting).collect();
                            app.loading.last_active_phase = Some(LoadingPhase::ServerStarting);
                            app.loading.loading_progress = 0.25;
                            app.add_log(
                                format!("Starting server with {}...", model.display_name),
                                crate::config::LogLevel::Info,
                            );
                        }
                    } else {
                        // Server already running, load via API

                        // Check if we reached the limit of models to load (based on Max Concurrent Predictions)
                        let active_count = app
                            .model_states
                            .values()
                            .filter(|s| {
                                matches!(
                                    s,
                                    crate::models::ModelState::Loaded { .. }
                                        | crate::models::ModelState::Loading
                                )
                            })
                            .count();

                        if let Some(max) = app.settings.max_concurrent_predictions
                            && active_count as u32 >= max
                        {
                            app.add_log(format!("Limit reached: already {} model(s) loaded (Max Concurrent Predictions limit: {})", active_count, max), crate::config::LogLevel::Warning);
                            return;
                        }

                        app.ui.last_error_message = None;
                        app.pending.pending_api_load = Some(model.display_name.clone());
                        app.loading.loading_phases =
                            std::iter::once(LoadingPhase::LoadingModel).collect();
                        app.loading.last_active_phase = Some(LoadingPhase::LoadingModel);
                        app.loading.loading_progress = 0.5;
                        app.add_log(
                            format!("Loading {} via API...", model.display_name),
                            crate::config::LogLevel::Info,
                        );
                    }
                }
            }
        }
        KeyCode::Char('u') => {
            if let Some(idx) = app.selected_model_idx {
                let model = app.models[idx].clone();
                if let Some(crate::models::ModelState::Loaded { .. }) =
                    app.model_states.get(&model.display_name)
                {
                    app.ui.global_mode = GlobalMode::Confirmation {
                        selected: false,
                        kind: crate::tui::app::ConfirmationKind::Unload,
                        display_name: model.display_name.clone(),
                        detail: Some(model.path.to_string_lossy().to_string()),
                    };
                    app.pending.pending_api_unload = Some(model.display_name.clone());
                } else {
                    app.add_log(
                        format!("{} is not loaded", model.display_name),
                        crate::config::LogLevel::Warning,
                    );
                }
            } else if app.server.server_handle.is_some() {
                app.add_log(
                    "Select a loaded model to unload",
                    crate::config::LogLevel::Warning,
                );
            } else if app.server_mode == crate::models::ServerMode::Router {
                // Router mode: no server running, no model loaded — fine
            } else {
                app.add_log(
                    "No model is currently loaded",
                    crate::config::LogLevel::Warning,
                );
            }
        }
        KeyCode::Delete if app.ui.active_panel == crate::tui::app::ActivePanel::Models => {
            if let Some(model) = app.selected_model() {
                let display_name = model.display_name.clone();
                let path_str = model.path.to_string_lossy().to_string();
                app.ui.global_mode = GlobalMode::Confirmation {
                    selected: false,
                    kind: crate::tui::app::ConfirmationKind::Delete,
                    display_name: display_name.clone(),
                    detail: Some(path_str),
                };
                app.add_log(
                    format!("Delete confirmation for {} shown", display_name),
                    crate::config::LogLevel::Info,
                );
            } else {
                app.add_log(
                    "No model selected to delete",
                    crate::config::LogLevel::Warning,
                );
            }
        }
        KeyCode::Char('d')
            if key
                .modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL) =>
        {
            if app.ui.active_panel != crate::tui::app::ActivePanel::Models {
                app.add_log(
                    "Press ⇥ to switch to Models panel, then ^D or Del to delete",
                    crate::config::LogLevel::Warning,
                );
                return;
            }
            if let Some(model) = app.selected_model() {
                let display_name = model.display_name.clone();
                let path_str = model.path.to_string_lossy().to_string();
                app.ui.global_mode = GlobalMode::Confirmation {
                    selected: false,
                    kind: crate::tui::app::ConfirmationKind::Delete,
                    display_name: display_name.clone(),
                    detail: Some(path_str),
                };
                app.add_log(
                    format!("Delete confirmation for {} shown", display_name),
                    crate::config::LogLevel::Info,
                );
            } else {
                app.add_log(
                    "No model selected to delete",
                    crate::config::LogLevel::Warning,
                );
            }
        }
        _ => {}
    }
}

fn get_sorted_indices(app: &App, filtered: &[usize]) -> Vec<usize> {
    let sort_by = match &app.models_mode {
        ModelsMode::List { sort_by } => *sort_by,
        _ => ListSort::Name,
    };

    let mut sorted = filtered.to_vec();
    sorted.sort_by(|&a, &b| {
        let model_a = &app.models[a];
        let model_b = &app.models[b];
        match sort_by {
            ListSort::Name => model_a.display_name.cmp(&model_b.display_name),
            ListSort::Status => {
                let state_a = app.model_states.get(&model_a.display_name);
                let state_b = app.model_states.get(&model_b.display_name);
                let prio_a = match state_a {
                    Some(crate::models::ModelState::Loaded { .. }) => 3,
                    Some(crate::models::ModelState::Loading) => 2,
                    Some(crate::models::ModelState::Benchmarking) => 1,
                    _ => 0,
                };
                let prio_b = match state_b {
                    Some(crate::models::ModelState::Loaded { .. }) => 3,
                    Some(crate::models::ModelState::Loading) => 2,
                    Some(crate::models::ModelState::Benchmarking) => 1,
                    _ => 0,
                };
                prio_b.cmp(&prio_a)
            }
            ListSort::Params => {
                let ka = &*model_a.path.to_string_lossy();
                let kb = &*model_b.path.to_string_lossy();
                let meta_a = app.search.gguf_metadata_cache.get(ka);
                let meta_b = app.search.gguf_metadata_cache.get(kb);
                let val_a = meta_a
                    .map(|m| {
                        let trimmed = m.model_parameters.trim();
                        let num_str = trimmed
                            .trim_end_matches(|c: char| c == 'B' || c == 'b')
                            .trim();
                        num_str.parse::<f64>().unwrap_or(0.0)
                    })
                    .unwrap_or(0.0);
                let val_b = meta_b
                    .map(|m| {
                        let trimmed = m.model_parameters.trim();
                        let num_str = trimmed
                            .trim_end_matches(|c: char| c == 'B' || c == 'b')
                            .trim();
                        num_str.parse::<f64>().unwrap_or(0.0)
                    })
                    .unwrap_or(0.0);
                val_b
                    .partial_cmp(&val_a)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }
            ListSort::Qual => {
                let ka = &*model_a.path.to_string_lossy();
                let kb = &*model_b.path.to_string_lossy();
                let meta_a = app.search.gguf_metadata_cache.get(ka);
                let meta_b = app.search.gguf_metadata_cache.get(kb);
                let rank_a = meta_a.map(|m| m.quality_rank).unwrap_or(0);
                let rank_b = meta_b.map(|m| m.quality_rank).unwrap_or(0);
                rank_b.cmp(&rank_a)
            }
            ListSort::Context => {
                let settings_a = app
                    .config
                    .resolve_settings(Some(model_a.display_name.as_str()), None);
                let settings_b = app
                    .config
                    .resolve_settings(Some(model_b.display_name.as_str()), None);
                settings_b.context_length.cmp(&settings_a.context_length)
            }
        }
    });
    sorted
}
