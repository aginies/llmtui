use std::future::Future;
use std::pin::Pin;

use crossterm::event::{KeyCode, KeyEvent};

use super::super::helpers::{picker_nav_down, picker_nav_up};
use crate::tui::app::{App, ConfirmationKind, GlobalMode};

use super::OverlayHandler;

pub struct ModelSettingsPickerHandler;

impl OverlayHandler for ModelSettingsPickerHandler {
    fn can_handle(&self, mode: &GlobalMode) -> bool {
        matches!(mode, GlobalMode::ModelSettingsPicker { .. })
    }

    fn handle<'a>(
        &'a self,
        app: &'a mut App,
        key: KeyEvent,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            if let GlobalMode::ModelSettingsPicker {
                entries, selected, ..
            } = &mut app.ui.global_mode
            {
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => picker_nav_up(selected),
                    KeyCode::Down | KeyCode::Char('j') => picker_nav_down(selected, entries.len()),
                    KeyCode::Enter => {
                        if *selected < entries.len() {
                            if *selected == 0 {
                                // "+ New profile" pseudo-entry → open the create dialog
                                app.ui.global_mode = GlobalMode::ProfileCreate {
                                    name: String::new(),
                                    description: String::new(),
                                    editing: true,
                                    edit_buffer: String::new(),
                                    edit_cursor_pos: 0,
                                    field: 0,
                                };
                            } else {
                                let name = entries[*selected].0.clone();
                                app.apply_profile_by_name(&name);
                                app.ui.global_mode = GlobalMode::Normal;
                            }
                        }
                    }
                    KeyCode::Char('d') => {
                        // Delete selected profile (the "+ New profile" entry is not deletable)
                        if *selected > 0 && *selected < entries.len() {
                            let name = entries[*selected].0.clone();
                            if name != "Default" {
                                // Ask for confirmation before deleting
                                app.ui.global_mode = GlobalMode::Confirmation {
                                    selected: false,
                                    kind: ConfirmationKind::DeleteSettingsProfile,
                                    display_name: name,
                                    detail: None,
                                };
                            }
                        }
                    }
                    KeyCode::Esc => {
                        app.ui.global_mode = GlobalMode::Normal;
                    }
                    _ => {}
                }
            }
        })
    }
}
