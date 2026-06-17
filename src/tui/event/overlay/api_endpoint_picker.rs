use std::future::Future;
use std::pin::Pin;

use crossterm::event::{KeyCode, KeyEvent};

use super::super::helpers::{TextEditor, sync_global_settings};
use crate::tui::app::{App, GlobalMode};

use super::OverlayHandler;

pub struct ApiEndpointPickerHandler;

impl OverlayHandler for ApiEndpointPickerHandler {
    fn can_handle(&self, mode: &GlobalMode) -> bool {
        matches!(mode, GlobalMode::ApiEndpointPicker { .. })
    }

    fn handle<'a>(
        &'a self,
        app: &'a mut App,
        key: KeyEvent,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            if let GlobalMode::ApiEndpointPicker {
                enabled,
                port,
                api_key,
                tls_enabled,
                tls_cert,
                tls_key,
                selected_field,
                editing,
                edit_buffer,
                edit_cursor_pos,
            } = &mut app.ui.global_mode
            {
                match key.code {
                    KeyCode::Enter => {
                        if *editing {
                            if *selected_field == 0i32
                                && let Ok(p) = edit_buffer.parse::<u16>()
                            {
                                app.settings.api_endpoint_port = p;
                                port.clone_from(edit_buffer);
                            }
                            if *selected_field == 1i32 {
                                app.settings.api_endpoint_key = if edit_buffer.is_empty() {
                                    None
                                } else {
                                    Some(edit_buffer.clone())
                                };
                                app.config.default.api_endpoint_key = if edit_buffer.is_empty() {
                                    None
                                } else {
                                    Some(edit_buffer.clone())
                                };
                                api_key.clone_from(edit_buffer);
                            }
                            if *selected_field == 2i32 {
                                *tls_enabled = !*tls_enabled;
                                app.config.default.server_tls_enabled = *tls_enabled;
                            }
                            if *selected_field == 3i32 {
                                app.config.default.server_tls_cert = if edit_buffer.is_empty() {
                                    None
                                } else {
                                    Some(edit_buffer.clone())
                                };
                                tls_cert.clone_from(edit_buffer);
                            }
                            if *selected_field == 4i32 {
                                app.config.default.server_tls_key = if edit_buffer.is_empty() {
                                    None
                                } else {
                                    Some(edit_buffer.clone())
                                };
                                tls_key.clone_from(edit_buffer);
                            }
                            *editing = false;
                            sync_global_settings(app);
                            return;
                        }
                        if *selected_field == -1 {
                            *enabled = !*enabled;
                            app.settings.api_endpoint_enabled = *enabled;
                            app.config.default.api_endpoint_enabled = *enabled;
                            sync_global_settings(app);
                            return;
                        }
                        if *selected_field == 0i32 {
                            edit_buffer.clone_from(port);
                            *editing = true;
                            *edit_cursor_pos = edit_buffer.chars().count();
                            return;
                        }
                        if *selected_field == 1i32 {
                            edit_buffer.clone_from(api_key);
                            *editing = true;
                            *edit_cursor_pos = edit_buffer.chars().count();
                            return;
                        }
                        if *selected_field == 2i32 {
                            *tls_enabled = !*tls_enabled;
                            app.config.default.server_tls_enabled = *tls_enabled;
                            sync_global_settings(app);
                            return;
                        }
                        if *selected_field == 3i32 {
                            edit_buffer.clone_from(tls_cert);
                            *editing = true;
                            *edit_cursor_pos = edit_buffer.chars().count();
                            return;
                        }
                        if *selected_field == 4i32 {
                            edit_buffer.clone_from(tls_key);
                            *editing = true;
                            *edit_cursor_pos = edit_buffer.chars().count();
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        if !*editing {
                            *selected_field = if *selected_field <= -1 {
                                4
                            } else {
                                *selected_field - 1
                            };
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if !*editing {
                            *selected_field = if *selected_field >= 4 {
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
