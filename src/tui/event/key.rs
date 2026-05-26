use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::widgets::TableState;
use tracing::debug;

use crate::backend::hub;
use crate::models::SearchSort;
use crate::tui::app::{App, ActivePanel, ConfirmationKind, GlobalMode, ModelsMode};
use super::helpers::{execute_confirmation, sync_global_settings};
use super::panel::{handle_downloads_key, handle_log_key, handle_models_key, handle_profiles_key, handle_settings_key, handle_system_prompt_presets_key};
use super::readme::{fetch_and_store_readme, fetch_readme_for_selected, handle_readme_key};
use super::benches::handle_rpc_workers_key;

pub async fn handle_key(app: &mut App, key: crossterm::event::KeyEvent) {
    debug!("Key: {:?}", key);

    // Skip all if in CmdLine overlay
    if matches!(app.global_mode, GlobalMode::CmdLine { .. }) {
        match key.code {
            KeyCode::Esc => {
                app.global_mode = GlobalMode::Normal;
            }
            KeyCode::Char('e') => {
                if let GlobalMode::CmdLine { cmd_line } = &app.global_mode {
                    let script = format!("#!/bin/bash\n# Exported from llm-manager\n\n{}\n", cmd_line);
                    if let Err(e) = std::fs::write("/tmp/test_llamaserver.sh", &script) {
                        app.add_log(format!("Failed to write script: {}", e), crate::config::LogLevel::Error);
                    } else {
                        app.add_log("Wrote server command to /tmp/test_llamaserver.sh", crate::config::LogLevel::Info);
                    }
                }
            }
            _ => {}
        }
        return;
    }



    // Skip all if in confirmation dialog
    if let GlobalMode::Confirmation { selected, kind } = &app.global_mode {
        match key.code {
            KeyCode::Char('y') => {
                execute_confirmation(app, *kind).await;
                app.global_mode = GlobalMode::Normal;
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                app.pending_deletion = None;
                app.pending_api_unload = None;
                app.pending_backend_deletion = None;
                app.global_mode = GlobalMode::Normal;
            }
            KeyCode::Enter => {
                if *selected {
                    execute_confirmation(app, *kind).await;
                } else {
                    app.pending_deletion = None;
                    app.pending_api_unload = None;
                    app.pending_backend_deletion = None;
                }
                app.global_mode = GlobalMode::Normal;
            }
            KeyCode::Tab | KeyCode::Left | KeyCode::Right => {
                app.global_mode = GlobalMode::Confirmation {
                    selected: !*selected,
                    kind: *kind,
                };
            }
            KeyCode::Char('h')
                if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                app.pending_deletion = None;
                app.pending_api_unload = None;
                app.pending_backend_deletion = None;
                app.global_mode = GlobalMode::Normal;
            }
            _ => {}
        }
        return;
    }

    // Skip all if in RpcManager overlay
    if matches!(app.global_mode, GlobalMode::RpcManager) {
        handle_rpc_workers_key(app, key);
        return;
    }

    // Skip all if in About overlay
    if let GlobalMode::About = &app.global_mode {
        app.global_mode = GlobalMode::Normal;
        app.set_redraw();
        return;
    }

    // Skip all if in tags modal
    if app.tags_editing {
        super::panel::tags::handle_tags_key(app, key);
        return;
    }

    // Open tags modal from settings panel
    if app.active_panel == ActivePanel::LlmSettings
        && key.code == KeyCode::Char('t')
        && !app.tags_editing {
            app.tags_editing = true;
            app.tags_insert_mode = true;
            app.tags_edit_buffer = String::new();
            app.tags_selected_idx = None;
            app.settings_render_cache = None;
            app.set_redraw();
            return;
        }

    // Skip all if in host picker
    if let GlobalMode::HostPicker { entries, selected } = &mut app.global_mode {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                *selected = selected.saturating_sub(1);
                app.set_redraw();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                *selected = (*selected + 1).min(entries.len().saturating_sub(1));
                app.set_redraw();
            }
            KeyCode::Enter => {
                let (ip, _) = entries[*selected].clone();
                app.settings.host = ip;
                app.global_mode = GlobalMode::Normal;
                sync_global_settings(app);
                app.set_redraw();
            }
            KeyCode::Char('d') => {
                *entries = App::fetch_host_picker_entries();
                *selected = 0;
                app.set_redraw();
            }
            KeyCode::Esc => {
                app.global_mode = GlobalMode::Normal;
                app.set_redraw();
            }
            KeyCode::Char('h')
                if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                app.global_mode = GlobalMode::Normal;
            }
            _ => {}
        }
        return;
    }

    // Profile picker
    if let GlobalMode::ProfilePicker { entries, selected } = &mut app.global_mode {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                *selected = selected.saturating_sub(1);
                app.set_redraw();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                *selected = (*selected + 1).min(entries.len().saturating_sub(1));
                app.set_redraw();
            }
            KeyCode::Enter => {
                if *selected < entries.len() {
                    let name = entries[*selected].0.clone();
                    let profile = app.config.merged_profiles().into_iter()
                        .find(|p| p.name == name)
                        .map(|p| p.clone());
                    if let Some(profile) = profile {
                        app.apply_profile(&profile);
                    }
                }
                app.global_mode = GlobalMode::Normal;
                app.set_redraw();
            }
            KeyCode::Esc => {
                app.global_mode = GlobalMode::Normal;
                app.set_redraw();
            }
            _ => {}
        }
        return;
    }

    // Prompt picker
    if let GlobalMode::PromptPicker { .. } = &mut app.global_mode {
        handle_prompt_picker_key(app, key);
        return;
    }

    // BenchTune Setup
    if let GlobalMode::BenchTuneSetup { .. } = &mut app.global_mode {
        handle_bench_tune_setup_key(app, key);
        return;
    }

    // Skip all if in backend picker
    if let GlobalMode::BackendPicker { .. } = &mut app.global_mode {
        handle_backend_picker_key(app, key);
        return;
    }

    // Skip all if in max concurrent picker
    if matches!(app.global_mode, GlobalMode::MaxConcurrentPicker { .. }) {
        handle_max_concurrent_picker_key(app, key);
        return;
    }

    // Handle normal mode
    match key.code {
        KeyCode::Char('p') => {
            if !app.download_progress.is_empty()
                && let Some(idx) = app.download_scroll_state.selected() {
                    let (is_downloading, filename) = {
                        if let Some(state) = app.download_progress.get(idx) {
                            match state.status {
                                crate::models::DownloadStatus::Downloading => (true, state.filename.clone()),
                                crate::models::DownloadStatus::Paused => (false, state.filename.clone()),
                                _ => (false, String::new()),
                            }
                        } else {
                            (false, String::new())
                        }
                    };
                    if is_downloading {
                        if let Some(state) = app.download_progress.get_mut(idx) {
                            state.status = crate::models::DownloadStatus::Paused;
                            state.bytes_per_second = 0.0;
                            if let Some(arc) = &state.download_state_arc {
                                arc.store(2, std::sync::atomic::Ordering::Relaxed);
                            }
                        }
                        app.add_log(format!("Paused download of {}", filename), crate::config::LogLevel::Info);
                    } else if !filename.is_empty() {
                        if let Some(state) = app.download_progress.get_mut(idx) {
                            state.status = crate::models::DownloadStatus::Downloading;
                            if let Some(arc) = &state.download_state_arc {
                                arc.store(1, std::sync::atomic::Ordering::Relaxed);
                            }
                        }
                        app.add_log(format!("Resumed download of {}", filename), crate::config::LogLevel::Info);
                    }
                    app.set_redraw();
                    return;
                }
        }
        KeyCode::Char('c')
            if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) =>
        {
            let loaded_count = app.model_states.values().filter(|s| matches!(s, crate::models::ModelState::Loaded { .. })).count();
            if loaded_count > 0 {
                app.global_mode = GlobalMode::Confirmation {
                    selected: false,
                    kind: ConfirmationKind::Exit,
                };
                app.set_redraw();
            } else {
                app.running = false;
            }
            return;
        }
        KeyCode::Esc if app.log_expanded && !app.filtering_local => {
            app.log_expanded = false;
            app.set_redraw();
            return;
        }
        KeyCode::Tab => {
            if app.global_mode == GlobalMode::Normal {
                if key.modifiers.contains(crossterm::event::KeyModifiers::SHIFT) {
                    app.focus_prev();
                } else {
                    app.focus_next();
                }
                return;
            }
        }
        KeyCode::Char('h')
            if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) && !key.modifiers.contains(crossterm::event::KeyModifiers::SHIFT) =>
        {
            app.panel_help = !app.panel_help;
            if app.panel_help {
                app.panel_help_offset = 0;
            }
            app.set_redraw();
            return;
        }
        KeyCode::F(1) => {
            app.active_panel = ActivePanel::Models;
            return;
        }
        KeyCode::F(2) => {
            if app.server_handle.is_none() {
                app.toggle_panel_visibility(1);
                if app.is_panel_visible(1) {
                    app.active_panel = ActivePanel::ServerSettings;
                }
            }
            return;
        }
        KeyCode::F(3) => {
            app.toggle_panel_visibility(2);
            if app.is_panel_visible(2) {
                app.active_panel = ActivePanel::ModelInfo;
            }
            return;
        }
        KeyCode::F(4) => {
            app.toggle_panel_visibility(3);
            if app.is_panel_visible(3) {
                app.active_panel = ActivePanel::LlmSettings;
            }
            return;
        }
        KeyCode::F(5) => {
            app.toggle_panel_visibility(4);
            return;
        }
        KeyCode::F(6) => {
            app.toggle_panel_visibility(5);
            if app.is_panel_visible(5) {
                app.active_panel = ActivePanel::Log;
            }
            return;
        }
        KeyCode::Left
            if key.modifiers.contains(crossterm::event::KeyModifiers::SHIFT) =>
        {
            app.left_pct = app.left_pct.saturating_sub(1).max(20);
            app.set_redraw();
            return;
        }
        KeyCode::Right
            if key.modifiers.contains(crossterm::event::KeyModifiers::SHIFT) =>
        {
            app.left_pct = app.left_pct.saturating_add(1).min(80);
            app.set_redraw();
            return;
        }
        KeyCode::F(7)
            if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) =>
        {
            app.panel_visibility |= 1 << 0;
            app.active_panel = ActivePanel::Models;
            app.set_redraw();
            return;
        }
        KeyCode::F(8)
            if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) =>
        {
            if app.server_handle.is_none() {
                app.panel_visibility |= 1 << 1;
                app.active_panel = ActivePanel::ServerSettings;
            }
            app.set_redraw();
            return;
        }
        KeyCode::F(9)
            if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) =>
        {
            app.panel_visibility |= 1 << 3;
            app.active_panel = ActivePanel::LlmSettings;
            app.set_redraw();
            return;
        }
        KeyCode::F(10)
            if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) =>
        {
            app.panel_visibility = 0b111111;
            app.log_expanded = false;
            app.set_redraw();
            return;
        }
        KeyCode::F(10) => {
            app.panel_visibility = 0b111111;
            app.log_expanded = false;
            app.set_redraw();
            return;
        }
        KeyCode::Char('k')
            if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL)
                && key.modifiers.contains(crossterm::event::KeyModifiers::ALT) =>
        {
            if let Some(handle) = app.server_handle.take() {
                let port = handle.port;
                app.pending_kill = Some(handle);
                app.add_log(format!("Killing llama-server on port {}", port), crate::config::LogLevel::Info);
                app.set_redraw();
            } else {
                app.add_log("No server is running", crate::config::LogLevel::Warning);
            }
            return;
        }
        KeyCode::F(9) => {
            app.panel_visibility = 0b111111;
            app.log_expanded = false;
            app.set_redraw();
            return;
        }
        KeyCode::Char('l')
            if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) =>
        {
            app.active_panel = ActivePanel::Log;
            app.set_redraw();
            return;
        }
        KeyCode::Char('k')
            if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) =>
        {
            let binary = app.config.llama_server.clone();
            let model = app.selected_model().cloned();
            let (_cmd, cmd_line) = crate::backend::server::build_server_cmd(
                &binary,
                model.as_ref(),
                &app.settings,
                &app.config,
                app.server_mode.clone(),
                app.router_max_models,
            );
            app.global_mode = GlobalMode::CmdLine { cmd_line };
            app.set_redraw();
            return;
        }
        KeyCode::Char('/') => {
            app.active_panel = ActivePanel::Models;
            app.models_mode = ModelsMode::Search {
                query: String::new(),
                results: Vec::new(),
                sort_by: SearchSort::Relevance,
                show_readme: true,
                page: 0,
                loading: false,
                has_more: true,
            };
            app.search_results_idx = Some(0);
            app.log_expanded = false;
            app.panel_visibility &= !(1 << 4);
            app.panel_visibility &= !(1 << 5);
            return;
        }
        KeyCode::Char('A') => {
            app.global_mode = GlobalMode::About;
            app.set_redraw();
            return;
        }
        _ => {}
    }

    // Handle search mode first (it takes priority)
    let is_search = matches!(app.models_mode, ModelsMode::Search { .. });
    if is_search && app.active_panel == ActivePanel::Models {
        handle_search_key(app, key).await;
        return;
    }

    // Handle files mode
    let is_files = matches!(app.models_mode, ModelsMode::Files { .. });
    if is_files && app.active_panel == ActivePanel::Models {
        handle_files_key(app, key).await;
        return;
    }

    // Handle bench_tune output view modal
    if app.bench_tune_output_view.is_some() {
        handle_bench_tune_output_key(app, key);
        return;
    }

    // Handle bench_tune mode
    if matches!(app.models_mode, ModelsMode::BenchTune { .. }) {
        handle_bench_tune_key(app, key).await;
        return;
    }

    // Skip normal key handling when panel help is showing
    if app.panel_help && !app.filtering_local {
        match key.code {
            KeyCode::Esc => {
                app.panel_help = false;
                app.set_redraw();
                return;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                app.panel_help_offset = app.panel_help_offset.saturating_add(1);
                app.set_redraw();
                return;
            }
            KeyCode::Char('k') | KeyCode::Up => {
                app.panel_help_offset = app.panel_help_offset.saturating_sub(1);
                app.set_redraw();
                return;
            }
            _ => {}
        }
        return;
    }

    // Global shortcuts for server settings
    if app.active_panel == ActivePanel::ServerSettings {
        handle_server_settings_key(app, key);
        return;
    }

    // Global shortcuts
    if key.code == KeyCode::Char('s') && key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
        app.save_model_settings();
        app.set_redraw();
        return;
    }

    match app.active_panel {
        ActivePanel::Models => handle_models_key(app, key).await,
        ActivePanel::Log => handle_log_key(app, key),
        ActivePanel::ServerSettings => { /* handled above */ }
        ActivePanel::LlmSettings => handle_settings_key(app, key),
        ActivePanel::Profiles => handle_profiles_key(app, key),
        ActivePanel::SystemPromptPresets => handle_system_prompt_presets_key(app, key),
        ActivePanel::SearchReadme => handle_readme_key(app, key),
        ActivePanel::ActiveModel => {}
        ActivePanel::ModelInfo => {}
        ActivePanel::Downloads => handle_downloads_key(app, key),
    }
}

fn handle_prompt_picker_key(app: &mut App, key: crossterm::event::KeyEvent) {
    if let GlobalMode::PromptPicker { entries, selected, editing, edit_buffer, edit_cursor_pos, confirm_delete } = &mut app.global_mode {
        if *confirm_delete {
            match key.code {
                KeyCode::Char('y') => {
                    if *selected < entries.len() {
                        let name = entries[*selected].0.clone();
                        if matches!(name.as_str(), "General" | "Coder" | "Thinker" | "Mathematician") {
                            *confirm_delete = false;
                            app.add_log("Cannot delete built-in preset", crate::config::LogLevel::Error);
                            app.set_redraw();
                            return;
                        } else {
                            entries.remove(*selected);
                            if *selected >= entries.len() && *selected > 0 {
                                *selected = entries.len() - 1;
                            }
                            if let Some(preset) = app.config.system_prompt_presets.iter().position(|p| p.name == name) {
                                app.config.system_prompt_presets.remove(preset);
                            }
                            let _ = app.config.save();
                        }
                    }
                    *confirm_delete = false;
                    app.set_redraw();
                }
                KeyCode::Char('n') | KeyCode::Esc => {
                    *confirm_delete = false;
                    app.set_redraw();
                }
                _ => {}
            }
            return;
        }

        if *editing {
            match key.code {
                KeyCode::Esc => { *editing = false; app.set_redraw(); }
                KeyCode::Enter => {
                    let byte_pos = edit_buffer.char_indices().nth(*edit_cursor_pos).map(|(i, _)| i).unwrap_or(edit_buffer.len());
                    edit_buffer.insert_str(byte_pos, "\n");
                    *edit_cursor_pos += 1;
                    app.set_redraw();
                }
                KeyCode::Char('s') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                    let mut saved = false;
                    if *selected < entries.len() {
                        let name = entries[*selected].0.clone();
                        let content = edit_buffer.clone();
                        if let Some(preset) = app.config.system_prompt_presets.iter_mut().find(|p| p.name == name) {
                            preset.content = content;
                            saved = app.config.save().is_ok();
                        }
                    }
                    let log_msg = if saved { "Saved preset" } else { "Failed to save preset" };
                    let log_level = if saved { crate::config::LogLevel::Info } else { crate::config::LogLevel::Error };
                    *editing = false;
                    app.add_log(log_msg, log_level);
                    app.set_redraw();
                }
                KeyCode::Char(c) => {
                    let char_pos = *edit_cursor_pos;
                    let byte_pos = edit_buffer.char_indices().nth(char_pos).map(|(i, _)| i).unwrap_or(edit_buffer.len());
                    edit_buffer.insert_str(byte_pos, &c.to_string());
                    *edit_cursor_pos += 1;
                    app.set_redraw();
                }
                KeyCode::Backspace => {
                    if *edit_cursor_pos > 0 {
                        let char_pos = *edit_cursor_pos - 1;
                        let byte_pos = edit_buffer.char_indices().nth(char_pos).map(|(i, _)| i).unwrap_or(edit_buffer.len());
                        let char_len = edit_buffer[byte_pos..].chars().next().unwrap_or('\0').len_utf8();
                        edit_buffer.drain(byte_pos..byte_pos + char_len);
                        *edit_cursor_pos -= 1;
                        app.set_redraw();
                    }
                }
                KeyCode::Left => { *edit_cursor_pos = edit_cursor_pos.saturating_sub(1); app.set_redraw(); }
                KeyCode::Right => { *edit_cursor_pos = (*edit_cursor_pos + 1).min(edit_buffer.chars().count()); app.set_redraw(); }
                _ => {}
            }
            return;
        }

        match key.code {
            KeyCode::Up | KeyCode::Char('k') => { *selected = selected.saturating_sub(1); app.set_redraw(); }
            KeyCode::Down | KeyCode::Char('j') => { *selected = (*selected + 1).min(entries.len().saturating_sub(1)); app.set_redraw(); }
            KeyCode::Enter => {
                let (name, _) = entries[*selected].clone();
                app.settings.system_prompt_preset_name = name.clone();
                app.resolve_system_prompt();
                app.global_mode = GlobalMode::Normal;
                app.set_redraw();
            }
            KeyCode::Char('e') => {
                *editing = true;
                *edit_cursor_pos = 0;
                if *selected < entries.len() {
                    let name = entries[*selected].0.clone();
                    if let Some(preset) = app.config.system_prompt_presets.iter().find(|p| p.name == name) {
                        *edit_buffer = preset.content.clone();
                    } else {
                        *edit_buffer = String::new();
                    }
                }
                app.set_redraw();
            }
            KeyCode::Char('n') => {
                let name = format!("Custom {}", entries.len() + 1);
                let preset = crate::config::SystemPromptPreset { name: name.clone(), description: "User-defined preset".into(), content: String::new() };
                app.config.system_prompt_presets.push(preset);
                entries.push((name, "User-defined preset".into()));
                *selected = entries.len() - 1;
                *editing = true;
                *edit_cursor_pos = 0;
                *edit_buffer = String::new();
                app.set_redraw();
            }
            KeyCode::Char('d') => {
                if *selected < entries.len() {
                    let name = &entries[*selected].0;
                    if matches!(name.as_str(), "General" | "Coder" | "Thinker" | "Mathematician") {
                        app.add_log("Cannot delete built-in preset", crate::config::LogLevel::Error);
                        app.set_redraw();
                        return;
                    }
                }
                *confirm_delete = true;
                app.set_redraw();
            }
            KeyCode::Esc => { app.global_mode = GlobalMode::Normal; app.set_redraw(); }
            _ => {}
        }
    }
}

fn handle_bench_tune_setup_key(app: &mut App, key: crossterm::event::KeyEvent) {
    if let GlobalMode::BenchTuneSetup { config, selected_idx, bench_mode_selection, editing_prompt, editing_kwargs } = &mut app.global_mode {
        match key.code {
            KeyCode::Char('m') if key.modifiers.contains(KeyModifiers::ALT) => {
                *bench_mode_selection = if *bench_mode_selection == 0 { 1 } else { 0 };
                config.bench_mode = match *bench_mode_selection { 0 => crate::models::BenchTuneMode::RuntimeOnly, _ => crate::models::BenchTuneMode::Full };
            }
            KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::ALT) => {
                *editing_prompt = !*editing_prompt;
                if *editing_prompt { app.edit_cursor_pos = config.prompt.len(); }
            }
            KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::ALT) => {
                app.editing_n_predict = !app.editing_n_predict;
                if app.editing_n_predict { app.n_predict_edit_buffer = config.n_predict.to_string(); }
            }
            KeyCode::Char('i') if key.modifiers.contains(KeyModifiers::ALT) => {
                app.editing_iters = !app.editing_iters;
                if app.editing_iters { app.iters_edit_buffer = config.num_iterations.to_string(); }
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::ALT) => {
                *editing_kwargs = !*editing_kwargs;
                if *editing_kwargs { app.edit_cursor_pos = config.chat_template_kwargs.as_deref().unwrap_or("").len(); }
            }
            KeyCode::Up | KeyCode::Char('k') | KeyCode::Left => {
                if *editing_prompt || *editing_kwargs {
                    if app.edit_cursor_pos > 0 { app.edit_cursor_pos = app.edit_cursor_pos.saturating_sub(1); }
                } else { *selected_idx = selected_idx.saturating_sub(1); }
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Right => {
                if *editing_prompt || *editing_kwargs {
                    let len = if *editing_prompt { config.prompt.len() } else { config.chat_template_kwargs.as_deref().map(|s| s.len()).unwrap_or(0) };
                    app.edit_cursor_pos = (app.edit_cursor_pos + 1).min(len);
                } else {
                    *selected_idx = (*selected_idx + 1).min(config.params_to_test.len().saturating_sub(1));
                }
            }
            KeyCode::Char(' ') => {
                if *editing_prompt {
                    if app.edit_cursor_pos <= config.prompt.len() { config.prompt.insert(app.edit_cursor_pos, ' '); app.edit_cursor_pos += 1; }
                } else if *editing_kwargs {
                    let kwargs = config.chat_template_kwargs.get_or_insert_with(String::new);
                    if app.edit_cursor_pos <= kwargs.len() { kwargs.insert(app.edit_cursor_pos, ' '); app.edit_cursor_pos += 1; }
                } else if *selected_idx < config.params_to_test.len() {
                    config.params_to_test[*selected_idx].enabled = !config.params_to_test[*selected_idx].enabled;
                }
            }
            KeyCode::Char(c) => {
                if *editing_prompt { config.prompt.insert(app.edit_cursor_pos, c); app.edit_cursor_pos += 1; }
                else if *editing_kwargs { let kwargs = config.chat_template_kwargs.get_or_insert_with(String::new); kwargs.insert(app.edit_cursor_pos, c); app.edit_cursor_pos += 1; }
                else if app.editing_n_predict { if c.is_ascii_digit() { app.n_predict_edit_buffer.push(c); } }
                else if app.editing_iters { if c.is_ascii_digit() { app.iters_edit_buffer.push(c); } }
            }
            KeyCode::Backspace => {
                if *editing_prompt {
                    if app.edit_cursor_pos > 0 { config.prompt.remove(app.edit_cursor_pos - 1); app.edit_cursor_pos = app.edit_cursor_pos.saturating_sub(1); }
                } else if *editing_kwargs {
                    let kwargs = config.chat_template_kwargs.get_or_insert_with(String::new);
                    if app.edit_cursor_pos > 0 { kwargs.remove(app.edit_cursor_pos - 1); app.edit_cursor_pos = app.edit_cursor_pos.saturating_sub(1); }
                } else if app.editing_n_predict { app.n_predict_edit_buffer.pop(); }
                else if app.editing_iters { app.iters_edit_buffer.pop(); }
                else { *selected_idx = selected_idx.saturating_sub(1); }
            }
            KeyCode::Delete => {
                if *editing_prompt { if app.edit_cursor_pos < config.prompt.len() { config.prompt.remove(app.edit_cursor_pos); } }
                else if *editing_kwargs { let kwargs = config.chat_template_kwargs.get_or_insert_with(String::new); if app.edit_cursor_pos < kwargs.len() { kwargs.remove(app.edit_cursor_pos); } }
                else if app.editing_n_predict { if !app.n_predict_edit_buffer.is_empty() { app.n_predict_edit_buffer.pop(); } }
                else if app.editing_iters { if !app.iters_edit_buffer.is_empty() { app.iters_edit_buffer.pop(); } }
            }
            KeyCode::Enter => {
                if *editing_prompt { *editing_prompt = false; }
                else if *editing_kwargs { *editing_kwargs = false; }
                else if app.editing_n_predict {
                    if let Ok(val) = app.n_predict_edit_buffer.parse::<u32>() { config.n_predict = val.clamp(1, 16384); }
                    app.editing_n_predict = false;
                } else if app.editing_iters {
                    if let Ok(val) = app.iters_edit_buffer.parse::<u32>() { config.num_iterations = val.max(1).min(100); }
                    app.editing_iters = false;
                } else {
                    let config_final = config.clone();
                    if let Some(idx) = app.selected_model_idx {
                        let model = app.models[idx].clone();
                        let settings = app.settings.clone();
                        app.global_mode = GlobalMode::Normal;
                        app.bench_tune_config = Some(config_final);
                        app.pending_spawn = Some((Some(model), settings));
                    }
                }
            }
            KeyCode::Esc => {
                if *editing_prompt { *editing_prompt = false; }
                else if *editing_kwargs { *editing_kwargs = false; }
                else if app.editing_n_predict { app.editing_n_predict = false; }
                else if app.editing_iters { app.editing_iters = false; }
                else { app.global_mode = GlobalMode::Normal; }
            }
            _ => {}
        }
        app.set_redraw();
    }
}

fn handle_backend_picker_key(app: &mut App, key: crossterm::event::KeyEvent) {
    if let GlobalMode::BackendPicker { entries, selected } = &mut app.global_mode {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => { *selected = selected.saturating_sub(1); app.set_redraw(); }
            KeyCode::Down | KeyCode::Char('j') => { *selected = (*selected + 1).min(entries.len().saturating_sub(1)); app.set_redraw(); }
            KeyCode::Enter => {
                let (backend, tag) = entries[*selected].clone();
                app.settings.backend = backend;
                app.settings.set_active_backend_version(tag.clone());
                if !crate::backend::hub::is_backend_version_installed(backend, tag.as_deref()) {
                    app.backend_resolving = true;
                    let tag_param = tag.clone();
                    if app.download_rx.is_none() {
                        let (tx, rx) = tokio::sync::broadcast::channel(10);
                        app.download_tx = Some(tx);
                        app.download_rx = Some(rx);
                    }
                    let (log_tx, log_rx) = tokio::sync::mpsc::channel(100);
                    app.server_log_rx = Some(log_rx);
                    let tx = app.download_tx.clone();
                    let handle = tokio::spawn(async move {
                        crate::backend::hub::resolve_backend_binary(backend, tag_param.as_deref(), Some(log_tx), tx).await.map_err(|e| e.to_string())
                    });
                    app.backend_resolve_handle = Some(handle);
                } else {
                    app.backend_resolving = false;
                }
                app.global_mode = GlobalMode::Normal;
                sync_global_settings(app);
                app.set_redraw();
            }
            KeyCode::Char('d') => {
                if let Some((backend, Some(tag))) = entries.get(*selected) {
                    app.pending_backend_deletion = Some((*backend, tag.clone()));
                    app.global_mode = GlobalMode::Confirmation { selected: false, kind: ConfirmationKind::DeleteBackend };
                    app.set_redraw();
                }
            }
            KeyCode::Esc => { app.global_mode = GlobalMode::Normal; app.set_redraw(); }
            KeyCode::Char('h') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => { app.global_mode = GlobalMode::Normal; }
            _ => {}
        }
    }
}

fn handle_max_concurrent_picker_key(app: &mut App, key: crossterm::event::KeyEvent) {
    if let GlobalMode::MaxConcurrentPicker { value } = &mut app.global_mode {
        match key.code {
            KeyCode::Char(c @ '0'..='9') => { if value.len() < 3 { value.push(c); } app.set_redraw(); }
            KeyCode::Backspace | KeyCode::Left => { value.pop(); app.set_redraw(); }
            KeyCode::Enter => {
                if let Ok(n) = value.parse::<u32>() { let n = n.clamp(1, 10); app.settings.max_concurrent_predictions = Some(n); sync_global_settings(app); app.update_vram_estimate(); }
                app.global_mode = GlobalMode::Normal;
                app.settings_render_cache = None;
                app.set_redraw();
            }
            KeyCode::Esc => { app.global_mode = GlobalMode::Normal; app.set_redraw(); }
            _ => {}
        }
    }
}

async fn handle_search_key(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.models_mode = ModelsMode::List;
            app.panel_visibility |= (1 << 4) | (1 << 5);
            app.set_redraw();
            return;
        }
        KeyCode::Enter => {
            let query = if let ModelsMode::Search { query, page, has_more, .. } = &mut app.models_mode {
                *page = 0; *has_more = true; query.clone()
            } else { return };
            if query.is_empty() { return; }
            app.add_log(format!("Searching for '{}'...", query), crate::config::LogLevel::Info);
            app.pending_search_load = Some((query, 0));
            app.search_loading = true;
            app.search_table_state = TableState::default();
            app.set_redraw();
            return;
        }
        KeyCode::Backspace => {
            if let ModelsMode::Search { query, .. } = &mut app.models_mode { query.pop(); }
            app.set_redraw();
            return;
        }
        KeyCode::Char('L') => {
            let model_id = if let ModelsMode::Search { results, .. } = &app.models_mode {
                app.search_results_idx.and_then(|idx| results.get(idx).map(|r| r.model_id.clone()))
            } else { None };
            if let Some(model_id) = model_id {
                app.add_log(format!("Loading files for {}...", model_id), crate::config::LogLevel::Info);
                match hub::list_gguf_files(&model_id).await {
                    Ok(files) => {
                        app.add_log(format!("Found {} GGUF files", files.len()), crate::config::LogLevel::Info);
                        if let ModelsMode::Search { query, results, .. } = &app.models_mode {
                            let selected_result = app.search_results_idx.and_then(|idx| results.get(idx).cloned());
                            app.files_table_state = TableState::default();
                            app.models_mode = ModelsMode::Files { model_id, files, selected_idx: Some(0), previous_query: query.clone(), previous_results: results.clone(), selected_result };
                        }
                    }
                    Err(e) => { app.add_log(format!("No GGUF files: {}", e), crate::config::LogLevel::Info); }
                }
            }
            return;
        }
        KeyCode::Char('S') => {
            if let ModelsMode::Search { sort_by, results, .. } = &mut app.models_mode {
                *sort_by = sort_by.next();
                results.sort_by(|a, b| match sort_by {
                    SearchSort::Downloads => b.downloads.cmp(&a.downloads),
                    SearchSort::Likes => b.likes.cmp(&a.likes),
                    SearchSort::Trending => b.trending_score.cmp(&a.trending_score),
                    SearchSort::CreatedAt => { let a_date = a.created_at.as_deref().unwrap_or(""); let b_date = b.created_at.as_deref().unwrap_or(""); b_date.cmp(a_date) }
                    SearchSort::Relevance => std::cmp::Ordering::Equal,
                });
                if !results.is_empty() { app.search_results_idx = Some(0); } else { app.search_results_idx = None; }
            }
            app.set_redraw();
            return;
        }
        KeyCode::Char('B') => {
            if let ModelsMode::Search { page, .. } = &app.models_mode && *page > 0 {
                let query = if let ModelsMode::Search { query, .. } = &app.models_mode { query.clone() } else { String::new() };
                let offset = (*page as u32 - 1) * 50;
                app.add_log(format!("Loading page {}...", *page - 1), crate::config::LogLevel::Info);
                if let ModelsMode::Search { page, .. } = &mut app.models_mode { *page -= 1; }
                app.pending_search_load = Some((query, offset));
                app.search_loading = true;
                app.set_redraw();
                return;
            }
            return;
        }
        KeyCode::Down => {
            let len = app.search_results_len();
            match app.search_results_idx {
                Some(idx) if idx + 1 < len => app.search_results_idx = Some(idx + 1),
                Some(idx) => {
                    if idx + 1 >= len && let ModelsMode::Search { has_more, loading, page, .. } = &app.models_mode && !*loading && *has_more {
                        let query = if let ModelsMode::Search { query, .. } = &app.models_mode { query.clone() } else { String::new() };
                        let offset = (*page as u32 + 1) * 50;
                        app.add_log("Loading more results...", crate::config::LogLevel::Info);
                        app.pending_search_load = Some((query, offset));
                        app.search_loading = true;
                        app.set_redraw();
                        return;
                    }
                    app.search_results_idx = Some(len.saturating_sub(1));
                }
                None if len > 0 => app.search_results_idx = Some(0),
                _ => {}
            }
            return;
        }
        KeyCode::Char('R') => {
            let model_id = if let ModelsMode::Search { results, .. } = &app.models_mode {
                app.search_results_idx.and_then(|idx| results.get(idx).map(|r| r.model_id.clone()))
            } else { None };
            if let Some(model_id) = model_id {
                app.add_log(format!("Fetching README for {}...", model_id), crate::config::LogLevel::Info);
                app.add_log("This may take a moment...", crate::config::LogLevel::Info);
                fetch_and_store_readme(app, model_id).await;
                if let ModelsMode::Search { show_readme, .. } = &mut app.models_mode { *show_readme = true; }
            }
            return;
        }
        KeyCode::Char(c) => {
            if let ModelsMode::Search { query, .. } = &mut app.models_mode { query.push(c); }
            app.set_redraw();
            return;
        }
        KeyCode::Up => {
            match app.search_results_idx {
                Some(idx) if idx > 0 => app.search_results_idx = Some(idx - 1),
                None => { let len = if let ModelsMode::Search { results, .. } = &app.models_mode { results.len() } else { 0 }; if len > 0 { app.search_results_idx = Some(0); } }
                _ => {}
            }
            app.set_redraw();
            return;
        }
        _ => {}
    }

    if let ModelsMode::Search { results, .. } = &app.models_mode {
        if let Some(idx) = app.search_results_idx {
            if let Some(r) = results.get(idx) {
                fetch_readme_for_selected(app, r.model_id.clone()).await;
            }
        }
    }
}

async fn handle_files_key(app: &mut App, key: crossterm::event::KeyEvent) {
    let model_id = if let ModelsMode::Files { model_id, .. } = &app.models_mode { Some(model_id.clone()) } else { None };

    match key.code {
        KeyCode::Esc => {
            if let ModelsMode::Files { previous_query, previous_results, .. } = std::mem::replace(&mut app.models_mode, ModelsMode::List) {
                let current_idx = app.search_results_idx;
                let should_reset = current_idx.is_some() && current_idx.unwrap() >= previous_results.len();
                app.models_mode = ModelsMode::Search { query: previous_query, results: previous_results, sort_by: SearchSort::Relevance, show_readme: true, page: 0, loading: false, has_more: true };
                app.search_results_idx = current_idx;
                if should_reset { app.search_results_idx = Some(0); }
            }
            app.set_redraw();
            return;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if let ModelsMode::Files { files, selected_idx, .. } = &mut app.models_mode {
                match *selected_idx { Some(idx) if idx > 0 => *selected_idx = Some(idx - 1), None if !files.is_empty() => *selected_idx = Some(0), _ => {} }
            }
            app.set_redraw();
            return;
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if let ModelsMode::Files { files, selected_idx, .. } = &mut app.models_mode {
                match *selected_idx { Some(idx) if idx + 1 < files.len() => *selected_idx = Some(idx + 1), None if !files.is_empty() => *selected_idx = Some(0), _ => {} }
            }
            app.set_redraw();
            return;
        }
        KeyCode::Enter => {
            let download_info = if let ModelsMode::Files { model_id, files, selected_idx, .. } = &app.models_mode {
                selected_idx.and_then(|idx| files.get(idx).map(|(f, s, u)| (model_id.clone(), f.clone(), u.clone(), *s)))
            } else { None };
            if let Some((model_id, filename, url, file_size)) = download_info {
                if app.download_progress.iter().any(|d| d.model_id == model_id && d.filename == filename) {
                    app.add_log("Download already in progress", crate::config::LogLevel::Warning);
                    return;
                }
                let models_dir = app.config.models_dir.clone();
                let file_path = models_dir.join(&filename);
                if file_path.exists() {
                    app.add_log("File already downloaded", crate::config::LogLevel::Warning);
                    return;
                }
                app.add_log(format!("Downloading {}...", filename), crate::config::LogLevel::Info);
                app.pending_download = Some((model_id, filename, url, file_size));
            }
            return;
        }
        _ => {}
    }

    if let Some(model_id) = model_id {
        fetch_readme_for_selected(app, model_id).await;
    }
}

fn handle_bench_tune_output_key(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Esc => { app.bench_tune_output_view = None; app.set_redraw(); return; }
        KeyCode::Down | KeyCode::Char('j') => { app.bench_tune_output_scroll = app.bench_tune_output_scroll.saturating_add(1); app.set_redraw(); return; }
        KeyCode::Up | KeyCode::Char('k') => { app.bench_tune_output_scroll = app.bench_tune_output_scroll.saturating_sub(1); app.set_redraw(); return; }
        KeyCode::PageDown => { app.bench_tune_output_scroll = app.bench_tune_output_scroll.saturating_add(10); app.set_redraw(); return; }
        KeyCode::PageUp => { app.bench_tune_output_scroll = app.bench_tune_output_scroll.saturating_sub(10); app.set_redraw(); return; }
        KeyCode::Left | KeyCode::Char('h') => { app.bench_tune_output_h_scroll = app.bench_tune_output_h_scroll.saturating_sub(5); app.set_redraw(); return; }
        KeyCode::Right | KeyCode::Char('l') => { app.bench_tune_output_h_scroll = app.bench_tune_output_h_scroll.saturating_add(5); app.set_redraw(); return; }
        KeyCode::Char('n') => {
            if let Some(mut result_idx) = app.bench_tune_output_view {
                if let Some(result) = app.bench_tune_results.get(result_idx) {
                    let max_iter_idx = result.outputs.len().saturating_sub(1);
                    if app.bench_tune_output_index < max_iter_idx { app.bench_tune_output_index += 1; app.bench_tune_output_scroll = 0; app.bench_tune_output_h_scroll = 0; }
                    else if result_idx < app.bench_tune_results.len().saturating_sub(1) { result_idx += 1; app.bench_tune_output_view = Some(result_idx); app.bench_tune_output_index = 0; app.bench_tune_output_scroll = 0; app.bench_tune_output_h_scroll = 0; }
                    app.set_redraw();
                }
            }
            return;
        }
        KeyCode::Char('p') => {
            if let Some(mut result_idx) = app.bench_tune_output_view {
                if app.bench_tune_output_index > 0 { app.bench_tune_output_index -= 1; app.bench_tune_output_scroll = 0; app.bench_tune_output_h_scroll = 0; }
                else if result_idx > 0 { result_idx -= 1; app.bench_tune_output_view = Some(result_idx); if let Some(prev_result) = app.bench_tune_results.get(result_idx) { app.bench_tune_output_index = prev_result.outputs.len().saturating_sub(1); } else { app.bench_tune_output_index = 0; } app.bench_tune_output_scroll = 0; app.bench_tune_output_h_scroll = 0; }
                app.set_redraw();
            }
            return;
        }
        _ => {}
    }
}

async fn handle_bench_tune_key(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            if let Some(handle) = app.server_handle.take() {
                let port = handle.port;
                app.add_log(format!("BenchTune: stopping server on port {}", port), crate::config::LogLevel::Info);
                let _ = crate::backend::server::kill_server(handle).await;
                app.server_handle = None;
                app.metrics_rx = None;
                app.metrics = Default::default();
            }
            if let Some(task) = app.bench_tune_task_handle.take() { task.abort(); }
            app.bench_tune_running = false;
            app.models_mode = ModelsMode::List;
            app.set_redraw();
            return;
        }
        KeyCode::Char('j') | KeyCode::Down => { app.bench_tune_result_row = app.bench_tune_result_row.saturating_add(1).min(app.bench_tune_results.len().saturating_sub(1)); app.set_redraw(); return; }
        KeyCode::Char('k') | KeyCode::Up => { app.bench_tune_result_row = app.bench_tune_result_row.saturating_sub(1); app.set_redraw(); return; }
        KeyCode::Enter => {
            if !app.bench_tune_results.is_empty() { app.bench_tune_output_view = Some(app.bench_tune_result_row); app.bench_tune_output_scroll = 0; app.bench_tune_output_index = 0; app.set_redraw(); return; }
        }
        _ => {}
    }
}

fn handle_server_settings_key(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => { app.server_settings_selected_idx = app.server_settings_selected_idx.saturating_sub(1); app.set_redraw(); }
        KeyCode::Down | KeyCode::Char('j') => { app.server_settings_selected_idx = (app.server_settings_selected_idx + 1).min(6); app.set_redraw(); }
        KeyCode::Enter => {
            match app.server_settings_selected_idx {
                0 => {
                    let entries = App::fetch_host_picker_entries();
                    app.host_picker_entries = entries;
                    app.host_picker_selected = 0;
                    app.global_mode = GlobalMode::HostPicker { entries: app.host_picker_entries.clone(), selected: 0 };
                }
                1 => {
                    let entries = app.fetch_backend_picker_entries();
                    app.backend_picker_entries = entries.clone();
                    let current_tag = app.settings.get_active_backend_version();
                    app.backend_picker_selected = entries.iter().position(|(b, t)| *b == app.settings.backend && t.as_ref() == current_tag).unwrap_or(0);
                    app.global_mode = GlobalMode::BackendPicker { entries, selected: app.backend_picker_selected };
                }
                2 => { app.settings.threads = (app.settings.threads % app.max_threads) + 1; }
                3 => { app.settings.threads_batch = (app.settings.threads_batch % 32) + 1; }
                4 => { app.server_mode = match app.server_mode { crate::models::ServerMode::Normal => crate::models::ServerMode::Router, crate::models::ServerMode::Router => crate::models::ServerMode::Bench, crate::models::ServerMode::Bench => crate::models::ServerMode::BenchTune, crate::models::ServerMode::BenchTune => crate::models::ServerMode::Normal }; }
                5 => { if app.server_handle.is_none() { app.settings.api_endpoint_enabled = !app.settings.api_endpoint_enabled; } }
                6 => { app.global_mode = GlobalMode::RpcManager; app.rpc_workers_selected_idx = 0; app.editing_rpc_worker = None; }
                _ => {}
            }
            sync_global_settings(app);
            app.set_redraw();
        }
        KeyCode::Left | KeyCode::Char('h') => {
            match app.server_settings_selected_idx { 2 => app.settings.threads = app.settings.threads.saturating_sub(1).max(1), 3 => app.settings.threads_batch = app.settings.threads_batch.saturating_sub(1).max(1), _ => {} }
            app.update_vram_estimate();
            sync_global_settings(app);
            app.settings_render_cache = None;
            app.set_redraw();
        }
        KeyCode::Right | KeyCode::Char('l') => {
            match app.server_settings_selected_idx { 2 => app.settings.threads = (app.settings.threads + 1).min(app.max_threads), 3 => app.settings.threads_batch = (app.settings.threads_batch + 1).min(64), _ => {} }
            app.update_vram_estimate();
            sync_global_settings(app);
            app.settings_render_cache = None;
            app.set_redraw();
        }
        _ => {}
    }
}
