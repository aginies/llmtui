use crossterm::event::{KeyCode, KeyModifiers};
use tracing::debug;

use super::helpers::{
    TextEditor, handle_fkey_focus, handle_fkey_show_all, handle_fkey_toggle, mark_settings_dirty,
    picker_nav_down, picker_nav_up, sync_global_settings,
};
use super::overlay::{OverlayRegistry, check_web_search_health};
use super::panel::{
    handle_downloads_key, handle_log_key, handle_models_key, handle_profiles_key,
    handle_settings_key, handle_system_prompt_presets_key,
};
use super::readme::{fetch_and_store_readme, fetch_readme_for_selected, handle_readme_key};

use crate::tui::app::pending_events::PendingEvent;

use crate::backend::hub;
use crate::models::{BenchTuneConfig, BenchTuneField, SearchSort};
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
        let port = if let Some(handle) = &app.server.server_handle {
            handle.port
        } else {
            app.settings.port
        };
        app.ui.global_mode = GlobalMode::DashboardUrl {
            host: app.settings.host.clone(),
            ws_port: app.config.default.ws_server_port.to_string(),
            api_port: app.settings.api_endpoint_port,
            llm_port: port,
            auth_key: app
                .config
                .default
                .api_endpoint_key
                .clone()
                .unwrap_or_default(),
            ws_enabled: app.config.default.ws_server_enabled,
            tls_enabled: app.server.running_server_tls.unwrap_or(false),
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
        let new_fields = settings::filtered_fields(app.settings_state.expert_mode, &app.settings);
        if app.settings_state.settings_selected_idx >= new_fields.len() {
            app.settings_state.settings_selected_idx = new_fields.len().saturating_sub(1);
        }

        mark_settings_dirty(app, false);
        return;
    }

    // Ctrl+Space: open command palette (global, Normal mode only)
    if key.code == KeyCode::Char(' ')
        && key.modifiers.contains(KeyModifiers::CONTROL)
        && !key.modifiers.contains(KeyModifiers::SHIFT)
        && app.ui.global_mode == GlobalMode::Normal
    {
        app.ui.global_mode = GlobalMode::CommandPalette {
            buffer: String::new(),
            cursor_pos: 0,
            selected: 0,
        };
        return;
    }

    // Ctrl+G: GGUF filename explanation (global, works from any panel)
    if key.code == KeyCode::Char('g') && key.modifiers.contains(KeyModifiers::CONTROL) {
        if let Some(fn_name) = crate::tui::command_palette::selected_filename(app) {
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

    // Ctrl+F2: toggle Server Settings panel (global, works from any overlay)
    if key.code == KeyCode::F(2) && key.modifiers.contains(KeyModifiers::CONTROL) {
        handle_fkey_toggle(app, 1, Some(ActivePanel::ServerSettings), false);
        return;
    }

    // Ctrl+F3: toggle LLM Settings panel (global, works from any overlay)
    if key.code == KeyCode::F(3) && key.modifiers.contains(KeyModifiers::CONTROL) {
        handle_fkey_toggle(app, 3, Some(ActivePanel::LlmSettings), false);
        return;
    }

    // Alt+F3: toggle LLM Settings panel (global, works from any overlay)
    if key.code == KeyCode::F(3) && key.modifiers.contains(KeyModifiers::ALT) {
        handle_fkey_toggle(app, 3, Some(ActivePanel::LlmSettings), false);
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

    // Open tags modal from settings panel (plain 't' only — Ctrl+T opens the settings profile picker)
    if app.ui.active_panel == ActivePanel::LlmSettings
        && key.code == KeyCode::Char('t')
        && key.modifiers.is_empty()
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
        KeyCode::Char('c')
            if key.modifiers.contains(KeyModifiers::ALT)
                && !app.download.download_progress.is_empty() =>
        {
            let selected_idx = app.download.download_scroll_state.selected().unwrap_or(0);
            app.cancel_download(selected_idx);
            return;
        }
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
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
                app.add_log(
                    crate::t!("log.stopping_benchmark"),
                    crate::config::LogLevel::Info,
                );
                let _ = app
                    .pending_tx
                    .send(PendingEvent::KillHandle { handle })
                    .await;
                return;
            }
        }
        KeyCode::Tab if app.ui.global_mode == GlobalMode::Normal => {
            if app.has_toasts() {
                app.dismiss_toast();
            }
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                app.focus_prev();
            } else {
                app.focus_next();
            }
            return;
        }
        KeyCode::Char('h')
            if key.modifiers.contains(KeyModifiers::CONTROL)
                && !key.modifiers.contains(KeyModifiers::SHIFT) =>
        {
            app.ui.panel_help = !app.ui.panel_help;
            if app.ui.panel_help {
                app.ui.panel_help_offset = 0;
            }
            return;
        }
        KeyCode::F(1) => {
            handle_fkey_focus(app, 0, ActivePanel::Models);
            return;
        }
        KeyCode::F(2) => {
            handle_fkey_focus(app, 1, ActivePanel::ServerSettings);
            return;
        }
        KeyCode::F(3) => {
            handle_fkey_focus(app, 3, ActivePanel::LlmSettings);
            return;
        }
        KeyCode::F(4) if key.modifiers.contains(KeyModifiers::CONTROL) => {
            handle_fkey_toggle(app, 2, None, false);
            return;
        }
        KeyCode::F(5) if key.modifiers.contains(KeyModifiers::CONTROL) => {
            handle_fkey_toggle(app, 4, None, false);
            return;
        }
        KeyCode::F(6) if key.modifiers.contains(KeyModifiers::CONTROL) => {
            handle_fkey_toggle(app, 5, None, false);
            return;
        }
        KeyCode::F(6) => {
            handle_fkey_focus(app, 5, ActivePanel::Log);
            return;
        }
        KeyCode::F(10) if key.modifiers.contains(KeyModifiers::CONTROL) => {
            handle_fkey_show_all(app);
            app.ui.active_panel = ActivePanel::Models;
            return;
        }
        KeyCode::F(10) => {
            app.hide_all_panels();
            app.ui.active_panel = ActivePanel::Models;
            return;
        }
        KeyCode::Left if key.modifiers.contains(KeyModifiers::SHIFT) => {
            app.ui.left_pct = app.ui.left_pct.saturating_sub(1).max(20);
            return;
        }
        KeyCode::Right if key.modifiers.contains(KeyModifiers::SHIFT) => {
            app.ui.left_pct = app.ui.left_pct.saturating_add(1).min(80);
            return;
        }
        KeyCode::Up if key.modifiers.contains(KeyModifiers::SHIFT) => {
            app.ui.server_settings_height = app.ui.server_settings_height.saturating_sub(1).max(3);
            return;
        }
        KeyCode::Down if key.modifiers.contains(KeyModifiers::SHIFT) => {
            app.ui.server_settings_height = app.ui.server_settings_height.saturating_add(1).min(20);
            return;
        }
        KeyCode::Char('k')
            if key.modifiers.contains(KeyModifiers::CONTROL)
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
        KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.ui.active_panel = ActivePanel::Log;
            return;
        }
        KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            let version_param = app
                .settings
                .get_active_backend_version()
                .map(|s| s.as_str());
            let binary_path = match hub::resolve_backend_binary(
                app.settings.backend,
                version_param,
                app.settings.llama_cpp_strix_halo_rocm.as_deref(),
                None,
                None,
            )
            .await
            {
                Ok(path) => path,
                Err(e) => {
                    app.add_log(
                        format!("Failed to resolve llama-server binary: {}", e),
                        crate::config::LogLevel::Error,
                    );
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
                false,
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
            app.ui.panel_visibility &= !(1 << 1);
            app.ui.panel_visibility &= !(1 << 3);
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

    // Dismiss toast on Enter (only in Normal mode, before any overlay handling)
    if matches!(app.ui.global_mode, GlobalMode::Normal)
        && app.has_toasts()
        && key.code == KeyCode::Enter
    {
        app.dismiss_toast();
        return;
    }

    // ── Mode-specific handling ────────────────────────────────────

    // Handle search mode first (it takes priority) - unless README panel has focus (except for Enter/Esc)
    let is_search = matches!(app.models_mode, ModelsMode::Search { .. })
        && (app.ui.active_panel != ActivePanel::SearchReadme
            || key.code == KeyCode::Enter
            || key.code == KeyCode::Esc);
    if is_search {
        handle_search_key(app, key).await;
        return;
    }

    // Handle files mode - unless README panel has focus (except for Enter/Esc)
    let is_files = matches!(app.models_mode, ModelsMode::Files { .. })
        && (app.ui.active_panel != ActivePanel::SearchReadme
            || key.code == KeyCode::Enter
            || key.code == KeyCode::Esc);
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
    if key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL) {
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
            app.invalidate_list_caches();
            return;
        }
        // List mode sort
        else if let ModelsMode::List { sort_by } = &mut app.models_mode {
            *sort_by = sort_by.next();
            app.invalidate_list_caches();
        }
        app.save_current_settings();
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
                KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    let mut saved = false;
                    let mut name_conflict = false;
                    if *selected < entries.len() {
                        let name = entries[*selected].0.clone();
                        let content = edit_buffer.clone();
                        if let Some(mut preset) =
                            app.config.system_prompt_presets.get(&name).cloned()
                        {
                            preset.content = content;
                            if app.config.system_prompt_presets.save(&preset) {
                                saved = app.config.save().is_ok();
                            } else {
                                name_conflict = true;
                            }
                        }
                    }
                    *editing = false;
                    if name_conflict {
                        app.add_log(
                            crate::t!("presets.name_conflict"),
                            crate::config::LogLevel::Warning,
                        );
                    } else {
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
                        app.add_log(log_msg, log_level);
                    }
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
            KeyCode::PageUp => {
                app.picker.prompt_picker_scroll_offset =
                    app.picker.prompt_picker_scroll_offset.saturating_sub(5);
            }
            KeyCode::PageDown => {
                app.picker.prompt_picker_scroll_offset =
                    app.picker.prompt_picker_scroll_offset.saturating_add(5);
            }
            KeyCode::Enter if *selected < entries.len() => {
                let (name, _) = entries[*selected].clone();
                app.settings.system_prompt_preset_name = name.clone();
                app.resolve_system_prompt();
                app.ui.global_mode = GlobalMode::Normal;
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
                if !app.config.system_prompt_presets.save(&preset) {
                    app.add_log(
                        crate::t!("presets.name_conflict"),
                        crate::config::LogLevel::Warning,
                    );
                    return;
                }
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
        editing_field,
        field_edit_buffer,
        field_edit_cursor,
        editing_param,
        editing_param_field,
        param_edit_buffer,
        param_edit_cursor_pos,
    } = &mut app.ui.global_mode
    {
        let alt = key.modifiers.contains(KeyModifiers::ALT);

        // Field toggles (Alt+P/N/I/C) work in any mode: they switch the active
        // text field and exit range editing.
        if alt {
            let field = match key.code {
                KeyCode::Char('p') => Some(BenchTuneField::Prompt),
                KeyCode::Char('n') => Some(BenchTuneField::NPredict),
                KeyCode::Char('i') => Some(BenchTuneField::Iters),
                KeyCode::Char('c') => Some(BenchTuneField::Kwargs),
                _ => None,
            };
            if let Some(f) = field {
                *editing_param = false;
                param_edit_buffer.clear();
                *editing_field = Some(f);
                *field_edit_buffer = bench_tune_field_value(config, f);
                *field_edit_cursor = field_edit_buffer.len();
                return;
            }
        }

        let in_text = editing_field.is_some();
        let in_param = *editing_param;

        if in_text {
            // ===== Single-field text editing (prompt/kwargs/tokens/iters) =====
            // `editing_field` is guaranteed `Some` in this branch.
            let field_opt = *editing_field;
            let Some(field) = field_opt else {
                unreachable!()
            };
            let numeric = matches!(field, BenchTuneField::NPredict | BenchTuneField::Iters);
            match key.code {
                KeyCode::Char(c) => {
                    if !numeric || c.is_ascii_digit() {
                        TextEditor {
                            buffer: field_edit_buffer,
                            cursor: field_edit_cursor,
                        }
                        .insert_char(c);
                    }
                }
                KeyCode::Left | KeyCode::Up => {
                    TextEditor {
                        buffer: field_edit_buffer,
                        cursor: field_edit_cursor,
                    }
                    .move_left();
                }
                KeyCode::Right | KeyCode::Down => {
                    TextEditor {
                        buffer: field_edit_buffer,
                        cursor: field_edit_cursor,
                    }
                    .move_right();
                }
                KeyCode::Backspace => {
                    TextEditor {
                        buffer: field_edit_buffer,
                        cursor: field_edit_cursor,
                    }
                    .backspace();
                }
                KeyCode::Delete => {
                    TextEditor {
                        buffer: field_edit_buffer,
                        cursor: field_edit_cursor,
                    }
                    .delete();
                }
                KeyCode::Enter => {
                    bench_tune_commit_field(config, field, field_edit_buffer);
                    *editing_field = None;
                    field_edit_buffer.clear();
                    *field_edit_cursor = 0;
                }
                KeyCode::Esc => {
                    *editing_field = None;
                    field_edit_buffer.clear();
                    *field_edit_cursor = 0;
                }
                _ => {}
            }
        } else if in_param {
            // ===== Range editing (Min/Max/Step) for the selected param =====
            match key.code {
                KeyCode::Char(c) if "0123456789.-eE".contains(c) => {
                    TextEditor {
                        buffer: param_edit_buffer,
                        cursor: param_edit_cursor_pos,
                    }
                    .insert_char(c);
                }
                KeyCode::Tab | KeyCode::BackTab => {
                    bench_tune_commit_range_field(
                        config,
                        *selected_idx,
                        *editing_param_field,
                        param_edit_buffer,
                    );
                    *editing_param_field = if key.code == KeyCode::Tab {
                        (*editing_param_field + 1).min(2)
                    } else if *editing_param_field <= 0 {
                        2
                    } else {
                        *editing_param_field - 1
                    };
                    bench_tune_refill_range_buffer(
                        config,
                        *selected_idx,
                        *editing_param_field,
                        param_edit_buffer,
                        param_edit_cursor_pos,
                    );
                }
                KeyCode::Backspace => {
                    TextEditor {
                        buffer: param_edit_buffer,
                        cursor: param_edit_cursor_pos,
                    }
                    .backspace();
                }
                KeyCode::Delete => {
                    TextEditor {
                        buffer: param_edit_buffer,
                        cursor: param_edit_cursor_pos,
                    }
                    .delete();
                }
                KeyCode::Enter => {
                    bench_tune_commit_range_field(
                        config,
                        *selected_idx,
                        *editing_param_field,
                        param_edit_buffer,
                    );
                    bench_tune_normalize_range(config, *selected_idx);
                    *editing_param = false;
                    param_edit_buffer.clear();
                    *param_edit_cursor_pos = 0;
                }
                KeyCode::Esc => {
                    *editing_param = false;
                    param_edit_buffer.clear();
                    *param_edit_cursor_pos = 0;
                }
                _ => {}
            }
        } else {
            // ===== Navigation =====
            let spec_idx = config.spec_type_idx();
            let n = config.params_to_test.len();
            match key.code {
                KeyCode::Up | KeyCode::Char('k') | KeyCode::Backspace => {
                    *selected_idx = bench_tune_prev_idx(*selected_idx, spec_idx);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    *selected_idx = bench_tune_next_idx(n, *selected_idx, spec_idx);
                }
                // Left/Right cycle the spec-type mode selector (shown up top).
                KeyCode::Left => bench_tune_cycle_spec(config, -1),
                KeyCode::Right => bench_tune_cycle_spec(config, 1),
                KeyCode::Char(' ') => {
                    if *selected_idx < n {
                        let spec_off = config.spec_off();
                        let p = &mut config.params_to_test[*selected_idx];
                        if p.name != "draft_tokens" || !spec_off {
                            p.enabled = !p.enabled;
                        }
                    }
                }
                KeyCode::Char('e') => {
                    if *selected_idx < n {
                        let spec_off = config.spec_off();
                        let name = config.params_to_test[*selected_idx].name.clone();
                        if name != "spec_type" && !(name == "draft_tokens" && spec_off) {
                            *editing_param = true;
                            *editing_param_field = 0;
                            param_edit_buffer.clear();
                            *param_edit_cursor_pos = 0;
                        }
                    }
                }
                KeyCode::Enter => {
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
                KeyCode::Esc => app.ui.global_mode = GlobalMode::Normal,
                _ => {}
            }
        }
    }
}

/// Current value of a single bench-tune field, as an editable string.
fn bench_tune_field_value(config: &BenchTuneConfig, field: BenchTuneField) -> String {
    match field {
        BenchTuneField::Prompt => config.prompt.clone(),
        BenchTuneField::Kwargs => config.chat_template_kwargs.clone().unwrap_or_default(),
        BenchTuneField::NPredict => config.n_predict.to_string(),
        BenchTuneField::Iters => config.num_iterations.to_string(),
    }
}

/// Commit the edited buffer to the given bench-tune field.
fn bench_tune_commit_field(config: &mut BenchTuneConfig, field: BenchTuneField, buffer: &str) {
    match field {
        BenchTuneField::Prompt => config.prompt = buffer.to_string(),
        BenchTuneField::Kwargs => config.chat_template_kwargs = Some(buffer.to_string()),
        BenchTuneField::NPredict => {
            if let Ok(val) = buffer.parse::<u32>() {
                config.n_predict = val.clamp(1, 16384);
            }
        }
        BenchTuneField::Iters => {
            if let Ok(val) = buffer.parse::<u32>() {
                config.num_iterations = val.clamp(1, 100);
            }
        }
    }
}

/// Next navigable param index (skips the spec-type row), moving down.
fn bench_tune_next_idx(n: usize, cur: usize, spec_idx: Option<usize>) -> usize {
    (cur + 1..n).find(|&i| Some(i) != spec_idx).unwrap_or(cur)
}

/// Previous navigable param index (skips the spec-type row), moving up.
fn bench_tune_prev_idx(cur: usize, spec_idx: Option<usize>) -> usize {
    (0..cur).rev().find(|&i| Some(i) != spec_idx).unwrap_or(cur)
}

/// Cycle the spec-type mode selector by `dir` (+1/-1), wrapping around.
fn bench_tune_cycle_spec(config: &mut BenchTuneConfig, dir: i32) {
    let Some(i) = config.spec_type_idx() else {
        return;
    };
    let len = config.params_to_test[i].variants.len();
    if len == 0 {
        return;
    }
    let cur = config.params_to_test[i].min as usize;
    let next = if dir > 0 {
        (cur + 1) % len
    } else {
        (cur + len - 1) % len
    };
    config.params_to_test[i].min = next as f64;
    if next == 0 {
        for p in &mut config.params_to_test {
            if p.name == "draft_tokens" {
                p.enabled = false;
            }
        }
    }
}

/// Commit the current range-field buffer (Min/Max/Step) to the param.
fn bench_tune_commit_range_field(
    config: &mut BenchTuneConfig,
    idx: usize,
    field: i32,
    buffer: &str,
) {
    if let Ok(val) = buffer.parse::<f64>() {
        let p = &mut config.params_to_test[idx];
        match field {
            0 => p.min = val,
            1 => p.max = val,
            2 if val > 0.0 => p.step = val,
            _ => {}
        }
    }
}

/// Refill the range-field buffer with the param's current value for `field`.
fn bench_tune_refill_range_buffer(
    config: &BenchTuneConfig,
    idx: usize,
    field: i32,
    buffer: &mut String,
    cursor: &mut usize,
) {
    let val = match field {
        0 => config.params_to_test[idx].min,
        1 => config.params_to_test[idx].max,
        _ => config.params_to_test[idx].step,
    };
    buffer.clear();
    *buffer = if field == 2 {
        val.to_string()
    } else {
        format!("{:.2}", val)
    };
    *cursor = buffer.len();
}

/// Clamp an edited range into a sane state (min<=max, step>0).
fn bench_tune_normalize_range(config: &mut BenchTuneConfig, idx: usize) {
    let (min_val, max_val, step_val) = {
        let p = &config.params_to_test[idx];
        (p.min, p.max, p.step)
    };
    let p = &mut config.params_to_test[idx];
    if min_val > max_val {
        p.min = max_val;
    }
    if step_val <= 0.0 {
        p.step = max_val - min_val;
    }
}

async fn handle_search_key(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.models_mode = ModelsMode::List {
                sort_by: crate::models::ListSort::Name,
            };
            app.ui.panel_visibility |= (1 << 1) | (1 << 3) | (1 << 4) | (1 << 5);
            app.invalidate_list_caches();
            return;
        }
        KeyCode::Enter | KeyCode::Char('f')
            if key.modifiers.contains(KeyModifiers::CONTROL) || key.code == KeyCode::Enter =>
        {
            let (model_id, readme_present) =
                if let ModelsMode::Search { results, .. } = &app.models_mode {
                    app.search
                        .search_results_idx
                        .and_then(|idx| results.get(idx))
                        .map_or((String::new(), false), |r| {
                            (r.model_id.clone(), r.readme.is_some())
                        })
                } else {
                    (String::new(), false)
                };
            if !model_id.is_empty() {
                // Fetch README only if not already cached
                if !readme_present {
                    app.add_log(
                        crate::t_fmt!("log.fetching_readme", model_id),
                        crate::config::LogLevel::Info,
                    );
                    fetch_and_store_readme(app, model_id.clone()).await;
                }
                if let ModelsMode::Search { show_readme, .. } = &mut app.models_mode {
                    *show_readme = true;
                }
                app.ui.active_panel = ActivePanel::Models;

                app.add_log(
                    crate::t_fmt!("log.loading_files", model_id),
                    crate::config::LogLevel::Info,
                );
                match hub::list_gguf_files(&model_id).await {
                    Ok(files) => {
                        app.add_log(
                            crate::t_fmt!("log.found_gguf_files", files.len()),
                            crate::config::LogLevel::Info,
                        );
                        if let ModelsMode::Search { query, results, .. } = &app.models_mode {
                            let selected_result = app
                                .search
                                .search_results_idx
                                .and_then(|idx| results.get(idx).cloned());
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
                            crate::t_fmt!("log.no_gguf_files", e),
                            crate::config::LogLevel::Info,
                        );
                    }
                }
            }
            return;
        }
        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
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
        KeyCode::Char('B') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if let ModelsMode::Search { page, .. } = &app.models_mode
                && *page > 0
            {
                let query = if let ModelsMode::Search { query, .. } = &app.models_mode {
                    query.clone()
                } else {
                    String::new()
                };
                let offset = (*page as u32 - 1) * app.config.search_limit;
                app.add_log(
                    crate::t_fmt!("log.loading_page", *page - 1),
                    crate::config::LogLevel::Info,
                );
                if let ModelsMode::Search { page, .. } = &mut app.models_mode {
                    *page -= 1;
                }
                let _ = app
                    .pending_tx
                    .send(PendingEvent::Search { query, offset })
                    .await;
                return;
            }
            return;
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let len = app.search_results_len();
            let at_end = matches!(app.search.search_results_idx, Some(idx) if idx + 1 >= len);
            let can_load_more = matches!(
                &app.models_mode,
                ModelsMode::Search {
                    has_more, loading, ..
                } if *has_more && !*loading
            );
            if at_end
                && can_load_more
                && let ModelsMode::Search {
                    query,
                    page,
                    loading,
                    ..
                } = &mut app.models_mode
            {
                *page += 1;
                *loading = true;
                let query = query.clone();
                let offset = *page as u32 * app.config.search_limit;
                app.add_log(crate::t!("log.loading_more"), crate::config::LogLevel::Info);
                let _ = app
                    .pending_tx
                    .send(PendingEvent::Search { query, offset })
                    .await;
                return;
            }
            match app.search.search_results_idx {
                Some(idx) if idx + 1 < len => app.search.search_results_idx = Some(idx + 1),
                Some(_) => {
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
        KeyCode::Char('R') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            let model_id = if let ModelsMode::Search { results, .. } = &app.models_mode {
                app.search
                    .search_results_idx
                    .and_then(|idx| results.get(idx).map(|r| r.model_id.clone()))
            } else {
                None
            };
            if let Some(ref model_id) = model_id {
                // Explicit refresh: always re-fetch
                app.add_log(
                    crate::t_fmt!("log.fetching_readme", model_id),
                    crate::config::LogLevel::Info,
                );
                app.add_log(
                    crate::t!("log.taking_moment"),
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
        KeyCode::Right => {
            let (model_id, readme_present) =
                if let ModelsMode::Search { results, .. } = &app.models_mode {
                    app.search
                        .search_results_idx
                        .and_then(|idx| results.get(idx))
                        .map_or((String::new(), false), |r| {
                            (r.model_id.clone(), r.readme.is_some())
                        })
                } else {
                    (String::new(), false)
                };
            if !model_id.is_empty() {
                if !readme_present {
                    app.add_log(
                        crate::t_fmt!("log.fetching_readme", model_id),
                        crate::config::LogLevel::Info,
                    );
                    fetch_and_store_readme(app, model_id.clone()).await;
                }
                if let ModelsMode::Search { show_readme, .. } = &mut app.models_mode {
                    *show_readme = true;
                }
            }
            app.ui.active_panel = ActivePanel::SearchReadme;
            return;
        }
        KeyCode::Char('c')
            if key.modifiers.contains(KeyModifiers::ALT)
                && !app.download.download_progress.is_empty() =>
        {
            let selected_idx = app.download.download_scroll_state.selected().unwrap_or(0);
            app.cancel_download(selected_idx);
            return;
        }
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
            } = std::mem::replace(
                &mut app.models_mode,
                ModelsMode::List {
                    sort_by: crate::models::ListSort::Name,
                },
            ) {
                app.invalidate_list_caches();
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
                    app.add_log(
                        crate::t!("log.already_downloaded"),
                        crate::config::LogLevel::Warning,
                    );
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
        KeyCode::Char('c')
            if key.modifiers.contains(KeyModifiers::ALT)
                && !app.download.download_progress.is_empty() =>
        {
            let selected_idx = app.download.download_scroll_state.selected().unwrap_or(0);
            app.cancel_download(selected_idx);
            return;
        }
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
            // Clear the [BENCHMARK: ...] prefix: the task result is dropped on
            // cancel, so reset any stuck Benchmarking states here.
            for state in app.model_states.values_mut() {
                if matches!(state, crate::models::ModelState::Benchmarking) {
                    *state = crate::models::ModelState::Available;
                }
            }
            app.pending.active_model_hint_dirty = true;
            // Don't abort the task — let it finish gracefully and send Cancelled status
            // Keep bench_tune_running = true so the app knows the task is still finishing up
            app.models_mode = ModelsMode::List {
                sort_by: crate::models::ListSort::Name,
            };
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
        KeyCode::Up if app.bench_tune.bench_tune_result_row > 0 => {
            app.bench_tune.bench_tune_result_row -= 1;
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
                    app.ui.global_mode = GlobalMode::ApiEndpointPicker {
                        enabled: app.settings.api_endpoint_enabled,
                        port: app.settings.api_endpoint_port.to_string(),
                        api_key: app
                            .config
                            .default
                            .api_endpoint_key
                            .clone()
                            .unwrap_or_default(),
                        tls_enabled: app.config.default.server_tls_enabled,
                        tls_cert: app
                            .config
                            .default
                            .server_tls_cert
                            .clone()
                            .unwrap_or_default(),
                        tls_key: app
                            .config
                            .default
                            .server_tls_key
                            .clone()
                            .unwrap_or_default(),
                        selected_field: -1,
                        editing: false,
                        edit_buffer: String::new(),
                        edit_cursor_pos: 0,
                    };
                }
                3 if !app.settings.api_endpoint_enabled => {}
                3 => {
                    app.ui.global_mode = GlobalMode::DashboardPicker {
                        enabled: app.config.default.ws_server_enabled,
                        port: app.config.default.ws_server_port.to_string(),
                        auth_key: app
                            .config
                            .default
                            .api_endpoint_key
                            .clone()
                            .unwrap_or_default(),
                        tls_enabled: app.config.default.server_tls_enabled,
                        tls_cert: app
                            .config
                            .default
                            .server_tls_cert
                            .clone()
                            .unwrap_or_default(),
                        tls_key: app
                            .config
                            .default
                            .server_tls_key
                            .clone()
                            .unwrap_or_default(),
                        selected_field: -1,
                        editing: false,
                        edit_buffer: String::new(),
                        edit_cursor_pos: 0,
                    };
                }
                4 if !app.settings.api_endpoint_enabled => {}
                4 => {
                    app.settings.chat_ui_enabled = !app.settings.chat_ui_enabled;
                    app.add_log(
                        if app.settings.chat_ui_enabled {
                            crate::t!("async.chat_ui_enabled")
                        } else {
                            crate::t!("async.chat_ui_disabled")
                        },
                        crate::config::LogLevel::Info,
                    );
                }
                5 if !app.settings.api_endpoint_enabled => {}
                5 => {
                    app.config.default.api_transfer_enabled =
                        !app.config.default.api_transfer_enabled;
                    app.config.save().ok();
                    app.add_log(
                        if app.config.default.api_transfer_enabled {
                            crate::t!("async.transfer_api_enabled")
                        } else {
                            crate::t!("async.transfer_api_disabled")
                        },
                        crate::config::LogLevel::Info,
                    );
                }
                6 => {
                    app.ui.global_mode = GlobalMode::LlamaServerOptionsPicker {
                        port: app.settings.port.to_string(),
                        threads: app.settings.threads,
                        threads_batch: app.settings.threads_batch,
                        log_level: app.config.default.log_level.clone(),
                        webui: app.settings.webui,
                        selected_field: -1,
                        mode_picker_selected: app.server_mode as usize,
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
                8 => {
                    let engine_url = app.config.default.web_search_engine_url.clone();
                    let engine = app.config.default.web_search_engine.clone();
                    let api_key = app.config.default.web_search_api_key.clone();
                    app.ui.global_mode = GlobalMode::WebSearchPicker {
                        enabled: app.config.default.web_search_enabled,
                        engine: app.config.default.web_search_engine.clone(),
                        engine_url: app.config.default.web_search_engine_url.clone(),
                        api_key: app.config.default.web_search_api_key.clone(),
                        selected_field: 0,
                        engine_picker_selected: 0,
                        editing: false,
                        edit_buffer: String::new(),
                        edit_cursor_pos: 0,
                        check_status: None,
                    };
                    if !engine_url.is_empty() {
                        let engine = engine.clone();
                        let engine_url = engine_url.clone();
                        let api_key = api_key.clone();
                        app.ui.needs_redraw = true;
                        let handle = tokio::spawn(async move {
                            check_web_search_health(
                                &engine,
                                &engine_url,
                                api_key.as_deref().unwrap_or(""),
                            )
                            .await
                        });
                        app.pending.web_search_check_handle = Some(handle);
                    }
                }
                9 => {
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
                }
                _ => {}
            }
            sync_global_settings(app);
        }
        KeyCode::Up | KeyCode::Char('k') => {
            // Skip Dashboard / Chat UI / File Transfer while the API endpoint is off.
            let skip_api_rows = !app.settings.api_endpoint_enabled;
            let mut idx = app.settings_state.server_settings_selected_idx;
            while idx > 0 {
                idx -= 1;
                if !(skip_api_rows && (3..=5).contains(&idx)) {
                    break;
                }
            }
            app.settings_state.server_settings_selected_idx = idx;
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let skip_api_rows = !app.settings.api_endpoint_enabled;
            let mut idx = app.settings_state.server_settings_selected_idx;
            while idx < 9 {
                idx += 1;
                if !(skip_api_rows && (3..=5).contains(&idx)) {
                    break;
                }
            }
            app.settings_state.server_settings_selected_idx = idx;
        }
        KeyCode::Left | KeyCode::Char('h') => {
            match app.settings_state.server_settings_selected_idx {
                2 => app.settings.threads = app.settings.threads.saturating_sub(1).max(1),
                3 => {
                    app.settings.threads_batch = app.settings.threads_batch.saturating_sub(1).max(1)
                }
                7 => {}
                _ => {}
            }
            mark_settings_dirty(app, true);
            sync_global_settings(app);
        }
        KeyCode::Right | KeyCode::Char('l') => {
            match app.settings_state.server_settings_selected_idx {
                2 => app.settings.threads = (app.settings.threads + 1).min(app.max_threads),
                3 => app.settings.threads_batch = (app.settings.threads_batch + 1).min(64),
                7 => {}
                _ => {}
            }
            mark_settings_dirty(app, true);
            sync_global_settings(app);
        }
        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.save_current_settings();
        }
        _ => {}
    }
}
