use crossterm::event::{KeyCode, KeyModifiers};

use super::super::helpers::{mark_settings_dirty, sync_global_settings};
use crate::config::builtin_profiles;
use crate::tui::app::{App, GlobalMode};
use crate::tui::settings;

pub fn handle_settings_key(app: &mut App, key: crossterm::event::KeyEvent) {
    let idx = app.settings_state.settings_selected_idx;

    // Build the field list
    let fields = settings::filtered_fields(app.settings_state.expert_mode);
    let field = fields.get(idx);
    let field_id = field.map(|f| f.id);

    // ── Navigation (highest priority for core interaction) ───────────────────

    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            if !app.settings_state.settings_edit_buffer.is_empty() {
                app.settings_state.settings_edit_buffer.clear();
            } else {
                app.settings_state.settings_selected_idx =
                    app.settings_state.settings_selected_idx.saturating_sub(1);
            }
            mark_settings_dirty(app, false);
            return;
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if !app.settings_state.settings_edit_buffer.is_empty() {
                app.settings_state.settings_edit_buffer.clear();
            } else {
                let count = fields.len();
                app.settings_state.settings_selected_idx =
                    (app.settings_state.settings_selected_idx + 1).min(count.saturating_sub(1));
            }
            mark_settings_dirty(app, false);
            return;
        }
        KeyCode::PageDown | KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if !app.settings_state.settings_edit_buffer.is_empty() {
                app.settings_state.settings_edit_buffer.clear();
            } else {
                let count = fields.len();
                app.settings_state.settings_selected_idx =
                    (app.settings_state.settings_selected_idx + 10).min(count.saturating_sub(1));
            }
            mark_settings_dirty(app, false);
            return;
        }
        KeyCode::PageUp | KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if !app.settings_state.settings_edit_buffer.is_empty() {
                app.settings_state.settings_edit_buffer.clear();
            } else {
                app.settings_state.settings_selected_idx =
                    app.settings_state.settings_selected_idx.saturating_sub(10);
            }
            mark_settings_dirty(app, false);
            return;
        }
        KeyCode::PageDown => {
            app.settings_state.settings_scroll_offset =
                app.settings_state.settings_scroll_offset.saturating_add(5);
            app.settings_state.settings_selected_idx =
                app.settings_state.settings_selected_idx.saturating_add(5);
            let count = fields.len();
            if app.settings_state.settings_selected_idx >= count {
                app.settings_state.settings_selected_idx = count.saturating_sub(1);
            }
            mark_settings_dirty(app, false);
            return;
        }
        KeyCode::PageUp => {
            app.settings_state.settings_scroll_offset =
                app.settings_state.settings_scroll_offset.saturating_sub(5);
            app.settings_state.settings_selected_idx =
                app.settings_state.settings_selected_idx.saturating_sub(5);
            mark_settings_dirty(app, false);
            return;
        }
        _ => {}
    }

    // ── Global shortcuts ─────────────────────────────────────────────────────

    // Ctrl+S: save settings
    if key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL) {
        app.save_model_settings();
        return;
    }

    // Ctrl+R: reset settings
    if key.code == KeyCode::Char('r') && key.modifiers.contains(KeyModifiers::CONTROL) {
        if app.is_settings_dirty() {
            app.ui.global_mode = GlobalMode::Confirmation {
                selected: false,
                kind: crate::tui::app::ConfirmationKind::Reset,
            };
            return;
        } else {
            app.reset_to_defaults();
            return;
        }
    }

    // Ctrl+P: open profile picker modal
    if key.code == KeyCode::Char('p') && key.modifiers.contains(KeyModifiers::CONTROL) {
        let builtin = builtin_profiles();
        let all_profiles = app.config.merged_profiles();
        app.picker.profile_picker_entries = all_profiles
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
        app.picker.profile_picker_selected = 0;
        app.ui.global_mode = GlobalMode::ProfilePicker {
            entries: app.picker.profile_picker_entries.clone(),
            selected: app.picker.profile_picker_selected,
            profiles: all_profiles.clone(),
        };
        return;
    }

    // Ctrl+E: toggle field (use SettingField if available)
    if key.code == KeyCode::Char('e') && key.modifiers.contains(KeyModifiers::CONTROL) {
        if let Some(f) = field {
            f.ctrl_e_toggle(&mut app.settings);
        }
        mark_settings_dirty(app, true);
        sync_global_settings(app);
        return;
    }

    // ── Field-specific handlers (match on field id) ─────────────────────────

    // System Prompt: open picker modal on Enter
    if field_id == Some("system_prompt_preset_name") && key.code == KeyCode::Enter
        && app.settings_state.settings_edit_buffer.is_empty()
    {
        app.picker.prompt_picker_entries = app
            .config
            .merged_presets()
            .iter()
            .map(|p| (p.name.clone(), p.description.clone()))
            .collect();
        app.picker.prompt_picker_selected = app
            .picker
            .prompt_picker_entries
            .iter()
            .position(|(name, _)| name == &app.settings.system_prompt_preset_name)
            .unwrap_or(0);
        app.ui.global_mode = GlobalMode::PromptPicker {
            entries: app.picker.prompt_picker_entries.clone(),
            selected: app.picker.prompt_picker_selected,
            editing: false,
            edit_buffer: String::new(),
            edit_cursor_pos: 0,
            confirm_delete: false,
        };
        return;
    }

    // Keep in memory (mlock): toggle on Enter
    if field_id == Some("mlock") && key.code == KeyCode::Enter
        && app.settings_state.settings_edit_buffer.is_empty()
    {
        app.settings.mlock = !app.settings.mlock;
        mark_settings_dirty(app, true);
        return;
    }

    // GPU Layers: arrow keys cycle Auto → 1 → 2 → ... → N → All → Auto
    if field_id == Some("gpu_layers_mode") {
        if !app.settings_state.settings_edit_buffer.is_empty() {
            app.settings_state.settings_edit_buffer.clear();
        } else if key.code == KeyCode::Enter {
            match &app.settings.gpu_layers_mode {
                crate::models::GpuLayersMode::Specific(n) => {
                    app.settings_state.settings_edit_buffer = n.to_string();
                }
                _ => {
                    let total = app.loading.model_total_layers;
                    app.settings.gpu_layers_mode =
                        crate::models::GpuLayersMode::Specific(total.clamp(1, 256));
                    mark_settings_dirty(app, true);
                }
            }
            return;
        } else if key.code == KeyCode::Left {
            let total = app.loading.model_total_layers;
            app.settings.gpu_layers_mode = match &app.settings.gpu_layers_mode {
                crate::models::GpuLayersMode::Auto => {
                    crate::models::GpuLayersMode::Specific(total.clamp(1, 256))
                }
                crate::models::GpuLayersMode::Specific(0) => crate::models::GpuLayersMode::Auto,
                crate::models::GpuLayersMode::Specific(n) if *n == 1 => {
                    crate::models::GpuLayersMode::Specific(0)
                }
                crate::models::GpuLayersMode::Specific(n) => {
                    crate::models::GpuLayersMode::Specific(n - 1)
                }
                crate::models::GpuLayersMode::All => {
                    let max = total.clamp(1, 256);
                    crate::models::GpuLayersMode::Specific(max)
                }
            };
            mark_settings_dirty(app, true);
            return;
        } else if key.code == KeyCode::Right {
            let total = app.loading.model_total_layers;
            app.settings.gpu_layers_mode = match &app.settings.gpu_layers_mode {
                crate::models::GpuLayersMode::Auto => crate::models::GpuLayersMode::Specific(1),
                crate::models::GpuLayersMode::Specific(n) if *n == total => {
                    crate::models::GpuLayersMode::All
                }
                crate::models::GpuLayersMode::Specific(n) => {
                    crate::models::GpuLayersMode::Specific(n + 1)
                }
                crate::models::GpuLayersMode::All => crate::models::GpuLayersMode::Auto,
            };
            mark_settings_dirty(app, true);
            return;
        }
    }

    // Flash Attention: toggle on Enter
    if field_id == Some("flash_attn") && key.code == KeyCode::Enter
        && app.settings_state.settings_edit_buffer.is_empty()
    {
        app.settings.flash_attn = !app.settings.flash_attn;
        mark_settings_dirty(app, true);
        return;
    }

    // KV Cache Offload: toggle on Enter
    if field_id == Some("kv_cache_offload") && key.code == KeyCode::Enter
        && app.settings_state.settings_edit_buffer.is_empty()
    {
        app.settings.kv_cache_offload = !app.settings.kv_cache_offload;
        mark_settings_dirty(app, true);
        return;
    }

    // Cache Type K: cycle on Enter with empty buffer; delegate buffer/arrow to generic handler
    if field_id == Some("cache_type_k") && key.code == KeyCode::Enter
        && app.settings_state.settings_edit_buffer.is_empty()
    {
        let val = app.settings.cache_type_k.unwrap_or(crate::models::CacheTypeK::F16).next();
        app.settings.cache_type_k = Some(val);
        mark_settings_dirty(app, true);
        return;
    }

    // Cache Type V: cycle on Enter with empty buffer; delegate buffer/arrow to generic handler
    if field_id == Some("cache_type_v") && key.code == KeyCode::Enter
        && app.settings_state.settings_edit_buffer.is_empty()
    {
        let val = app.settings.cache_type_v.unwrap_or(crate::models::CacheTypeV::F16).next();
        app.settings.cache_type_v = Some(val);
        mark_settings_dirty(app, true);
        return;
    }

    // Unified KV: toggle on Enter
    if field_id == Some("uniform_cache") && key.code == KeyCode::Enter
        && app.settings_state.settings_edit_buffer.is_empty()
    {
        app.settings.uniform_cache = !app.settings.uniform_cache;
        mark_settings_dirty(app, true);
        return;
    }

    // Max Concurrent Pred: Enter with empty buffer opens picker modal
    if field_id == Some("max_concurrent_predictions") && key.code == KeyCode::Enter
        && app.settings_state.settings_edit_buffer.is_empty()
    {
        let current = app
            .settings
            .max_concurrent_predictions
            .map(|v| v.to_string())
            .unwrap_or_else(|| "1".to_string());
        app.ui.global_mode = GlobalMode::MaxConcurrentPicker { value: current };
        mark_settings_dirty(app, false);
        return;
    }

    // Tags: open tags modal on Enter
    if field_id == Some("tags") && key.code == KeyCode::Enter
        && app.settings_state.settings_edit_buffer.is_empty()
    {
        app.edit.tags_editing = true;
        app.edit.tags_insert_mode = true;
        app.edit.tags_edit_buffer = String::new();
        app.edit.tags_selected_idx = None;
        mark_settings_dirty(app, false);
        return;
    }

    // Yarn RoPE: toggle on Enter
    if field_id == Some("rope_yarn_enabled") && key.code == KeyCode::Enter
        && app.settings_state.settings_edit_buffer.is_empty()
    {
        app.settings.rope_yarn_enabled = !app.settings.rope_yarn_enabled;
        mark_settings_dirty(app, false);
        return;
    }

    // Yarn Params: open modal on Enter
    if field_id == Some("yarn_params") && key.code == KeyCode::Enter
        && app.settings_state.settings_edit_buffer.is_empty()
    {
        app.ui.global_mode = GlobalMode::YarnRoPESettings {
            scale: format!("{:.2}", app.settings.rope_scale),
            freq_base: format!("{:.2}", app.settings.rope_freq_base),
            freq_scale: format!("{:.2}", app.settings.rope_freq_scale),
            selected_field: -1,
            editing: false,
            edit_buffer: String::new(),
            edit_cursor_pos: 0,
        };
        mark_settings_dirty(app, false);
        return;
    }

    // Spec type: open picker on Enter
    if field_id == Some("is_mtp") && key.code == KeyCode::Enter
        && app.settings_state.settings_edit_buffer.is_empty()
    {
        let entries = vec![
            "Off".to_string(),
            "draft-mtp".to_string(),
            "draft-simple".to_string(),
            "draft-eagle3".to_string(),
            "ngram-simple".to_string(),
            "ngram-map-k".to_string(),
            "ngram-map-k4v".to_string(),
            "ngram-mod".to_string(),
            "ngram-cache".to_string(),
        ];
        let spec_type = app.settings.spec_type.clone();
        let selected = entries.iter().position(|e| e == &spec_type).unwrap_or(0);
        app.ui.global_mode = GlobalMode::SpecTypePicker { entries, selected };
        mark_settings_dirty(app, false);
        return;
    }

    // ── Navigation & general edit handlers ──────────────────────────────────

    match key.code {
        KeyCode::Left | KeyCode::Backspace => {
            if !app.settings_state.settings_edit_buffer.is_empty() {
                app.settings_state.settings_edit_buffer.pop();
            } else if let Some(f) = field {
                f.adjust(&mut app.settings, -1, app.loading.model_n_ctx_train);
                if field_id == Some("max_concurrent_predictions") {
                    sync_global_settings(app);
                }
                mark_settings_dirty(app, true);
            }
            mark_settings_dirty(app, false);
        }
        KeyCode::Right => {
            if let Some(f) = field {
                f.adjust(&mut app.settings, 1, app.loading.model_n_ctx_train);
            }
            if field_id == Some("max_concurrent_predictions") {
                sync_global_settings(app);
            }
            mark_settings_dirty(app, true);
        }
        KeyCode::Char(c @ '0'..='9') => {
            app.settings_state.settings_edit_buffer.push(c);
        }
        KeyCode::Char('-') => {
            app.settings_state.settings_edit_buffer.push('-');
        }
        KeyCode::Char('.') => {
            if !app.settings_state.settings_edit_buffer.contains('.') {
                app.settings_state.settings_edit_buffer.push('.');
            }
        }
        KeyCode::Enter => {
            if !app.settings_state.settings_edit_buffer.is_empty() {
                if let Some(f) = field {
                    f.apply_edit(&mut app.settings, &app.settings_state.settings_edit_buffer);
                }
                if field_id == Some("max_concurrent_predictions") {
                    sync_global_settings(app);
                }
                app.settings_state.settings_edit_buffer.clear();
                mark_settings_dirty(app, true);
            }
            mark_settings_dirty(app, false);
        }
        KeyCode::Esc => {
            if !app.settings_state.settings_edit_buffer.is_empty() {
                app.settings_state.settings_edit_buffer.clear();
                mark_settings_dirty(app, true);
            }
        }
        _ => {}
    }
}
