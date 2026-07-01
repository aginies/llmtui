use std::future::Future;
use std::pin::Pin;

use crossterm::event::{KeyCode, KeyEvent};

use super::super::helpers::{mark_settings_dirty, picker_nav_down, picker_nav_up};
use crate::tui::app::{App, GlobalMode};

use super::OverlayHandler;

pub struct SpecTypePickerHandler;

impl OverlayHandler for SpecTypePickerHandler {
    fn can_handle(&self, mode: &GlobalMode) -> bool {
        matches!(mode, GlobalMode::SpecTypePicker { .. })
    }

    fn handle<'a>(
        &'a self,
        app: &'a mut App,
        key: KeyEvent,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            if let GlobalMode::SpecTypePicker { entries, selected } = &mut app.ui.global_mode {
                match key.code {
                    KeyCode::Enter => {
                        let selected_entry = entries[*selected].clone();
                        app.settings.spec_type = if selected_entry == "Off" {
                            String::new()
                        } else {
                            selected_entry
                        };
                        mark_settings_dirty(app, false);
                        app.ui.global_mode = GlobalMode::Normal;
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
