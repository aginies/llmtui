use crossterm::event::KeyCode;

use crate::tui::app::App;

pub fn handle_system_prompt_presets_key(app: &mut App, key: crossterm::event::KeyEvent) {
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
                if let Some(preset_name) = app.editing_preset {
                    // We need the name from the merged list
                    let all_presets = app.config.merged_presets();
                    if let Some(preset) = all_presets.get(preset_name) {
                        if let Some(mut p) = app.config.system_prompt_presets.get(&preset.name).cloned() {
                            p.content = app.settings_edit_buffer.clone();
                            app.config.system_prompt_presets.save(&p);
                        }
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
    let all_presets = app.config.merged_presets();
    let total = all_presets.len();
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
            if let Some(preset) = all_presets.get(app.settings_selected_idx) {
                let name = preset.name.clone();
                app.settings.system_prompt_preset_name = name.clone();
                app.resolve_system_prompt();
                app.active_panel = crate::tui::app::ActivePanel::LlmSettings;
                app.add_log(format!("Applied preset: {}", name), crate::config::LogLevel::Info);
            }
        }
        KeyCode::Char('e') => {
            // Edit the selected preset
            if let Some(preset) = all_presets.get(app.settings_selected_idx) {
                app.settings_edit_buffer = preset.content.clone();
                app.edit_cursor_pos = app.settings_edit_buffer.chars().count();
                app.editing_preset = Some(app.settings_selected_idx);
            }
        }
        KeyCode::Char('n') => {
            // Create a new preset
            let name = format!("Custom {}", app.config.system_prompt_presets.user_presets().len() + 1);
            let preset = crate::config::SystemPromptPreset {
                name: name.clone(),
                description: "User-defined preset".into(),
                content: String::new(),
            };
            app.config.system_prompt_presets.save(&preset);
            // Select the new preset and enter edit mode
            app.settings_selected_idx = app.config.merged_presets().len() - 1;
            app.settings_edit_buffer = String::new();
            app.edit_cursor_pos = 0;
            app.editing_preset = Some(app.settings_selected_idx);
        }
        KeyCode::Char('d') => {
            // Delete custom preset (not built-in)
            if app.settings_selected_idx >= crate::config::builtin_system_prompt_presets().len() {
                let preset = all_presets[app.settings_selected_idx].clone();
                app.config.system_prompt_presets.delete(&preset.name);
                let new_total = app.config.merged_presets().len();
                app.settings_selected_idx = app.settings_selected_idx.min(new_total.saturating_sub(1));
                app.add_log(format!("Deleted preset: {}", preset.name), crate::config::LogLevel::Info);
                if let Err(e) = app.config.save() {
                    app.add_log(format!("Failed to save: {}", e), crate::config::LogLevel::Error);
                }
            }
        }
        KeyCode::Esc => {
            app.active_panel = crate::tui::app::ActivePanel::LlmSettings;
        }
        _ => {}
    }
}
