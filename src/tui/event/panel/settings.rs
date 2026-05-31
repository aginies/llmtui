use crossterm::event::{KeyCode, KeyModifiers};

use crate::config::builtin_profiles;
use crate::tui::app::{App, GlobalMode};
use crate::tui::settings;
use super::super::helpers::sync_global_settings;

pub fn handle_settings_key(app: &mut App, key: crossterm::event::KeyEvent) {
    let idx = app.settings_state.settings_selected_idx;

    // Build the field list
    let fields = if app.settings_state.expert_mode {
        settings::standard_fields().into_iter().chain(settings::expert_fields()).collect::<Vec<_>>()
    } else {
        settings::standard_fields()
    };
    let field = fields.get(idx);

    // Global settings shortcuts (highest priority)
    if key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL) {
        app.save_model_settings();
        return;
    }

    // Reset settings to defaults via confirmation dialog (highest priority when dirty)
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
        app.update_vram_estimate();
        sync_global_settings(app);
        app.settings_state.settings_render_cache = None;
        return;
    }

    match key.code {
        // Max Concurrent Pred: Enter opens picker modal
        _ if idx == 11 && key.code == KeyCode::Enter => {
            if app.settings_state.settings_edit_buffer.is_empty() {
                let current = app.settings.max_concurrent_predictions
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "1".to_string());
                app.ui.global_mode = GlobalMode::MaxConcurrentPicker {
                    value: current,
                };
                app.settings_state.settings_render_cache = None;
            } else {
                app.settings_state.settings_edit_buffer.clear();
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if !app.settings_state.settings_edit_buffer.is_empty() {
                app.settings_state.settings_edit_buffer.clear();
            } else {
                app.settings_state.settings_selected_idx = app.settings_state.settings_selected_idx.saturating_sub(1);
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if !app.settings_state.settings_edit_buffer.is_empty() {
                app.settings_state.settings_edit_buffer.clear();
            } else {
                let count = fields.len();
                app.settings_state.settings_selected_idx = (app.settings_state.settings_selected_idx + 1).min(count.saturating_sub(1));
            }
        }
        // Spec type toggle: toggle on Enter
        _ if idx == 26 => {
            if !app.settings_state.settings_edit_buffer.is_empty() {
                app.settings_state.settings_edit_buffer.clear();
            } else if key.code == KeyCode::Enter {
                app.settings.is_mtp = !app.settings.is_mtp;
                app.update_vram_estimate();
                app.settings_state.settings_render_cache = None;
            }
        }
        // System Prompt: open picker modal on Enter
        _ if idx == 0 => {
            if !app.settings_state.settings_edit_buffer.is_empty() {
                app.settings_state.settings_edit_buffer.clear();
            } else if key.code == KeyCode::Enter {
                app.picker.prompt_picker_entries = app.config.merged_presets()
                    .iter()
                    .map(|p| (p.name.clone(), p.description.clone()))
                    .collect();
                app.picker.prompt_picker_selected = app.picker.prompt_picker_entries.iter()
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
            }
        }
        // Keep in memory (mlock): toggle on Enter
        _ if idx == 2 => {
            if !app.settings_state.settings_edit_buffer.is_empty() {
                app.settings_state.settings_edit_buffer.clear();
            } else if key.code == KeyCode::Enter {
                app.settings.mlock = !app.settings.mlock;
                app.update_vram_estimate();
                app.settings_state.settings_render_cache = None;
            }
        }
        // GPU Layers: arrow keys cycle Auto → 1 → 2 → ... → N → All → Auto
        _ if idx == 3 => {
            if !app.settings_state.settings_edit_buffer.is_empty() {
                app.settings_state.settings_edit_buffer.clear();
            } else if key.code == KeyCode::Enter {
                // Enter from specific number: open picker; from Auto/All: cycle to max
                match &app.settings.gpu_layers_mode {
                    crate::models::GpuLayersMode::Specific(n) => {
                        app.settings_state.settings_edit_buffer = n.to_string();
                    }
                    _ => {
                        let total = app.loading.model_total_layers;
                        app.settings.gpu_layers_mode = crate::models::GpuLayersMode::Specific(total.max(1).min(256));
                        app.update_vram_estimate();
                        app.settings_state.settings_render_cache = None;
                    }
                }
            } else if key.code == KeyCode::Left {
                let total = app.loading.model_total_layers;
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
                app.settings_state.settings_render_cache = None;
            } else if key.code == KeyCode::Right {
                let total = app.loading.model_total_layers;
                app.settings.gpu_layers_mode = match &app.settings.gpu_layers_mode {
                    crate::models::GpuLayersMode::Auto => crate::models::GpuLayersMode::Specific(1),
                    crate::models::GpuLayersMode::Specific(n) if *n == total => crate::models::GpuLayersMode::All,
                    crate::models::GpuLayersMode::Specific(n) => crate::models::GpuLayersMode::Specific(n + 1),
                    crate::models::GpuLayersMode::All => crate::models::GpuLayersMode::Auto,
                };
                app.update_vram_estimate();
                app.settings_state.settings_render_cache = None;
            }
        }
        // Flash Attention: toggle on Enter
        _ if idx == 4 => {
            if !app.settings_state.settings_edit_buffer.is_empty() {
                app.settings_state.settings_edit_buffer.clear();
            } else if key.code == KeyCode::Enter {
                app.settings.flash_attn = !app.settings.flash_attn;
                app.update_vram_estimate();
                app.settings_state.settings_render_cache = None;
            }
        }
        // KV Cache Offload: toggle on Enter
        _ if idx == 5 => {
            if !app.settings_state.settings_edit_buffer.is_empty() {
                app.settings_state.settings_edit_buffer.clear();
            } else if key.code == KeyCode::Enter {
                app.settings.kv_cache_offload = !app.settings.kv_cache_offload;
                app.update_vram_estimate();
                app.settings_state.settings_render_cache = None;
            }
        }
        // Cache Type K: cycle on Enter, or apply typed number
        _ if idx == 6 => {
            if !app.settings_state.settings_edit_buffer.is_empty() {
                if key.code == KeyCode::Enter {
                    if let Ok(n) = app.settings_state.settings_edit_buffer.parse::<u8>() {
                        app.settings.cache_type_k = Some(crate::models::CacheTypeK::from_u8(n));
                        app.update_vram_estimate();
                    }
                    app.settings_state.settings_edit_buffer.clear();
                    app.settings_state.settings_render_cache = None;
                } else {
                    app.settings_state.settings_edit_buffer.clear();
                }
            } else if key.code == KeyCode::Enter {
                let mut val = app.settings.cache_type_k.unwrap_or(crate::models::CacheTypeK::F16);
                val = val.next();
                app.settings.cache_type_k = Some(val);
                app.update_vram_estimate();
                app.settings_state.settings_render_cache = None;
            }
        }
        // Cache Type V: cycle on Enter, or apply typed number
        _ if idx == 7 => {
            if !app.settings_state.settings_edit_buffer.is_empty() {
                if key.code == KeyCode::Enter {
                    if let Ok(n) = app.settings_state.settings_edit_buffer.parse::<u8>() {
                        app.settings.cache_type_v = Some(crate::models::CacheTypeV::from_u8(n));
                        app.update_vram_estimate();
                    }
                    app.settings_state.settings_edit_buffer.clear();
                    app.settings_state.settings_render_cache = None;
                } else {
                    app.settings_state.settings_edit_buffer.clear();
                }
            } else if key.code == KeyCode::Enter {
                let mut val = app.settings.cache_type_v.unwrap_or(crate::models::CacheTypeV::F16);
                val = val.next();
                app.settings.cache_type_v = Some(val);
                app.update_vram_estimate();
                app.settings_state.settings_render_cache = None;
            }
        }
        // Unified KV: toggle on Enter
        _ if idx == 10 => {
            if !app.settings_state.settings_edit_buffer.is_empty() {
                app.settings_state.settings_edit_buffer.clear();
            } else if key.code == KeyCode::Enter {
                app.settings.uniform_cache = !app.settings.uniform_cache;
                app.update_vram_estimate();
                app.settings_state.settings_render_cache = None;
            }
        }
        // Tags: open tags modal on Enter
        _ if idx == 22 => {
            if !app.settings_state.settings_edit_buffer.is_empty() {
                app.settings_state.settings_edit_buffer.clear();
            } else if key.code == KeyCode::Enter {
                // Open tags modal
                app.edit.tags_editing = true;
                app.edit.tags_insert_mode = true;
                app.edit.tags_edit_buffer = String::new();
                app.edit.tags_selected_idx = None;
                app.settings_state.settings_render_cache = None;
            }
        }
        // LLama.cpp Version: cycle on Enter, or open picker
        _ if idx == 23 => {
            if !app.settings_state.settings_edit_buffer.is_empty() {
                app.settings_state.settings_edit_buffer.clear();
            } else if key.code == KeyCode::Enter {
                // Open version picker for the current backend (not implemented)
            }
        }
        // Yarn RoPE: toggle on Enter
        _ if idx == 24 => {
            if !app.settings_state.settings_edit_buffer.is_empty() {
                app.settings_state.settings_edit_buffer.clear();
            } else if key.code == KeyCode::Enter {
                app.settings.rope_yarn_enabled = !app.settings.rope_yarn_enabled;
                app.settings_state.settings_render_cache = None;
            }
        }
        // Yarn Params: open modal on Enter
        _ if idx == 25 => {
            if !app.settings_state.settings_edit_buffer.is_empty() {
                app.settings_state.settings_edit_buffer.clear();
            } else if key.code == KeyCode::Enter {
                app.ui.global_mode = GlobalMode::YarnRoPESettings {
                    scale: format!("{:.2}", app.settings.rope_scale),
                    freq_base: format!("{:.2}", app.settings.rope_freq_base),
                    freq_scale: format!("{:.2}", app.settings.rope_freq_scale),
                    selected_field: -1,
                    editing: false,
                    edit_buffer: String::new(),
                    edit_cursor_pos: 0,
                };
                app.settings_state.settings_render_cache = None;
            }
        }
        KeyCode::PageDown | KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if !app.settings_state.settings_edit_buffer.is_empty() {
                app.settings_state.settings_edit_buffer.clear();
            } else {
                let count = fields.len();
                app.settings_state.settings_selected_idx = (app.settings_state.settings_selected_idx + 10).min(count.saturating_sub(1));
            }
        }
        KeyCode::PageUp | KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if !app.settings_state.settings_edit_buffer.is_empty() {
                app.settings_state.settings_edit_buffer.clear();
            } else {
                app.settings_state.settings_selected_idx = app.settings_state.settings_selected_idx.saturating_sub(10);
            }
        }
        KeyCode::PageDown => {
            app.settings_state.settings_scroll_offset = app.settings_state.settings_scroll_offset.saturating_add(5);
            app.settings_state.settings_selected_idx = app.settings_state.settings_selected_idx.saturating_add(5);
        }
        KeyCode::PageUp => {
            app.settings_state.settings_scroll_offset = app.settings_state.settings_scroll_offset.saturating_sub(5);
            app.settings_state.settings_selected_idx = app.settings_state.settings_selected_idx.saturating_sub(5);
        }
        KeyCode::Left | KeyCode::Backspace => {
            if !app.settings_state.settings_edit_buffer.is_empty() {
                app.settings_state.settings_edit_buffer.pop();
            } else if let Some(f) = field {
                f.adjust(&mut app.settings, -1, app.loading.model_n_ctx_train);
                if idx == 11 {
                    sync_global_settings(app);
                }
                app.update_vram_estimate();
            }
            app.settings_state.settings_render_cache = None;
        }
        KeyCode::Right => {
            if let Some(f) = field {
                f.adjust(&mut app.settings, 1, app.loading.model_n_ctx_train);
            }
            if idx == 11 {
                sync_global_settings(app);
            }
            app.update_vram_estimate();
            app.settings_state.settings_render_cache = None;
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
                if idx == 11 {
                    sync_global_settings(app);
                }
                app.settings_state.settings_edit_buffer.clear();
                app.update_vram_estimate();
            }
            app.settings_state.settings_render_cache = None;
        }
        KeyCode::Esc => {
            if !app.settings_state.settings_edit_buffer.is_empty() {
                app.settings_state.settings_edit_buffer.clear();
                app.update_vram_estimate();
            }
        }
        _ => {}
    }
}
