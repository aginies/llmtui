use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::widgets::TableState;
use tracing::debug;

use super::helpers::{
    handle_fkey_show, handle_fkey_show_all, handle_fkey_toggle,
    mark_settings_dirty, sync_global_settings, TextEditor, picker_nav_up, picker_nav_down,
};
use super::overlay::OverlayRegistry;
use super::panel::{
    handle_downloads_key, handle_log_key, handle_models_key, handle_profiles_key,
    handle_settings_key, handle_system_prompt_presets_key,
};
use super::readme::{fetch_and_store_readme, fetch_readme_for_selected, handle_readme_key};

use crate::tui::app::pending_events::PendingEvent;

use crate::backend::hub;
use crate::models::SearchSort;
use crate::tui::app::{ActivePanel, App, ConfirmationKind, GlobalMode, ModelsMode};
use crate::tui::settings;


static OVERLAY_REGISTRY: std::sync::LazyLock<OverlayRegistry> =
    std::sync::LazyLock::new(OverlayRegistry::new);

pub async fn handle_key(app: &mut App, key: crossterm::event::KeyEvent) {
    debug!("Key: {:?}", key);

    // Global shortcuts that CREATE overlays (before overlay dispatch)
    // "/" in search mode opens search input
    if matches!(app.ui.global_mode, GlobalMode::Normal)
        && matches!(app.models_mode, ModelsMode::Search { .. })
        && key.code == KeyCode::Char('/')
        && !key.modifiers.contains(KeyModifiers::CONTROL)
    {
        app.ui.global_mode = GlobalMode::SearchInput {
            buffer: app.search.search_input.clone().unwrap_or_default(),
            cursor_pos: app
                .search
                .search_input
                .as_ref()
                .map(|s| s.chars().count())
                .unwrap_or(0),
        };
        return;
    }

    // Ctrl+U: open DashboardUrl modal
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('u') {
        app.ui.global_mode = GlobalMode::DashboardUrl {
            host: app.settings.host.clone(),
            port: app.config.default.ws_server_port.to_string(),
            auth_key: app
                .config
                .default
                .ws_server_auth_key
                .clone()
                .unwrap_or_default(),
            ws_enabled: app.config.default.ws_server_enabled,
            tls_enabled: app.config.default.ws_server_tls_enabled,
        };
        return;
    }

    // Ctrl+X: toggle expert mode
    if key.code == KeyCode::Char('x') && key.modifiers.contains(KeyModifiers::CONTROL) {
        app.settings_state.expert_mode = !app.settings_state.expert_mode;
        app.add_log(
            format!(
                "Expert mode: {}",
                if app.settings_state.expert_mode {
                    "ENABLED"
                } else {
                    "DISABLED"
                }
            ),
            crate::config::LogLevel::Info,
        );

        // Ensure selected index is still valid in the new mode
        let new_fields = settings::filtered_fields(app.settings_state.expert_mode);
        if app.settings_state.settings_selected_idx >= new_fields.len() {
            app.settings_state.settings_selected_idx = new_fields.len().saturating_sub(1);
        }

        mark_settings_dirty(app, false);
        return;
    }

    // Ctrl+G: GGUF filename explanation (global, works from any panel)
    if key.code == KeyCode::Char('g') && key.modifiers.contains(KeyModifiers::CONTROL) {
        let filename = match &app.models_mode {
            ModelsMode::List { .. } => app.selected_model().map(|m| m.display_name.clone()),
            ModelsMode::Search { results, .. } => {
                if let Some(idx) = app.search.search_results_idx {
                    if idx < results.len() {
                        Some(results[idx].model_id.clone())
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            ModelsMode::Files {
                files,
                selected_idx,
                ..
            } => selected_idx.and_then(|idx| files.get(idx).map(|(f, _, _)| f.clone())),
            ModelsMode::BenchTune => None,
        };
        if let Some(fn_name) = filename {
            let explanation = crate::tui::gguf_naming::get_explanation(
                &fn_name,
                &mut app.search.gguf_naming_cache,
            );
            app.ui.global_mode = GlobalMode::GgufNaming {
                explanation,
                filename: fn_name,
            };
        } else {
            app.add_log(
                "No filename available for GGUF explanation",
                crate::config::LogLevel::Warning,
            );
        }
        return;
    }

    // Ctrl+P: open profile picker modal (global, works from any panel)
    if key.code == KeyCode::Char('p') && key.modifiers.contains(KeyModifiers::CONTROL) {
        let all_profiles = app.config.merged_profiles();
        app.picker.profile_picker_entries = all_profiles
            .iter()
            .map(|p| {
                let desc = p.description.clone();
                (p.name.clone(), desc)
            })
            .collect();
        app.picker.profile_picker_selected = 0;
        app.ui.global_mode = GlobalMode::ProfilePicker {
            entries: app.picker.profile_picker_entries.clone(),
            selected: app.picker.profile_picker_selected,
            profiles: all_profiles.clone(),
        };
        return;
    }

    // Ctrl+L: cycle UI language (en → fr → it → en)
    if key.code == KeyCode::Char('l') && key.modifiers.contains(KeyModifiers::CONTROL) {
        let current = crate::tui::i18n::get_language();
        let next = match current.as_str() {
            "fr" => "it",
            "it" => "en",
            _ => "fr",
        };
        crate::tui::i18n::set_language(next);
        app.config.language = next.to_string();
        app.config.save().ok();
        app.add_log(
            format!("Language changed to {}", next.to_uppercase()),
            crate::config::LogLevel::Info,
        );
        return;
    }

    // Ctrl+O: show onboarding wizard (re-triggerable)
    if key.code == KeyCode::Char('o') && key.modifiers.contains(KeyModifiers::CONTROL) {
        app.config.onboarding_complete = false;
        app.config.save().ok();
        app.ui.global_mode = GlobalMode::Onboarding { step: 0 };
        return;
    }

    // Dispatch to overlay handler if an overlay is active
    // (when no overlay matches, dispatch returns without doing anything, flow continues)
    if OVERLAY_REGISTRY.dispatch(app, key).await {
        return;
    }

    // Tags modal (not a GlobalMode variant — handled separately)
    if app.edit.tags_editing {
        super::panel::tags::handle_tags_key(app, key);
        return;
    }

    // Open tags modal from settings panel
    if app.ui.active_panel == ActivePanel::LlmSettings
        && key.code == KeyCode::Char('t')
        && !app.edit.tags_editing
    {
        app.edit.tags_editing = true;
        app.edit.tags_insert_mode = true;
        app.edit.tags_edit_buffer = String::new();
        app.edit.tags_selected_idx = None;
        mark_settings_dirty(app, false);
        return;
    }

    // ── Normal mode key handling ──────────────────────────────────
    match key.code {
        KeyCode::Char('p') => {
            if !app.download.download_progress.is_empty()
                && let Some(idx) = app.download.download_scroll_state.selected()
            {
                let (is_downloading, filename) = {
                    if let Some(state) = app.download.download_progress.get(idx) {
                        match state.status {
                            crate::models::DownloadStatus::Downloading => {
                                (true, state.filename.clone())
                            }
                            crate::models::DownloadStatus::Paused => {
                                (false, state.filename.clone())
                            }
                            _ => (false, String::new()),
                        }
                    } else {
                        (false, String::new())
                    }
                };
                if is_downloading {
                    if let Some(state) = app.download.download_progress.get_mut(idx) {
                        state.status = crate::models::DownloadStatus::Pausing;
                        state.download_state = 4;
                        state.bytes_per_second = 0.0;
                        if let Some(arc) = &state.download_state_arc {
                            arc.store(4u8, std::sync::atomic::Ordering::Relaxed);
                        }
                    }
                    app.add_log(
                        format!("Pausing download of {}", filename),
                        crate::config::LogLevel::Info,
                    );
                } else if !filename.is_empty() {
                    if let Some(state) = app.download.download_progress.get_mut(idx) {
                        state.status = crate::models::DownloadStatus::Downloading;
                        if let Some(arc) = &state.download_state_arc {
                            arc.store(1u8, std::sync::atomic::Ordering::Relaxed);
                        }
                    }
                    app.add_log(
                        format!("Resumed download of {}", filename),
                        crate::config::LogLevel::Info,
                    );
                }
                return;
            }
        }
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::ALT) => {
            if !app.download.download_progress.is_empty() {
                let selected_idx = app.download.download_scroll_state.selected().unwrap_or(0);
                app.cancel_download(selected_idx);
                return;
            }
        }
        KeyCode::Char('c')
            if key
                .modifiers
                .contains(KeyModifiers::CONTROL) =>
        {
            let loaded_count = app
                .model_states
                .values()
                .filter(|s| matches!(s, crate::models::ModelState::Loaded { .. }))
                .count();
            if loaded_count > 0 {
                app.ui.global_mode = GlobalMode::Confirmation {
                    selected: false,
                    kind: ConfirmationKind::Exit,
                    display_name: String::new(),
                    detail: None,
                };
            } else {
                app.running = false;
            }
            return;
        }
        KeyCode::Esc => {
            if app.log.log_expanded && !app.search.filtering_local {
                app.log.log_expanded = false;
                return;
            }
            let is_benchmarking = app
                .model_states
                .values()
                .any(|s| matches!(s, crate::models::ModelState::Benchmarking));
            if is_benchmarking
                && app.server_mode == crate::models::ServerMode::Bench
                && let Some(handle) = app.server.server_handle.take()
            {
                app.add_log(crate::t!("log.stopping_benchmark"), crate::config::LogLevel::Info);
                let _ = app
                    .pending_tx
                    .send(PendingEvent::KillHandle { handle })
                    .await;
                return;
            }
        }
        KeyCode::Tab => {
            if app.ui.global_mode == GlobalMode::Normal {
                if key
                    .modifiers
                    .contains(KeyModifiers::SHIFT)
                {
                    app.focus_prev();
                } else {
                    app.focus_next();
                }
                return;
            }
        }
        KeyCode::Char('h')
            if key
                .modifiers
                .contains(KeyModifiers::CONTROL)
                && !key
                    .modifiers
                    .contains(KeyModifiers::SHIFT) =>
        {
            app.ui.panel_help = !app.ui.panel_help;
            if app.ui.panel_help {
                app.ui.panel_help_offset = 0;
            }
            return;
        }
        KeyCode::F(1) => {
            app.ui.active_panel = ActivePanel::Models;
            return;
        }
        KeyCode::F(2) => {
            handle_fkey_toggle(app, 1, Some(ActivePanel::ServerSettings), true);
            return;
        }
        KeyCode::F(3) => {
            handle_fkey_toggle(app, 2, Some(ActivePanel::ModelInfo), false);
            return;
        }
        KeyCode::F(4) => {
            handle_fkey_toggle(app, 3, Some(ActivePanel::LlmSettings), false);
            return;
        }
        KeyCode::F(5) => {
            handle_fkey_toggle(app, 4, None, false);
            return;
        }
        KeyCode::F(6) => {
            handle_fkey_toggle(app, 5, Some(ActivePanel::Log), false);
            return;
        }
        KeyCode::Left
            if key
                .modifiers
                .contains(KeyModifiers::SHIFT) =>
        {
            app.ui.left_pct = app.ui.left_pct.saturating_sub(1).max(20);
            return;
        }
        KeyCode::Right
            if key
                .modifiers
                .contains(KeyModifiers::SHIFT) =>
        {
            app.ui.left_pct = app.ui.left_pct.saturating_add(1).min(80);
            return;
        }
        KeyCode::F(7)
            if key
                .modifiers
                .contains(KeyModifiers::CONTROL) =>
        {
            handle_fkey_show(app, 0, ActivePanel::Models, false);
            return;
        }
        KeyCode::F(8)
            if key
                .modifiers
                .contains(KeyModifiers::CONTROL) =>
        {
            handle_fkey_show(app, 1, ActivePanel::ServerSettings, true);
            return;
        }
        KeyCode::F(9)
            if key
                .modifiers
                .contains(KeyModifiers::CONTROL) =>
        {
            handle_fkey_show(app, 3, ActivePanel::LlmSettings, false);
            return;
        }
        KeyCode::F(10)
            if key
                .modifiers
                .contains(KeyModifiers::CONTROL) =>
        {
            handle_fkey_show_all(app);
            return;
        }
        KeyCode::F(10) => {
            handle_fkey_show_all(app);
            return;
        }
        KeyCode::Char('k')
            if key
                .modifiers
                .contains(KeyModifiers::CONTROL)
                && key.modifiers.contains(KeyModifiers::ALT) =>
        {
            if let Some(handle) = app.server.server_handle.take() {
                let port = handle.port;
                let _ = app
                    .pending_tx
                    .send(PendingEvent::KillHandle { handle })
                    .await;
                app.add_log(
                    format!("Killing llama-server on port {}", port),
                    crate::config::LogLevel::Info,
                );
            } else {
                app.add_log(crate::t!("log.no_server"), crate::config::LogLevel::Warning);
            }
            return;
        }
        KeyCode::F(9) => {
            app.ui.panel_visibility = 0b111111;
            app.log.log_expanded = false;
            return;
        }
        KeyCode::Char('l')
            if key
                .modifiers
                .contains(KeyModifiers::CONTROL) =>
        {
            app.ui.active_panel = ActivePanel::Log;
            return;
        }
        KeyCode::Char('k')
            if key
                .modifiers
                .contains(KeyModifiers::CONTROL) =>
        {
            let version_param = app.settings.get_active_backend_version().map(|s| s.as_str());
            let binary_path = match hub::resolve_backend_binary(
                app.settings.backend,
                version_param,
                None,
                None,
            ).await {
                Ok(path) => path,
                Err(e) => {
                    app.add_log(format!("Failed to resolve llama-server binary: {}", e), crate::config::LogLevel::Error);
                    return;
                }
            };
            let model = app.selected_model().cloned();
            let (_cmd, cmd_line) = crate::backend::server::build_server_cmd(
                &binary_path,
                model.as_ref(),
                &app.settings,
                &app.config,
                app.server_mode,
                app.router_max_models,
            );
            app.ui.global_mode = GlobalMode::CmdLine { cmd_line };
            return;
        }
        KeyCode::Char('/') => {
            app.ui.active_panel = ActivePanel::Models;
            app.models_mode = ModelsMode::Search {
                query: String::new(),
                results: Vec::new(),
                sort_by: SearchSort::Relevance,
                show_readme: true,
                page: 0,
                loading: false,
                has_more: true,
            };
            app.search.search_input = None;
            app.ui.global_mode = GlobalMode::SearchInput {
                buffer: String::new(),
                cursor_pos: 0,
            };
            app.search.search_results_idx = Some(0);
            app.log.log_expanded = false;
            app.ui.panel_visibility &= !(1 << 4);
            app.ui.panel_visibility &= !(1 << 5);
            return;
        }
        KeyCode::Char('A') => {
            app.ui.global_mode = GlobalMode::About;
            return;
        }
        _ => {}
    }

    // ── Mode-specific handling ────────────────────────────────────

    // Handle search mode first (it takes priority) - unless README panel has focus (except for Enter key)
    let is_search = matches!(app.models_mode, ModelsMode::Search { .. })
        && (app.ui.active_panel != ActivePanel::SearchReadme || key.code == KeyCode::Enter);
    if is_search {
        handle_search_key(app, key).await;
        return;
    }

    // Handle files mode - unless README panel has focus (except for Enter key)
    let is_files = matches!(app.models_mode, ModelsMode::Files { .. })
        && (app.ui.active_panel != ActivePanel::SearchReadme || key.code == KeyCode::Enter);
    if is_files {
        handle_files_key(app, key).await;
        return;
    }

    // Handle bench_tune output view modal
    if app.bench_tune.bench_tune_output_view.is_some() {
        handle_bench_tune_output_key(app, key);
        return;
    }

    // Handle bench_tune mode
    if matches!(app.models_mode, ModelsMode::BenchTune) {
        handle_bench_tune_key(app, key).await;
        return;
    }

    // Skip normal key handling when panel help is showing
    if app.ui.panel_help && !app.search.filtering_local {
        match key.code {
            KeyCode::Esc => {
                app.ui.panel_help = false;
                return;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                app.ui.panel_help_offset = app.ui.panel_help_offset.saturating_add(1);
                return;
            }
            KeyCode::Char('k') | KeyCode::Up => {
                app.ui.panel_help_offset = app.ui.panel_help_offset.saturating_sub(1);
                return;
            }
            _ => {}
        }
        return;
    }

    // Global shortcuts for server settings
    if app.ui.active_panel == ActivePanel::ServerSettings {
        handle_server_settings_key(app, key);
        return;
    }

    // Global shortcuts
    if key.code == KeyCode::Char('s')
        && key
            .modifiers
            .contains(KeyModifiers::CONTROL)
    {
        // Ctrl+S: sort in search mode (takes priority over save)
        if matches!(app.models_mode, ModelsMode::Search { .. }) {
            if let ModelsMode::Search {
                sort_by, results, ..
            } = &mut app.models_mode
            {
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
                    app.search.search_results_idx = Some(0);
                } else {
                    app.search.search_results_idx = None;
                }
            }
            return;
        }
        // List mode sort
        else if let ModelsMode::List { sort_by } = &mut app.models_mode {
            *sort_by = sort_by.next();
        }
        app.save_model_settings();
        return;
    }

    match app.ui.active_panel {
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

// ── Extracted overlay handlers (used by overlay module) ─────────

pub(super) fn handle_prompt_picker_key(app: &mut App, key: crossterm::event::KeyEvent) {
    if let GlobalMode::PromptPicker {
        entries,
        selected,
        editing,
        edit_buffer,
        edit_cursor_pos,
        confirm_delete,
    } = &mut app.ui.global_mode
    {
        if *confirm_delete {
            match key.code {
                KeyCode::Char('y') => {
                    if *selected < entries.len() {
                        let name = entries[*selected].0.clone();
                        if matches!(
                            name.as_str(),
                            "General" | "Coder" | "Thinker" | "Mathematician"
                        ) {
                            *confirm_delete = false;
                            app.add_log(
                                "Cannot delete built-in preset",
                                crate::config::LogLevel::Error,
                            );
                            return;
                        } else {
                            entries.remove(*selected);
                            if *selected >= entries.len() && *selected > 0 {
                                *selected = entries.len() - 1;
                            }
                            let _ = app.config.system_prompt_presets.delete(&name);
                            let _ = app.config.save();
                        }
                    }
                    *confirm_delete = false;
                }
                KeyCode::Char('n') | KeyCode::Esc => {
                    *confirm_delete = false;
                }
                _ => {}
            }
            return;
        }

        if *editing {
            match key.code {
                KeyCode::Enter => {
                    TextEditor {
                        buffer: edit_buffer,
                        cursor: edit_cursor_pos,
                    }
                    .insert_newline();
                }
                KeyCode::Char('s')
                    if key
                        .modifiers
                        .contains(KeyModifiers::CONTROL) =>
                {
                    let mut saved = false;
                    if *selected < entries.len() {
                        let name = entries[*selected].0.clone();
                        let content = edit_buffer.clone();
                        if let Some(mut preset) =
                            app.config.system_prompt_presets.get(&name).cloned()
                        {
                            preset.content = content;
                            app.config.system_prompt_presets.save(&preset);
                            saved = app.config.save().is_ok();
                        }
                    }
                    let log_msg = if saved {
                        "Saved preset"
                    } else {
                        "Failed to save preset"
                    };
                    let log_level = if saved {
                        crate::config::LogLevel::Info
                    } else {
                        crate::config::LogLevel::Error
                    };
                    *editing = false;
                    app.add_log(log_msg, log_level);
                }
                KeyCode::Char(c) => {
                    TextEditor {
                        buffer: edit_buffer,
                        cursor: edit_cursor_pos,
                    }
                    .insert_char(c);
                }
                KeyCode::Backspace => {
                    TextEditor {
                        buffer: edit_buffer,
                        cursor: edit_cursor_pos,
                    }
                    .backspace();
                }
                KeyCode::Esc => {
                    *editing = false;
                }
                _ => {}
            }
            return;
        }

        match key.code {
            KeyCode::Up | KeyCode::Char('k') => picker_nav_up(selected),
            KeyCode::Down | KeyCode::Char('j') => picker_nav_down(selected, entries.len()),
            KeyCode::Enter => {
                if *selected < entries.len() {
                    let (name, _) = entries[*selected].clone();
                    app.settings.system_prompt_preset_name = name.clone();
                    app.resolve_system_prompt();
                    app.ui.global_mode = GlobalMode::Normal;
                }
            }
            KeyCode::Char('e') => {
                *editing = true;
                *edit_cursor_pos = 0;
                if *selected < entries.len() {
                    let name = entries[*selected].0.clone();
                    if let Some(preset) = app.config.system_prompt_presets.get(&name) {
                        *edit_buffer = preset.content.clone();
                    } else {
                        *edit_buffer = String::new();
                    }
                }
            }
            KeyCode::Char('>') | KeyCode::Char('n') => {
                let name = format!(
                    "Custom {}",
                    app.config.system_prompt_presets.user_presets().len() + 1
                );
                let preset = crate::config::SystemPromptPreset {
                    name: name.clone(),
                    description: "User-defined preset".into(),
                    content: String::new(),
                };
                app.config.system_prompt_presets.save(&preset);
                entries.push((name.clone(), "User-defined preset".into()));
                *selected = entries.len() - 1;
                *editing = true;
                *edit_cursor_pos = 0;
                *edit_buffer = String::new();
            }
            KeyCode::Char('d') => {
                if *selected < entries.len() {
                    let name = &entries[*selected].0;
                    if matches!(
                        name.as_str(),
                        "General" | "Coder" | "Thinker" | "Mathematician"
                    ) {
                        app.add_log(
                            "Cannot delete built-in preset",
                            crate::config::LogLevel::Error,
                        );
                        return;
                    }
                }
                *confirm_delete = true;
            }
            KeyCode::Esc => {
                app.ui.global_mode = GlobalMode::Normal;
            }
            _ => {}
        }
    }
}

pub(super) async fn handle_bench_tune_setup_key(app: &mut App, key: crossterm::event::KeyEvent) {
    if let GlobalMode::BenchTuneSetup {
        config,
        selected_idx,
        editing_param,
        editing_param_field,
        param_edit_buffer,
        param_edit_cursor_pos,
        bench_mode_selection,
        editing_prompt,
        editing_kwargs,
    } = &mut app.ui.global_mode
    {
        match key.code {
            KeyCode::Char('m') if key.modifiers.contains(KeyModifiers::ALT) => {
                *bench_mode_selection = if *bench_mode_selection == 0 { 1 } else { 0 };
                config.bench_mode = match *bench_mode_selection {
                    0 => crate::models::BenchTuneMode::RuntimeOnly,
                    _ => crate::models::BenchTuneMode::Full,
                };
            }
            KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::ALT) => {
                *editing_prompt = !*editing_prompt;
                if *editing_prompt {
                    app.edit.edit_cursor_pos = config.prompt.len();
                }
            }
            KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::ALT) => {
                app.edit.editing_n_predict = !app.edit.editing_n_predict;
                if app.edit.editing_n_predict {
                    app.edit.n_predict_edit_buffer = config.n_predict.to_string();
                }
            }
            KeyCode::Char('i') if key.modifiers.contains(KeyModifiers::ALT) => {
                app.edit.editing_iters = !app.edit.editing_iters;
                if app.edit.editing_iters {
                    app.edit.iters_edit_buffer = config.num_iterations.to_string();
                }
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::ALT) => {
                *editing_kwargs = !*editing_kwargs;
                if *editing_kwargs {
                    app.edit.edit_cursor_pos =
                        config.chat_template_kwargs.as_deref().unwrap_or("").len();
                }
            }
            KeyCode::Char('e')
                if !*editing_prompt
                    && !*editing_kwargs
                    && !app.edit.editing_n_predict
                    && !app.edit.editing_iters
                    && !*editing_param =>
            {
                if *selected_idx < config.params_to_test.len() {
                    let is_spec_off = config
                        .params_to_test
                        .iter()
                        .find(|p| p.name == "spec_type")
                        .map(|p| p.min as usize == 0)
                        .unwrap_or(true);
                    let p = &config.params_to_test[*selected_idx];
                    if p.name != "spec_type" && !(p.name == "draft_tokens" && is_spec_off) {
                        *editing_param = true;
                        if !p.variants.is_empty() {
                            *editing_param_field = -1;
                            param_edit_buffer.clear();
                        } else {
                            *editing_param_field = 0;
                            param_edit_buffer.clone_from(&p.min.to_string());
                            *param_edit_cursor_pos = param_edit_buffer.len();
                        }
                    }
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if *editing_prompt || *editing_kwargs {
                    if app.edit.edit_cursor_pos > 0 {
                        app.edit.edit_cursor_pos = app.edit.edit_cursor_pos.saturating_sub(1);
                    }
                } else {
                    *selected_idx = selected_idx.saturating_sub(1);
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if *editing_prompt || *editing_kwargs {
                    let len = if *editing_prompt {
                        config.prompt.len()
                    } else {
                        config
                            .chat_template_kwargs
                            .as_deref()
                            .map(|s: &str| s.len())
                            .unwrap_or(0)
                    };
                    app.edit.edit_cursor_pos = (app.edit.edit_cursor_pos + 1).min(len);
                } else {
                    *selected_idx =
                        (*selected_idx + 1).min(config.params_to_test.len().saturating_sub(1));
                }
            }
            KeyCode::Left => {
                if *editing_prompt || *editing_kwargs {
                    if app.edit.edit_cursor_pos > 0 {
                        app.edit.edit_cursor_pos = app.edit.edit_cursor_pos.saturating_sub(1);
                    }
                } else {
                    *selected_idx = selected_idx.saturating_sub(1);
                }
            }
            KeyCode::Right => {
                if *editing_prompt || *editing_kwargs {
                    let len = if *editing_prompt {
                        config.prompt.len()
                    } else {
                        config
                            .chat_template_kwargs
                            .as_deref()
                            .map(|s: &str| s.len())
                            .unwrap_or(0)
                    };
                    app.edit.edit_cursor_pos = (app.edit.edit_cursor_pos + 1).min(len);
                } else {
                    *selected_idx =
                        (*selected_idx + 1).min(config.params_to_test.len().saturating_sub(1));
                }
            }
            KeyCode::Tab => {
                if *editing_param
                    && config.params_to_test[*selected_idx].variants.is_empty() {
                        *editing_param_field = (*editing_param_field + 1).min(2);
                        let p = &config.params_to_test[*selected_idx];
                        let val = match *editing_param_field {
                            0 => p.min,
                            1 => p.max,
                            2 => p.step,
                            _ => 0.0,
                        };
                        param_edit_buffer.clear();
                        if *editing_param_field == 2 {
                            *param_edit_buffer = val.to_string();
                        } else {
                            *param_edit_buffer = format!("{:.2}", val);
                        }
                        *param_edit_cursor_pos = param_edit_buffer.len();
                    }
            }
            KeyCode::BackTab => {
                if *editing_param
                    && config.params_to_test[*selected_idx].variants.is_empty() {
                        *editing_param_field = if *editing_param_field <= 0 {
                            2
                        } else {
                            *editing_param_field - 1
                        };
                        let p = &config.params_to_test[*selected_idx];
                        let val = match *editing_param_field {
                            0 => p.min,
                            1 => p.max,
                            2 => p.step,
                            _ => 0.0,
                        };
                        param_edit_buffer.clear();
                        if *editing_param_field == 2 {
                            *param_edit_buffer = val.to_string();
                        } else {
                            *param_edit_buffer = format!("{:.2}", val);
                        }
                        *param_edit_cursor_pos = param_edit_buffer.len();
                    }
            }
            KeyCode::Char('+')
                if *editing_param && !config.params_to_test[*selected_idx].variants.is_empty() =>
            {
                let current_idx = if *editing_param_field < -1 {
                    (*editing_param_field + 2) as usize
                } else {
                    0
                };
                let variants = &config.params_to_test[*selected_idx].variants;
                *editing_param_field = -(((current_idx + 1) % variants.len()) as i32 + 2);
            }
            KeyCode::Char('-')
                if *editing_param && !config.params_to_test[*selected_idx].variants.is_empty() =>
            {
                let current_idx = if *editing_param_field < -1 {
                    (*editing_param_field + 2) as usize
                } else {
                    0
                };
                let variants = &config.params_to_test[*selected_idx].variants;
                *editing_param_field =
                    -(((current_idx + variants.len() - 1) % variants.len()) as i32 + 2);
            }
            KeyCode::Char(' ') => {
                if *editing_prompt {
                    if app.edit.edit_cursor_pos <= config.prompt.len() {
                        config.prompt.insert(app.edit.edit_cursor_pos, ' ');
                        app.edit.edit_cursor_pos += 1;
                    }
                } else if *editing_kwargs {
                    let kwargs = config.chat_template_kwargs.get_or_insert_with(String::new);
                    if app.edit.edit_cursor_pos <= kwargs.len() {
                        kwargs.insert(app.edit.edit_cursor_pos, ' ');
                        app.edit.edit_cursor_pos += 1;
                    }
                } else if *selected_idx < config.params_to_test.len() {
                    let is_spec_off = config
                        .params_to_test
                        .iter()
                        .find(|p| p.name == "spec_type")
                        .map(|p| p.min as usize == 0)
                        .unwrap_or(true);
                    let p = &mut config.params_to_test[*selected_idx];
                    if p.name != "spec_type" && !(p.name == "draft_tokens" && is_spec_off) {
                        p.enabled = !p.enabled;
                    }
                }
            }
            KeyCode::Char(c)
                if *editing_param && config.params_to_test[*selected_idx].variants.is_empty() =>
            {
                if "0123456789.-eE".contains(c) {
                    TextEditor {
                        buffer: param_edit_buffer,
                        cursor: param_edit_cursor_pos,
                    }
                    .insert_char(c);
                }
            }
            KeyCode::Char(c)
                if *editing_param && !config.params_to_test[*selected_idx].variants.is_empty() =>
            {
                if c.is_ascii_digit() {
                    let idx = c.to_digit(10).unwrap() as usize;
                    let variants = &config.params_to_test[*selected_idx].variants;
                    if idx < variants.len() {
                        *editing_param_field = -(idx as i32 + 2);
                    }
                }
            }
            KeyCode::Char(c) => {
                if *editing_prompt {
                    config.prompt.insert(app.edit.edit_cursor_pos, c);
                    app.edit.edit_cursor_pos += 1;
                } else if *editing_kwargs {
                    let kwargs = config.chat_template_kwargs.get_or_insert_with(String::new);
                    kwargs.insert(app.edit.edit_cursor_pos, c);
                    app.edit.edit_cursor_pos += 1;
                } else if app.edit.editing_n_predict {
                    if c.is_ascii_digit() {
                        app.edit.n_predict_edit_buffer.push(c);
                    }
                } else if app.edit.editing_iters && c.is_ascii_digit() {
                    app.edit.iters_edit_buffer.push(c);
                }
            }
            KeyCode::Backspace if *editing_param => {
                TextEditor {
                    buffer: param_edit_buffer,
                    cursor: param_edit_cursor_pos,
                }
                .backspace();
            }
            KeyCode::Backspace => {
                if *editing_prompt {
                    if app.edit.edit_cursor_pos > 0 {
                        config.prompt.remove(app.edit.edit_cursor_pos - 1);
                        app.edit.edit_cursor_pos = app.edit.edit_cursor_pos.saturating_sub(1);
                    }
                } else if *editing_kwargs {
                    let kwargs = config.chat_template_kwargs.get_or_insert_with(String::new);
                    if app.edit.edit_cursor_pos > 0 {
                        kwargs.remove(app.edit.edit_cursor_pos - 1);
                        app.edit.edit_cursor_pos = app.edit.edit_cursor_pos.saturating_sub(1);
                    }
                } else if app.edit.editing_n_predict {
                    app.edit.n_predict_edit_buffer.pop();
                } else if app.edit.editing_iters {
                    app.edit.iters_edit_buffer.pop();
                } else {
                    *selected_idx = selected_idx.saturating_sub(1);
                }
            }
            KeyCode::Delete if *editing_param => {
                if *param_edit_cursor_pos < param_edit_buffer.len()
                    && let Some((byte_pos, _ch)) =
                        param_edit_buffer.char_indices().nth(*param_edit_cursor_pos)
                {
                    param_edit_buffer.remove(byte_pos);
                }
            }
            KeyCode::Delete => {
                if *editing_prompt {
                    if app.edit.edit_cursor_pos < config.prompt.len() {
                        config.prompt.remove(app.edit.edit_cursor_pos);
                    }
                } else if *editing_kwargs {
                    let kwargs = config.chat_template_kwargs.get_or_insert_with(String::new);
                    if app.edit.edit_cursor_pos < kwargs.len() {
                        kwargs.remove(app.edit.edit_cursor_pos);
                    }
                } else if app.edit.editing_n_predict {
                    if !app.edit.n_predict_edit_buffer.is_empty() {
                        app.edit.n_predict_edit_buffer.pop();
                    }
                } else if app.edit.editing_iters && !app.edit.iters_edit_buffer.is_empty() {
                    app.edit.iters_edit_buffer.pop();
                }
            }
            KeyCode::Enter => {
                if *editing_param {
                    if *selected_idx < config.params_to_test.len() {
                        if !config.params_to_test[*selected_idx].variants.is_empty() {
                            // Cycle to next variant, stay in editing mode
                            let current_idx = if *editing_param_field < -1 {
                                (*editing_param_field + 2) as usize
                            } else {
                                0
                            };
                            let variants = &config.params_to_test[*selected_idx].variants;
                            let next_idx = (current_idx + 1) % variants.len();
                            *editing_param_field = -(next_idx as i32 + 2);
                        } else if let Ok(val) = param_edit_buffer.parse::<f64>() {
                            match *editing_param_field {
                                0 => config.params_to_test[*selected_idx].min = val,
                                1 => config.params_to_test[*selected_idx].max = val,
                                2 => {
                                    if val > 0.0 {
                                        config.params_to_test[*selected_idx].step = val;
                                    }
                                }
                                _ => {}
                            }
                        }
                        let min_val = config.params_to_test[*selected_idx].min;
                        let max_val = config.params_to_test[*selected_idx].max;
                        let step_val = config.params_to_test[*selected_idx].step;
                        if min_val > max_val {
                            config.params_to_test[*selected_idx].min = max_val;
                        }
                        if step_val <= 0.0 {
                            config.params_to_test[*selected_idx].step = max_val - min_val;
                        }
                    }
                    if !config.params_to_test[*selected_idx].variants.is_empty() {
                        // Stay in variant editing mode
                    } else {
                        *editing_param = false;
                        param_edit_buffer.clear();
                    }
                } else if *editing_prompt {
                    *editing_prompt = false;
                } else if *editing_kwargs {
                    *editing_kwargs = false;
                } else if app.edit.editing_n_predict {
                    if let Ok(val) = app.edit.n_predict_edit_buffer.parse::<u32>() {
                        config.n_predict = val.clamp(1, 16384);
                    }
                    app.edit.editing_n_predict = false;
                } else if app.edit.editing_iters {
                    if let Ok(val) = app.edit.iters_edit_buffer.parse::<u32>() {
                        config.num_iterations = val.clamp(1, 100);
                    }
                    app.edit.editing_iters = false;
                } else if *selected_idx < config.params_to_test.len()
                    && config.params_to_test[*selected_idx].name == "spec_type"
                {
                    let p = &mut config.params_to_test[*selected_idx];
                    let current_idx = p.min as usize;
                    let next_idx = (current_idx + 1) % p.variants.len();
                    p.min = next_idx as f64;

                    // If spec_type becomes "Off" (index 0), disable draft_tokens
                    if next_idx == 0 {
                        for param in &mut config.params_to_test {
                            if param.name == "draft_tokens" {
                                param.enabled = false;
                            }
                        }
                    }
                } else {
                    let config_final = config.clone();
                    if let Some(idx) = app.selected_model_idx {
                        let model = app.models[idx].clone();
                        let settings = app.settings.clone();
                        app.ui.global_mode = GlobalMode::Normal;
                        app.bench_tune.bench_tune_config = Some(config_final);
                        let _ = app
                            .pending_tx
                            .send(PendingEvent::Spawn {
                                model: Some(model),
                                settings,
                            })
                            .await;
                    }
                }
            }
            KeyCode::Esc => {
                if *editing_param {
                    *editing_param = false;
                    param_edit_buffer.clear();
                } else if *editing_prompt {
                    *editing_prompt = false;
                } else if *editing_kwargs {
                    *editing_kwargs = false;
                } else if app.edit.editing_n_predict {
                    app.edit.editing_n_predict = false;
                } else if app.edit.editing_iters {
                    app.edit.editing_iters = false;
                } else {
                    app.ui.global_mode = GlobalMode::Normal;
                }
            }
            _ => {}
        }
    }
}

async fn handle_search_key(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.models_mode = ModelsMode::List { sort_by: crate::models::ListSort::Name };
            app.ui.panel_visibility |= (1 << 4) | (1 << 5);
            return;
        }
        KeyCode::Enter | KeyCode::Char('f')
            if key
                .modifiers
                .contains(KeyModifiers::CONTROL)
                || key.code == KeyCode::Enter =>
        {
            let model_id = if let ModelsMode::Search { results, .. } = &app.models_mode {
                app.search
                    .search_results_idx
                    .and_then(|idx| results.get(idx).map(|r| r.model_id.clone()))
            } else {
                None
            };
            if let Some(ref model_id) = model_id {
                // Fetch and display README first
                app.add_log(
                    format!("Fetching README for {}...", model_id),
                    crate::config::LogLevel::Info,
                );
                fetch_and_store_readme(app, model_id.clone()).await;
                if let ModelsMode::Search { show_readme, .. } = &mut app.models_mode {
                    *show_readme = true;
                }
                app.ui.active_panel = ActivePanel::Models;

                app.add_log(
                    format!("Loading files for {}...", model_id),
                    crate::config::LogLevel::Info,
                );
                match hub::list_gguf_files(model_id).await {
                    Ok(files) => {
                        app.add_log(
                            format!("Found {} GGUF files", files.len()),
                            crate::config::LogLevel::Info,
                        );
                        if let ModelsMode::Search { query, results, .. } = &app.models_mode {
                            let selected_result = app
                                .search
                                .search_results_idx
                                .and_then(|idx| results.get(idx).cloned());
                            app.search.files_table_state = TableState::default();
                            app.models_mode = ModelsMode::Files {
                                model_id: model_id.clone(),
                                files,
                                selected_idx: Some(0),
                                previous_query: query.clone(),
                                previous_results: results.clone(),
                                selected_result,
                            };
                        }
                    }
                    Err(e) => {
                        app.add_log(
                            format!("No GGUF files: {}", e),
                            crate::config::LogLevel::Info,
                        );
                    }
                }
            }
            return;
        }
        KeyCode::Char('s')
            if key
                .modifiers
                .contains(KeyModifiers::CONTROL) =>
        {
            if let ModelsMode::Search {
                sort_by, results, ..
            } = &mut app.models_mode
            {
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
                    app.search.search_results_idx = Some(0);
                } else {
                    app.search.search_results_idx = None;
                }
            }
            return;
        }
        KeyCode::Char('B')
            if key
                .modifiers
                .contains(KeyModifiers::CONTROL) =>
        {
            if let ModelsMode::Search { page, .. } = &app.models_mode
                && *page > 0
            {
                let query = if let ModelsMode::Search { query, .. } = &app.models_mode {
                    query.clone()
                } else {
                    String::new()
                };
                let offset = (*page as u32 - 1) * 50;
                app.add_log(
                    format!("Loading page {}...", *page - 1),
                    crate::config::LogLevel::Info,
                );
                if let ModelsMode::Search { page, .. } = &mut app.models_mode {
                    *page -= 1;
                }
                let _ = app
                    .pending_tx
                    .send(PendingEvent::Search {
                        query,
                        offset,
                    })
                    .await;
                return;
            }
            return;
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let len = app.search_results_len();
            match app.search.search_results_idx {
                Some(idx) if idx + 1 < len => app.search.search_results_idx = Some(idx + 1),
                Some(idx) => {
                    if idx + 1 >= len
                        && let ModelsMode::Search {
                            has_more,
                            loading,
                            page,
                            ..
                        } = &app.models_mode
                        && !*loading
                        && *has_more
                    {
                        let query = if let ModelsMode::Search { query, .. } = &app.models_mode {
                            query.clone()
                        } else {
                            String::new()
                        };
                        let offset = (*page as u32 + 1) * 50;
                        app.add_log(crate::t!("log.loading_more"), crate::config::LogLevel::Info);
                        let _ = app
                            .pending_tx
                            .send(PendingEvent::Search {
                                query,
                                offset,
                            })
                            .await;
                        return;
                    }
                    app.search.search_results_idx = Some(len.saturating_sub(1));
                }
                None if len > 0 => app.search.search_results_idx = Some(0),
                _ => {}
            }
            return;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            match app.search.search_results_idx {
                Some(idx) if idx > 0 => app.search.search_results_idx = Some(idx - 1),
                None => {
                    let len = app.search_results_len();
                    if len > 0 {
                        app.search.search_results_idx = Some(0);
                    }
                }
                _ => {}
            }
            return;
        }
        KeyCode::Char('R')
            if key
                .modifiers
                .contains(KeyModifiers::CONTROL) =>
        {
            let model_id = if let ModelsMode::Search { results, .. } = &app.models_mode {
                app.search
                    .search_results_idx
                    .and_then(|idx| results.get(idx).map(|r| r.model_id.clone()))
            } else {
                None
            };
            if let Some(ref model_id) = model_id {
                app.add_log(
                    format!("Fetching README for {}...", model_id),
                    crate::config::LogLevel::Info,
                );
                app.add_log(crate::t!("log.taking_moment"), crate::config::LogLevel::Info);
                fetch_and_store_readme(app, model_id.clone()).await;
                if let ModelsMode::Search { show_readme, .. } = &mut app.models_mode {
                    *show_readme = true;
                }
            }
            app.ui.active_panel = ActivePanel::SearchReadme;
            return;
        }
        KeyCode::Right => {
            let model_id = if let ModelsMode::Search { results, .. } = &app.models_mode {
                app.search
                    .search_results_idx
                    .and_then(|idx| results.get(idx).map(|r| r.model_id.clone()))
            } else {
                None
            };
            if let Some(ref model_id) = model_id {
                app.add_log(
                    format!("Fetching README for {}...", model_id),
                    crate::config::LogLevel::Info,
                );
                fetch_and_store_readme(app, model_id.clone()).await;
                if let ModelsMode::Search { show_readme, .. } = &mut app.models_mode {
                    *show_readme = true;
                }
            }
            app.ui.active_panel = ActivePanel::SearchReadme;
            return;
        }
        _ => {}
    }

    if !matches!(app.ui.global_mode, GlobalMode::Normal) {
        return;
    }

    if let ModelsMode::Search { results, .. } = &app.models_mode
        && let Some(idx) = app.search.search_results_idx
        && let Some(r) = results.get(idx)
    {
        fetch_readme_for_selected(app, r.model_id.clone()).await;
    }
}

async fn handle_files_key(app: &mut App, key: crossterm::event::KeyEvent) {
    let model_id = if let ModelsMode::Files { model_id, .. } = &app.models_mode {
        Some(model_id.clone())
    } else {
        None
    };

    match key.code {
        KeyCode::Esc => {
            if let ModelsMode::Files {
                previous_query,
                previous_results,
                ..
            } = std::mem::replace(&mut app.models_mode, ModelsMode::List { sort_by: crate::models::ListSort::Name })
            {
                let current_idx = app.search.search_results_idx;
                let should_reset =
                    current_idx.is_some() && current_idx.unwrap() >= previous_results.len();
                app.models_mode = ModelsMode::Search {
                    query: previous_query,
                    results: previous_results,
                    sort_by: SearchSort::Relevance,
                    show_readme: true,
                    page: 0,
                    loading: false,
                    has_more: true,
                };
                app.search.search_results_idx = current_idx;
                if should_reset {
                    app.search.search_results_idx = Some(0);
                }
            }
            return;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if let ModelsMode::Files {
                files,
                selected_idx,
                ..
            } = &mut app.models_mode
            {
                match *selected_idx {
                    Some(idx) if idx > 0 => *selected_idx = Some(idx - 1),
                    None if !files.is_empty() => *selected_idx = Some(0),
                    _ => {}
                }
            }
            return;
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if let ModelsMode::Files {
                files,
                selected_idx,
                ..
            } = &mut app.models_mode
            {
                match *selected_idx {
                    Some(idx) if idx + 1 < files.len() => *selected_idx = Some(idx + 1),
                    None if !files.is_empty() => *selected_idx = Some(0),
                    _ => {}
                }
            }
            return;
        }
        KeyCode::Enter => {
            let download_info = if let ModelsMode::Files {
                model_id,
                files,
                selected_idx,
                ..
            } = &app.models_mode
            {
                selected_idx.and_then(|idx| {
                    files.get(idx).map(|(f, s, u): &(_, _, _)| {
                        (model_id.clone(), f.clone(), u.clone(), *s, model_id.clone())
                    })
                })
            } else {
                None
            };
            if let Some((model_id, filename, url, file_size, subdir)) = download_info {
                if app
                    .download
                    .download_progress
                    .iter()
                    .any(|d| d.model_id == model_id && d.filename == filename)
                {
                    app.add_log(
                        "Download already in progress",
                        crate::config::LogLevel::Warning,
                    );
                    return;
                }
                let models_dir = app.config.models_dirs.first().cloned().unwrap_or_default();
                let dest_dir = models_dir.join(&subdir);
                let basename = std::path::Path::new(&filename)
                    .file_name()
                    .unwrap_or_default();
                let file_path = dest_dir.join(basename);
                if file_path.exists() {
                    app.add_log(crate::t!("log.already_downloaded"), crate::config::LogLevel::Warning);
                    return;
                }
                app.add_log(
                    format!("Downloading {}...", filename),
                    crate::config::LogLevel::Info,
                );
                let _ = app
                    .pending_tx
                    .send(PendingEvent::Download {
                        model_id,
                        filename,
                        url,
                        file_size,
                        subdir,
                    })
                    .await;
            }
            return;
        }
        KeyCode::Right => {
            app.ui.active_panel = ActivePanel::SearchReadme;
            return;
        }
        _ => {}
    }

    if let Some(ref model_id) = model_id {
        fetch_readme_for_selected(app, model_id.clone()).await;
    }
}

fn handle_bench_tune_output_key(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.bench_tune.bench_tune_output_view = None;
        }
        KeyCode::Down => {
            app.bench_tune.bench_tune_output_scroll =
                app.bench_tune.bench_tune_output_scroll.saturating_add(1);
        }
        KeyCode::Up => {
            app.bench_tune.bench_tune_output_scroll =
                app.bench_tune.bench_tune_output_scroll.saturating_sub(1);
        }
        KeyCode::PageDown => {
            app.bench_tune.bench_tune_output_scroll =
                app.bench_tune.bench_tune_output_scroll.saturating_add(10);
        }
        KeyCode::PageUp => {
            app.bench_tune.bench_tune_output_scroll =
                app.bench_tune.bench_tune_output_scroll.saturating_sub(10);
        }
        KeyCode::Right => {
            if let Some(mut result_idx) = app.bench_tune.bench_tune_output_view
                && let Some(result) = app.bench_tune.bench_tune_results.get(result_idx)
            {
                let max_iter_idx = result.outputs.len().saturating_sub(1);
                if app.bench_tune.bench_tune_output_index < max_iter_idx {
                    app.bench_tune.bench_tune_output_index += 1;
                    app.bench_tune.bench_tune_output_scroll = 0;
                    app.bench_tune.bench_tune_output_h_scroll = 0;
                } else if result_idx < app.bench_tune.bench_tune_results.len().saturating_sub(1) {
                    result_idx += 1;
                    app.bench_tune.bench_tune_output_view = Some(result_idx);
                    app.bench_tune.bench_tune_output_index = 0;
                    app.bench_tune.bench_tune_output_scroll = 0;
                    app.bench_tune.bench_tune_output_h_scroll = 0;
                }
            }
        }
        KeyCode::Left => {
            if let Some(mut result_idx) = app.bench_tune.bench_tune_output_view {
                if app.bench_tune.bench_tune_output_index > 0 {
                    app.bench_tune.bench_tune_output_index -= 1;
                    app.bench_tune.bench_tune_output_scroll = 0;
                    app.bench_tune.bench_tune_output_h_scroll = 0;
                } else if result_idx > 0 {
                    result_idx -= 1;
                    app.bench_tune.bench_tune_output_view = Some(result_idx);
                    if let Some(prev_result) = app.bench_tune.bench_tune_results.get(result_idx) {
                        app.bench_tune.bench_tune_output_index =
                            prev_result.outputs.len().saturating_sub(1);
                    } else {
                        app.bench_tune.bench_tune_output_index = 0;
                    }
                    app.bench_tune.bench_tune_output_scroll = 0;
                    app.bench_tune.bench_tune_output_h_scroll = 0;
                }
            }
        }
        _ => {}
    }
}

async fn handle_bench_tune_key(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            // Signal cancellation to the benchmark task so it can send Cancelled status
            if let Some(cancel_tx) = &app.bench_tune.bench_tune_cancel_tx {
                let _ = cancel_tx.send(true);
                app.add_log(
                    "BenchTune: cancellation requested",
                    crate::config::LogLevel::Info,
                );
            }
            // Clean up server handle (don't kill the server — let the benchmark task handle it)
            if app.server.server_handle.take().is_some() {
                app.server.metrics_rx = None;
                app.metrics = Default::default();
            }
            // Clean up orphaned task handle so tokio can drop it
            if app.server.bench_tune_task_handle.take().is_some() {
                app.add_log(
                    "BenchTune: task handle cleaned up",
                    crate::config::LogLevel::Info,
                );
            }
            // Don't abort the task — let it finish gracefully and send Cancelled status
            // Keep bench_tune_running = true so the app knows the task is still finishing up
            app.models_mode = ModelsMode::List { sort_by: crate::models::ListSort::Name };
        }
        KeyCode::Enter => {
            let result_idx = app.bench_tune.bench_tune_result_row;
            if let Some(result) = app.bench_tune.bench_tune_results.get(result_idx)
                && !result.outputs.is_empty()
            {
                app.bench_tune.bench_tune_output_view = Some(result_idx);
                app.bench_tune.bench_tune_output_index = 0;
                app.bench_tune.bench_tune_output_scroll = 0;
            }
        }
        KeyCode::Down => {
            let len = app.bench_tune.bench_tune_results.len();
            if len > 0 {
                app.bench_tune.bench_tune_result_row =
                    (app.bench_tune.bench_tune_result_row + 1).min(len - 1);
            }
        }
        KeyCode::Up => {
            if app.bench_tune.bench_tune_result_row > 0 {
                app.bench_tune.bench_tune_result_row -= 1;
            }
        }
        _ => {}
    }
}

fn handle_server_settings_key(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Enter => {
            match app.settings_state.server_settings_selected_idx {
                0 => {
                    let entries = App::fetch_host_picker_entries();
                    app.picker.host_picker_entries = entries;
                    app.picker.host_picker_selected = 0;
                    app.ui.global_mode = GlobalMode::HostPicker {
                        entries: app.picker.host_picker_entries.clone(),
                        selected: 0,
                    };
                }
                1 => {
                    let entries = app.fetch_backend_picker_entries();
                    app.picker.backend_picker_entries = entries.clone();
                    let current_tag = app.settings.get_active_backend_version();
                    app.picker.backend_picker_selected = entries
                        .iter()
                        .position(|(b, t): &(_, _)| {
                            *b == app.settings.backend && t.as_ref() == current_tag
                        })
                        .unwrap_or(0);
                    app.ui.global_mode = GlobalMode::BackendPicker {
                        entries,
                        selected: app.picker.backend_picker_selected,
                    };
                }
                2 => {
                    app.settings.threads = (app.settings.threads % app.max_threads) + 1;
                }
                3 => {
                    app.settings.threads_batch = (app.settings.threads_batch % 32) + 1;
                }
                4 => {
                    app.server_mode = match app.server_mode {
                        crate::models::ServerMode::Normal => crate::models::ServerMode::Bench,
                        crate::models::ServerMode::Bench => crate::models::ServerMode::BenchTune,
                        crate::models::ServerMode::BenchTune => crate::models::ServerMode::Normal,
                        _ => crate::models::ServerMode::Normal,
                    };
                }
                5 => {
                    if app.server.server_handle.is_none() {
                        app.settings.api_endpoint_enabled = !app.settings.api_endpoint_enabled;
                    }
                }
                6 => {
                    app.ui.global_mode = GlobalMode::DashboardPicker {
                        enabled: app.config.default.ws_server_enabled,
                        port: app.config.default.ws_server_port.to_string(),
                        auth_key: app
                            .config
                            .default
                            .ws_server_auth_key
                            .clone()
                            .unwrap_or_default(),
                        tls_enabled: app.config.default.ws_server_tls_enabled,
                        tls_cert: app
                            .config
                            .default
                            .ws_server_tls_cert
                            .clone()
                            .unwrap_or_default(),
                        tls_key: app
                            .config
                            .default
                            .ws_server_tls_key
                            .clone()
                            .unwrap_or_default(),
                        selected_field: -1,
                        editing: false,
                        edit_buffer: String::new(),
                        edit_cursor_pos: 0,
                    };
                }
                7 => {
                    app.ui.global_mode = GlobalMode::RpcManager;
                    app.picker.rpc_workers_selected_idx = 0;
                    app.picker.editing_rpc_worker = None;
                }
                _ => {}
            }
            sync_global_settings(app);
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.settings_state.server_settings_selected_idx = app
                .settings_state
                .server_settings_selected_idx
                .saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.settings_state.server_settings_selected_idx += 1;
        }
        KeyCode::Left | KeyCode::Char('h') => {
            match app.settings_state.server_settings_selected_idx {
                2 => app.settings.threads = app.settings.threads.saturating_sub(1).max(1),
                3 => {
                    app.settings.threads_batch = app.settings.threads_batch.saturating_sub(1).max(1)
                }
                _ => {}
            }
            mark_settings_dirty(app, true);
            sync_global_settings(app);
        }
        KeyCode::Right | KeyCode::Char('l') => {
            match app.settings_state.server_settings_selected_idx {
                2 => app.settings.threads = (app.settings.threads + 1).min(app.max_threads),
                3 => app.settings.threads_batch = (app.settings.threads_batch + 1).min(64),
                _ => {}
            }
            mark_settings_dirty(app, true);
            sync_global_settings(app);
        }
        _ => {}
    }
}
