use std::future::Future;
use std::pin::Pin;

use crossterm::event::{KeyCode, KeyEvent};

use super::super::helpers::{mark_settings_dirty, sync_global_settings};
use crate::tui::app::{App, GlobalMode};

use super::OverlayHandler;

pub struct MaxConcurrentPickerHandler;

impl OverlayHandler for MaxConcurrentPickerHandler {
    fn can_handle(&self, mode: &GlobalMode) -> bool {
        matches!(mode, GlobalMode::MaxConcurrentPicker { .. })
    }

    fn handle<'a>(
        &'a self,
        app: &'a mut App,
        key: KeyEvent,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            if let GlobalMode::MaxConcurrentPicker { value } = &mut app.ui.global_mode {
                match key.code {
                    KeyCode::Esc => {
                        app.ui.global_mode = GlobalMode::Normal;
                    }
                    KeyCode::Char(c) if c.is_ascii_digit()
                        && value.len() < 3 => {
                            value.push(c);
                        }
                    KeyCode::Backspace => {
                        value.pop();
                    }
                    KeyCode::Enter => {
                        if let Ok(n) = value.parse::<u32>() {
                            let n = n.clamp(1, 10);
                            app.settings.max_concurrent_predictions = Some(n);
                            sync_global_settings(app);
                            mark_settings_dirty(app, true);
                        }
                        app.ui.global_mode = GlobalMode::Normal;
                        mark_settings_dirty(app, false);
                    }
                    _ => {}
                }
            }
        })
    }
}
