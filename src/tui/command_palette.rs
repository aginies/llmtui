//! Command palette: a fuzzy action finder for the TUI.
//!
//! Provides a registry of the actions available in the current app state
//! (panel focus, overlays, common toggles), a dependency-free fuzzy
//! subsequence matcher, and the execution of the selected action.
//!
//! Opened with Ctrl+Space (see `event/key.rs`). The overlay state lives in
//! `GlobalMode::CommandPalette`, its key handling in
//! `event/overlay/command_palette.rs` and its rendering in
//! `render/overlays.rs`.

use crate::backend::hub;
use crate::models::SearchSort;
use crate::tui::app::pending_events::PendingEvent;
use crate::tui::app::{ActivePanel, App, ConfirmationKind, GlobalMode, ModelsMode};
use crate::tui::settings;

/// Identifies an action that can be executed from the command palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteAction {
    // Panel focus
    FocusModels,
    FocusServerSettings,
    FocusLlmSettings,
    FocusLog,
    FocusProfiles,
    FocusSystemPrompts,
    FocusDownloads,
    // Mode switches / overlays
    SearchModels,
    About,
    RpcWorkers,
    Help,
    CommandLine,
    DashboardUrl,
    ProfilePicker,
    CycleLanguage,
    Onboarding,
    WebSearchSettings,
    GgufNaming,
    // Toggles / commands
    ToggleExpert,
    SaveSettings,
    CycleSort,
    KillServer,
    Exit,
}

/// One entry in the palette's action list.
pub struct PaletteEntry {
    pub action: PaletteAction,
    /// i18n key for the display name.
    pub name_key: &'static str,
    /// Shortcut label, for display only (never parsed).
    pub shortcut: &'static str,
    /// Extra lowercase search terms to improve discoverability.
    pub keywords: &'static str,
}

/// The filename that Ctrl+G / the palette would explain, based on the
/// current mode and selection.
pub fn selected_filename(app: &App) -> Option<String> {
    match &app.models_mode {
        ModelsMode::List { .. } => app.selected_model().map(|m| m.display_name.clone()),
        ModelsMode::Search { results, .. } => app
            .search
            .search_results_idx
            .and_then(|idx| results.get(idx))
            .map(|r| r.model_id.clone()),
        ModelsMode::Files {
            files,
            selected_idx,
            ..
        } => selected_idx
            .and_then(|idx| files.get(idx))
            .map(|(f, _, _)| f.clone()),
        ModelsMode::BenchTune => None,
        ModelsMode::Empty => None,
    }
}

/// Build the list of actions available in the current app state.
///
/// Some entries are conditional: "Kill llama-server" only while a server is
/// running, "Explain GGUF filename" only when a filename is selected,
/// "Focus Downloads panel" only while a download is in progress, and
/// "Cycle sort order" only in list/search mode.
pub fn available_actions(app: &App) -> Vec<PaletteEntry> {
    let mut entries = vec![
        PaletteEntry {
            action: PaletteAction::FocusModels,
            name_key: "palette.action.focus_models",
            shortcut: "F1",
            keywords: "models list local gguf files",
        },
        PaletteEntry {
            action: PaletteAction::FocusServerSettings,
            name_key: "palette.action.focus_server_settings",
            shortcut: "F2",
            keywords: "server llama settings config",
        },
        PaletteEntry {
            action: PaletteAction::FocusLlmSettings,
            name_key: "palette.action.focus_llm_settings",
            shortcut: "F3",
            keywords: "llm inference parameters context",
        },
        PaletteEntry {
            action: PaletteAction::FocusLog,
            name_key: "palette.action.focus_log",
            shortcut: "F6",
            keywords: "log output server console",
        },
        PaletteEntry {
            action: PaletteAction::FocusProfiles,
            name_key: "palette.action.focus_profiles",
            shortcut: "Tab",
            keywords: "profiles settings saved",
        },
        PaletteEntry {
            action: PaletteAction::FocusSystemPrompts,
            name_key: "palette.action.focus_system_prompts",
            shortcut: "Tab",
            keywords: "presets system prompt templates",
        },
        PaletteEntry {
            action: PaletteAction::SearchModels,
            name_key: "palette.action.search",
            shortcut: "/",
            keywords: "huggingface hub find download",
        },
        PaletteEntry {
            action: PaletteAction::About,
            name_key: "palette.action.about",
            shortcut: "Ctrl+A",
            keywords: "about version gpl license",
        },
        PaletteEntry {
            action: PaletteAction::RpcWorkers,
            name_key: "palette.action.rpc_workers",
            shortcut: "",
            keywords: "rpc workers manage server",
        },
        PaletteEntry {
            action: PaletteAction::Help,
            name_key: "palette.action.help",
            shortcut: "Ctrl+H",
            keywords: "help panel shortcuts keys",
        },
        PaletteEntry {
            action: PaletteAction::CommandLine,
            name_key: "palette.action.command_line",
            shortcut: "Ctrl+K",
            keywords: "command line preview server args",
        },
        PaletteEntry {
            action: PaletteAction::DashboardUrl,
            name_key: "palette.action.dashboard_url",
            shortcut: "Ctrl+U",
            keywords: "dashboard url browser web",
        },
        PaletteEntry {
            action: PaletteAction::ProfilePicker,
            name_key: "palette.action.profile_picker",
            shortcut: "Ctrl+P",
            keywords: "profile picker apply settings preset",
        },
        PaletteEntry {
            action: PaletteAction::CycleLanguage,
            name_key: "palette.action.cycle_language",
            shortcut: "Ctrl+L",
            keywords: "language locale en fr it translate",
        },
        PaletteEntry {
            action: PaletteAction::Onboarding,
            name_key: "palette.action.onboarding",
            shortcut: "Ctrl+O",
            keywords: "onboarding wizard first run setup tour",
        },
        PaletteEntry {
            action: PaletteAction::WebSearchSettings,
            name_key: "palette.action.web_search",
            shortcut: "",
            keywords: "web search engine searxng api key",
        },
        PaletteEntry {
            action: PaletteAction::ToggleExpert,
            name_key: "palette.action.toggle_expert",
            shortcut: "Ctrl+X",
            keywords: "expert mode advanced fields",
        },
        PaletteEntry {
            action: PaletteAction::SaveSettings,
            name_key: "palette.action.save_settings",
            shortcut: "Ctrl+S",
            keywords: "save settings apply config",
        },
        PaletteEntry {
            action: PaletteAction::Exit,
            name_key: "palette.action.exit",
            shortcut: "Ctrl+C",
            keywords: "exit quit close app",
        },
    ];
    if app.download.downloading {
        entries.push(PaletteEntry {
            action: PaletteAction::FocusDownloads,
            name_key: "palette.action.focus_downloads",
            shortcut: "Tab",
            keywords: "downloads progress transfer",
        });
    }
    if selected_filename(app).is_some() {
        entries.push(PaletteEntry {
            action: PaletteAction::GgufNaming,
            name_key: "palette.action.gguf_naming",
            shortcut: "Ctrl+G",
            keywords: "gguf filename explain quantization",
        });
    }
    if let ModelsMode::List { .. } | ModelsMode::Search { .. } = app.models_mode {
        entries.push(PaletteEntry {
            action: PaletteAction::CycleSort,
            name_key: "palette.action.cycle_sort",
            shortcut: "Ctrl+S",
            keywords: "sort order relevance downloads likes",
        });
    }
    if app.server.server_handle.is_some() {
        entries.push(PaletteEntry {
            action: PaletteAction::KillServer,
            name_key: "palette.action.kill_server",
            shortcut: "Ctrl+Alt+K",
            keywords: "kill stop server llama process",
        });
    }
    entries
}

/// Fuzzy subsequence match of `query` against `text` (case-insensitive,
/// whitespace in the query ignored).
///
/// Returns `None` if `query` is not a subsequence of `text`, otherwise a
/// score where higher is better: word-start matches and consecutive runs
/// score more, gaps between matches score less.
pub fn fuzzy_score(query: &str, text: &str) -> Option<u32> {
    let q: Vec<char> = query
        .chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(|c| c.to_lowercase())
        .collect();
    if q.is_empty() {
        return Some(0);
    }
    let t: Vec<char> = text.chars().flat_map(|c| c.to_lowercase()).collect();
    let mut score: i32 = 0;
    let mut ti = 0usize;
    let mut prev: Option<usize> = None;
    for &qc in &q {
        let mut found = None;
        while ti < t.len() {
            if t[ti] == qc {
                found = Some(ti);
                break;
            }
            ti += 1;
        }
        let pos = found?;
        let word_start = pos == 0 || matches!(t[pos - 1], ' ' | '-' | '_' | '(' | '/' | '.');
        let mut gain = 1;
        if word_start {
            gain += 4;
        }
        if prev == Some(pos.saturating_sub(1)) {
            gain += 3;
        }
        score += gain;
        if let Some(p) = prev {
            let gap = pos.saturating_sub(p + 1);
            score -= (gap * 2).min(8) as i32;
        }
        prev = Some(pos);
        ti = pos + 1;
    }
    Some(score.max(0) as u32)
}

/// Return the indices into `entries` that match `query`, best score first.
///
/// An empty query returns all entries in registry order. Ties are broken by
/// shorter display name, then registry order.
pub fn filter_entries(entries: &[PaletteEntry], query: &str) -> Vec<usize> {
    let mut scored: Vec<(usize, u32, usize)> = Vec::new();
    for (i, e) in entries.iter().enumerate() {
        let name = crate::tui::i18n::t(e.name_key);
        let haystack = format!("{} {}", name, e.keywords);
        if let Some(score) = fuzzy_score(query, &haystack) {
            scored.push((i, score, name.len()));
        }
    }
    scored.sort_by(|a, b| b.1.cmp(&a.1).then(a.2.cmp(&b.2)).then(a.0.cmp(&b.0)));
    scored.into_iter().map(|(i, _, _)| i).collect()
}

/// Execute a palette action. The palette itself is closed first; the action
/// may open another overlay.
pub async fn execute_action(app: &mut App, action: PaletteAction) {
    app.ui.global_mode = GlobalMode::Normal;
    match action {
        PaletteAction::FocusModels => {
            crate::tui::event::helpers::handle_fkey_focus(app, 0, ActivePanel::Models);
        }
        PaletteAction::FocusServerSettings => {
            crate::tui::event::helpers::handle_fkey_focus(app, 1, ActivePanel::ServerSettings);
        }
        PaletteAction::FocusLlmSettings => {
            crate::tui::event::helpers::handle_fkey_focus(app, 3, ActivePanel::LlmSettings);
        }
        PaletteAction::FocusLog => {
            crate::tui::event::helpers::handle_fkey_focus(app, 5, ActivePanel::Log);
        }
        PaletteAction::FocusProfiles => {
            app.ui.active_panel = ActivePanel::Profiles;
        }
        PaletteAction::FocusSystemPrompts => {
            app.ui.active_panel = ActivePanel::SystemPromptPresets;
        }
        PaletteAction::FocusDownloads => {
            app.ui.active_panel = ActivePanel::Downloads;
        }
        PaletteAction::SearchModels => {
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
        }
        PaletteAction::About => {
            app.ui.global_mode = GlobalMode::About;
        }
        PaletteAction::RpcWorkers => {
            app.ui.global_mode = GlobalMode::RpcManager;
            app.picker.rpc_workers_selected_idx = 0;
            app.picker.editing_rpc_worker = None;
        }
        PaletteAction::Help => {
            app.ui.panel_help = true;
            app.ui.panel_help_offset = 0;
        }
        PaletteAction::CommandLine => {
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
        }
        PaletteAction::DashboardUrl => {
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
        }
        PaletteAction::ProfilePicker => {
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
        }
        PaletteAction::CycleLanguage => {
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
        PaletteAction::Onboarding => {
            app.config.onboarding_complete = false;
            app.config.save().ok();
            app.ui.global_mode = GlobalMode::Onboarding { step: 0 };
        }
        PaletteAction::WebSearchSettings => {
            let engine_url = app.config.default.web_search_engine_url.clone();
            let engine = app.config.default.web_search_engine.clone();
            let api_key = app.config.default.web_search_api_key.clone();
            app.ui.global_mode = GlobalMode::WebSearchPicker {
                enabled: app.config.default.web_search_enabled,
                engine: engine.clone(),
                engine_url: engine_url.clone(),
                api_key,
                selected_field: 0,
                engine_picker_selected: 0,
                editing: false,
                edit_buffer: String::new(),
                edit_cursor_pos: 0,
                check_status: None,
            };
            if !engine_url.is_empty() {
                app.ui.needs_redraw = true;
                let handle = tokio::spawn(async move {
                    crate::tui::event::check_web_search_health(&engine, &engine_url, "").await
                });
                app.pending.web_search_check_handle = Some(handle);
            }
        }
        PaletteAction::GgufNaming => {
            if let Some(fn_name) = selected_filename(app) {
                let explanation = crate::tui::gguf_naming::get_explanation(
                    &fn_name,
                    &mut app.search.gguf_naming_cache,
                );
                app.ui.global_mode = GlobalMode::GgufNaming {
                    explanation,
                    filename: fn_name,
                };
            }
        }
        PaletteAction::ToggleExpert => {
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
            let new_fields =
                settings::filtered_fields(app.settings_state.expert_mode, &app.settings);
            if app.settings_state.settings_selected_idx >= new_fields.len() {
                app.settings_state.settings_selected_idx = new_fields.len().saturating_sub(1);
            }
            crate::tui::event::helpers::mark_settings_dirty(app, false);
        }
        PaletteAction::SaveSettings => {
            app.save_current_settings();
        }
        PaletteAction::CycleSort => {
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
                app.invalidate_list_caches();
            } else if let ModelsMode::List { sort_by } = &mut app.models_mode {
                *sort_by = sort_by.next();
                app.invalidate_list_caches();
            }
        }
        PaletteAction::KillServer => {
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
        }
        PaletteAction::Exit => {
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_subsequence_matches() {
        assert!(fuzzy_score("help", "Show panel help").is_some());
        assert!(fuzzy_score("hlp", "Show panel help").is_some());
        assert!(fuzzy_score("HELP", "Show panel help").is_some());
        assert!(fuzzy_score("h e l p", "Show panel help").is_some());
    }

    #[test]
    fn non_subsequence_fails() {
        assert_eq!(fuzzy_score("xyz", "Show panel help"), None);
        // Order matters: "ph" is not a subsequence of "help".
        assert_eq!(fuzzy_score("ph", "help"), None);
    }

    #[test]
    fn empty_query_matches_all() {
        assert_eq!(fuzzy_score("", "anything"), Some(0));
        assert_eq!(fuzzy_score("   ", "anything"), Some(0));
    }

    #[test]
    fn word_start_and_runs_score_higher() {
        // "kill" at a word start beats "kill" mid-word ("skill").
        let word_start = fuzzy_score("kill", "Kill llama-server").unwrap();
        let mid_word = fuzzy_score("kill", "Unskill the model").unwrap();
        assert!(word_start > mid_word);
        // Consecutive runs beat scattered matches.
        let run = fuzzy_score("sort", "sort order").unwrap();
        let scattered = fuzzy_score("sort", "s o r t").unwrap();
        assert!(run > scattered);
    }

    #[test]
    fn filter_orders_by_score_and_keeps_all_on_empty_query() {
        let entries = vec![
            PaletteEntry {
                action: PaletteAction::FocusLog,
                name_key: "palette.action.focus_log",
                shortcut: "",
                keywords: "",
            },
            PaletteEntry {
                action: PaletteAction::FocusModels,
                name_key: "palette.action.focus_models",
                shortcut: "",
                keywords: "",
            },
        ];
        let all = filter_entries(&entries, "");
        assert_eq!(all, vec![0, 1]);
        // "log" matches the log entry via its name...
        let m = filter_entries(&entries, "log");
        assert!(m.contains(&0));
        // ...and the models entry only via its name tail ("models" has no "log").
        assert!(!m.contains(&1));
    }
}
