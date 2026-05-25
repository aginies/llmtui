use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Position, Rect};
use ratatui::widgets::TableState;
use tracing::debug;

use crate::backend::hub;
use crate::config::builtin_profiles;

use crate::models::{ModelSettings, SearchSort};
use crate::tui::app::{App, ActivePanel, GlobalMode, ModelsMode, LoadingPhase, ConfirmationKind};

async fn execute_confirmation(app: &mut App, kind: ConfirmationKind) {
    match kind {
        ConfirmationKind::Exit => {
            app.running = false;
        }
        ConfirmationKind::Reset => {
            app.reset_to_defaults();
        }
        ConfirmationKind::Delete => {
            if let Some(model) = app.selected_model() {
                let display_name = model.display_name.clone();
                app.add_log(format!("Deleting model {}...", display_name), crate::config::LogLevel::Info);
            }
        }
        ConfirmationKind::Unload => {
            if let Some((name, _)) = &app.pending_api_unload {
                app.add_log(format!("Unloading {} via API...", name), crate::config::LogLevel::Info);
            }
        }
        ConfirmationKind::DeleteBackend => {
            // Handled in main.rs loop by looking at pending_backend_deletion
            // and confirming global_mode transitioned back to Normal
        }
    }
}

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
                    // Cancelled (No)
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
        handle_tags_key(app, key);
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
    if let GlobalMode::PromptPicker { entries, selected, editing, edit_buffer, edit_cursor_pos, confirm_delete } = &mut app.global_mode {
        // Delete confirmation
        if *confirm_delete {
            match key.code {
                KeyCode::Char('y') => {
                    if *selected < entries.len() {
                        let name = entries[*selected].0.clone();
                        if matches!(name.as_str(), "General" | "Coder" | "Thinker" | "Mathematician") {
                            let log_msg = "Cannot delete built-in preset";
                            let log_level = crate::config::LogLevel::Error;
                            *confirm_delete = false;
                            app.add_log(log_msg, log_level);
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

        // Edit mode
        if *editing {
            match key.code {
                KeyCode::Esc => {
                    *editing = false;
                    app.set_redraw();
                }
                KeyCode::Enter => {
                    let byte_pos = edit_buffer.char_indices()
                        .nth(*edit_cursor_pos)
                        .map(|(i, _)| i)
                        .unwrap_or(edit_buffer.len());
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
                    let log_level = if saved {
                        crate::config::LogLevel::Info
                    } else {
                        crate::config::LogLevel::Error
                    };
                    *editing = false;
                    app.add_log(log_msg, log_level);
                    app.set_redraw();
                }
                KeyCode::Char(c) => {
                    let char_pos = *edit_cursor_pos;
                    let byte_pos = edit_buffer.char_indices()
                        .nth(char_pos)
                        .map(|(i, _)| i)
                        .unwrap_or(edit_buffer.len());
                    edit_buffer.insert_str(byte_pos, &c.to_string());
                    *edit_cursor_pos += 1;
                    app.set_redraw();
                }
                KeyCode::Backspace => {
                    if *edit_cursor_pos > 0 {
                        let char_pos = *edit_cursor_pos - 1;
                        let byte_pos = edit_buffer.char_indices()
                            .nth(char_pos)
                            .map(|(i, _)| i)
                            .unwrap_or(edit_buffer.len());
                        let char_len = edit_buffer[byte_pos..].chars().next().unwrap_or('\0').len_utf8();
                        edit_buffer.drain(byte_pos..byte_pos + char_len);
                        *edit_cursor_pos -= 1;
                        app.set_redraw();
                    }
                }
                KeyCode::Left => {
                    *edit_cursor_pos = edit_cursor_pos.saturating_sub(1);
                    app.set_redraw();
                }
                KeyCode::Right => {
                    *edit_cursor_pos = (*edit_cursor_pos + 1).min(edit_buffer.chars().count());
                    app.set_redraw();
                }
                _ => {}
            }
            return;
        }

        // List mode
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
                let preset = crate::config::SystemPromptPreset {
                    name: name.clone(),
                    description: "User-defined preset".into(),
                    content: String::new(),
                };
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
            KeyCode::Esc => {
                app.global_mode = GlobalMode::Normal;
                app.set_redraw();
            }
            _ => {}
        }
        return;
    }

    // BenchTune Setup
    if let GlobalMode::BenchTuneSetup { config, selected_idx, bench_mode_selection, editing_prompt, editing_kwargs } = &mut app.global_mode {
        match key.code {
            KeyCode::Char('m') if key.modifiers.contains(KeyModifiers::ALT) => {
                // Toggle benchmark mode
                *bench_mode_selection = if *bench_mode_selection == 0 { 1 } else { 0 };
                config.bench_mode = match *bench_mode_selection {
                    0 => crate::models::BenchTuneMode::RuntimeOnly,
                    _ => crate::models::BenchTuneMode::Full,
                };
            }
            KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::ALT) => {
                // Toggle prompt editing
                *editing_prompt = !*editing_prompt;
                if *editing_prompt {
                    app.edit_cursor_pos = config.prompt.len();
                }
            }
            KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::ALT) => {
                // Toggle n_predict editing
                app.editing_n_predict = !app.editing_n_predict;
                if app.editing_n_predict {
                    app.n_predict_edit_buffer = config.n_predict.to_string();
                }
            }
            KeyCode::Char('i') if key.modifiers.contains(KeyModifiers::ALT) => {
                // Toggle iterations editing
                app.editing_iters = !app.editing_iters;
                if app.editing_iters {
                    app.iters_edit_buffer = config.num_iterations.to_string();
                }
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::ALT) => {
                // Toggle chat template kwargs editing
                *editing_kwargs = !*editing_kwargs;
                if *editing_kwargs {
                    app.edit_cursor_pos = config.chat_template_kwargs.as_deref().unwrap_or("").len();
                }
            }
            KeyCode::Up | KeyCode::Char('k') | KeyCode::Left => {
                if *editing_prompt || *editing_kwargs {
                    // Move cursor left
                    if app.edit_cursor_pos > 0 {
                        app.edit_cursor_pos = app.edit_cursor_pos.saturating_sub(1);
                    }
                } else {
                    *selected_idx = selected_idx.saturating_sub(1);
                }
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Right => {
                if *editing_prompt || *editing_kwargs {
                    // Move cursor right
                    let len = if *editing_prompt { config.prompt.len() } else { config.chat_template_kwargs.as_deref().map(|s| s.len()).unwrap_or(0) };
                    app.edit_cursor_pos = (app.edit_cursor_pos + 1).min(len);
                } else {
                    *selected_idx = (*selected_idx + 1).min(config.params_to_test.len().saturating_sub(1));
                }
            }
            KeyCode::Char(' ') => {
                if *editing_prompt {
                    // Insert space in prompt
                    if app.edit_cursor_pos <= config.prompt.len() {
                        config.prompt.insert(app.edit_cursor_pos, ' ');
                        app.edit_cursor_pos += 1;
                    }
                } else if *editing_kwargs {
                    let kwargs = config.chat_template_kwargs.get_or_insert_with(String::new);
                    if app.edit_cursor_pos <= kwargs.len() {
                        kwargs.insert(app.edit_cursor_pos, ' ');
                        app.edit_cursor_pos += 1;
                    }
                } else {
                    // Toggle parameter
                    if *selected_idx < config.params_to_test.len() {
                        config.params_to_test[*selected_idx].enabled = !config.params_to_test[*selected_idx].enabled;
                    }
                }
            }
            KeyCode::Char(c) => {
                if *editing_prompt {
                    config.prompt.insert(app.edit_cursor_pos, c);
                    app.edit_cursor_pos += 1;
                } else if *editing_kwargs {
                    let kwargs = config.chat_template_kwargs.get_or_insert_with(String::new);
                    kwargs.insert(app.edit_cursor_pos, c);
                    app.edit_cursor_pos += 1;
                } else if app.editing_n_predict {
                    if c.is_ascii_digit() {
                        app.n_predict_edit_buffer.push(c);
                    }
                } else if app.editing_iters {
                    if c.is_ascii_digit() {
                        app.iters_edit_buffer.push(c);
                    }
                }
            }
            KeyCode::Backspace => {
                if *editing_prompt {
                    if app.edit_cursor_pos > 0 {
                        config.prompt.remove(app.edit_cursor_pos - 1);
                        app.edit_cursor_pos = app.edit_cursor_pos.saturating_sub(1);
                    }
                } else if *editing_kwargs {
                    let kwargs = config.chat_template_kwargs.get_or_insert_with(String::new);
                    if app.edit_cursor_pos > 0 {
                        kwargs.remove(app.edit_cursor_pos - 1);
                        app.edit_cursor_pos = app.edit_cursor_pos.saturating_sub(1);
                    }
                } else if app.editing_n_predict {
                    app.n_predict_edit_buffer.pop();
                } else if app.editing_iters {
                    app.iters_edit_buffer.pop();
                } else {
                    *selected_idx = selected_idx.saturating_sub(1);
                }
            }
            KeyCode::Delete => {
                if *editing_prompt {
                    if app.edit_cursor_pos < config.prompt.len() {
                        config.prompt.remove(app.edit_cursor_pos);
                    }
                } else if *editing_kwargs {
                    let kwargs = config.chat_template_kwargs.get_or_insert_with(String::new);
                    if app.edit_cursor_pos < kwargs.len() {
                        kwargs.remove(app.edit_cursor_pos);
                    }
                } else if app.editing_n_predict {
                    if !app.n_predict_edit_buffer.is_empty() {
                        app.n_predict_edit_buffer.pop();
                    }
                } else if app.editing_iters {
                    if !app.iters_edit_buffer.is_empty() {
                        app.iters_edit_buffer.pop();
                    }
                }
            }
            KeyCode::Enter => {
                if *editing_prompt {
                    *editing_prompt = false;
                } else if *editing_kwargs {
                    *editing_kwargs = false;
                } else if app.editing_n_predict {
                    if let Ok(val) = app.n_predict_edit_buffer.parse::<u32>() {
                        let clamped = val.clamp(1, 16384);
                        config.n_predict = clamped;
                    }
                    app.editing_n_predict = false;
                } else if app.editing_iters {
                    if let Ok(val) = app.iters_edit_buffer.parse::<u32>() {
                        config.num_iterations = val.max(1).min(100);
                    }
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
                if *editing_prompt {
                    *editing_prompt = false;
                } else if *editing_kwargs {
                    *editing_kwargs = false;
                } else if app.editing_n_predict {
                    app.editing_n_predict = false;
                } else if app.editing_iters {
                    app.editing_iters = false;
                } else {
                    app.global_mode = GlobalMode::Normal;
                }
            }
            _ => {}
        }
        app.set_redraw();
        return;
    }

    // Skip all if in backend picker
    if let GlobalMode::BackendPicker { entries, selected } = &mut app.global_mode {
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
                let (backend, tag) = entries[*selected].clone();
                app.settings.backend = backend;
                
                // Set the version field for this backend
                app.settings.set_active_backend_version(tag.clone());

                // If not installed, trigger immediate resolution in background
                if !crate::backend::hub::is_backend_version_installed(backend, tag.as_deref()) {
                    app.backend_resolving = true;
                    let tag_param = tag.clone();
                    
                    // Ensure download channel exists so progress reporting works
                    if app.download_rx.is_none() {
                        let (tx, rx) = tokio::sync::broadcast::channel(10);
                        app.download_tx = Some(tx);
                        app.download_rx = Some(rx);
                    }
                    
                    // Create a log channel for backend resolution
                    let (log_tx, log_rx) = tokio::sync::mpsc::channel(100);
                    app.server_log_rx = Some(log_rx);

                    let tx = app.download_tx.clone();
                    let handle = tokio::spawn(async move {
                        crate::backend::hub::resolve_backend_binary(backend, tag_param.as_deref(), Some(log_tx), tx).await
                            .map_err(|e| e.to_string())
                    });
                    app.backend_resolve_handle = Some(handle);
                } else {
                    // Selected backend is already installed, unblock loading
                    app.backend_resolving = false;
                }

                app.global_mode = GlobalMode::Normal;
                sync_global_settings(app);
                app.set_redraw();
            }
            KeyCode::Char('d') => {
                if let Some((backend, Some(tag))) = entries.get(*selected) {
                    app.pending_backend_deletion = Some((*backend, tag.clone()));
                    app.global_mode = GlobalMode::Confirmation {
                        selected: false,
                        kind: ConfirmationKind::DeleteBackend,
                    };
                    app.set_redraw();
                }
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
            if !app.download_progress.is_empty()
                && let Some(idx) = app.download_scroll_state.selected() {
                    let mut cancelled_name = None;
                    if let Some(state) = app.download_progress.get_mut(idx)
                        && let Some(token) = &state.cancel_token {
                            token.store(true, std::sync::atomic::Ordering::Relaxed);
                            state.cancelled = true;
                            cancelled_name = Some(state.filename.clone());
                        }
                    if let Some(name) = cancelled_name {
                        app.add_log(format!("Cancelling download of {}...", name), crate::config::LogLevel::Info);
                        app.set_redraw();
                        return;
                    }
                }
            
            // Check if any models are loaded before exiting
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
            // Toggle panel help
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
            app.toggle_panel_visibility(1);
            if app.is_panel_visible(1) && app.server_handle.is_none() {
                app.active_panel = ActivePanel::ServerSettings;
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
            if app.is_panel_visible(4) {
                app.active_panel = ActivePanel::ActiveModel;
            }
            return;
        }
        KeyCode::F(6) => {
            app.toggle_panel_visibility(5);
            if app.is_panel_visible(5) {
                app.active_panel = ActivePanel::Log;
            }
            return;
        }
        // Shift+Left/Right to resize horizontal panel split
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
        KeyCode::Char(c @ '1'..='6') | KeyCode::Char(c @ '9') => {
            let is_search = matches!(app.models_mode, ModelsMode::Search { .. }) && app.active_panel == ActivePanel::Models;
            let is_llm_settings = app.active_panel == ActivePanel::LlmSettings;
            let is_editing_preset = app.active_panel == ActivePanel::SystemPromptPresets && app.editing_preset.is_some();
            
            if !is_search && !is_llm_settings && !is_editing_preset {
                match c {
                    '1' => {
                        app.panel_visibility |= 1 << 0;
                        app.active_panel = ActivePanel::Models;
                    }
                    '2' => {
                        app.panel_visibility |= 1 << 1;
                        if app.server_handle.is_none() {
                            app.active_panel = ActivePanel::ServerSettings;
                        }
                    }
                    '4' => {
                        app.panel_visibility |= 1 << 3;
                        app.active_panel = ActivePanel::LlmSettings;
                    }
                    '6' => {
                        app.panel_visibility |= 1 << 5;
                        app.active_panel = ActivePanel::Log;
                    }
                    '9' => {
                        app.panel_visibility = 0b111111;
                        app.log_expanded = false;
                    }
                    _ => {}
                }
                app.set_redraw();
                return;
            }
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
            // Focus Log
            app.active_panel = ActivePanel::Log;
            app.set_redraw();
            return;
        }
        KeyCode::Char('k')
            if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) =>
        {
            // Toggle CmdLine overlay (hidden shortcut) — compute on demand
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
            // Hide Active Model (4) and Log (5) panels by default in search mode
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
        match key.code {
            KeyCode::Esc => {
                app.models_mode = ModelsMode::List;
                // Restore Active Model (4) and Log (5) panels when exiting search mode
                app.panel_visibility |= (1 << 4) | (1 << 5);
                app.set_redraw();
                return;
            }
            KeyCode::Enter => {
                let query = if let ModelsMode::Search { query, page, has_more, .. } = &mut app.models_mode {
                    *page = 0;
                    *has_more = true;
                    query.clone()
                } else {
                    return;
                };

                if query.is_empty() {
                    return;
                }

                app.add_log(format!("Searching for '{}'...", query), crate::config::LogLevel::Info);
                app.pending_search_load = Some((query, 0));
                app.search_loading = true;
                app.search_table_state = TableState::default();
                app.set_redraw();
                return;
            }
            KeyCode::Backspace => {
                if let ModelsMode::Search { query, .. } = &mut app.models_mode {
                    query.pop();
                }
                app.set_redraw();
                return;
            }
            KeyCode::Char('L') => {
                let model_id = if let ModelsMode::Search { results, .. } = &app.models_mode {
                    app.search_results_idx.and_then(|idx| results.get(idx).map(|r| r.model_id.clone()))
                } else {
                    None
                };

                if let Some(model_id) = model_id {
                    app.add_log(format!("Loading files for {}...", model_id), crate::config::LogLevel::Info);
                    match hub::list_gguf_files(&model_id).await {
                        Ok(files) => {
                            app.add_log(format!("Found {} GGUF files", files.len()), crate::config::LogLevel::Info);
                            // Now clone only when we know the operation succeeded
                            if let ModelsMode::Search { query, results, .. } = &app.models_mode {
                                let selected_result = app.search_results_idx.and_then(|idx| results.get(idx).cloned());
                                app.files_table_state = TableState::default();
                                app.models_mode = crate::tui::app::ModelsMode::Files {
                                    model_id,
                                    files,
                                    selected_idx: Some(0),
                                    previous_query: query.clone(),
                                    previous_results: results.clone(),
                                    selected_result,
                                };
                            }
                        }
                        Err(e) => {
                            app.add_log(format!("No GGUF files: {}", e), crate::config::LogLevel::Info);
                        }
                    }
                }
                return;
            }
            KeyCode::Char('S') => {
                // Cycle sort mode
                if let ModelsMode::Search { sort_by, results, .. } = &mut app.models_mode {
                    *sort_by = sort_by.next();
                    results.sort_by(|a, b| match sort_by {
                        SearchSort::Downloads => b.downloads.cmp(&a.downloads),
                        SearchSort::Likes => b.likes.cmp(&a.likes),
                        SearchSort::Trending => b.trending_score.cmp(&a.trending_score),
                        SearchSort::CreatedAt => {
                            let a_date = a.created_at.as_deref().unwrap_or("");
                            let b_date = b.created_at.as_deref().unwrap_or("");
                            b_date.cmp(a_date)
                        }
                        SearchSort::Relevance => std::cmp::Ordering::Equal,
                    });
                    if !results.is_empty() {
                        app.search_results_idx = Some(0);
                    } else {
                        app.search_results_idx = None;
                    }
                }
                app.set_redraw();
                return;
            }
         KeyCode::Char('B') => {
                // Go back one page
                if let ModelsMode::Search { page, .. } = &app.models_mode
                    && *page > 0 {
                        let query = if let ModelsMode::Search { query, .. } = &app.models_mode {
                            query.clone()
                        } else {
                            String::new()
                        };
                        let offset = (*page as u32 - 1) * 50;
                        app.add_log(format!("Loading page {}...", *page - 1), crate::config::LogLevel::Info);
                        // Mutate via mutable borrow
                        if let ModelsMode::Search { page, .. } = &mut app.models_mode {
                            *page -= 1;
                        }
                        app.pending_search_load = Some((query, offset));
                        app.search_loading = true;
                        // Keep the current results while loading
                        app.set_redraw();
                        return;
                    }
                return;
            }
            KeyCode::Down => {
                let len = app.search_results_len();
                match app.search_results_idx {
                    Some(idx) if idx + 1 < len => app.search_results_idx = Some(idx + 1),
                    // At last item, load more
                    Some(idx) => {
                        if idx + 1 >= len
                            && let ModelsMode::Search { has_more, loading, page, .. } = &app.models_mode
                                && !*loading && *has_more {
                                    let query = if let ModelsMode::Search { query, .. } = &app.models_mode {
                                        query.clone()
                                    } else {
                                        String::new()
                                    };
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
                // When not shown, fetch and display it.
                let model_id = if let ModelsMode::Search { results, .. } = &app.models_mode {
                    app.search_results_idx.and_then(|idx| results.get(idx).map(|r| r.model_id.clone()))
                } else {
                    None
                };
                if let Some(model_id) = model_id {
                    app.add_log(format!("Fetching README for {}...", model_id), crate::config::LogLevel::Info);
                    app.add_log("This may take a moment...", crate::config::LogLevel::Info);
                    fetch_and_store_readme(app, model_id).await;
                    if let ModelsMode::Search { show_readme, .. } = &mut app.models_mode {
                        *show_readme = true;
                    }
                }
                return;
            }
            KeyCode::Char(c) => {
                if let ModelsMode::Search { query, .. } = &mut app.models_mode {
                    query.push(c);
                }
                app.set_redraw();
                return;
            }
            KeyCode::Up => {
                match app.search_results_idx {
                    Some(idx) if idx > 0 => app.search_results_idx = Some(idx - 1),
                    None => {
                        let len = if let ModelsMode::Search { results, .. } = &app.models_mode { results.len() } else { 0 };
                        if len > 0 { app.search_results_idx = Some(0); }
                    }
                    _ => {}
                }
                app.set_redraw();
                return;
            }
            _ => {}
        }

        // Auto-fetch README for the selected model (outside the match, using current index)
        if let ModelsMode::Search { results, .. } = &app.models_mode {
            if let Some(idx) = app.search_results_idx {
                if let Some(r) = results.get(idx) {
                    fetch_readme_for_selected(app, r.model_id.clone()).await;
                }
            }
        }
        return;
    }

    // Handle files mode
    let is_files = matches!(app.models_mode, ModelsMode::Files { .. });
    if is_files && app.active_panel == ActivePanel::Models {
        // Extract model_id before the match so we can call async fn after
        let model_id = if let ModelsMode::Files { model_id, .. } = &app.models_mode {
            Some(model_id.clone())
        } else {
            None
        };

        match key.code {
            KeyCode::Esc => {
                // Move data out instead of cloning
                if let ModelsMode::Files { previous_query, previous_results, .. } =
                    std::mem::replace(&mut app.models_mode, ModelsMode::List) {
                    let current_idx = app.search_results_idx;
                    let should_reset = current_idx.is_some() && current_idx.unwrap() >= previous_results.len();

                     app.models_mode = ModelsMode::Search {
                        query: previous_query,
                        results: previous_results,
                        sort_by: SearchSort::Relevance,
                        show_readme: true,
                        page: 0,
                        loading: false,
                        has_more: true,
                    };

                    app.search_results_idx = current_idx;
                    if should_reset {
                        app.search_results_idx = Some(0);
                    }
                }
                app.set_redraw();
                return;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let ModelsMode::Files { files, selected_idx, .. } = &mut app.models_mode {
                    match *selected_idx {
                        Some(idx) if idx > 0 => *selected_idx = Some(idx - 1),
                        None if !files.is_empty() => *selected_idx = Some(0),
                        _ => {}
                    }
                }
                app.set_redraw();
                return;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let ModelsMode::Files { files, selected_idx, .. } = &mut app.models_mode {
                    match *selected_idx {
                        Some(idx) if idx + 1 < files.len() => *selected_idx = Some(idx + 1),
                        None if !files.is_empty() => *selected_idx = Some(0),
                        _ => {}
                    }
                }
                app.set_redraw();
                return;
            }
            KeyCode::Enter => {
                let download_info = if let ModelsMode::Files { model_id, files, selected_idx, .. } = &app.models_mode {
                    selected_idx.and_then(|idx| files.get(idx).map(|(f, _s, u)| (model_id.clone(), f.clone(), u.clone())))
                } else {
                    None
                };

                if let Some((model_id, filename, url)) = download_info {
                    // Check if download is already in progress
                    if app.download_progress.iter().any(|d| d.model_id == model_id && d.filename == filename) {
                        app.add_log("Download already in progress", crate::config::LogLevel::Warning);
                        return;
                    }
                    // Check if file already exists locally
                    let models_dir = app.config.models_dir.clone();
                    let file_path = models_dir.join(&filename);
                    if file_path.exists() {
                        app.add_log("File already downloaded", crate::config::LogLevel::Warning);
                        return;
                    }
                    app.add_log(format!("Downloading {}...", filename), crate::config::LogLevel::Info);
                    app.pending_download = Some((model_id, filename, url));
                }
                return;
            }
            _ => {}
        }

        // Auto-fetch README for the selected model (outside the match)
        if let Some(model_id) = model_id {
            fetch_readme_for_selected(app, model_id).await;
        }
        return;
    }

    // Handle bench_tune output view modal
    if app.bench_tune_output_view.is_some() {
        match key.code {
            KeyCode::Esc => {
                app.bench_tune_output_view = None;
                app.set_redraw();
                return;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                app.bench_tune_output_scroll = app.bench_tune_output_scroll.saturating_add(1);
                app.set_redraw();
                return;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                app.bench_tune_output_scroll = app.bench_tune_output_scroll.saturating_sub(1);
                app.set_redraw();
                return;
            }
            KeyCode::PageDown => {
                app.bench_tune_output_scroll = app.bench_tune_output_scroll.saturating_add(10);
                app.set_redraw();
                return;
            }
            KeyCode::PageUp => {
                app.bench_tune_output_scroll = app.bench_tune_output_scroll.saturating_sub(10);
                app.set_redraw();
                return;
            }
            KeyCode::Left | KeyCode::Char('h') => {
                app.bench_tune_output_h_scroll = app.bench_tune_output_h_scroll.saturating_sub(5);
                app.set_redraw();
                return;
            }
            KeyCode::Right | KeyCode::Char('l') => {
                app.bench_tune_output_h_scroll = app.bench_tune_output_h_scroll.saturating_add(5);
                app.set_redraw();
                return;
            }
            KeyCode::Char('n') => {
                if let Some(mut result_idx) = app.bench_tune_output_view {
                    if let Some(result) = app.bench_tune_results.get(result_idx) {
                        let max_iter_idx = result.outputs.len().saturating_sub(1);
                        if app.bench_tune_output_index < max_iter_idx {
                            app.bench_tune_output_index += 1;
                            app.bench_tune_output_scroll = 0;
                            app.bench_tune_output_h_scroll = 0;
                        } else if result_idx < app.bench_tune_results.len().saturating_sub(1) {
                            result_idx += 1;
                            app.bench_tune_output_view = Some(result_idx);
                            app.bench_tune_output_index = 0;
                            app.bench_tune_output_scroll = 0;
                            app.bench_tune_output_h_scroll = 0;
                        }
                        app.set_redraw();
                    }
                }
                return;
            }
            KeyCode::Char('p') => {
                if let Some(mut result_idx) = app.bench_tune_output_view {
                    if app.bench_tune_output_index > 0 {
                        app.bench_tune_output_index -= 1;
                        app.bench_tune_output_scroll = 0;
                        app.bench_tune_output_h_scroll = 0;
                    } else if result_idx > 0 {
                        result_idx -= 1;
                        app.bench_tune_output_view = Some(result_idx);
                        if let Some(prev_result) = app.bench_tune_results.get(result_idx) {
                            app.bench_tune_output_index = prev_result.outputs.len().saturating_sub(1);
                        } else {
                            app.bench_tune_output_index = 0;
                        }
                        app.bench_tune_output_scroll = 0;
                        app.bench_tune_output_h_scroll = 0;
                    }
                    app.set_redraw();
                }
                return;
            }
            _ => {}
        }
        return;
    }

    // Handle bench_tune mode
    if matches!(app.models_mode, ModelsMode::BenchTune { .. }) {
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
                if let Some(task) = app.bench_tune_task_handle.take() {
                    task.abort();
                }
                app.bench_tune_running = false;
                app.models_mode = ModelsMode::List;
                app.set_redraw();
                return;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                app.bench_tune_result_row = app.bench_tune_result_row.saturating_add(1).min(app.bench_tune_results.len().saturating_sub(1));
                app.set_redraw();
                return;
            }
            KeyCode::Char('k') | KeyCode::Up => {
                app.bench_tune_result_row = app.bench_tune_result_row.saturating_sub(1);
                app.set_redraw();
                return;
            }
            KeyCode::Enter => {
                if !app.bench_tune_results.is_empty() {
                    app.bench_tune_output_view = Some(app.bench_tune_result_row);
                    app.bench_tune_output_scroll = 0;
                    app.bench_tune_output_index = 0;
                    app.set_redraw();
                    return;
                }
            }
            _ => {}
        }
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

    // Global shortcuts for server settings (only active when ServerSettings is focused)
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
async fn fetch_and_store_readme(app: &mut App, model_id: String) {
    match crate::backend::hub::fetch_readme(&model_id).await {
        Ok(readme) => {
            if let ModelsMode::Search { results, .. } = &mut app.models_mode
                && let Some(idx) = app.search_results_idx
                && let Some(r) = results.get_mut(idx)
            {
                r.readme = Some(readme);
            }
            app.add_log("README loaded.", crate::config::LogLevel::Info);
        }
        Err(e) => {
            app.add_log(format!("Failed to fetch README: {}", e), crate::config::LogLevel::Error);
        }
    }
}

async fn fetch_readme_for_selected(app: &mut App, model_id: String) {
    if let ModelsMode::Search { results, show_readme, .. } = &app.models_mode
        && *show_readme
            && let Some(idx) = app.search_results_idx
                && let Some(r) = results.get(idx)
                    && r.readme.is_none() {
                        app.add_log(format!("Fetching README for {}...", model_id), crate::config::LogLevel::Info);
                        fetch_and_store_readme(app, model_id).await;
                    }
}

fn handle_readme_key(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            if let ModelsMode::Search { show_readme, .. } = &mut app.models_mode {
                *show_readme = false;
                app.active_panel = ActivePanel::Models;
            }
            if let ModelsMode::Files { .. } = &app.models_mode {
                app.active_panel = ActivePanel::Models;
            }
            app.set_redraw();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.readme_scroll_offset = app.readme_scroll_offset.saturating_sub(1);
            app.set_redraw();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.readme_scroll_offset = app.readme_scroll_offset.saturating_add(1);
            app.set_redraw();
        }
        _ => {}
    }
}

async fn handle_models_key(app: &mut App, key: crossterm::event::KeyEvent) {
    if app.filtering_local {
        match key.code {
            KeyCode::Esc => {
                app.filtering_local = false;
                app.local_filter.clear();
                app.on_model_selection_change();
            }
            KeyCode::Enter => {
                app.filtering_local = false;
            }
            KeyCode::Char(c) => {
                app.local_filter.push(c);
                app.on_model_selection_change();
            }
            KeyCode::Backspace => {
                app.local_filter.pop();
                app.on_model_selection_change();
            }
            _ => {}
        }
        app.set_redraw();
        return;
    }

    match key.code {
        KeyCode::Char('f') => {
            if matches!(app.models_mode, ModelsMode::List) {
                app.filtering_local = true;
                if app.selected_model_idx.is_none() {
                    let filtered = app.get_filtered_model_indices();
                    if !filtered.is_empty() {
                        app.selected_model_idx = Some(filtered[0]);
                        app.on_model_selection_change();
                    }
                }
                app.set_redraw();
                return;
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            let filtered = app.get_filtered_model_indices();
            if let Some(idx) = app.selected_model_idx {
                if let Some(pos) = filtered.iter().position(|&i| i == idx) {
                    if pos > 0 {
                        app.selected_model_idx = Some(filtered[pos - 1]);
                        app.on_model_selection_change();
                    }
                } else if !filtered.is_empty() {
                    app.selected_model_idx = Some(filtered[0]);
                    app.on_model_selection_change();
                }
            } else if !filtered.is_empty() {
                app.selected_model_idx = Some(filtered[0]);
                app.on_model_selection_change();
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let filtered = app.get_filtered_model_indices();
            if let Some(idx) = app.selected_model_idx {
                if let Some(pos) = filtered.iter().position(|&i| i == idx) {
                    if pos + 1 < filtered.len() {
                        app.selected_model_idx = Some(filtered[pos + 1]);
                        app.on_model_selection_change();
                    }
                } else if !filtered.is_empty() {
                    app.selected_model_idx = Some(filtered[0]);
                    app.on_model_selection_change();
                }
            } else if !filtered.is_empty() {
                app.selected_model_idx = Some(filtered[0]);
                app.on_model_selection_change();
            }
        }
        KeyCode::Enter | KeyCode::Char('l') => {
            if app.backend_resolving {
                app.add_log("Wait for backend installation to finish...", crate::config::LogLevel::Info);
                return;
            }
            if let Some(idx) = app.selected_model_idx {
                let model = app.models[idx].clone();
                let already_loaded = matches!(
                    app.model_states.get(&model.display_name),
                    Some(crate::models::ModelState::Loaded { .. })
                );
                if already_loaded {
                    app.add_log(format!("{} is already loaded", model.display_name), crate::config::LogLevel::Info);
                } else {
                    app.update_model_metadata();
                    let settings = app.selected_model_settings();
                    
                    if let Some(handle) = &app.server_handle
                        && !crate::backend::server::check_health(&handle.host, handle.port).await {
                            app.add_log("Router unresponsive, restarting...", crate::config::LogLevel::Info);
                            if let Some(h) = app.server_handle.take() {
                                app.pending_kill = Some(h);
                            }
                        }

                    if app.server_handle.is_none() {
                        // Start server (with model in CLI for normal mode, without model for router mode)
                        app.last_error_message = None;
                        
                       if app.server_mode == crate::models::ServerMode::BenchTune {
                            let bench_tune_config = crate::models::BenchTuneConfig::new(
                                model.path.clone(),
                                3, // Default iterations
                                crate::models::BENCHMARK_PROMPT.to_string(),
                            );
                            app.global_mode = GlobalMode::BenchTuneSetup {
                                config: bench_tune_config,
                                selected_idx: 0,
                                bench_mode_selection: 0,
                                editing_prompt: false,
                                editing_kwargs: false,
                            };
                            return;
                        }
                        if app.server_mode == crate::models::ServerMode::Router {
                            // Router mode: start server without a model, then load via /load API
                            app.pending_spawn = Some((None, settings.clone()));
                            // Queue the load so it triggers once server is ready
                            app.pending_api_load = Some((model.display_name.clone(), Some(model.path.to_string_lossy().to_string())));
                            app.loading_phases = std::iter::once(LoadingPhase::ServerStarting).collect();
                            app.last_active_phase = Some(LoadingPhase::ServerStarting);
                            app.loading_progress = 0.25;
                            app.add_log(format!("Starting router server..."), crate::config::LogLevel::Info);
                        } else {
                            // Normal mode: start server WITH the specific model directly
                            app.pending_spawn = Some((Some(model.clone()), settings));
                            app.loading_phases = std::iter::once(LoadingPhase::ServerStarting).collect();
                            app.last_active_phase = Some(LoadingPhase::ServerStarting);
                            app.loading_progress = 0.25;
                            app.add_log(format!("Starting server with {}...", model.display_name), crate::config::LogLevel::Info);
                        }
                    } else {
                        // Server already running, load via API
                        
                        // Check if we reached the limit of models to load (based on Max Concurrent Predictions)
                        let active_count = app.model_states.values().filter(|s| 
                            matches!(s, crate::models::ModelState::Loaded { .. } | crate::models::ModelState::Loading)
                        ).count();
                        
                        if let Some(max) = app.settings.max_concurrent_predictions
                            && active_count as u32 >= max {
                                app.add_log(format!("Limit reached: already {} model(s) loaded (Max Concurrent Predictions limit: {})", active_count, max), crate::config::LogLevel::Warning);
                            return;
                        }

                        app.last_error_message = None;
                        app.pending_api_load = Some((model.display_name.clone(), Some(model.path.to_string_lossy().to_string())));
                        app.loading_phases = std::iter::once(LoadingPhase::LoadingModel).collect();
                        app.last_active_phase = Some(LoadingPhase::LoadingModel);
                        app.loading_progress = 0.5;
                        app.add_log(format!("Loading {} via API...", model.display_name), crate::config::LogLevel::Info);
                    }
                }
            }
        }
        KeyCode::Char('u') => {
            if let Some(idx) = app.selected_model_idx {
                let model = app.models[idx].clone();
                if let Some(crate::models::ModelState::Loaded { .. }) = app.model_states.get(&model.display_name) {
                    app.global_mode = GlobalMode::Confirmation {
                        selected: false,
                        kind: ConfirmationKind::Unload,
                    };
                    app.pending_api_unload = Some((model.display_name.clone(), Some(model.path.to_string_lossy().to_string())));
                } else {
                    app.add_log(format!("{} is not loaded", model.display_name), crate::config::LogLevel::Warning);
                }
            } else if app.server_handle.is_some() {
                app.add_log("Select a loaded model to unload", crate::config::LogLevel::Warning);
            } else if app.server_mode == crate::models::ServerMode::Router {
                // Router mode: no server running, no model loaded — fine
            } else {
                app.add_log("No model is currently loaded", crate::config::LogLevel::Warning);
            }
        }        KeyCode::Char('d') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
            if app.active_panel != ActivePanel::Models {
                app.add_log("Press Tab to switch to Models panel, then Ctrl+D to delete", crate::config::LogLevel::Warning);
                return;
            }
            if let Some(model) = app.selected_model() {
                let display_name = model.display_name.clone();
                app.pending_deletion = Some(model.path.clone());
                app.global_mode = GlobalMode::Confirmation { selected: false, kind: ConfirmationKind::Delete };
                app.add_log(format!("Delete confirmation for {} shown", display_name), crate::config::LogLevel::Info);
            } else {
                app.add_log("No model selected to delete", crate::config::LogLevel::Warning);
            }
        }
        _ => {}
    }
}

fn handle_log_key(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Enter if !app.log_expanded => {
            app.log_expanded = true;
            app.set_redraw();
        }
        KeyCode::Esc if app.log_expanded => {
            app.log_expanded = false;
            app.set_redraw();
        }
        KeyCode::Char('f') => {
            app.log_follow = !app.log_follow;
            app.set_redraw();
        }
        KeyCode::Char('g') | KeyCode::Home => {
            app.log_scroll_offset = 0;
            app.log_follow = false;
            app.set_redraw();
        }
        KeyCode::Char('G') | KeyCode::End => {
            app.log_follow = true;
            app.set_redraw();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.log_scroll_offset = app.log_scroll_offset.saturating_sub(1);
            app.log_follow = false;
            app.set_redraw();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.log_scroll_offset = app.log_scroll_offset + 1;
            // Get inner height (approximate, since we don't have layout here)
            // But we can just use the total lines check
            if app.log_scroll_offset >= app.log_total_lines.saturating_sub(5) {
                 app.log_follow = true;
            } else {
                 app.log_follow = false;
            }
            app.set_redraw();
        }
        KeyCode::PageUp => {
            app.log_scroll_offset = app.log_scroll_offset.saturating_sub(15);
            app.log_follow = false;
            app.set_redraw();
        }
        KeyCode::PageDown => {
            app.log_scroll_offset = app.log_scroll_offset + 15;
            if app.log_scroll_offset >= app.log_total_lines.saturating_sub(5) {
                app.log_follow = true;
            }
            app.set_redraw();
        }
        _ => {}
    }
}

fn sync_global_settings(app: &mut App) {
    let changed = app.config.default.host != app.settings.host
        || app.config.default.port != app.settings.port
        || app.config.default.backend != app.settings.backend
        || app.config.default.parallel != app.settings.parallel
        || app.config.default.max_concurrent_predictions != app.settings.max_concurrent_predictions
        || app.config.default.threads != app.settings.threads
        || app.config.default.threads_batch != app.settings.threads_batch
        || app.config.default.api_endpoint_enabled != app.settings.api_endpoint_enabled
        || app.config.default.api_endpoint_port != app.settings.api_endpoint_port
        || app.config.default.server_mode != app.server_mode
        || app.config.default.router_max_models != app.router_max_models
        || app.config.default.llama_cpp_version_cpu != app.settings.llama_cpp_version_cpu
        || app.config.default.llama_cpp_version_vulkan != app.settings.llama_cpp_version_vulkan
        || app.config.default.llama_cpp_version_rocm != app.settings.llama_cpp_version_rocm
        || app.config.default.llama_cpp_version_rocm_lemonade != app.settings.llama_cpp_version_rocm_lemonade
        || app.config.default.llama_cpp_version_cuda != app.settings.llama_cpp_version_cuda;
    if !changed {
        return;
    }
    app.config.default.host = app.settings.host.clone();
    app.config.default.port = app.settings.port;
    app.config.default.backend = app.settings.backend;
    app.config.default.parallel = app.settings.parallel;
    app.config.default.max_concurrent_predictions = app.settings.max_concurrent_predictions;
    app.config.default.threads = app.settings.threads;
    app.config.default.threads_batch = app.settings.threads_batch;
    app.config.default.api_endpoint_enabled = app.settings.api_endpoint_enabled;
    app.config.default.api_endpoint_port = app.settings.api_endpoint_port;
    app.config.default.server_mode = app.server_mode.clone();
    app.config.default.router_max_models = app.router_max_models;
    app.config.default.llama_cpp_version_cpu = app.settings.llama_cpp_version_cpu.clone();
    app.config.default.llama_cpp_version_vulkan = app.settings.llama_cpp_version_vulkan.clone();
    app.config.default.llama_cpp_version_rocm = app.settings.llama_cpp_version_rocm.clone();
    app.config.default.llama_cpp_version_rocm_lemonade = app.settings.llama_cpp_version_rocm_lemonade.clone();
    app.config.default.llama_cpp_version_cuda = app.settings.llama_cpp_version_cuda.clone();
    if let Err(e) = app.config.save() {
        app.add_log(
            format!("Failed to save global settings: {}", e),
            crate::config::LogLevel::Error,
        );
    }
}

fn handle_server_settings_key(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            app.server_settings_selected_idx = app.server_settings_selected_idx.saturating_sub(1);
            app.set_redraw();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.server_settings_selected_idx = (app.server_settings_selected_idx + 1).min(6);
            app.set_redraw();
        }
        KeyCode::Enter => {
            match app.server_settings_selected_idx {
                0 => {
                    // Open host picker
                    let entries = App::fetch_host_picker_entries();
                    app.host_picker_entries = entries;
                    app.host_picker_selected = 0;
                    app.global_mode = crate::tui::app::GlobalMode::HostPicker {
                        entries: app.host_picker_entries.clone(),
                        selected: 0,
                    };
                }
                1 => {
                    // Open backend picker
                    let entries = app.fetch_backend_picker_entries();
                    app.backend_picker_entries = entries.clone();
                    
                    // Find current selection index
                    let current_tag = app.settings.get_active_backend_version();
                    
                    app.backend_picker_selected = entries.iter()
                        .position(|(b, t)| *b == app.settings.backend && t.as_ref() == current_tag)
                        .unwrap_or(0);

                    app.global_mode = crate::tui::app::GlobalMode::BackendPicker {
                        entries,
                        selected: app.backend_picker_selected,
                    };
                }
                2 => {
                    // Cycle threads (1-max)
                    app.settings.threads = (app.settings.threads % app.max_threads) + 1;
                }
                3 => {
                    // Cycle threads batch (1-32)
                    app.settings.threads_batch = (app.settings.threads_batch % 32) + 1;
                }
                4 => {
                    // Toggle server mode
                    app.server_mode = match app.server_mode {
                        crate::models::ServerMode::Normal => crate::models::ServerMode::Router,
                        crate::models::ServerMode::Router => crate::models::ServerMode::Bench,
                        crate::models::ServerMode::Bench => crate::models::ServerMode::BenchTune,
                        crate::models::ServerMode::BenchTune => crate::models::ServerMode::Normal,
                    };
                }
                5 => {
                    // Toggle API endpoint (disabled while server is running)
                    if app.server_handle.is_none() {
                        app.settings.api_endpoint_enabled = !app.settings.api_endpoint_enabled;
                    }
                }
                6 => {
                    // Open RPC Workers modal
                    app.global_mode = crate::tui::app::GlobalMode::RpcManager;
                    app.rpc_workers_selected_idx = 0;
                    app.editing_rpc_worker = None;
                }
                _ => {}
            }
            sync_global_settings(app);
            app.set_redraw();
        }
        KeyCode::Left | KeyCode::Char('h') => {
            match app.server_settings_selected_idx {
                2 => app.settings.threads = app.settings.threads.saturating_sub(1).max(1),
                3 => app.settings.threads_batch = app.settings.threads_batch.saturating_sub(1).max(1),
                _ => {}
            }
            app.update_vram_estimate();
            sync_global_settings(app);
            app.settings_render_cache = None;
            app.set_redraw();
        }
        KeyCode::Right | KeyCode::Char('l') => {
            match app.server_settings_selected_idx {
                2 => app.settings.threads = (app.settings.threads + 1).min(app.max_threads),
                3 => app.settings.threads_batch = (app.settings.threads_batch + 1).min(64),
                _ => {}
            }
            app.update_vram_estimate();
            sync_global_settings(app);
            app.settings_render_cache = None;
            app.set_redraw();
        }
        _ => {}
    }
}

fn handle_downloads_key(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            app.download_scroll_state.select_previous();
            app.set_redraw();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.download_scroll_state.select_next();
            app.set_redraw();
        }
        KeyCode::Char('c')
            if key.modifiers.contains(crossterm::event::KeyModifiers::ALT) =>
        {
            if let Some(idx) = app.download_scroll_state.selected() {
                let filename = app.download_progress.get(idx).map(|d| d.filename.clone());
                if let Some(state) = app.download_progress.get_mut(idx)
                    && let Some(token) = &state.cancel_token {
                        token.store(true, std::sync::atomic::Ordering::Relaxed);
                        state.cancelled = true;
                        if let Some(ref name) = filename {
                            app.add_log(format!("Cancelling download of {}...", name), crate::config::LogLevel::Info);
                        }
                    }
            }
            app.set_redraw();
        }
        _ => {}
    }
}

fn handle_rpc_workers_key(app: &mut App, key: crossterm::event::KeyEvent) {
    let editing = app.editing_rpc_worker.is_some();
    
    if editing {
        match key.code {
            KeyCode::Enter => {
                if !app.settings_edit_buffer.is_empty() {
                    // Parse: [Name], IP, Port
                    let parts: Vec<&str> = app.settings_edit_buffer.split(',').map(|s| s.trim()).collect();
                    let (name, ip_str, port_str) = match parts.len() {
                        1 => ("".to_string(), parts[0].to_string(), "50052".to_string()),
                        2 => {
                            // Check if second part is a port
                            if parts[1].parse::<u16>().is_ok() {
                                ("".to_string(), parts[0].to_string(), parts[1].to_string())
                            } else {
                                (parts[0].to_string(), parts[1].to_string(), "50052".to_string())
                            }
                        }
                        3 => (parts[0].to_string(), parts[1].to_string(), parts[2].to_string()),
                        _ => ("".to_string(), "".to_string(), "0".to_string()),
                    };

                    // Validate IP
                    let is_valid_ip = ip_str.parse::<std::net::IpAddr>().is_ok();
                    // Validate Port (Unix range 1-65535, though 0 is technically reserved)
                    let port = port_str.parse::<u32>().unwrap_or(0);
                    let is_valid_port = port > 0 && port <= 65535;

                    if !is_valid_ip {
                        app.add_log(format!("Invalid IP address: {}", ip_str), crate::config::LogLevel::Error);
                    } else if !is_valid_port {
                        app.add_log(format!("Invalid port (1-65535): {}", port_str), crate::config::LogLevel::Error);
                    } else {
                        let worker = crate::config::RpcWorker {
                            selected: true,
                            name,
                            ip: ip_str,
                            port: port as u16,
                        };
                        
                        if let Some(idx) = app.editing_rpc_worker {
                            if idx < app.config.rpc_workers.len() {
                                app.config.rpc_workers[idx] = worker;
                            } else {
                                app.config.rpc_workers.push(worker);
                            }
                        }
                        let _ = app.config.save();
                        app.add_log("RPC worker saved.", crate::config::LogLevel::Info);
                    }
                }
                app.editing_rpc_worker = None;
                app.settings_edit_buffer.clear();
                app.edit_cursor_pos = 0;
            }
            KeyCode::Esc => {
                app.editing_rpc_worker = None;
                app.settings_edit_buffer.clear();
                app.edit_cursor_pos = 0;
            }
            KeyCode::Char(c) => {
                let byte_idx = app.settings_edit_buffer.char_indices().nth(app.edit_cursor_pos).map(|(i, _)| i).unwrap_or(app.settings_edit_buffer.len());
                app.settings_edit_buffer.insert(byte_idx, c);
                app.edit_cursor_pos += 1;
            }
            KeyCode::Backspace => {
                if app.edit_cursor_pos > 0 {
                    app.edit_cursor_pos -= 1;
                    let byte_idx = app.settings_edit_buffer.char_indices().nth(app.edit_cursor_pos).map(|(i, _)| i).unwrap_or(0);
                    app.settings_edit_buffer.remove(byte_idx);
                }
            }
            KeyCode::Delete => {
                if app.edit_cursor_pos < app.settings_edit_buffer.chars().count() {
                    let byte_idx = app.settings_edit_buffer.char_indices().nth(app.edit_cursor_pos).map(|(i, _)| i).unwrap_or(app.settings_edit_buffer.len());
                    app.settings_edit_buffer.remove(byte_idx);
                }
            }
            KeyCode::Left => {
                app.edit_cursor_pos = app.edit_cursor_pos.saturating_sub(1);
            }
            KeyCode::Right => {
                app.edit_cursor_pos = (app.edit_cursor_pos + 1).min(app.settings_edit_buffer.chars().count());
            }
            _ => {}
        }
    } else {
        match key.code {
            KeyCode::Esc => {
                app.global_mode = crate::tui::app::GlobalMode::Normal;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                app.rpc_workers_selected_idx = app.rpc_workers_selected_idx.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let total = app.config.rpc_workers.len();
                if total > 0 {
                    app.rpc_workers_selected_idx = (app.rpc_workers_selected_idx + 1).min(total - 1);
                }
            }
            KeyCode::Char(' ') => {
                if let Some(worker) = app.config.rpc_workers.get_mut(app.rpc_workers_selected_idx) {
                    worker.selected = !worker.selected;
                    let _ = app.config.save();
                }
            }
            KeyCode::Char('n') => {
                app.editing_rpc_worker = Some(app.config.rpc_workers.len());
                app.settings_edit_buffer.clear();
                app.edit_cursor_pos = 0;
            }
            KeyCode::Char('e') => {
                if let Some(worker) = app.config.rpc_workers.get(app.rpc_workers_selected_idx) {
                    app.editing_rpc_worker = Some(app.rpc_workers_selected_idx);
                    app.settings_edit_buffer = format!("{}, {}, {}", worker.name, worker.ip, worker.port);
                    app.edit_cursor_pos = app.settings_edit_buffer.len();
                }
            }
            KeyCode::Char('d') => {
                if !app.config.rpc_workers.is_empty() {
                    app.config.rpc_workers.remove(app.rpc_workers_selected_idx);
                    if app.rpc_workers_selected_idx >= app.config.rpc_workers.len() && !app.config.rpc_workers.is_empty() {
                        app.rpc_workers_selected_idx = app.config.rpc_workers.len() - 1;
                    }
                    let _ = app.config.save();
                }
            }
            _ => {}
        }
    }
    app.set_redraw();
}

// Settings field indices for navigation and editing
// Loading: 0: Context, 1: Prompt, 2: Keep in memory (mlock)
// GPU: 3: GPU Layers, 4: Flash Attention, 5: KV Cache Offload, 6: Cache Type K, 7: Cache Type V, 8: Active Experts
// Evaluation: 9: Eval Batch, 10: Unified KV, 11: Max Concurrent Pred
// Sampling: 12: Seed, 13: Temp, 14: Top-k, 15: Top-p, 16: Min P, 17: Max Tokens
// Repetition: 18: Rep. Penalty, 19: Rep. Last N, 20: Presence, 21: Frequency
// Total: 21 fields (20 editable)

fn apply_numeric_setting(settings: &mut ModelSettings, idx: usize, buf: &str, _max_threads: u32, max_context: u32) {
    match idx {
        // Loading
        1 => {
            if let Ok(v) = buf.parse::<u32>() {
                let mut val = v.max(128);
                if max_context > 0 {
                    val = val.min(max_context);
                }
                settings.context_length = val;
            }
        }
    3 => {
            if let Ok(v) = buf.parse::<i32>() {
                settings.gpu_layers_mode = if v < 0 {
                    crate::models::GpuLayersMode::All
                } else {
                    crate::models::GpuLayersMode::Specific(v as u32)
                };
            }
        }
        8 => { if let Ok(v) = buf.parse::<i32>() { settings.expert_count = v.clamp(-1, 99); } }
        // Evaluation
        9 => { if let Ok(v) = buf.parse::<u32>() { settings.batch_size = v.max(1); } }
        10 => { if let Ok(v) = buf.parse::<u32>() { settings.uniform_cache = v != 0; } }
        11 => { if let Ok(v) = buf.parse::<u32>() { settings.max_concurrent_predictions = Some(v.clamp(1, 10)); } }
        // Sampling
        12 => { if let Ok(v) = buf.parse::<i32>() { settings.seed = v; } }
        13 => { if let Ok(v) = buf.parse::<i32>() { settings.temperature = (v as f32 / 100.0).clamp(0.0, 2.0); } }
        14 => { if let Ok(v) = buf.parse::<i32>() { settings.top_k = v.max(0); } }
        15 => { if let Ok(v) = buf.parse::<i32>() { settings.top_p = (v as f32 / 100.0).clamp(0.0, 1.0); } }
        16 => { if let Ok(v) = buf.parse::<i32>() { settings.min_p = (v as f32 / 100.0).clamp(0.0, 1.0); } }
        17 => { if let Ok(v) = buf.parse::<i32>() { settings.max_tokens = if v == 0 { None } else { Some(v as u32) }; } }
        18 => { if let Ok(v) = buf.parse::<i32>() { settings.repeat_penalty = (v as f32 / 100.0).clamp(0.0, 2.0); } }
        19 => { if let Ok(v) = buf.parse::<i32>() { settings.repeat_last_n = v.max(0); } }
        20 => {
            if let Ok(v) = buf.parse::<i32>() {
                settings.presence_penalty = Some((v as f32 / 100.0).clamp(0.0, 1.0));
            }
        }
        21 => {
            if let Ok(v) = buf.parse::<i32>() {
                settings.frequency_penalty = Some((v as f32 / 100.0).clamp(0.0, 1.0));
            }
        }
        _ => {}
    }
}

fn adjust_setting(settings: &mut ModelSettings, idx: usize, delta: i32, _max_threads: u32, max_context: u32, total_layers: u32) {
    match idx {
        // Loading
        1 => {
            let mut val = (settings.context_length as i32 + delta * 128).max(128) as u32;
            if max_context > 0 {
                val = val.min(max_context);
            }
            settings.context_length = val;
        }
        3 => {
            settings.gpu_layers_mode = match (delta, &settings.gpu_layers_mode) {
                (1, crate::models::GpuLayersMode::Auto) => crate::models::GpuLayersMode::Specific(1),
                (1, crate::models::GpuLayersMode::Specific(n)) => crate::models::GpuLayersMode::Specific(n + 1),
                (1, crate::models::GpuLayersMode::All) => crate::models::GpuLayersMode::Auto,
                (-1, crate::models::GpuLayersMode::Auto) => crate::models::GpuLayersMode::All,
                (-1, crate::models::GpuLayersMode::Specific(n)) if *n == 0 => crate::models::GpuLayersMode::Auto,
                (-1, crate::models::GpuLayersMode::Specific(n)) if *n == 1 => crate::models::GpuLayersMode::Specific(0),
                (-1, crate::models::GpuLayersMode::Specific(n)) => crate::models::GpuLayersMode::Specific(n - 1),
                (-1, crate::models::GpuLayersMode::All) => {
                    let n = if total_layers > 0 { total_layers.min(256) } else { 256 };
                    crate::models::GpuLayersMode::Specific(n)
                }
                _ => settings.gpu_layers_mode,
            };
        }
        4 => settings.flash_attn = !settings.flash_attn,
        5 => settings.kv_cache_offload = !settings.kv_cache_offload,
        6 => {
            let mut val = settings.cache_type_k.unwrap_or(crate::models::CacheTypeK::F16);
            val = if delta > 0 { val.next() } else { val.prev() };
            settings.cache_type_k = Some(val);
        }
        7 => {
            let mut val = settings.cache_type_v.unwrap_or(crate::models::CacheTypeV::F16);
            val = if delta > 0 { val.next() } else { val.prev() };
            settings.cache_type_v = Some(val);
        }
        8 => settings.expert_count = (settings.expert_count + delta).clamp(-1, 99),
        // Evaluation
        9 => settings.batch_size = (settings.batch_size as i32 + delta * 64).max(1) as u32,
        10 => settings.uniform_cache = !settings.uniform_cache,
        11 => {
            match settings.max_concurrent_predictions {
                Some(n) => settings.max_concurrent_predictions = Some(((n as i32) + delta).clamp(1, 10) as u32),
                None => settings.max_concurrent_predictions = Some(1),
            }
        }
        // Sampling
        12 => settings.seed = (settings.seed + delta).max(-1),
        13 => settings.temperature = ((settings.temperature * 100.0 + delta as f32 * 5.0) / 100.0).clamp(0.0, 2.0),
        14 => settings.top_k = (settings.top_k + delta).max(1),
        15 => settings.top_p = ((settings.top_p * 100.0 + delta as f32 * 5.0) / 100.0).clamp(0.0, 1.0),
        16 => settings.min_p = ((settings.min_p * 100.0 + delta as f32 * 5.0) / 100.0).clamp(0.0, 1.0),
        17 => {
            let current = settings.max_tokens.unwrap_or(2048);
            settings.max_tokens = Some((current as i32 + delta * 16).max(16) as u32);
        }
        // Repetition
        18 => settings.repeat_penalty = ((settings.repeat_penalty * 100.0 + delta as f32 * 5.0) / 100.0).clamp(1.0, 2.0),
        19 => settings.repeat_last_n += delta,
        20 => {
            let current = settings.presence_penalty.unwrap_or(0.0);
            settings.presence_penalty = Some(((current * 100.0 + delta as f32 * 5.0) / 100.0).clamp(-2.0, 2.0));
        }
        21 => {
            let current = settings.frequency_penalty.unwrap_or(0.0);
            settings.frequency_penalty = Some(((current * 100.0 + delta as f32 * 5.0) / 100.0).clamp(-2.0, 2.0));
        }
        _ => {}
    }
}

fn handle_tags_key(app: &mut App, key: crossterm::event::KeyEvent) {
    let tags = &app.settings.tags;
    let selected = app.tags_selected_idx;
    let edit_buf = &app.tags_edit_buffer;
    let insert_mode = app.tags_insert_mode;

    match key.code {
        // Close modal
        KeyCode::Esc => {
            app.tags_editing = false;
            app.tags_edit_buffer.clear();
            app.tags_selected_idx = None;
            app.tags_insert_mode = false;
            app.settings_render_cache = None;
            app.set_redraw();
        }
        // Save and close modal
        KeyCode::Enter => {
            if insert_mode && !edit_buf.is_empty() {
                // Add new tag
                let new_tag = edit_buf.trim().to_string();
                if !new_tag.is_empty() {
                    app.settings.tags.push(new_tag);
                }
                app.tags_edit_buffer.clear();
                app.tags_insert_mode = false;
                app.tags_selected_idx = None;
                app.settings_render_cache = None;
            } else if !insert_mode {
                // Edit selected tag
                if let Some(idx) = selected {
                    if !edit_buf.is_empty() {
                        let trimmed = edit_buf.trim();
                        if trimmed.is_empty() {
                            // Delete tag if edit buffer is empty
                            app.settings.tags.remove(idx);
                        } else {
                            // Update tag
                            if idx < app.settings.tags.len() {
                                app.settings.tags[idx] = trimmed.to_string();
                            }
                        }
                    }
                    app.tags_edit_buffer.clear();
                    app.tags_selected_idx = None;
                    app.tags_insert_mode = false;
                    app.settings_render_cache = None;
                } else {
                    // No tag selected, close modal
                    app.tags_editing = false;
                    app.tags_edit_buffer.clear();
                    app.tags_selected_idx = None;
                    app.tags_insert_mode = false;
                    app.settings_render_cache = None;
                }
            } else {
                // Just close modal without adding
                app.tags_editing = false;
                app.tags_edit_buffer.clear();
                app.tags_selected_idx = None;
                app.tags_insert_mode = false;
                app.settings_render_cache = None;
            }
            app.set_redraw();
        }
        // Navigate tags
        KeyCode::Up | KeyCode::Char('k') => {
            app.tags_edit_buffer.clear();
            if insert_mode {
                app.tags_insert_mode = false;
                app.tags_selected_idx = Some(tags.len().saturating_sub(1));
            } else if let Some(idx) = selected {
                app.tags_selected_idx = Some(idx.saturating_sub(1));
            } else {
                app.tags_selected_idx = Some(tags.len().saturating_sub(1));
            }
            app.set_redraw();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.tags_edit_buffer.clear();
            if insert_mode {
                app.tags_insert_mode = false;
                app.tags_selected_idx = Some(tags.len().saturating_sub(1));
            } else if let Some(idx) = selected {
                app.tags_selected_idx = Some((idx + 1).min(tags.len().saturating_sub(1)));
            } else if !tags.is_empty() {
                app.tags_selected_idx = Some(0);
            } else {
                app.tags_insert_mode = true;
            }
            app.set_redraw();
        }
        // Edit selected tag
        KeyCode::Char('e') | KeyCode::Char('i') => {
            if !insert_mode {
                if let Some(idx) = selected {
                    app.tags_edit_buffer = tags[idx].clone();
                    app.set_redraw();
                }
            }
        }
        // Delete selected tag
        KeyCode::Char('d') | KeyCode::Delete => {
            if !insert_mode {
                if let Some(idx) = selected {
                    if idx < app.settings.tags.len() {
                        app.settings.tags.remove(idx);
                        app.tags_selected_idx = None;
                        app.tags_edit_buffer.clear();
                        app.tags_insert_mode = false;
                        app.settings_render_cache = None;
                    }
                }
            }
            app.set_redraw();
        }
        // Add new tag
        KeyCode::Char('a') => {
            app.tags_insert_mode = true;
            app.tags_selected_idx = None;
            app.tags_edit_buffer.clear();
            app.set_redraw();
        }
        // Input characters for tag editing
        KeyCode::Char(c) => {
            app.tags_edit_buffer.push(c);
            app.set_redraw();
        }
        KeyCode::Backspace => {
            if !app.tags_edit_buffer.is_empty() {
                app.tags_edit_buffer.pop();
            } else if !insert_mode {
                // Move to previous tag if no edit buffer
                if let Some(idx) = selected {
                    app.tags_selected_idx = Some(idx.saturating_sub(1));
                }
            }
            app.set_redraw();
        }
        KeyCode::Tab => {
            // Toggle between insert and edit mode
            if insert_mode {
                app.tags_insert_mode = false;
                if !tags.is_empty() {
                    app.tags_selected_idx = Some(tags.len().saturating_sub(1));
                }
            } else {
                app.tags_insert_mode = true;
                app.tags_selected_idx = None;
            }
            app.settings_edit_buffer.clear();
            app.set_redraw();
        }
        _ => {}
    }
}

fn handle_settings_key(app: &mut App, key: crossterm::event::KeyEvent) {
    let idx = app.settings_selected_idx;

    // Global settings shortcuts (highest priority)
    if key.code == KeyCode::Char('s') && key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
        app.save_model_settings();
        app.set_redraw();
        return;
    }

    // Reset settings to defaults via confirmation dialog (highest priority when dirty)
    if key.code == KeyCode::Char('r') && key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
        if app.is_settings_dirty() {
            app.global_mode = GlobalMode::Confirmation {
                selected: false,
                kind: ConfirmationKind::Reset,
            };
            return;
        } else {
            app.reset_to_defaults();
            return;
        }
    }

      // Ctrl+P: open profile picker modal
    if key.code == KeyCode::Char('p') && key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
        let builtin = builtin_profiles();
        let mut all_profiles: Vec<crate::config::Profile> = builtin.to_vec();
        for p in &app.config.profiles {
            if !builtin.iter().any(|b| b.name == p.name) {
                all_profiles.push(p.clone());
            }
        }
        app.profile_picker_entries = all_profiles
            .iter()
            .map(|p| {
                let is_builtin = builtin.iter().any(|b| b.name == p.name);
                let desc = if is_builtin {
                    "built-in".to_string()
                } else {
                    p.description.clone()
                };
                (p.name.clone(), desc)
            })
            .collect();
        app.profile_picker_selected = 0;
        app.global_mode = crate::tui::app::GlobalMode::ProfilePicker {
            entries: app.profile_picker_entries.clone(),
            selected: app.profile_picker_selected,
        };
        app.set_redraw();
        return;
    }

    // Enable/Disable toggle
    if key.code == KeyCode::Char('e') && key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
        match idx {
            6 => { // cache_type_k
                app.settings.cache_type_k = if app.settings.cache_type_k.is_some() { None } else { Some(crate::models::CacheTypeK::F16) };
            }
            7 => { // cache_type_v
                app.settings.cache_type_v = if app.settings.cache_type_v.is_some() { None } else { Some(crate::models::CacheTypeV::F16) };
            }
            17 => { // max_tokens
                app.settings.max_tokens = if app.settings.max_tokens.is_some() { None } else { Some(2048) };
            }
            20 => { // presence_penalty
                app.settings.presence_penalty = if app.settings.presence_penalty.is_some() { None } else { Some(0.0) };
            }
            21 => { // frequency_penalty
                app.settings.frequency_penalty = if app.settings.frequency_penalty.is_some() { None } else { Some(0.0) };
            }
            11 => { // max_concurrent_predictions
                app.settings.max_concurrent_predictions = if app.settings.max_concurrent_predictions.is_some() { None } else { Some(1) };
            }
            _ => {}
        }
        app.update_vram_estimate();
        sync_global_settings(app);
        app.settings_render_cache = None;
        app.set_redraw();
        return;
    }

    match key.code {
        // Max Concurrent Pred: Enter opens picker modal
        _ if idx == 11 && key.code == KeyCode::Enter => {
            if app.settings_edit_buffer.is_empty() {
                let current = app.settings.max_concurrent_predictions
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "1".to_string());
                app.global_mode = GlobalMode::MaxConcurrentPicker {
                    value: current,
                };
                app.settings_render_cache = None;
                app.set_redraw();
            } else {
                app.settings_edit_buffer.clear();
                app.set_redraw();
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if !app.settings_edit_buffer.is_empty() {
                app.settings_edit_buffer.clear();
                app.set_redraw();
            } else {
                app.settings_selected_idx = app.settings_selected_idx.saturating_sub(1);
                app.set_redraw();
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if !app.settings_edit_buffer.is_empty() {
                app.settings_edit_buffer.clear();
                app.set_redraw();
            } else {
                let count = 24; // Total LLM settings (0-23)
                app.settings_selected_idx = (app.settings_selected_idx + 1).min(count - 1);
                app.set_redraw();
            }
        }        // System Prompt: open picker modal on Enter
        _ if idx == 0 => {
            if !app.settings_edit_buffer.is_empty() {
                app.settings_edit_buffer.clear();
                app.set_redraw();
            } else if key.code == KeyCode::Enter {
                app.prompt_picker_entries = app.config.system_prompt_presets
                    .iter()
                    .map(|p| (p.name.clone(), p.description.clone()))
                    .collect();
                app.prompt_picker_selected = app.prompt_picker_entries.iter()
                    .position(|(name, _)| name == &app.settings.system_prompt_preset_name)
                    .unwrap_or(0);
                app.global_mode = crate::tui::app::GlobalMode::PromptPicker {
                    entries: app.prompt_picker_entries.clone(),
                    selected: app.prompt_picker_selected,
                    editing: false,
                    edit_buffer: String::new(),
                    edit_cursor_pos: 0,
                    confirm_delete: false,
                };
                app.set_redraw();
            }
        }
        // Keep in memory (mlock): toggle on Enter
        _ if idx == 2 => {
            if !app.settings_edit_buffer.is_empty() {
                app.settings_edit_buffer.clear();
                app.set_redraw();
            } else if key.code == KeyCode::Enter {
                app.settings.mlock = !app.settings.mlock;
                app.update_vram_estimate();
                app.settings_render_cache = None;
                app.set_redraw();
            }
       }
       // GPU Layers: arrow keys cycle Auto → 1 → 2 → ... → N → All → Auto
        _ if idx == 3 => {
            if !app.settings_edit_buffer.is_empty() {
                app.settings_edit_buffer.clear();
                app.set_redraw();
            } else if key.code == KeyCode::Enter {
                // Enter from specific number: open picker; from Auto/All: cycle to max
                match &app.settings.gpu_layers_mode {
                    crate::models::GpuLayersMode::Specific(n) => {
                        app.settings_edit_buffer = n.to_string();
                    }
                    _ => {
                        let total = app.model_total_layers;
                        app.settings.gpu_layers_mode = crate::models::GpuLayersMode::Specific(total.max(1).min(256));
                        app.update_vram_estimate();
                        app.settings_render_cache = None;
                    }
                }
                app.set_redraw();
            } else if key.code == KeyCode::Left {
                let total = app.model_total_layers;
                app.settings.gpu_layers_mode = match &app.settings.gpu_layers_mode {
                    crate::models::GpuLayersMode::Auto => crate::models::GpuLayersMode::Specific(total.max(1).min(256)),
                    crate::models::GpuLayersMode::Specific(0) => crate::models::GpuLayersMode::Auto,
                    crate::models::GpuLayersMode::Specific(n) if *n == 1 => crate::models::GpuLayersMode::Specific(0),
                    crate::models::GpuLayersMode::Specific(n) => crate::models::GpuLayersMode::Specific(n - 1),
                    crate::models::GpuLayersMode::All => {
                        let max = total.max(1).min(256);
                        crate::models::GpuLayersMode::Specific(max)
                    }
                };
                app.update_vram_estimate();
                app.settings_render_cache = None;
                app.set_redraw();
            } else if key.code == KeyCode::Right {
                let total = app.model_total_layers;
                app.settings.gpu_layers_mode = match &app.settings.gpu_layers_mode {
                    crate::models::GpuLayersMode::Auto => crate::models::GpuLayersMode::Specific(1),
                    crate::models::GpuLayersMode::Specific(n) if *n == total => crate::models::GpuLayersMode::All,
                    crate::models::GpuLayersMode::Specific(n) => crate::models::GpuLayersMode::Specific(n + 1),
                    crate::models::GpuLayersMode::All => crate::models::GpuLayersMode::Auto,
                };
                app.update_vram_estimate();
                app.settings_render_cache = None;
                app.set_redraw();
            }
        }
        // Flash Attention: toggle on Enter
        _ if idx == 4 => {
            if !app.settings_edit_buffer.is_empty() {
                app.settings_edit_buffer.clear();
                app.set_redraw();
            } else if key.code == KeyCode::Enter {
                app.settings.flash_attn = !app.settings.flash_attn;
                app.update_vram_estimate();
                app.settings_render_cache = None;
                app.set_redraw();
            }
        }
        // KV Cache Offload: toggle on Enter
        _ if idx == 5 => {
            if !app.settings_edit_buffer.is_empty() {
                app.settings_edit_buffer.clear();
                app.set_redraw();
            } else if key.code == KeyCode::Enter {
                app.settings.kv_cache_offload = !app.settings.kv_cache_offload;
                app.update_vram_estimate();
                app.settings_render_cache = None;
                app.set_redraw();
            }
        }
        // Cache Type K: cycle on Enter, or apply typed number
        _ if idx == 6 => {
            if !app.settings_edit_buffer.is_empty() {
                if key.code == KeyCode::Enter {
                    if let Ok(n) = app.settings_edit_buffer.parse::<u8>() {
                        app.settings.cache_type_k = Some(crate::models::CacheTypeK::from_u8(n));
                        app.update_vram_estimate();
                    }
                    app.settings_edit_buffer.clear();
                    app.settings_render_cache = None;
                    app.set_redraw();
                } else {
                    app.settings_edit_buffer.clear();
                    app.set_redraw();
                }
            } else if key.code == KeyCode::Enter {
                let mut val = app.settings.cache_type_k.unwrap_or(crate::models::CacheTypeK::F16);
                val = val.next();
                app.settings.cache_type_k = Some(val);
                app.update_vram_estimate();
                app.settings_render_cache = None;
                app.set_redraw();
            }
        }
        // Cache Type V: cycle on Enter, or apply typed number
        _ if idx == 7 => {
            if !app.settings_edit_buffer.is_empty() {
                if key.code == KeyCode::Enter {
                    if let Ok(n) = app.settings_edit_buffer.parse::<u8>() {
                        app.settings.cache_type_v = Some(crate::models::CacheTypeV::from_u8(n));
                        app.update_vram_estimate();
                    }
                    app.settings_edit_buffer.clear();
                    app.settings_render_cache = None;
                    app.set_redraw();
                } else {
                    app.settings_edit_buffer.clear();
                    app.set_redraw();
                }
            } else if key.code == KeyCode::Enter {
                let mut val = app.settings.cache_type_v.unwrap_or(crate::models::CacheTypeV::F16);
                val = val.next();
                app.settings.cache_type_v = Some(val);
                app.update_vram_estimate();
                app.settings_render_cache = None;
                app.set_redraw();
            }
        }
        // Unified KV: toggle on Enter
        _ if idx == 10 => {
            if !app.settings_edit_buffer.is_empty() {
                app.settings_edit_buffer.clear();
                app.set_redraw();
            } else if key.code == KeyCode::Enter {
                app.settings.uniform_cache = !app.settings.uniform_cache;
                app.update_vram_estimate();
                app.settings_render_cache = None;
                app.set_redraw();
            }
        }
        // Tags: open tags modal on Enter
        _ if idx == 22 => {
            if !app.settings_edit_buffer.is_empty() {
                app.settings_edit_buffer.clear();
                app.set_redraw();
            } else if key.code == KeyCode::Enter {
                // Open tags modal
                app.tags_editing = true;
                app.tags_insert_mode = true;
                app.tags_edit_buffer = String::new();
                app.tags_selected_idx = None;
                app.settings_render_cache = None;
                app.set_redraw();
            }
        }
        // LLama.cpp Version: cycle on Enter, or open picker
        _ if idx == 23 => {
            if !app.settings_edit_buffer.is_empty() {
                app.settings_edit_buffer.clear();
                app.set_redraw();
            } else if key.code == KeyCode::Enter {
                // Open version picker for the current backend (not implemented)
            }
        }
        KeyCode::PageDown | KeyCode::Char('d') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
            if !app.settings_edit_buffer.is_empty() {
                app.settings_edit_buffer.clear();
                app.set_redraw();
            } else {
                app.settings_selected_idx = (app.settings_selected_idx + 10).min(22);
                app.set_redraw();
            }
        }
        KeyCode::PageUp | KeyCode::Char('u') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
            if !app.settings_edit_buffer.is_empty() {
                app.settings_edit_buffer.clear();
                app.set_redraw();
            } else {
                app.settings_selected_idx = app.settings_selected_idx.saturating_sub(10);
                app.set_redraw();
            }
        }
        KeyCode::PageDown => {
            app.settings_scroll_offset = app.settings_scroll_offset.saturating_add(5);
            app.settings_selected_idx = app.settings_selected_idx.saturating_add(5);
            app.set_redraw();
        }
        KeyCode::PageUp => {
            app.settings_scroll_offset = app.settings_scroll_offset.saturating_sub(5);
            app.settings_selected_idx = app.settings_selected_idx.saturating_sub(5);
            app.set_redraw();
        }
        KeyCode::Left | KeyCode::Backspace => {
            if !app.settings_edit_buffer.is_empty() {
                app.settings_edit_buffer.pop();
            } else {
                adjust_setting(&mut app.settings, idx, -1, app.max_threads, app.model_n_ctx_train, app.model_total_layers);
                if idx == 11 {
                    sync_global_settings(app);
                }
                app.update_vram_estimate();
            }
            app.settings_render_cache = None;
            app.set_redraw();
        }
        KeyCode::Right => {
            adjust_setting(&mut app.settings, idx, 1, app.max_threads, app.model_n_ctx_train, app.model_total_layers);
            if idx == 11 {
                sync_global_settings(app);
            }
            app.update_vram_estimate();
            app.settings_render_cache = None;
            app.set_redraw();
        }
        KeyCode::Char(c @ '0'..='9') => {
            app.settings_edit_buffer.push(c);
        }
        KeyCode::Char('-') => {
            app.settings_edit_buffer.push('-');
        }
        KeyCode::Char('.') => {
            if !app.settings_edit_buffer.contains('.') {
                app.settings_edit_buffer.push('.');
            }
        }
    KeyCode::Enter => {
            if !app.settings_edit_buffer.is_empty() {
                apply_numeric_setting(&mut app.settings, idx, &app.settings_edit_buffer, app.max_threads, app.model_n_ctx_train);
                if idx == 11 {
                    sync_global_settings(app);
                }
                app.settings_edit_buffer.clear();
                app.update_vram_estimate();
            }
            app.settings_render_cache = None;
            app.set_redraw();
        }
        KeyCode::Esc => {
            if !app.settings_edit_buffer.is_empty() {
                app.settings_edit_buffer.clear();
                app.update_vram_estimate();
            }
            app.set_redraw();
        }
        _ => {}
    }
}

fn handle_max_concurrent_picker_key(app: &mut App, key: crossterm::event::KeyEvent) {
    if let GlobalMode::MaxConcurrentPicker { value } = &mut app.global_mode {
        match key.code {
            KeyCode::Char(c @ '0'..='9') => {
                if value.len() < 3 {
                    value.push(c);
                }
                app.set_redraw();
            }
            KeyCode::Backspace | KeyCode::Left => {
                value.pop();
                app.set_redraw();
            }
            KeyCode::Enter => {
                if let Ok(n) = value.parse::<u32>() {
                    let n = n.clamp(1, 10);
                    app.settings.max_concurrent_predictions = Some(n);
                    sync_global_settings(app);
                    app.update_vram_estimate();
                }
                app.global_mode = GlobalMode::Normal;
                app.settings_render_cache = None;
                app.set_redraw();
            }
            KeyCode::Esc => {
                app.global_mode = GlobalMode::Normal;
                app.set_redraw();
            }
            _ => {}
        }
    }
}

fn handle_profiles_key(app: &mut App, key: crossterm::event::KeyEvent) {
    let builtin = builtin_profiles();
    
    // Build merged profile list (same as render logic)
    let mut all_profiles: Vec<crate::config::Profile> = builtin.to_vec();
    for p in &app.config.profiles {
        if !builtin.iter().any(|b| b.name == p.name) {
            all_profiles.push(p.clone());
        }
    }
    let total = all_profiles.len();

    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            app.settings_selected_idx = app.settings_selected_idx.saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if total > 0 {
                app.settings_selected_idx = (app.settings_selected_idx + 1).min(total - 1);
            }
        }
        KeyCode::PageUp => {
            app.profiles_scroll_offset = app.profiles_scroll_offset.saturating_sub(5);
        }
        KeyCode::PageDown => {
            app.profiles_scroll_offset = app.profiles_scroll_offset.saturating_add(5);
        }
        KeyCode::Enter => {
            // Use the merged list for applying profiles
            if let Some(profile) = all_profiles.get(app.settings_selected_idx) {
                let profile = profile.clone();
                app.apply_profile(&profile);
                app.active_panel = ActivePanel::LlmSettings;
            }
        }
        KeyCode::Char('s') => {
            // Save current settings as a new profile
            app.save_current_as_profile("New Profile");
            app.active_panel = ActivePanel::LlmSettings;
        }
        KeyCode::Char('d') => {
            // Delete the selected user profile (not built-in)
            if app.delete_profile(app.settings_selected_idx) {
                let new_total = app.config.merged_profiles().len();
                if new_total > 0 && app.settings_selected_idx >= new_total {
                    app.settings_selected_idx = new_total - 1;
                }
            }
        }
        KeyCode::Esc => {
            app.active_panel = ActivePanel::LlmSettings;
        }
        _ => {}
    }
}

fn handle_system_prompt_presets_key(app: &mut App, key: crossterm::event::KeyEvent) {
    // If in edit mode
    if app.editing_preset.is_some() {
        match key.code {
            KeyCode::Esc => {
                app.editing_preset = None;
            }
        KeyCode::Enter => {
            let byte_idx = app.settings_edit_buffer.char_indices().nth(app.edit_cursor_pos).map(|(i, _)| i).unwrap_or(app.settings_edit_buffer.len());
            app.settings_edit_buffer.insert(byte_idx, '\n');
            app.edit_cursor_pos += 1;
        }
        KeyCode::Char('s') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
            // Save
            if let Some(preset_idx) = app.editing_preset {
                if let Some(preset) = app.config.system_prompt_presets.get_mut(preset_idx) {
                    preset.content = app.settings_edit_buffer.clone();
                }
            }
            app.editing_preset = None;
            app.add_log("Saved preset", crate::config::LogLevel::Info);
            if let Err(e) = app.config.save() {
                app.add_log(format!("Failed to save: {}", e), crate::config::LogLevel::Error);
            }
        }
        KeyCode::Char(c) => {
            let byte_idx = app.settings_edit_buffer.char_indices().nth(app.edit_cursor_pos).map(|(i, _)| i).unwrap_or(app.settings_edit_buffer.len());
            app.settings_edit_buffer.insert(byte_idx, c);
            app.edit_cursor_pos += 1;
        }
        KeyCode::Backspace => {
            if app.edit_cursor_pos > 0 {
                app.edit_cursor_pos -= 1;
                let byte_idx = app.settings_edit_buffer.char_indices().nth(app.edit_cursor_pos).map(|(i, _)| i).unwrap_or(0);
                app.settings_edit_buffer.remove(byte_idx);
            }
        }
        KeyCode::Delete => {
            if app.edit_cursor_pos < app.settings_edit_buffer.chars().count() {
                let byte_idx = app.settings_edit_buffer.char_indices().nth(app.edit_cursor_pos).map(|(i, _)| i).unwrap_or(app.settings_edit_buffer.len());
                app.settings_edit_buffer.remove(byte_idx);
            }
        }
        KeyCode::Left => {
            app.edit_cursor_pos = app.edit_cursor_pos.saturating_sub(1);
        }
        KeyCode::Right => {
            app.edit_cursor_pos = (app.edit_cursor_pos + 1).min(app.settings_edit_buffer.chars().count());
        }
        _ => {}
    }
    return;
}

// List mode
let total = app.config.system_prompt_presets.len();
match key.code {
    KeyCode::Up | KeyCode::Char('k') => {
        app.settings_selected_idx = app.settings_selected_idx.saturating_sub(1);
    }
    KeyCode::Down | KeyCode::Char('j') => {
        if total > 0 {
            app.settings_selected_idx = (app.settings_selected_idx + 1).min(total - 1);
        }
    }
    KeyCode::PageUp => {
        app.system_prompt_presets_scroll_offset = app.system_prompt_presets_scroll_offset.saturating_sub(5);
    }
    KeyCode::PageDown => {
        app.system_prompt_presets_scroll_offset = app.system_prompt_presets_scroll_offset.saturating_add(5);
    }
    KeyCode::Enter => {
        // Apply the selected preset
        if let Some(preset) = app.config.system_prompt_presets.get(app.settings_selected_idx) {
            let name = preset.name.clone();
            app.settings.system_prompt_preset_name = name.clone();
            app.resolve_system_prompt();
            app.active_panel = ActivePanel::LlmSettings;
            app.add_log(format!("Applied preset: {}", name), crate::config::LogLevel::Info);
        }
    }
    KeyCode::Char('e') => {
        // Edit the selected preset
        if let Some(preset) = app.config.system_prompt_presets.get(app.settings_selected_idx) {
            app.settings_edit_buffer = preset.content.clone();
            app.edit_cursor_pos = app.settings_edit_buffer.chars().count();
            app.editing_preset = Some(app.settings_selected_idx);
        }
    }
    KeyCode::Char('n') => {
        // Create a new preset
        let name = format!("Custom {}", app.config.system_prompt_presets.len() + 1);
        let preset = crate::config::SystemPromptPreset {
            name: name.clone(),
            description: "User-defined preset".into(),
            content: String::new(),
        };
        app.config.system_prompt_presets.push(preset);
        // Select the new preset and enter edit mode
        app.settings_selected_idx = app.config.system_prompt_presets.len() - 1;
        app.settings_edit_buffer = String::new();
        app.edit_cursor_pos = 0;
        app.editing_preset = Some(app.settings_selected_idx);
    }
        KeyCode::Char('d') => {
            // Delete custom preset (not built-in)
            if app.settings_selected_idx >= crate::config::builtin_system_prompt_presets().len() {
                let name = if let Some(p) = app.config.system_prompt_presets.get(app.settings_selected_idx) {
                    p.name.clone()
                } else {
                    return;
                };
                app.config.system_prompt_presets.remove(app.settings_selected_idx);
                app.settings_selected_idx = app.settings_selected_idx.min(app.config.system_prompt_presets.len().saturating_sub(1));
                app.add_log(format!("Deleted preset: {}", name), crate::config::LogLevel::Info);
                if let Err(e) = app.config.save() {
                    app.add_log(format!("Failed to save: {}", e), crate::config::LogLevel::Error);
                }
            }
        }
        KeyCode::Esc => {
            app.active_panel = ActivePanel::LlmSettings;
        }
        _ => {}
    }
}

pub fn handle_mouse(app: &mut App, mouse: MouseEvent, area: Rect) {
    let pos = Position::new(mouse.column, mouse.row);

    if app.log_expanded {
        let chunks = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                ratatui::layout::Constraint::Length(1), // status bar
                ratatui::layout::Constraint::Fill(1),   // log
            ])
            .split(area);

        if chunks[1].contains(pos) {
            match mouse.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    app.active_panel = ActivePanel::Log;
                    app.set_redraw();
                }
                MouseEventKind::ScrollUp => {
                    app.log_scroll_offset = app.log_scroll_offset.saturating_sub(1);
                    app.log_follow = false;
                    app.set_redraw();
                }
                MouseEventKind::ScrollDown => {
                    app.log_scroll_offset = app.log_scroll_offset + 1;
                    if app.log_scroll_offset >= app.log_total_lines.saturating_sub(5) {
                        app.log_follow = true;
                    } else {
                        app.log_follow = false;
                    }
                    app.set_redraw();
                }
                _ => {}
            }
        }
        return;
    }

    // If actively resizing, continue updating even if mouse moved outside the border area
    if let Some(ref rs) = app.resize_state {
        match mouse.kind {
            MouseEventKind::Drag(_) => {
                let dx = pos.x as i16 - rs.start_x as i16;
                let delta = (dx * 100 / rs.container.width as i16).max(-5).min(5);
                app.left_pct = (rs.start_pct as i16 + delta).clamp(20, 80) as u16;
                app.set_redraw();
            }
            MouseEventKind::Up(MouseButton::Left) => {
                app.resize_state = None;
                app.set_redraw();
            }
            _ => {}
        }
        return;
    }

    // Default layout
    let chunks = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            ratatui::layout::Constraint::Length(1), // status bar
            ratatui::layout::Constraint::Fill(1),   // top panels
            ratatui::layout::Constraint::Length(5), // active model
            ratatui::layout::Constraint::Min(5),    // log
        ])
        .split(area);

    // 1. Check Log panel
    if chunks[3].contains(pos) {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                app.active_panel = ActivePanel::Log;
                app.set_redraw();
            }
            MouseEventKind::ScrollUp => {
                app.log_scroll_offset = app.log_scroll_offset.saturating_sub(1);
                app.log_follow = false;
                app.active_panel = ActivePanel::Log;
                app.set_redraw();
            }
            MouseEventKind::ScrollDown => {
                app.log_scroll_offset = app.log_scroll_offset + 1;
                if app.log_scroll_offset >= app.log_total_lines.saturating_sub(5) {
                    app.log_follow = true;
                } else {
                    app.log_follow = false;
                }
                app.active_panel = ActivePanel::Log;
                app.set_redraw();
            }
            _ => {}
        }
        return;
    }

    // 2. Check Top panels
    if chunks[1].contains(pos) {
        let left_pct = app.left_pct.max(20).min(80);
        let top_chunks = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([
                ratatui::layout::Constraint::Fill(left_pct),
                ratatui::layout::Constraint::Fill(100 - left_pct),
            ])
            .split(chunks[1]);

        // Check for resize drag on the vertical border between left and right panels
        let border_x = top_chunks[0].right().saturating_sub(1);
        let border_y_start = chunks[1].top();
        let border_y_end = chunks[1].bottom().saturating_sub(1);
        let on_border = (pos.x as i16 - border_x as i16).abs() <= 2
            && pos.y as i16 >= border_y_start as i16
            && pos.y as i16 <= border_y_end as i16;

        if on_border {
            match mouse.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    app.resize_state = Some(crate::tui::app::ResizeState {
                        start_x: pos.x,
                        start_pct: app.left_pct,
                        container: chunks[1],
                    });
                    app.set_redraw();
                }
                MouseEventKind::Drag(_) => {
                    if let Some(ref rs) = app.resize_state {
                        let dx = pos.x as i16 - rs.start_x as i16;
                        let delta = (dx * 100 / rs.container.width as i16).max(-5).min(5);
                        app.left_pct = (rs.start_pct as i16 + delta).clamp(20, 80) as u16;
                        app.set_redraw();
                    }
                }
                MouseEventKind::Up(MouseButton::Left) => {
                    app.resize_state = None;
                    app.set_redraw();
                }
                MouseEventKind::ScrollUp => {
                    app.left_pct = app.left_pct.saturating_sub(1).max(20);
                    app.set_redraw();
                }
                MouseEventKind::ScrollDown => {
                    app.left_pct = app.left_pct.saturating_add(1).min(80);
                    app.set_redraw();
                }
                _ => {}
            }
            return;
        }

        // Right side: Settings/Profiles
        if top_chunks[1].contains(pos) {
            match mouse.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    let server_running = app.server_handle.is_some();
                    app.active_panel = match app.active_panel {
                        ActivePanel::LlmSettings if !server_running => ActivePanel::ServerSettings,
                        ActivePanel::ServerSettings if !server_running => ActivePanel::LlmSettings,
                        ActivePanel::LlmSettings => ActivePanel::LlmSettings,
                        ActivePanel::ServerSettings => ActivePanel::LlmSettings,
                        ActivePanel::Profiles => ActivePanel::Profiles,
                        ActivePanel::SystemPromptPresets => ActivePanel::SystemPromptPresets,
                        ActivePanel::SearchReadme => ActivePanel::SearchReadme,
                        _ if !server_running => ActivePanel::ServerSettings,
                        _ => ActivePanel::LlmSettings,
                    };
                    app.set_redraw();
                }
                MouseEventKind::ScrollUp => {
                    match app.active_panel {
                        ActivePanel::LlmSettings => {
                            app.settings_scroll_offset = app.settings_scroll_offset.saturating_sub(1);
                            app.set_redraw();
                        }
                        ActivePanel::Profiles => {
                            app.profiles_scroll_offset = app.profiles_scroll_offset.saturating_sub(1);
                            app.set_redraw();
                        }
                        ActivePanel::SystemPromptPresets => {
                            app.system_prompt_presets_scroll_offset = app.system_prompt_presets_scroll_offset.saturating_sub(1);
                            app.set_redraw();
                        }
                        _ => {}
                    }
                }
                MouseEventKind::ScrollDown => {
                    match app.active_panel {
                        ActivePanel::LlmSettings => {
                            app.settings_scroll_offset = app.settings_scroll_offset.saturating_add(1);
                            app.set_redraw();
                        }
                        ActivePanel::Profiles => {
                            app.profiles_scroll_offset = app.profiles_scroll_offset.saturating_add(1);
                            app.set_redraw();
                        }
                        ActivePanel::SystemPromptPresets => {
                            app.system_prompt_presets_scroll_offset = app.system_prompt_presets_scroll_offset.saturating_add(1);
                            app.set_redraw();
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
            return;
        }

        // Left side: Models + Info
        if top_chunks[0].contains(pos) {
            let info_height = (crate::tui::panel::tabbed::get_info_lines(app, top_chunks[0].width).len() as u16 + 2).max(3);
            let left_chunks = ratatui::layout::Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .constraints([
                    ratatui::layout::Constraint::Min(5),
                    ratatui::layout::Constraint::Length(info_height),
                ])
                .split(top_chunks[0]);

            if left_chunks[0].contains(pos)
                && let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
                    app.active_panel = ActivePanel::Models;
                    app.set_redraw();
                }
        }
    }
}
