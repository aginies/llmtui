use crossterm::event::KeyCode;

use crate::tui::app::App;

pub fn handle_tags_key(app: &mut App, key: crossterm::event::KeyEvent) {
    let tags = &app.settings.tags;
    let selected = app.edit.tags_selected_idx;
    let edit_buf = &app.edit.tags_edit_buffer;
    let insert_mode = app.edit.tags_insert_mode;

    match key.code {
        // Close modal
        KeyCode::Esc => {
            app.edit.tags_editing = false;
            app.edit.tags_edit_buffer.clear();
            app.edit.tags_selected_idx = None;
            app.edit.tags_insert_mode = false;
            app.settings_state.settings_render_cache = None;
        }
        // Save and close modal
        KeyCode::Enter => {
            if insert_mode && !edit_buf.is_empty() {
                // Add new tag
                let new_tag = edit_buf.trim().to_string();
                if !new_tag.is_empty() {
                    app.settings.tags.push(new_tag);
                }
                app.edit.tags_edit_buffer.clear();
                app.edit.tags_insert_mode = false;
                app.edit.tags_selected_idx = None;
                app.settings_state.settings_render_cache = None;
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
                    app.edit.tags_edit_buffer.clear();
                    app.edit.tags_selected_idx = None;
                    app.edit.tags_insert_mode = false;
                    app.settings_state.settings_render_cache = None;
                } else {
                    // No tag selected, close modal
                    app.edit.tags_editing = false;
                    app.edit.tags_edit_buffer.clear();
                    app.edit.tags_selected_idx = None;
                    app.edit.tags_insert_mode = false;
                    app.settings_state.settings_render_cache = None;
                }
            } else {
                // Just close modal without adding
                app.edit.tags_editing = false;
                app.edit.tags_edit_buffer.clear();
                app.edit.tags_selected_idx = None;
                app.edit.tags_insert_mode = false;
                app.settings_state.settings_render_cache = None;
            }
        }
        // Navigate tags
        KeyCode::Up | KeyCode::Char('k') => {
            app.edit.tags_edit_buffer.clear();
            if insert_mode {
                app.edit.tags_insert_mode = false;
                app.edit.tags_selected_idx = Some(tags.len().saturating_sub(1));
            } else if let Some(idx) = selected {
                app.edit.tags_selected_idx = Some(idx.saturating_sub(1));
            } else {
                app.edit.tags_selected_idx = Some(tags.len().saturating_sub(1));
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.edit.tags_edit_buffer.clear();
            if insert_mode {
                app.edit.tags_insert_mode = false;
                app.edit.tags_selected_idx = Some(tags.len().saturating_sub(1));
            } else if let Some(idx) = selected {
                app.edit.tags_selected_idx = Some((idx + 1).min(tags.len().saturating_sub(1)));
            } else if !tags.is_empty() {
                app.edit.tags_selected_idx = Some(0);
            } else {
                app.edit.tags_insert_mode = true;
            }
        }
        // Edit selected tag
        KeyCode::Char('e') | KeyCode::Char('i') => {
            if !insert_mode && let Some(idx) = selected {
                app.edit.tags_edit_buffer = tags[idx].clone();
            }
        }
        // Delete selected tag
        KeyCode::Char('d') | KeyCode::Delete => {
            if !insert_mode
                && let Some(idx) = selected
                && idx < app.settings.tags.len()
            {
                app.settings.tags.remove(idx);
                app.edit.tags_selected_idx = None;
                app.edit.tags_edit_buffer.clear();
                app.edit.tags_insert_mode = false;
                app.settings_state.settings_render_cache = None;
            }
        }
        // Add new tag
        KeyCode::Char('a') => {
            app.edit.tags_insert_mode = true;
            app.edit.tags_selected_idx = None;
            app.edit.tags_edit_buffer.clear();
        }
        // Input characters for tag editing
        KeyCode::Char(c) => {
            app.edit.tags_edit_buffer.push(c);
        }
        KeyCode::Backspace => {
            if !app.edit.tags_edit_buffer.is_empty() {
                app.edit.tags_edit_buffer.pop();
            } else if !insert_mode {
                // Move to previous tag if no edit buffer
                if let Some(idx) = selected {
                    app.edit.tags_selected_idx = Some(idx.saturating_sub(1));
                }
            }
        }
        KeyCode::Tab => {
            // Toggle between insert and edit mode
            if insert_mode {
                app.edit.tags_insert_mode = false;
                if !tags.is_empty() {
                    app.edit.tags_selected_idx = Some(tags.len().saturating_sub(1));
                }
            } else {
                app.edit.tags_insert_mode = true;
                app.edit.tags_selected_idx = None;
            }
            app.settings_state.settings_edit_buffer.clear();
        }
        _ => {}
    }
}
