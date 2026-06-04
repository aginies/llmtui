use std::pin::Pin;
use std::future::Future;

use crossterm::event::{KeyCode, KeyEvent};

use super::super::helpers::{picker_nav_up, picker_nav_down};
use crate::tui::app::{App, GlobalMode};

use super::OverlayHandler;

pub struct ProfilePickerHandler;

impl OverlayHandler for ProfilePickerHandler {
    fn name(&self) -> &'static str {
        "ProfilePicker"
    }

    fn can_handle(&self, mode: &GlobalMode) -> bool {
        matches!(mode, GlobalMode::ProfilePicker { .. })
    }

    fn handle<'a>(
        &'a self,
        app: &'a mut App,
        key: KeyEvent,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            if let GlobalMode::ProfilePicker {
                entries, selected, ..
            } = &mut app.ui.global_mode
            {
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => picker_nav_up(selected),
                    KeyCode::Down | KeyCode::Char('j') => picker_nav_down(selected, entries.len()),
                    KeyCode::Enter => {
                        if *selected < entries.len() {
                            let name = entries[*selected].0.clone();
                            let profile = app
                                .config
                                .merged_profiles()
                                .into_iter()
                                .find(|p| p.name == name);
                            if let Some(profile) = profile {
                                app.apply_profile(&profile);
                            }
                        }
                        app.ui.global_mode = GlobalMode::Normal;
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
