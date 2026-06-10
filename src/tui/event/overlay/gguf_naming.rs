use std::future::Future;
use std::pin::Pin;

use crossterm::event::{KeyCode, KeyEvent};

use crate::tui::app::{App, GlobalMode};

use super::OverlayHandler;

pub struct GgufNamingHandler;

impl OverlayHandler for GgufNamingHandler {
    fn can_handle(&self, mode: &GlobalMode) -> bool {
        matches!(mode, GlobalMode::GgufNaming { .. })
    }

    fn handle<'a>(
        &'a self,
        app: &'a mut App,
        key: KeyEvent,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            if let GlobalMode::GgufNaming { .. } = &app.ui.global_mode {
                match key.code {
                    KeyCode::Esc | KeyCode::Enter => {
                        app.ui.global_mode = GlobalMode::Normal;
                    }
                    _ => {}
                }
            }
        })
    }
}
