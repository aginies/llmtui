use std::future::Future;
use std::pin::Pin;

use crossterm::event::{KeyCode, KeyEvent};

use crate::tui::app::{App, GlobalMode};

use super::OverlayHandler;

pub struct ProfileCreateHandler;

impl OverlayHandler for ProfileCreateHandler {
    fn can_handle(&self, mode: &GlobalMode) -> bool {
        matches!(mode, GlobalMode::ProfileCreate { .. })
    }

    fn handle<'a>(
        &'a self,
        app: &'a mut App,
        key: KeyEvent,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            if let GlobalMode::ProfileCreate {
                name,
                description,
                editing,
                edit_buffer,
                edit_cursor_pos,
                field,
            } = &mut app.ui.global_mode
            {
                match key.code {
                    // Tab: switch between name and description fields
                    KeyCode::Tab => {
                        // Commit current edit before switching
                        if *editing {
                            if *field == 0 {
                                *name = edit_buffer.clone();
                            } else {
                                *description = edit_buffer.clone();
                            }
                            edit_buffer.clear();
                            *edit_cursor_pos = 0;
                        }
                        *field = if *field == 0 { 1 } else { 0 };
                        *editing = true;
                    }
                    // Enter: commit field and move to next, or save if on last field
                    KeyCode::Enter => {
                        if *editing {
                            if *field == 0 {
                                *name = edit_buffer.clone();
                                edit_buffer.clear();
                                *edit_cursor_pos = 0;
                                *field = 1;
                                *editing = true;
                            } else {
                                *description = edit_buffer.clone();
                                edit_buffer.clear();
                                *edit_cursor_pos = 0;
                                // Save the profile and return to the picker
                                // (stay in the dialog if the save failed — error is logged)
                                if app.save_as_profile() {
                                    app.open_model_settings_picker();
                                }
                            }
                        } else if *field == 0 {
                            // Not editing, move to description
                            *field = 1;
                            *editing = true;
                        } else {
                            // On description, not editing — save
                            if app.save_as_profile() {
                                app.open_model_settings_picker();
                            }
                        }
                    }
                    // Esc: cancel — return to the profile picker
                    KeyCode::Esc => {
                        app.open_model_settings_picker();
                    }
                    // Backspace: delete character
                    KeyCode::Backspace => {
                        if *editing && !edit_buffer.is_empty() {
                            edit_buffer.pop();
                            *edit_cursor_pos = edit_buffer.len();
                        } else if *editing {
                            // Clear the field
                            if *field == 0 {
                                name.clear();
                            } else {
                                description.clear();
                            }
                            *editing = false;
                        }
                    }
                    // Character input (includes space)
                    KeyCode::Char(c) if *editing => {
                        edit_buffer.push(c);
                        *edit_cursor_pos = edit_buffer.len();
                    }
                    _ => {}
                }
            }
        })
    }
}
