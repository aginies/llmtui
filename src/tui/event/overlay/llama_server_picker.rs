use std::future::Future;
use std::pin::Pin;

use crossterm::event::{KeyCode, KeyEvent};

use super::super::helpers::{TextEditor, sync_global_settings, wrap_field_picker};
use crate::tui::app::{App, GlobalMode};

use super::OverlayHandler;

pub struct LlamaServerPickerHandler;

impl OverlayHandler for LlamaServerPickerHandler {
    fn can_handle(&self, mode: &GlobalMode) -> bool {
        matches!(mode, GlobalMode::LlamaServerOptionsPicker { .. })
    }

    fn handle<'a>(
        &'a self,
        app: &'a mut App,
        key: KeyEvent,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            if let GlobalMode::LlamaServerOptionsPicker {
                port,
                threads,
                threads_batch,
                log_level,
                selected_field,
                mode_picker_selected,
                editing,
                edit_buffer,
                edit_cursor_pos,
            } = &mut app.ui.global_mode
            {
                match key.code {
                    KeyCode::Enter => {
                        if *editing {
                            if *selected_field == -1 {
                                if let Ok(p) = edit_buffer.parse::<u16>() {
                                    app.settings.port = p;
                                    port.clone_from(edit_buffer);
                                }
                            } else if *selected_field == 0 {
                                if let Ok(t) = edit_buffer.parse::<u32>() {
                                    app.settings.threads = t.max(1);
                                    *threads = app.settings.threads;
                                }
                            } else if *selected_field == 1 {
                                if let Ok(tb) = edit_buffer.parse::<u32>() {
                                    app.settings.threads_batch = tb.max(1);
                                    *threads_batch = app.settings.threads_batch;
                                }
                            }
                            *editing = false;
                            sync_global_settings(app);
                            return;
                        }
                        match *selected_field {
                            -1 => {
                                edit_buffer.clone_from(port);
                                *editing = true;
                                *edit_cursor_pos = edit_buffer.chars().count();
                            }
                            0 => {
                                *edit_buffer = app.settings.threads.to_string();
                                *editing = true;
                                *edit_cursor_pos = edit_buffer.chars().count();
                            }
                            1 => {
                                *edit_buffer = app.settings.threads_batch.to_string();
                                *editing = true;
                                *edit_cursor_pos = edit_buffer.chars().count();
                            }
                            2 => {
                                let modes = crate::models::ServerMode::all();
                                *mode_picker_selected = modes
                                    .iter()
                                    .position(|m| *m == app.server_mode)
                                    .unwrap_or(0);
                            }
                            3 => {
                                let levels = ["error", "warn", "info", "trace", "debug"];
                                if let Some(pos) = levels.iter().position(|l| *l == log_level.as_str())
                                {
                                    let next = levels[(pos + 1) % levels.len()];
                                    app.config.default.log_level = next.to_string();
                                    log_level.clone_from(&next.to_string());
                                }
                            }
                            _ => {}
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') if !*editing => {
                        wrap_field_picker(selected_field, -1, 3, true);
                    }
                    KeyCode::Down | KeyCode::Char('j') if !*editing => {
                        wrap_field_picker(selected_field, -1, 3, false);
                    }
                    KeyCode::Right | KeyCode::Char('l') if !*editing && *selected_field == 2 => {
                        let modes = crate::models::ServerMode::all();
                        *mode_picker_selected = (*mode_picker_selected + 1) % modes.len();
                        app.server_mode = modes[*mode_picker_selected];
                        sync_global_settings(app);
                    }
                    KeyCode::Left | KeyCode::Char('h') if !*editing && *selected_field == 2 => {
                        let modes = crate::models::ServerMode::all();
                        *mode_picker_selected = if *mode_picker_selected == 0 {
                            modes.len() - 1
                        } else {
                            *mode_picker_selected - 1
                        };
                        app.server_mode = modes[*mode_picker_selected];
                        sync_global_settings(app);
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
                    KeyCode::Left if *editing => {
                        TextEditor {
                            buffer: edit_buffer,
                            cursor: edit_cursor_pos,
                        }
                        .move_left();
                    }
                    KeyCode::Right if *editing => {
                        TextEditor {
                            buffer: edit_buffer,
                            cursor: edit_cursor_pos,
                        }
                        .move_right();
                    }
                    KeyCode::Home if *editing => {
                        TextEditor {
                            buffer: edit_buffer,
                            cursor: edit_cursor_pos,
                        }
                        .home();
                    }
                    KeyCode::End if *editing => {
                        TextEditor {
                            buffer: edit_buffer,
                            cursor: edit_cursor_pos,
                        }
                        .end();
                    }
                    _ => {}
                }
            }
        })
    }
}
