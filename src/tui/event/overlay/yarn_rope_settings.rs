use std::pin::Pin;
use std::future::Future;

use crossterm::event::{KeyCode, KeyEvent};

use super::super::helpers::TextEditor;
use crate::tui::app::{App, GlobalMode};

use super::OverlayHandler;

pub struct YarnRoPESettingsHandler;

impl OverlayHandler for YarnRoPESettingsHandler {
    fn can_handle(&self, mode: &GlobalMode) -> bool {
        matches!(mode, GlobalMode::YarnRoPESettings { .. })
    }

    fn handle<'a>(
        &'a self,
        app: &'a mut App,
        key: KeyEvent,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            if let GlobalMode::YarnRoPESettings {
                scale,
                freq_base,
                freq_scale,
                selected_field,
                editing,
                edit_buffer,
                edit_cursor_pos,
                ..
            } = &mut app.ui.global_mode
            {
                match key.code {
                    KeyCode::Enter => {
                        if *editing {
                            match *selected_field {
                                0i32 => {
                                    if let Ok(p) = edit_buffer.parse::<f32>() {
                                        app.settings.rope_scale = p;
                                    }
                                    scale.clone_from(edit_buffer);
                                }
                                1i32 => {
                                    if let Ok(p) = edit_buffer.parse::<f32>() {
                                        app.settings.rope_freq_base = p;
                                    }
                                    freq_base.clone_from(edit_buffer);
                                }
                                2i32 => {
                                    if let Ok(p) = edit_buffer.parse::<f32>() {
                                        app.settings.rope_freq_scale = p;
                                    }
                                    freq_scale.clone_from(edit_buffer);
                                }
                                _ => {}
                            }
                            *editing = false;
                            return;
                        }
                        if *selected_field == -1 {
                            app.settings.rope_yarn_enabled = !app.settings.rope_yarn_enabled;
                            *editing = true;
                            *selected_field = -1;
                            edit_buffer.clone_from(&format!("{}", app.settings.rope_yarn_enabled));
                            *edit_cursor_pos = edit_buffer.chars().count();
                            return;
                        }
                        if *selected_field == 0i32 {
                            edit_buffer.clone_from(scale);
                            *editing = true;
                            *edit_cursor_pos = edit_buffer.chars().count();
                            return;
                        }
                        if *selected_field == 1i32 {
                            edit_buffer.clone_from(freq_base);
                            *editing = true;
                            *edit_cursor_pos = edit_buffer.chars().count();
                            return;
                        }
                        if *selected_field == 2i32 {
                            edit_buffer.clone_from(freq_scale);
                            *editing = true;
                            *edit_cursor_pos = edit_buffer.chars().count();
                        }
                    }
                    KeyCode::Char(' ') => {
                        if *selected_field == -1 {
                            app.settings.rope_yarn_enabled = !app.settings.rope_yarn_enabled;
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        if !*editing {
                            *selected_field = if *selected_field <= -1 {
                                2
                            } else {
                                *selected_field - 1
                            };
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if !*editing {
                            *selected_field = if *selected_field >= 2 {
                                -1
                            } else {
                                *selected_field + 1
                            };
                        }
                    }
                    KeyCode::Esc => {
                        if *editing {
                            *editing = false;
                            edit_buffer.clear();
                        } else {
                            app.ui.global_mode = GlobalMode::Normal;
                        }
                    }
                    KeyCode::Char(c) if *editing => {
                        TextEditor {
                            buffer: edit_buffer,
                            cursor: edit_cursor_pos,
                        }
                        .insert_char(c);
                    }
                    KeyCode::Backspace if *editing => {
                        TextEditor {
                            buffer: edit_buffer,
                            cursor: edit_cursor_pos,
                        }
                        .backspace();
                    }
                    _ => {}
                }
            }
        })
    }
}
