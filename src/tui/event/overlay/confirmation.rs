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
            let kind_ref = match &app.ui.global_mode {
                GlobalMode::Confirmation { kind, .. } => *kind,
                _ => return,
            };
            if let GlobalMode::Confirmation {
                selected,
                kind: _,
                display_name,
                detail,
            } = &app.ui.global_mode
            {
                let picker_selected = detail
                    .as_ref()
                    .and_then(|d| d.split(':').last().and_then(|s| s.parse::<usize>().ok()));

                match key.code {
                    KeyCode::Char('y') => {
                        let kind_copy = kind_ref;
                        let display_name_copy = display_name.clone();
                        let detail_copy = detail.clone();
                        execute_confirmation(app, kind_copy, display_name_copy, detail_copy).await;
                        if matches!(kind_copy, ConfirmationKind::DeleteBackend) {
                            let new_entries = app.fetch_backend_picker_entries();
                            let new_selected = picker_selected.unwrap_or(0).min(new_entries.len().saturating_sub(1));
                            app.ui.global_mode = GlobalMode::BackendPicker {
                                entries: new_entries,
                                selected: new_selected,
                            };
                            return;
                        }
                        app.ui.global_mode = GlobalMode::Normal;
                    }
                    KeyCode::Char('n') | KeyCode::Esc => {
                        app.pending.pending_api_unload = None;
                        if matches!(kind_ref, ConfirmationKind::DeleteBackend) {
                            let new_entries = app.fetch_backend_picker_entries();
                            let new_selected = picker_selected.unwrap_or(0).min(new_entries.len().saturating_sub(1));
                            app.ui.global_mode = GlobalMode::BackendPicker {
                                entries: new_entries,
                                selected: new_selected,
                            };
                            return;
                        }
                        app.ui.global_mode = GlobalMode::Normal;
                    }
                    KeyCode::Enter => {
                        let kind_enter = kind_ref;
                        if *selected {
                            let display_name_copy = display_name.clone();
                            let detail_copy = detail.clone();
                            execute_confirmation(app, kind_enter, display_name_copy, detail_copy).await;
                            if matches!(kind_enter, ConfirmationKind::DeleteBackend) {
                                let new_entries = app.fetch_backend_picker_entries();
                                let new_selected = picker_selected.unwrap_or(0).min(new_entries.len().saturating_sub(1));
                                app.ui.global_mode = GlobalMode::BackendPicker {
                                    entries: new_entries,
                                    selected: new_selected,
                                };
                                return;
                            }
                        } else {
                            app.pending.pending_api_unload = None;
                        }
                        app.ui.global_mode = GlobalMode::Normal;
                    }
                    KeyCode::Tab | KeyCode::Left | KeyCode::Right => {
                        app.ui.global_mode = GlobalMode::Confirmation {
                            selected: !*selected,
                            kind: kind_ref,
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
                        if matches!(kind_ref, ConfirmationKind::DeleteBackend) {
                            let new_entries = app.fetch_backend_picker_entries();
                            let new_selected = picker_selected.unwrap_or(0).min(new_entries.len().saturating_sub(1));
                            app.ui.global_mode = GlobalMode::BackendPicker {
                                entries: new_entries,
                                selected: new_selected,
                            };
                            return;
                        }
                        app.ui.global_mode = GlobalMode::Normal;
                    }
                    _ => {}
                }
            }
        })
    }
}
