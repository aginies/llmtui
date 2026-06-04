use std::pin::Pin;
use std::future::Future;

use crossterm::event::KeyEvent;

use crate::tui::app::{App, GlobalMode};

use super::OverlayHandler;

pub struct PromptPickerHandler;

impl OverlayHandler for PromptPickerHandler {
    fn name(&self) -> &'static str {
        "PromptPicker"
    }

    fn can_handle(&self, mode: &GlobalMode) -> bool {
        matches!(mode, GlobalMode::PromptPicker { .. })
    }

    fn handle<'a>(
        &'a self,
        app: &'a mut App,
        key: KeyEvent,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            super::super::key::handle_prompt_picker_key(app, key);
        })
    }
}
