use std::pin::Pin;
use std::future::Future;

use crossterm::event::{KeyCode, KeyEvent};

use super::super::helpers::{sync_global_settings, picker_nav_up, picker_nav_down};
use crate::tui::app::{App, GlobalMode};

use super::OverlayHandler;

pub struct HostPickerHandler;

impl OverlayHandler for HostPickerHandler {
    fn can_handle(&self, mode: &GlobalMode) -> bool {
        matches!(mode, GlobalMode::HostPicker { .. })
    }

    fn handle<'a>(
        &'a self,
        app: &'a mut App,
        key: KeyEvent,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            if let GlobalMode::HostPicker { entries, selected } = &mut app.ui.global_mode {
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => picker_nav_up(selected),
                    KeyCode::Down | KeyCode::Char('j') => picker_nav_down(selected, entries.len()),
                    KeyCode::Enter => {
                        if *selected < entries.len() {
                            let (ip, _) = entries[*selected].clone();
                            app.settings.host = ip;
                            app.ui.global_mode = GlobalMode::Normal;
                            sync_global_settings(app);
                        }
                    }
                    KeyCode::Char('d') => {
                        *entries = App::fetch_host_picker_entries();
                        *selected = 0;
                    }
                    KeyCode::Esc => {
                        app.ui.global_mode = GlobalMode::Normal;
                    }
                    KeyCode::Char('h')
                        if key
                            .modifiers
                            .contains(crossterm::event::KeyModifiers::CONTROL) =>
                    {
                        app.ui.global_mode = GlobalMode::Normal;
                    }
                    _ => {}
                }
            }
        })
    }
}
