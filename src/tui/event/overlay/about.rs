use std::pin::Pin;
use std::future::Future;

use crossterm::event::{KeyCode, KeyEvent};

use crate::tui::app::{App, GlobalMode};

use super::OverlayHandler;

pub struct AboutHandler;

impl OverlayHandler for AboutHandler {
    fn name(&self) -> &'static str {
        "About"
    }

    fn can_handle(&self, mode: &GlobalMode) -> bool {
        matches!(mode, GlobalMode::About)
    }

    fn handle<'a>(
        &'a self,
        app: &'a mut App,
        key: KeyEvent,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            if let GlobalMode::About = &app.ui.global_mode
                && let KeyCode::Esc = key.code {
                    app.ui.global_mode = GlobalMode::Normal;
                }
        })
    }
}
