use crossterm::event::{KeyCode, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Position, Rect};
use ratatui::widgets::TableState;
use tracing::debug;

use crate::backend::hub;
use crate::config::builtin_profiles;

use crate::models::{ModelSettings, SearchSort};
use crate::tui::app::{App, ActivePanel, GlobalMode, ModelsMode, LoadingPhase, ConfirmationKind};

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
                }
                app.global_mode = GlobalMode::Normal;
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                app.pending_deletion = None;
                app.pending_api_unload = None;
                app.global_mode = GlobalMode::Normal;
            }
            KeyCode::Enter => {
                if *selected {
                    // Confirmed (Yes)
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
                    }
                } else {
                    // Cancelled (No)
                    app.pending_deletion = None;
                    app.pending_api_unload = None;
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
                app.global_mode = GlobalMode::Normal;
            }
            _ => {}
        }
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
            KeyCode::Esc | KeyCode::Char('h')
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
            if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) && !key.modifiers.contains(crossterm::event::KeyModifiers::SHIFT) =>
        {
            // Toggle panel help
            if app.panel_help {
                app.panel_help = false;
            } else {
                app.panel_help = true;
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
        KeyCode::Char('p') => {
            app.active_panel = ActivePanel::Profiles;
            return;
        }
        _ => {}
    }

    // Handle search mode first (it takes priority)
    let is_search = matches!(app.models_mode, ModelsMode::Search { .. });
    if is_search && app.active_panel == ActivePanel::Models {
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
                        SearchSort::Relevance => a.downloads.cmp(&b.downloads),
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
                    // Spawn a task to fetch the README without blocking the UI
                    let handle = tokio::spawn(async move {
                        hub::fetch_readme(&model_id).await
                    });
                    match handle.await {
                        Ok(Ok(readme)) => {
                            if let ModelsMode::Search { results, .. } = &mut app.models_mode
                                && let Some(idx) = app.search_results_idx
                                    && let Some(r) = results.get_mut(idx) {
                                        r.readme = Some(readme);
                                    }
                            app.add_log("README loaded.", crate::config::LogLevel::Info);
                        }
                        Ok(Err(e)) => {
                            app.add_log(format!("Failed to fetch README: {}", e), crate::config::LogLevel::Error);
                            if let ModelsMode::Search { results, .. } = &mut app.models_mode
                                && let Some(idx) = app.search_results_idx
                                    && let Some(r) = results.get_mut(idx) {
                                        r.readme = Some(String::new());
                                    }
                        }
                        Err(e) => {
                            app.add_log(format!("Task failed: {}", e), crate::config::LogLevel::Error);
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

  

    // Skip normal key handling when panel help is showing
    if app.panel_help {
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
        ActivePanel::Downloads => handle_downloads_key(app, key),
        ActivePanel::ServerSettings => { /* handled above */ }
        ActivePanel::LlmSettings => handle_settings_key(app, key),
        ActivePanel::Profiles => handle_profiles_key(app, key),
        ActivePanel::SystemPromptPresets => handle_system_prompt_presets_key(app, key),
       ActivePanel::SearchReadme => handle_readme_key(app, key),
        ActivePanel::ActiveModel => {}
        ActivePanel::ModelInfo => {}
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
                if let Some(state) = app.download_progress.get_mut(idx)
                    && let Some(token) = &state.cancel_token {
                        token.store(true, std::sync::atomic::Ordering::Relaxed);
                        state.cancelled = true;
                        cancelled_name = Some(state.filename.clone());
                    }
                if let Some(name) = cancelled_name {
                    app.add_log(format!("Cancelling download of {}...", name), crate::config::LogLevel::Info);
                }
            }
        }
        _ => {}
    }
    app.set_redraw();
}

async fn fetch_readme_for_selected(app: &mut App, model_id: String) {
    if let ModelsMode::Search { results, show_readme, .. } = &app.models_mode
        && *show_readme
            && let Some(idx) = app.search_results_idx
                && let Some(r) = results.get(idx)
                    && r.readme.is_none() {
                        app.add_log(format!("Fetching README for {}...", model_id), crate::config::LogLevel::Info);
                        let handle = tokio::spawn(async move {
                            hub::fetch_readme(&model_id).await
                        });
                        match handle.await {
                            Ok(Ok(readme)) => {
                                if let ModelsMode::Search { results, .. } = &mut app.models_mode
                                    && let Some(r) = results.get_mut(idx) {
                                        r.readme = Some(readme);
                                    }
                                app.add_log("README loaded.", crate::config::LogLevel::Info);
                            }
                            Ok(Err(e)) => {
                                app.add_log(format!("Failed to fetch README: {}", e), crate::config::LogLevel::Error);
                            }
                            Err(e) => {
                                app.add_log(format!("Task failed: {}", e), crate::config::LogLevel::Error);
                            }
                        }
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
                        
                        if app.server_mode == crate::models::ServerMode::Router {
                            // Router mode: start server without a model, then load via /load API
                            app.pending_spawn = Some((None, settings.clone()));
                            // Queue the load so it triggers once server is ready
                            app.pending_api_load = Some((model.display_name.clone(), Some(model.path.to_string_lossy().to_string())));
                            app.loading_phases = vec![LoadingPhase::ServerStarting];
                            app.loading_progress = 0.25;
                            app.add_log(format!("Starting router server..."), crate::config::LogLevel::Info);
                        } else {
                            // Normal mode: start server WITH the specific model directly
                            app.pending_spawn = Some((Some(model.clone()), settings));
                            app.loading_phases = vec![LoadingPhase::ServerStarting];
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
                        app.loading_phases = vec![LoadingPhase::LoadingModel];
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
        || app.config.default.router_max_models != app.router_max_models;
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
    let _ = app.config.save();
}

fn handle_server_settings_key(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            app.server_settings_selected_idx = app.server_settings_selected_idx.saturating_sub(1);
            app.set_redraw();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.server_settings_selected_idx = (app.server_settings_selected_idx + 1).min(5);
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
                    // Cycle backend
                    app.settings.backend = match app.settings.backend {
                        crate::models::Backend::Cpu => crate::models::Backend::Vulkan,
                        crate::models::Backend::Vulkan => crate::models::Backend::Rocrm,
                        crate::models::Backend::Rocrm => crate::models::Backend::Cpu,
                    };
                    app.update_vram_estimate();
                    app.settings_render_cache = None;
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
                        crate::models::ServerMode::Router => crate::models::ServerMode::Normal,
                    };
                }
                5 => {
                    // Toggle API endpoint (disabled while server is running)
                    if app.server_handle.is_none() {
                        app.settings.api_endpoint_enabled = !app.settings.api_endpoint_enabled;
                    }
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
        0 => {
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

fn adjust_setting(settings: &mut ModelSettings, idx: usize, delta: i32, _max_threads: u32, max_context: u32) {
    match idx {
        // Loading
        0 => {
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
                (-1, crate::models::GpuLayersMode::All) => crate::models::GpuLayersMode::Specific(999),
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
 let count = 22; // Total LLM settings
                app.settings_selected_idx = (app.settings_selected_idx + 1).min(count - 1);
                app.set_redraw();
            }
        }
        // System Prompt: open presets panel on Enter
        _ if idx == 1 => {
            if !app.settings_edit_buffer.is_empty() {
                app.settings_edit_buffer.clear();
                app.set_redraw();
            } else if key.code == KeyCode::Enter {
                app.active_panel = ActivePanel::SystemPromptPresets;
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
        KeyCode::PageDown | KeyCode::Char('d') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
            if !app.settings_edit_buffer.is_empty() {
                app.settings_edit_buffer.clear();
                app.set_redraw();
            } else {
                app.settings_selected_idx = (app.settings_selected_idx + 10).min(21);
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
                adjust_setting(&mut app.settings, idx, -1, app.max_threads, app.model_n_ctx_train);
                if idx == 11 {
                    sync_global_settings(app);
                }
                app.update_vram_estimate();
            }
            app.settings_render_cache = None;
            app.set_redraw();
        }
        KeyCode::Right => {
            adjust_setting(&mut app.settings, idx, 1, app.max_threads, app.model_n_ctx_train);
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
                // Recalculate total after deletion
                let mut new_all: Vec<crate::config::Profile> = builtin.to_vec();
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
                if let Some(preset_idx) = app.editing_preset
                    && let Some(preset) = app.config.system_prompt_presets.get_mut(preset_idx) {
                        preset.content = app.settings_edit_buffer.clone();
                    }
                app.editing_preset = None;
                app.add_log("Saved preset", crate::config::LogLevel::Info);
                if let Err(e) = app.config.save() {
                    app.add_log(format!("Failed to save: {}", e), crate::config::LogLevel::Error);
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
                app.add_log(format!("Applied preset: {}", name), crate::config::LogLevel::Info);
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
