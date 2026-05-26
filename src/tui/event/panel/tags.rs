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
            app.set_redraw();
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
            app.set_redraw();
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
            app.set_redraw();
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
            app.set_redraw();
        }
        // Edit selected tag
        KeyCode::Char('e') | KeyCode::Char('i') => {
            if !insert_mode {
                if let Some(idx) = selected {
                    app.edit.tags_edit_buffer = tags[idx].clone();
                    app.set_redraw();
                }
            }
        }
        // Delete selected tag
        KeyCode::Char('d') | KeyCode::Delete => {
            if !insert_mode {
                if let Some(idx) = selected {
                    if idx < app.settings.tags.len() {
                        app.settings.tags.remove(idx);
                        app.edit.tags_selected_idx = None;
                        app.edit.tags_edit_buffer.clear();
                        app.edit.tags_insert_mode = false;
                        app.settings_state.settings_render_cache = None;
                    }
                }
            }
            app.set_redraw();
        }
        // Add new tag
        KeyCode::Char('a') => {
            app.edit.tags_insert_mode = true;
            app.edit.tags_selected_idx = None;
            app.edit.tags_edit_buffer.clear();
            app.set_redraw();
        }
        // Input characters for tag editing
        KeyCode::Char(c) => {
            app.edit.tags_edit_buffer.push(c);
            app.set_redraw();
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
            app.set_redraw();
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
            app.set_redraw();
        }
        _ => {}
    }
}
