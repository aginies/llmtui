use std::future::Future;
use std::pin::Pin;

use crossterm::event::{KeyCode, KeyEvent};

use crate::tui::app::{App, GlobalMode};

use super::OverlayHandler;

pub struct DashboardUrlHandler;

impl OverlayHandler for DashboardUrlHandler {
    fn can_handle(&self, mode: &GlobalMode) -> bool {
        matches!(mode, GlobalMode::DashboardUrl { .. })
    }

    fn handle<'a>(
        &'a self,
        app: &'a mut App,
        key: KeyEvent,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            if matches!(app.ui.global_mode, GlobalMode::DashboardUrl { .. }) {
                match key.code {
                    KeyCode::Esc => {
                        app.ui.global_mode = GlobalMode::Normal;
                    }
                    _ => {}
                }
            }
        })
    }
}
