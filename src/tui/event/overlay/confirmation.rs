use std::future::Future;
use std::pin::Pin;

use crossterm::event::{KeyCode, KeyEvent};

use super::super::helpers::execute_confirmation;
use crate::tui::app::{App, ConfirmationKind, GlobalMode};

use super::OverlayHandler;

pub struct ConfirmationHandler;

impl OverlayHandler for ConfirmationHandler {
    fn can_handle(&self, mode: &GlobalMode) -> bool {
        matches!(mode, GlobalMode::Confirmation { .. })
    }

    fn handle<'a>(
        &'a self,
        app: &'a mut App,
        key: KeyEvent,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            if let GlobalMode::Confirmation {
                selected,
                kind,
                display_name,
                detail,
            } = &app.ui.global_mode
            {
                match key.code {
                    KeyCode::Char('y') => {
                        let kind_copy = *kind;
                        let display_name_copy = display_name.clone();
                        let detail_copy = detail.clone();
                        execute_confirmation(app, kind_copy, display_name_copy, detail_copy).await;
                        if matches!(kind_copy, ConfirmationKind::DeleteBackend) {
                            let new_entries = app.fetch_backend_picker_entries();
                            if let GlobalMode::BackendPicker { entries, selected } =
                                &mut app.ui.global_mode
                            {
                                *entries = new_entries;
                                if *selected >= entries.len() {
                                    *selected = entries.len().saturating_sub(1);
                                }
                            }
                            // Stay on backend picker view after deletion
                            return;
                        }
                        app.ui.global_mode = GlobalMode::Normal;
                    }
                    KeyCode::Char('n') | KeyCode::Esc => {
                        app.pending.pending_api_unload = None;
                        app.ui.global_mode = GlobalMode::Normal;
                    }
                    KeyCode::Enter => {
                        if *selected {
                            let display_name_copy = display_name.clone();
                            let detail_copy = detail.clone();
                            execute_confirmation(app, *kind, display_name_copy, detail_copy).await;
                        } else {
                            app.pending.pending_api_unload = None;
                        }
                        app.ui.global_mode = GlobalMode::Normal;
                    }
                    KeyCode::Tab | KeyCode::Left | KeyCode::Right => {
                        app.ui.global_mode = GlobalMode::Confirmation {
                            selected: !*selected,
                            kind: *kind,
                            display_name: display_name.clone(),
                            detail: detail.clone(),
                        };
                    }
                    KeyCode::Char('h')
                        if key
                            .modifiers
                            .contains(crossterm::event::KeyModifiers::CONTROL) =>
                    {
                        app.pending.pending_api_unload = None;
                        app.ui.global_mode = GlobalMode::Normal;
                    }
                    _ => {}
                }
            }
        })
    }
}
