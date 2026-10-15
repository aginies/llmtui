use std::future::Future;
use std::pin::Pin;

use crossterm::event::{KeyCode, KeyEvent};

use super::super::helpers::{mark_settings_dirty, picker_nav_down, picker_nav_up};
use crate::tui::app::{App, GlobalMode};

use super::OverlayHandler;

pub struct TensorSplitPickerHandler;

impl OverlayHandler for TensorSplitPickerHandler {
    fn can_handle(&self, mode: &GlobalMode) -> bool {
        matches!(mode, GlobalMode::TensorSplitPicker { .. })
    }

    fn handle<'a>(
        &'a self,
        app: &'a mut App,
        key: KeyEvent,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            if let GlobalMode::TensorSplitPicker {
                entries,
                selected,
                editing,
                edit_buffer,
                edit_cursor_pos,
            } = &mut app.ui.global_mode
            {
                if *editing {
                    match key.code {
                        KeyCode::Enter => {
                            let value = edit_buffer.trim().to_string();
                            if crate::tui::settings::is_valid_tensor_split(&value) {
                                app.settings.tensor_split = value;
                                mark_settings_dirty(app, true);
                                app.ui.global_mode = GlobalMode::Normal;
                            } else {
                                app.add_log(
                                    crate::t!("dialog.tensor_split.invalid").to_string(),
                                    crate::config::LogLevel::Warning,
                                );
                            }
                        }
                        KeyCode::Esc => {
                            *editing = false;
                            *edit_buffer = String::new();
                            *edit_cursor_pos = 0;
                        }
                        KeyCode::Char(c) => {
                            if c.is_ascii_graphic() || c == ' ' {
                                let byte_idx = edit_buffer
                                    .char_indices()
                                    .nth(*edit_cursor_pos)
                                    .map(|(i, _)| i)
                                    .unwrap_or(edit_buffer.len());
                                edit_buffer.insert(byte_idx, c);
                                *edit_cursor_pos += 1;
                            }
                        }
                        KeyCode::Backspace if *edit_cursor_pos > 0 => {
                            *edit_cursor_pos -= 1;
                            let byte_idx = edit_buffer
                                .char_indices()
                                .nth(*edit_cursor_pos)
                                .map(|(i, _)| i)
                                .unwrap_or(0);
                            edit_buffer.remove(byte_idx);
                        }
                        KeyCode::Delete if *edit_cursor_pos < edit_buffer.chars().count() => {
                            let byte_idx = edit_buffer
                                .char_indices()
                                .nth(*edit_cursor_pos)
                                .map(|(i, _)| i)
                                .unwrap_or(edit_buffer.len());
                            edit_buffer.remove(byte_idx);
                        }
                        KeyCode::Left => {
                            *edit_cursor_pos = edit_cursor_pos.saturating_sub(1);
                        }
                        KeyCode::Right => {
                            *edit_cursor_pos =
                                (*edit_cursor_pos + 1).min(edit_buffer.chars().count());
                        }
                        _ => {}
                    }
                    return;
                }

                match key.code {
                    KeyCode::Enter => {
                        if *selected < entries.len() {
                            // The last entry is always "Manual entry"
                            if *selected == entries.len() - 1 {
                                let value = entries[*selected].1.clone();
                                let cursor = value.chars().count();
                                *editing = true;
                                *edit_buffer = value;
                                *edit_cursor_pos = cursor;
                            } else {
                                let value = entries[*selected].1.clone();
                                app.settings.tensor_split = value;
                                mark_settings_dirty(app, true);
                                app.ui.global_mode = GlobalMode::Normal;
                            }
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') => picker_nav_up(selected),
                    KeyCode::Down | KeyCode::Char('j') => picker_nav_down(selected, entries.len()),
                    KeyCode::Esc => {
                        app.ui.global_mode = GlobalMode::Normal;
                    }
                    _ => {}
                }
            }
        })
    }
}
