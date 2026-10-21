//! Single key handler for the generic list picker (`app.list_picker`).
//!
//! Replaces the per-picker key handlers that used to live in the overlay
//! registry (spec-type, tensor-split). All variants share navigation, inline
//! commit differs, and that is routed by [`ListPickerKind`] in
//! [`enter_list_picker`]. Adding a new list picker therefore needs no new arm
//! here.

use crossterm::event::{KeyCode, KeyEvent};

use crate::config::LogLevel;
use crate::tui::app::{App, ListPicker, ListPickerKind};

use super::helpers::{mark_settings_dirty, picker_nav_down, picker_nav_up};

/// Outcome of processing one key against the generic list picker.
enum ListPickerOutcome {
    /// Keep the picker, with the (possibly mutated) state.
    Keep(ListPicker),
    /// Close the picker.
    Close,
}

/// What an Enter press inside the list (non-editing) view should do.
enum ListPickerEnter {
    /// Commit the selected value and close.
    Close,
    /// Enter inline editing of the selected value.
    Edit { buffer: String, cursor: usize },
}

/// Handle one key while the generic list picker is open.
pub async fn handle_list_picker(app: &mut App, key: KeyEvent) {
    // Take the picker out so the commit step can borrow `&mut App` freely.
    let Some(picker) = app.list_picker.take() else {
        return;
    };
    let outcome = process_list_picker(app, picker, key).await;
    app.list_picker = match outcome {
        ListPickerOutcome::Keep(picker) => Some(picker),
        ListPickerOutcome::Close => None,
    };
}

async fn process_list_picker(
    app: &mut App,
    mut picker: ListPicker,
    key: KeyEvent,
) -> ListPickerOutcome {
    // Inline single-field editing.
    if picker.editing {
        match key.code {
            KeyCode::Enter => {
                if commit_edit(app, &picker) {
                    return ListPickerOutcome::Close;
                }
            }
            KeyCode::Esc => {
                picker.editing = false;
                picker.edit_buffer = String::new();
                picker.edit_cursor = 0;
            }
            KeyCode::Char(c) if c.is_ascii_graphic() || c == ' ' => insert_char(&mut picker, c),
            KeyCode::Backspace if picker.edit_cursor > 0 => backspace(&mut picker),
            KeyCode::Delete if picker.edit_cursor < picker.edit_buffer.chars().count() => {
                delete_char(&mut picker)
            }
            KeyCode::Left => picker.edit_cursor = picker.edit_cursor.saturating_sub(1),
            KeyCode::Right => {
                picker.edit_cursor =
                    (picker.edit_cursor + 1).min(picker.edit_buffer.chars().count())
            }
            _ => {}
        }
        return ListPickerOutcome::Keep(picker);
    }

    // List (non-editing) view.
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => picker_nav_up(&mut picker.selected),
        KeyCode::Down | KeyCode::Char('j') => {
            let n = picker.len();
            picker_nav_down(&mut picker.selected, n);
        }
        KeyCode::PageUp => picker.scroll_offset = picker.scroll_offset.saturating_sub(5),
        KeyCode::PageDown => picker.scroll_offset = picker.scroll_offset.saturating_add(5),
        KeyCode::Esc => return ListPickerOutcome::Close,
        KeyCode::Enter => {
            // Compute the commit decision first so the immutable borrow of
            // `picker` ends before the arms below mutate it.
            let enter = enter_list_picker(app, &picker).await;
            match enter {
                ListPickerEnter::Close => return ListPickerOutcome::Close,
                ListPickerEnter::Edit { buffer, cursor } => {
                    picker.editing = true;
                    picker.edit_buffer = buffer;
                    picker.edit_cursor = cursor;
                }
            }
        }
        _ => {}
    }
    ListPickerOutcome::Keep(picker)
}

/// Commit an edited value (tensor-split manual entry). Returns `true` if the
/// picker should close.
fn commit_edit(app: &mut App, picker: &ListPicker) -> bool {
    if picker.kind == ListPickerKind::TensorSplit {
        let value = picker.edit_buffer.trim().to_string();
        if crate::tui::settings::is_valid_tensor_split(&value) {
            app.settings.tensor_split = value;
            mark_settings_dirty(app, true);
            return true;
        } else {
            app.add_log(
                crate::t!("dialog.tensor_split.invalid").to_string(),
                LogLevel::Warning,
            );
        }
    }
    false
}

/// Enter handling, routed by [`ListPickerKind`].
async fn enter_list_picker(app: &mut App, picker: &ListPicker) -> ListPickerEnter {
    let value = picker
        .entries
        .get(picker.selected)
        .map(|e| e.value.clone())
        .unwrap_or_default();

    match picker.kind {
        ListPickerKind::SpecType => {
            app.settings.spec_type = if value == "Off" { String::new() } else { value };
            mark_settings_dirty(app, false);
            ListPickerEnter::Close
        }
        ListPickerKind::TensorSplit => {
            // The trailing entry is always the manual editor.
            if picker.selected == picker.len().saturating_sub(1) {
                let cursor = value.chars().count();
                return ListPickerEnter::Edit {
                    buffer: value,
                    cursor,
                };
            }
            app.settings.tensor_split = value;
            mark_settings_dirty(app, true);
            ListPickerEnter::Close
        }
    }
}

fn insert_char(picker: &mut ListPicker, c: char) {
    let byte_idx = picker
        .edit_buffer
        .char_indices()
        .nth(picker.edit_cursor)
        .map(|(i, _)| i)
        .unwrap_or(picker.edit_buffer.len());
    picker.edit_buffer.insert(byte_idx, c);
    picker.edit_cursor += 1;
}

fn backspace(picker: &mut ListPicker) {
    picker.edit_cursor -= 1;
    let byte_idx = picker
        .edit_buffer
        .char_indices()
        .nth(picker.edit_cursor)
        .map(|(i, _)| i)
        .unwrap_or(0);
    picker.edit_buffer.remove(byte_idx);
}

fn delete_char(picker: &mut ListPicker) {
    let byte_idx = picker
        .edit_buffer
        .char_indices()
        .nth(picker.edit_cursor)
        .map(|(i, _)| i)
        .unwrap_or(picker.edit_buffer.len());
    picker.edit_buffer.remove(byte_idx);
}
