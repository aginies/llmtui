use crossterm::event::{KeyCode, KeyModifiers};

use crate::config::builtin_profiles;
use crate::models::ModelSettings;
use crate::tui::app::{App, GlobalMode};
use super::super::helpers::sync_global_settings;

// Settings field indices for navigation and editing
// Loading: 0: Prompt, 1: Context, 2: Keep in memory (mlock)
// GPU: 3: GPU Layers, 4: Flash Attention, 5: KV Cache Offload, 6: Cache Type K, 7: Cache Type V, 8: Active Experts
// Evaluation: 9: Eval Batch, 10: Unified KV, 11: Max Concurrent Pred
// Sampling: 12: Seed, 13: Temp, 14: Top-k, 15: Top-p, 16: Min P, 17: Max Tokens
// Repetition: 18: Rep. Penalty, 19: Rep. Last N, 20: Presence, 21: Frequency
// Tags: 22, Backend: 23
// Yarn RoPE: 24: Yarn RoPE, 25: Yarn Params
// MTP: 26: Enable MTP, 27: Draft Tokens
// Total: 28 fields (27 editable)

pub fn apply_numeric_setting(settings: &mut ModelSettings, idx: usize, buf: &str, _max_threads: u32, max_context: u32) {
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
        27 => { if let Ok(v) = buf.parse::<u32>() { settings.draft_tokens = v.min(16); } }
        _ => {}
    }
}

pub fn adjust_setting(settings: &mut ModelSettings, idx: usize, delta: i32, _max_threads: u32, max_context: u32, total_layers: u32) {
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
        24 => settings.rope_yarn_enabled = !settings.rope_yarn_enabled,
        25 => {
            // Yarn Params: no direct adjust, handled via modal
        }
        26 => settings.is_mtp = !settings.is_mtp,
        27 => settings.draft_tokens = (settings.draft_tokens as i32 + delta).max(0).min(16) as u32,
        _ => {}
    }
}

pub fn handle_settings_key(app: &mut App, key: crossterm::event::KeyEvent) {
    let idx = app.settings_state.settings_selected_idx;

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
        };
        return;
    }

    // Enable/Disable toggle
    if key.code == KeyCode::Char('e') && key.modifiers.contains(KeyModifiers::CONTROL) {
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
            26 => { // is_mtp
                app.settings.is_mtp = !app.settings.is_mtp;
            }
            _ => {}
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
                let count = 28; // Total LLM settings (0-27)
                app.settings_state.settings_selected_idx = (app.settings_state.settings_selected_idx + 1).min(count - 1);
            }
        }
        // Enable MTP: toggle on Enter
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
                app.settings_state.settings_selected_idx = (app.settings_state.settings_selected_idx + 10).min(27);
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
            } else {
                adjust_setting(&mut app.settings, idx, -1, app.max_threads, app.loading.model_n_ctx_train, app.loading.model_total_layers);
                if idx == 11 {
                    sync_global_settings(app);
                }
                app.update_vram_estimate();
            }
            app.settings_state.settings_render_cache = None;
        }
        KeyCode::Right => {
            adjust_setting(&mut app.settings, idx, 1, app.max_threads, app.loading.model_n_ctx_train, app.loading.model_total_layers);
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
                apply_numeric_setting(&mut app.settings, idx, &app.settings_state.settings_edit_buffer, app.max_threads, app.loading.model_n_ctx_train);
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
