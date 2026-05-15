use crossterm::event::{KeyCode, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Position, Rect};
use tracing::debug;

use crate::backend::hub;
use crate::config::builtin_profiles;

use crate::models::{ModelSettings, SearchSort};
use crate::tui::app::{App, ActivePanel, GlobalMode, ModelsMode, LoadingPhase};

pub async fn handle_key(app: &mut App, key: crossterm::event::KeyEvent) {
    debug!("Key: {:?}", key);

    // Skip all if in delete confirmation (top priority)
    if app.global_mode == GlobalMode::DeleteConfirmation {
        match key.code {
            KeyCode::Char('y') | KeyCode::Enter => {
                if let Some(model) = app.selected_model() {
                    let path = model.path.clone();
                    let name = model.name.clone();
                    app.add_log(&format!("Queuing deletion of: {}", name));
                    app.pending_deletion = Some(path);
                }
                app.global_mode = GlobalMode::Normal;
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                app.global_mode = GlobalMode::Normal;
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

    // Handle normal mode
    match key.code {
        KeyCode::Char('c')
            if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) =>
        {
            if !app.download_progress.is_empty() {
                if let Some(idx) = app.download_scroll_state.selected() {
                    let mut cancelled_name = None;
                    if let Some(state) = app.download_progress.get_mut(idx) {
                        if let Some(token) = &state.cancel_token {
                            token.store(true, std::sync::atomic::Ordering::Relaxed);
                            state.cancelled = true;
                            cancelled_name = Some(state.filename.clone());
                        }
                    }
                    if let Some(name) = cancelled_name {
                        app.add_log(format!("Cancelling download of {}...", name));
                        app.set_redraw();
                        return;
                    }
                }
            }
            app.running = false;
            return;
        }
        KeyCode::Esc if app.log_expanded => {
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
            if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) =>
        {
            // Toggle help
            if app.global_mode == GlobalMode::Help {
                app.global_mode = GlobalMode::Normal;
            } else {
                app.global_mode = GlobalMode::Help;
            }
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
        KeyCode::Char('/') => {
            app.active_panel = ActivePanel::Models;
            app.models_mode = ModelsMode::Search {
                query: String::new(),
                results: Vec::new(),
                sort_by: SearchSort::Relevance,
                show_readme: true,
            };
            app.search_results_idx = Some(0);
            return;
        }
        KeyCode::Char('p') => {
            app.active_panel = ActivePanel::Profiles;
            return;
        }
        _ => {}
    }

    // Handle search mode first (it takes priority)
    let is_search = matches!(app.models_mode, ModelsMode::Search { .. });
    if is_search {
        // Extract model_id before the match so we can call async fn after
        let model_id = app.search_results_idx.and_then(|idx| {
            if let ModelsMode::Search { results, .. } = &app.models_mode {
                results.get(idx).map(|r| r.model_id.clone())
            } else {
                None
            }
        });

        match key.code {
            KeyCode::Esc => {
                app.models_mode = ModelsMode::List;
                app.set_redraw();
                return;
            }
            KeyCode::Enter => {
                // If README is expanded, let it handle Enter
                if app.readme_expanded {
                    return;
                }
                let query = if let ModelsMode::Search { query, .. } = &app.models_mode {
                    query.clone()
                } else {
                    return;
                };

                if query.is_empty() {
                    return;
                }

                app.add_log(&format!("Searching for '{}'...", query));
                match hub::search_models(&query, 50).await {
                    Ok(res) => {
                        if let ModelsMode::Search { results, .. } = &mut app.models_mode {
                            *results = res.clone();
                        }
                        app.search_results_idx = Some(0);
                        app.selected_model_idx = None;
                        app.add_log(&format!("Found {} models", res.len()));
                    }
                    Err(e) => {
                        app.add_log(&format!("Search failed: {}", e));
                    }
                }
                return;
            }
            KeyCode::Backspace => {
                if let ModelsMode::Search { query, .. } = &mut app.models_mode {
                    query.pop();
                }
                app.set_redraw();
                return;
            }
            KeyCode::Char('l') => {
                let model_id = if let ModelsMode::Search { results, .. } = &app.models_mode {
                    app.search_results_idx.and_then(|idx| results.get(idx).map(|r| r.model_id.clone()))
                } else {
                    None
                };

                if let Some(model_id) = model_id {
                    app.add_log(&format!("Loading files for {}...", model_id));
                    match hub::list_gguf_files(&model_id).await {
                        Ok(files) => {
                            app.add_log(&format!("Found {} GGUF files", files.len()));
                            // Now clone only when we know the operation succeeded
                            if let ModelsMode::Search { query, results, .. } = &app.models_mode {
                                let selected_result = app.search_results_idx.and_then(|idx| results.get(idx).cloned());
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
                            app.add_log(&format!("No GGUF files: {}", e));
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
                        SearchSort::Relevance => a.downloads.cmp(&b.downloads),
                    });
                }
                app.set_redraw();
                return;
            }
            KeyCode::Char('R') => {
                // When README is shown, expand to fullscreen.
                // When not shown, fetch and display it.
                let model_id = if let ModelsMode::Search { results, show_readme, .. } = &app.models_mode {
                    if *show_readme {
                        // README is shown — expand to fullscreen
                        app.readme_expanded = true;
                        app.set_redraw();
                        return;
                    }
                    app.search_results_idx.and_then(|idx| results.get(idx).map(|r| r.model_id.clone()))
                } else {
                    None
                };
                if let Some(model_id) = model_id {
                    app.add_log(&format!("Fetching README for {}...", model_id));
                    app.add_log("This may take a moment...");
                    // Spawn a task to fetch the README without blocking the UI
                    let handle = tokio::spawn(async move {
                        hub::fetch_readme(&model_id).await
                    });
                    match handle.await {
                        Ok(Ok(readme)) => {
                            if let ModelsMode::Search { results, .. } = &mut app.models_mode {
                                if let Some(idx) = app.search_results_idx {
                                    if let Some(r) = results.get_mut(idx) {
                                        r.readme = Some(readme);
                                    }
                                }
                            }
                            app.add_log("README loaded.");
                        }
                        Ok(Err(e)) => {
                            app.add_log(&format!("Failed to fetch README: {}", e));
                        }
                        Err(e) => {
                            app.add_log(&format!("Task failed: {}", e));
                        }
                    }
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
            KeyCode::Down => {
                let len = if let ModelsMode::Search { results, .. } = &app.models_mode { results.len() } else { 0 };
                match app.search_results_idx {
                    Some(idx) if idx + 1 < len => app.search_results_idx = Some(idx + 1),
                    None if len > 0 => app.search_results_idx = Some(0),
                    _ => {}
                }
                app.set_redraw();
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

    // Handle files mode
    let is_files = matches!(app.models_mode, ModelsMode::Files { .. });
    if is_files {
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
                        app.add_log("Download already in progress");
                        return;
                    }
                    // Check if file already exists locally
                    let models_dir = app.config.models_dir.clone();
                    let file_path = models_dir.join(&filename);
                    if file_path.exists() {
                        app.add_log("File already downloaded");
                        return;
                    }
                    app.add_log(&format!("Downloading {}...", filename));
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

    // Skip normal key handling when help is showing
    if app.global_mode == GlobalMode::Help {
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
        ActivePanel::Downloads => handle_downloads_key(app, key),
        ActivePanel::ServerSettings => { /* handled above */ }
        ActivePanel::LlmSettings => handle_settings_key(app, key),
        ActivePanel::Profiles => handle_profiles_key(app, key),
        ActivePanel::SystemPromptPresets => handle_system_prompt_presets_key(app, key),
        ActivePanel::SearchReadme => handle_readme_key(app, key),
    }
}

fn handle_downloads_key(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            let i = match app.download_scroll_state.selected() {
                Some(i) => {
                    if i == 0 {
                        app.download_progress.len() - 1
                    } else {
                        i - 1
                    }
                }
                None => 0,
            };
            app.download_scroll_state.select(Some(i));
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let i = match app.download_scroll_state.selected() {
                Some(i) => {
                    if i >= app.download_progress.len() - 1 {
                        0
                    } else {
                        i + 1
                    }
                }
                None => 0,
            };
            app.download_scroll_state.select(Some(i));
        }
        KeyCode::Char('c') => {
            if let Some(idx) = app.download_scroll_state.selected() {
                let mut cancelled_name = None;
                if let Some(state) = app.download_progress.get_mut(idx) {
                    if let Some(token) = &state.cancel_token {
                        token.store(true, std::sync::atomic::Ordering::Relaxed);
                        state.cancelled = true;
                        cancelled_name = Some(state.filename.clone());
                    }
                }
                if let Some(name) = cancelled_name {
                    app.add_log(format!("Cancelling download of {}...", name));
                }
            }
        }
        _ => {}
    }
    app.set_redraw();
}

async fn fetch_readme_for_selected(app: &mut App, model_id: String) {
    if let ModelsMode::Search { results, show_readme, .. } = &app.models_mode {
        if *show_readme {
            if let Some(idx) = app.search_results_idx {
                if let Some(r) = results.get(idx) {
                    if r.readme.is_none() {
                        app.add_log(&format!("Fetching README for {}...", model_id));
                        let handle = tokio::spawn(async move {
                            hub::fetch_readme(&model_id).await
                        });
                        match handle.await {
                            Ok(Ok(readme)) => {
                                if let ModelsMode::Search { results, .. } = &mut app.models_mode {
                                    if let Some(r) = results.get_mut(idx) {
                                        r.readme = Some(readme);
                                    }
                                }
                                app.add_log("README loaded.");
                            }
                            Ok(Err(e)) => {
                                app.add_log(&format!("Failed to fetch README: {}", e));
                            }
                            Err(e) => {
                                app.add_log(&format!("Task failed: {}", e));
                            }
                        }
                    }
                }
            }
        }
    }
}

fn handle_readme_key(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Enter if !app.readme_expanded => {
            app.readme_expanded = true;
            app.set_redraw();
        }
        KeyCode::Esc if app.readme_expanded => {
            app.readme_expanded = false;
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
        KeyCode::Char('h') => {
            app.readme_scroll_offset_x = app.readme_scroll_offset_x.saturating_sub(1);
            app.set_redraw();
        }
        KeyCode::Char('l') => {
            app.readme_scroll_offset_x = app.readme_scroll_offset_x.saturating_add(1);
            app.set_redraw();
        }
        _ => {}
    }
}

async fn handle_models_key(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            match app.selected_model_idx {
                Some(idx) if idx > 0 => {
                    app.selected_model_idx = Some(idx - 1);
                    app.on_model_selection_change();
                }
                None if !app.models.is_empty() => {
                    app.selected_model_idx = Some(0);
                    app.on_model_selection_change();
                }
                _ => {}
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            match app.selected_model_idx {
                Some(idx) if idx + 1 < app.models.len() => {
                    app.selected_model_idx = Some(idx + 1);
                    app.on_model_selection_change();
                }
                None if !app.models.is_empty() => {
                    app.selected_model_idx = Some(0);
                    app.on_model_selection_change();
                }
                _ => {}
            }
        }
        KeyCode::Enter | KeyCode::Char('l') => {
            if let Some(idx) = app.selected_model_idx {
                let model = app.models[idx].clone();
                let already_loaded = matches!(
                    app.model_states.get(&model.display_name),
                    Some(crate::models::ModelState::Loaded { .. })
                );
                if already_loaded {
                    app.add_log(&format!("{} is already loaded", model.display_name));
                } else {
                    app.update_model_metadata();
                    let settings = app.selected_model_settings();
                    
                    if let Some(handle) = &app.server_handle {
                        if !crate::backend::server::check_health(&handle.host, handle.port).await {
                            app.add_log("Router unresponsive, restarting...");
                            if let Some(h) = app.server_handle.take() {
                                app.pending_kill = Some(h);
                            }
                        }
                    }

                    if app.server_handle.is_none() {
                        // Start server WITH the specific model directly
                        app.last_error_message = None;
                        app.pending_spawn = Some((Some(model.clone()), settings));
                        // No pending_api_load here because it's already in the CLI command
                        app.loading_phases = vec![LoadingPhase::ServerStarting];
                        app.loading_progress = 0.25;
                        app.add_log(&format!("Starting server with {}...", model.display_name));
                    } else {
                        // Router already running, load via API
                        app.last_error_message = None;
                        app.pending_api_load = Some((model.display_name.clone(), Some(model.path.to_string_lossy().to_string())));
                        app.loading_phases = vec![LoadingPhase::LoadingModel];
                        app.loading_progress = 0.5;
                        app.add_log(&format!("Loading {} via API...", model.display_name));
                    }
                }
            }
        }
        KeyCode::Char('u') => {
            if let Some(idx) = app.selected_model_idx {
                let model = app.models[idx].clone();
                if let Some(crate::models::ModelState::Loaded { .. }) = app.model_states.get(&model.display_name) {
                    app.add_log(&format!("Unloading {} via API...", model.display_name));
                    app.pending_api_unload = Some((model.display_name.clone(), Some(model.path.to_string_lossy().to_string())));
                } else {
                    app.add_log(&format!("{} is not loaded", model.display_name));
                }
            } else if app.server_handle.is_some() {
                app.add_log("Select a loaded model to unload");
            } else {
                app.add_log("No model is currently loaded");
            }
        }        KeyCode::Char('d') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
            if app.selected_model().is_some() {
                app.global_mode = GlobalMode::DeleteConfirmation;
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
        KeyCode::Char('g') => {
            app.log_scroll_offset = 0;
            app.set_redraw();
        }
        KeyCode::Char('G') => {
            app.log_scroll_offset = app.log_entries.len().saturating_sub(1) as u16;
            app.set_redraw();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.log_scroll_offset = app.log_scroll_offset.saturating_sub(1);
            app.set_redraw();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.log_scroll_offset = (app.log_scroll_offset + 1).min(app.log_entries.len().saturating_sub(1) as u16);
            app.set_redraw();
        }
        _ => {}
    }
}

fn handle_server_settings_key(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            app.server_settings_selected_idx = app.server_settings_selected_idx.saturating_sub(1);
            app.set_redraw();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.server_settings_selected_idx = (app.server_settings_selected_idx + 1).min(2);
            app.set_redraw();
        }
        KeyCode::Enter => {
            if app.server_settings_selected_idx == 0 {
                // Toggle host
                app.settings.host = if app.settings.host == "127.0.0.1" {
                    "0.0.0.0".to_string()
                } else {
                    "127.0.0.1".to_string()
                };
            } else if app.server_settings_selected_idx == 1 {
                // Cycle backend
                app.settings.backend = match app.settings.backend {
                    crate::models::Backend::Cpu => crate::models::Backend::Vulkan,
                    crate::models::Backend::Vulkan => crate::models::Backend::Cpu,
                };
                app.update_vram_estimate();
            } else {
                // Cycle parallel (1-10)
                app.settings.parallel = (app.settings.parallel % 10) + 1;
                app.update_vram_estimate();
            }
            app.set_redraw();
        }
        KeyCode::Left | KeyCode::Char('h') if app.server_settings_selected_idx == 2 => {
            app.settings.parallel = app.settings.parallel.saturating_sub(1).max(1);
            app.update_vram_estimate();
            app.set_redraw();
        }
        KeyCode::Right | KeyCode::Char('l') if app.server_settings_selected_idx == 2 => {
            app.settings.parallel = (app.settings.parallel + 1).min(10);
            app.update_vram_estimate();
            app.set_redraw();
        }
        _ => {}
    }
}

// Settings field indices for navigation and editing
// Loading: 0: Context, 1: Threads, 2: Threads Batch, 3: Prompt (string), 4: Reasoning Mode
// GPU: 5: GPU Layers, 6: Flash Attention, 7: KV Cache Offload, 8: Cache Type K, 9: Cache Type V
// Evaluation: 10: Eval Batch, 11: Max Predictions (removed), 11: Unified KV
// Sampling: 12: Seed, 13: Temp, 14: Top-k, 15: Top-p, 16: Min P, 17: Max Tokens
// Repetition: 18: Rep. Penalty, 19: Rep. Last N, 20: Presence, 21: Frequency
// Total: 22 fields

fn apply_numeric_setting(settings: &mut ModelSettings, idx: usize, buf: &str, max_threads: u32, max_context: u32) {
    match idx {
        // Loading
        0 => {
            if let Ok(v) = buf.parse::<u32>() {
                let mut val = v.max(128);
                if max_context > 0 {
                    val = val.min(max_context);
                }
                settings.context_length = val;
            }
        }
        1 => { if let Ok(v) = buf.parse::<u32>() { settings.threads = v.max(1).min(max_threads); } }
        2 => { if let Ok(v) = buf.parse::<u32>() { settings.threads_batch = v.max(1); } }
        // GPU Offload
        5 => { if let Ok(v) = buf.parse::<i32>() { settings.gpu_layers = v.clamp(0, 999); } }
        // Evaluation
        10 => { if let Ok(v) = buf.parse::<u32>() { settings.batch_size = v.max(1); } }
        // Sampling
        12 => { if let Ok(v) = buf.parse::<i32>() { settings.seed = v.max(-1); } }
        13 => { if let Ok(v) = buf.parse::<f32>() { settings.temperature = v.clamp(0.0, 2.0); } }
        14 => { if let Ok(v) = buf.parse::<i32>() { settings.top_k = v.max(1); } }
        15 => { if let Ok(v) = buf.parse::<f32>() { settings.top_p = v.clamp(0.0, 1.0); } }
        16 => { if let Ok(v) = buf.parse::<f32>() { settings.min_p = v.clamp(0.0, 1.0); } }
        17 => { if let Ok(v) = buf.parse::<u32>() { settings.max_tokens = v.max(16); } }
        // Repetition
        18 => { if let Ok(v) = buf.parse::<f32>() { settings.repeat_penalty = v.clamp(1.0, 2.0); } }
        19 => { if let Ok(v) = buf.parse::<i32>() { settings.repeat_last_n = v; } }
        20 => { if let Ok(v) = buf.parse::<f32>() { settings.presence_penalty = v.clamp(-2.0, 2.0); } }
        21 => { if let Ok(v) = buf.parse::<f32>() { settings.frequency_penalty = v.clamp(-2.0, 2.0); } }
        _ => {}
    }
}

fn adjust_setting(settings: &mut ModelSettings, idx: usize, delta: i32, max_threads: u32, max_context: u32) {
    match idx {
        // Loading
        0 => {
            let mut val = (settings.context_length as i32 + delta * 128).max(128) as u32;
            if max_context > 0 {
                val = val.min(max_context);
            }
            settings.context_length = val;
        }
        1 => settings.threads = (settings.threads as i32 + delta).clamp(1, max_threads as i32) as u32,
        2 => settings.threads_batch = (settings.threads_batch as i32 + delta).max(1) as u32,
        // GPU Offload
        5 => settings.gpu_layers = (settings.gpu_layers + delta).max(0),
        6 => settings.flash_attn = !settings.flash_attn,
        7 => settings.kv_cache_offload = !settings.kv_cache_offload,
        8 => settings.cache_type_k = if delta > 0 { settings.cache_type_k.next() } else { settings.cache_type_k.prev() },
        9 => settings.cache_type_v = if delta > 0 { settings.cache_type_v.next() } else { settings.cache_type_v.prev() },
        // Evaluation
        10 => settings.batch_size = (settings.batch_size as i32 + delta * 64).max(1) as u32,
        11 => settings.uniform_cache = !settings.uniform_cache,
        // Sampling
        12 => settings.seed = (settings.seed + delta).max(-1),
        13 => settings.temperature = ((settings.temperature * 100.0 + delta as f32 * 5.0) / 100.0).clamp(0.0, 2.0),
        14 => settings.top_k = (settings.top_k + delta).max(1),
        15 => settings.top_p = ((settings.top_p * 100.0 + delta as f32 * 5.0) / 100.0).clamp(0.0, 1.0),
        16 => settings.min_p = ((settings.min_p * 100.0 + delta as f32 * 5.0) / 100.0).clamp(0.0, 1.0),
        17 => settings.max_tokens = (settings.max_tokens as i32 + delta * 16).max(16) as u32,
        // Repetition
        18 => settings.repeat_penalty = ((settings.repeat_penalty * 100.0 + delta as f32 * 5.0) / 100.0).clamp(1.0, 2.0),
        19 => settings.repeat_last_n += delta,
        20 => settings.presence_penalty = ((settings.presence_penalty * 100.0 + delta as f32 * 5.0) / 100.0).clamp(-2.0, 2.0),
        21 => settings.frequency_penalty = ((settings.frequency_penalty * 100.0 + delta as f32 * 5.0) / 100.0).clamp(-2.0, 2.0),
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

    match key.code {
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
                let count = 22; // Total editable LLM settings
                app.settings_selected_idx = (app.settings_selected_idx + 1).min(count - 1);
                app.set_redraw();
            }
        }
        // System Prompt: open presets panel on Enter
        _ if idx == 3 => {
            if !app.settings_edit_buffer.is_empty() {
                app.settings_edit_buffer.clear();
                app.set_redraw();
            } else if key.code == KeyCode::Enter {
                app.active_panel = ActivePanel::SystemPromptPresets;
                app.set_redraw();
            }
        }
        // Reasoning mode: cycle on Enter
        _ if idx == 4 => {
            if !app.settings_edit_buffer.is_empty() {
                app.settings_edit_buffer.clear();
                app.set_redraw();
            } else if key.code == KeyCode::Enter {
                app.settings.reasoning_mode = match app.settings.reasoning_mode {
                    crate::models::ReasoningMode::Default => crate::models::ReasoningMode::Gemma,
                    crate::models::ReasoningMode::Gemma => crate::models::ReasoningMode::Default,
                };
            }
        }
        // GPU Layers: interactive mode for typing layer count
        _ if idx == 5 => {
            if app.settings_edit_buffer.is_empty() {
                app.settings_edit_buffer = app.settings.gpu_layers.to_string();
                app.set_redraw();
            }
        }
        // Flash Attention: toggle on Enter
        _ if idx == 6 => {
            if !app.settings_edit_buffer.is_empty() {
                app.settings_edit_buffer.clear();
                app.set_redraw();
            } else if key.code == KeyCode::Enter {
                app.settings.flash_attn = !app.settings.flash_attn;
                app.update_vram_estimate();
                app.set_redraw();
            }
        }
        // KV Cache Offload: toggle on Enter
        _ if idx == 7 => {
            if !app.settings_edit_buffer.is_empty() {
                app.settings_edit_buffer.clear();
                app.set_redraw();
            } else if key.code == KeyCode::Enter {
                app.settings.kv_cache_offload = !app.settings.kv_cache_offload;
                app.update_vram_estimate();
                app.set_redraw();
            }
        }
        // Cache Type K: cycle on Enter
        _ if idx == 8 => {
            if !app.settings_edit_buffer.is_empty() {
                app.settings_edit_buffer.clear();
                app.set_redraw();
            } else if key.code == KeyCode::Enter {
                app.settings.cache_type_k = app.settings.cache_type_k.next();
                app.update_vram_estimate();
                app.set_redraw();
            }
        }
        // Cache Type V: cycle on Enter
        _ if idx == 9 => {
            if !app.settings_edit_buffer.is_empty() {
                app.settings_edit_buffer.clear();
                app.set_redraw();
            } else if key.code == KeyCode::Enter {
                app.settings.cache_type_v = app.settings.cache_type_v.next();
                app.update_vram_estimate();
                app.set_redraw();
            }
        }
        // Unified KV: toggle on Enter
        _ if idx == 12 => {
            if !app.settings_edit_buffer.is_empty() {
                app.settings_edit_buffer.clear();
                app.set_redraw();
            } else if key.code == KeyCode::Enter {
                app.settings.uniform_cache = !app.settings.uniform_cache;
                app.update_vram_estimate();
                app.set_redraw();
            }
        }
        KeyCode::PageDown | KeyCode::Char('d') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
            if !app.settings_edit_buffer.is_empty() {
                app.settings_edit_buffer.clear();
                app.set_redraw();
            } else {
                app.settings_selected_idx = (app.settings_selected_idx + 10).min(20);
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
            app.set_redraw();
        }
        KeyCode::PageUp => {
            app.settings_scroll_offset = app.settings_scroll_offset.saturating_sub(5);
            app.set_redraw();
        }
        KeyCode::Left | KeyCode::Backspace => {
            if !app.settings_edit_buffer.is_empty() {
                app.settings_edit_buffer.pop();
            } else {
                adjust_setting(&mut app.settings, idx, -1, app.max_threads, app.model_n_ctx_train);
                app.update_vram_estimate();
            }
            app.set_redraw();
        }
        KeyCode::Right => {
            adjust_setting(&mut app.settings, idx, 1, app.max_threads, app.model_n_ctx_train);
            app.update_vram_estimate();
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
                if idx == 5 {
                    // GPU Layers: parse as layer count
                    if let Ok(v) = app.settings_edit_buffer.parse::<i32>() {
                        app.settings.gpu_layers = v.clamp(0, 999);
                    }
                } else {
                    apply_numeric_setting(&mut app.settings, idx, &app.settings_edit_buffer, app.max_threads, app.model_n_ctx_train);
                }
                app.settings_edit_buffer.clear();
                app.update_vram_estimate();
            }
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

fn handle_profiles_key(app: &mut App, key: crossterm::event::KeyEvent) {
    let builtin = builtin_profiles();
    
    // Build merged profile list (same as render logic)
    let mut all_profiles: Vec<crate::config::Profile> = builtin.iter().cloned().collect();
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
                // Recalculate total after deletion
                let mut new_all: Vec<crate::config::Profile> = builtin.iter().cloned().collect();
                for p in &app.config.profiles {
                    if !builtin.iter().any(|b| b.name == p.name) {
                        new_all.push(p.clone());
                    }
                }
                let new_total = new_all.len();
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
                app.edit_cursor_pos += 1;
                app.settings_edit_buffer.insert(app.edit_cursor_pos - 1, '\n');
            }
            KeyCode::Char('s') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                // Save
                if let Some(preset_idx) = app.editing_preset {
                    if let Some(preset) = app.config.system_prompt_presets.get_mut(preset_idx) {
                        preset.content = app.settings_edit_buffer.clone();
                    }
                }
                app.editing_preset = None;
                app.add_log("Saved preset");
                if let Err(e) = app.config.save() {
                    app.add_log(&format!("Failed to save: {}", e));
                }
            }
            KeyCode::Char(c) => {
                app.settings_edit_buffer.insert(app.edit_cursor_pos, c);
                app.edit_cursor_pos += 1;
            }
            KeyCode::Backspace => {
                if app.edit_cursor_pos > 0 {
                    app.edit_cursor_pos -= 1;
                    app.settings_edit_buffer.remove(app.edit_cursor_pos);
                }
            }
            KeyCode::Left => {
                app.edit_cursor_pos = app.edit_cursor_pos.saturating_sub(1);
            }
            KeyCode::Right => {
                app.edit_cursor_pos = app.edit_cursor_pos.min(app.settings_edit_buffer.len());
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
            app.settings_selected_idx = (app.settings_selected_idx + 1).min(total - 1);
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
                app.add_log(&format!("Applied preset: {}", name));
            }
        }
        KeyCode::Char('e') => {
            // Edit the selected preset
            app.editing_preset = Some(app.settings_selected_idx);
            app.edit_cursor_pos = 0;
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
            app.editing_preset = Some(app.settings_selected_idx);
            app.edit_cursor_pos = 0;
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
                app.add_log(&format!("Deleted preset: {}", name));
                if let Err(e) = app.config.save() {
                    app.add_log(&format!("Failed to save: {}", e));
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
                    app.set_redraw();
                }
                MouseEventKind::ScrollDown => {
                    let max_offset = app.log_entries.len().saturating_sub(1) as u16;
                    app.log_scroll_offset = (app.log_scroll_offset + 1).min(max_offset);
                    app.set_redraw();
                }
                _ => {}
            }
        }
        return;
    }

    if app.readme_expanded {
        let chunks = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                ratatui::layout::Constraint::Length(1), // status bar
                ratatui::layout::Constraint::Fill(1),   // README
            ])
            .split(area);

        if chunks[1].contains(pos) {
            match mouse.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    app.active_panel = ActivePanel::SearchReadme;
                    app.set_redraw();
                }
                MouseEventKind::ScrollUp => {
                    app.readme_scroll_offset = app.readme_scroll_offset.saturating_sub(1);
                    app.set_redraw();
                }
                MouseEventKind::ScrollDown => {
                    app.readme_scroll_offset = app.readme_scroll_offset.saturating_add(1);
                    app.set_redraw();
                }
                _ => {}
            }
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
                app.active_panel = ActivePanel::Log;
                app.set_redraw();
            }
            MouseEventKind::ScrollDown => {
                let max_offset = app.log_entries.len().saturating_sub(1) as u16;
                app.log_scroll_offset = (app.log_scroll_offset + 1).min(max_offset);
                app.active_panel = ActivePanel::Log;
                app.set_redraw();
            }
            _ => {}
        }
        return;
    }

    // 2. Check Top panels
    if chunks[1].contains(pos) {
        let top_chunks = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([
                ratatui::layout::Constraint::Percentage(66),
                ratatui::layout::Constraint::Fill(1),
            ])
            .split(chunks[1]);

        // Right side: Settings/Profiles
        if top_chunks[1].contains(pos) {
            match mouse.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    app.active_panel = match app.active_panel {
                        ActivePanel::LlmSettings => ActivePanel::ServerSettings,
                        ActivePanel::ServerSettings => ActivePanel::LlmSettings,
                        ActivePanel::Profiles => ActivePanel::Profiles,
                        ActivePanel::SystemPromptPresets => ActivePanel::SystemPromptPresets,
                        ActivePanel::SearchReadme => ActivePanel::SearchReadme,
                        _ => ActivePanel::ServerSettings,
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

            if left_chunks[0].contains(pos) {
                if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
                    app.active_panel = ActivePanel::Models;
                    app.set_redraw();
                }
            }
        }
    }
}
