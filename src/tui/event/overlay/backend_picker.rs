use std::pin::Pin;
use std::future::Future;

use crossterm::event::{KeyCode, KeyEvent};

use super::super::helpers::{sync_global_settings, picker_nav_up, picker_nav_down};
use crate::tui::app::{App, ConfirmationKind, GlobalMode};

use super::OverlayHandler;

pub struct BackendPickerHandler;

impl OverlayHandler for BackendPickerHandler {
    fn can_handle(&self, mode: &GlobalMode) -> bool {
        matches!(mode, GlobalMode::BackendPicker { .. })
    }

    fn handle<'a>(
        &'a self,
        app: &'a mut App,
        key: KeyEvent,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            if let GlobalMode::BackendPicker { entries, selected } = &mut app.ui.global_mode {
                match key.code {
                    KeyCode::Esc => {
                        app.ui.global_mode = GlobalMode::Normal;
                    }
                    KeyCode::Up | KeyCode::Char('k') => picker_nav_up(selected),
                    KeyCode::Down | KeyCode::Char('j') => picker_nav_down(selected, entries.len()),
                    KeyCode::Enter => {
                        let (backend, tag) = entries[*selected].clone();
                        app.settings.backend = backend;
                        app.settings.set_active_backend_version(tag.clone());
                        if !crate::backend::hub::is_backend_version_installed(backend, tag.as_deref()) {
                            app.pending.backend_resolving = true;
                            let tag_param = tag.clone();
                            if app.download.download_rx.is_none() {
                                let (tx, rx) = tokio::sync::broadcast::channel(10);
                                app.download.download_tx = Some(tx);
                                app.download.download_rx = Some(rx);
                            }
                            let (log_tx, log_rx) = tokio::sync::mpsc::channel(100);
                            app.server.server_log_rx = Some(log_rx);
                            let tx = app.download.download_tx.clone();
                            let handle = tokio::spawn(async move {
                                crate::backend::hub::resolve_backend_binary(
                                    backend,
                                    tag_param.as_deref(),
                                    Some(log_tx),
                                    tx,
                                )
                                .await
                                .map_err(|e| e.to_string())
                            });
                            app.pending.backend_resolve_handle = Some(handle);
                        } else {
                            app.pending.backend_resolving = false;
                        }
                        app.ui.global_mode = GlobalMode::Normal;
                        sync_global_settings(app);
                    }
                    KeyCode::Char('d') | KeyCode::Delete => {
                        if let Some((backend, Some(tag))) = entries.get(*selected) {
                            let backend_slug = backend.slug().to_string();
                            app.ui.global_mode = GlobalMode::Confirmation {
                                selected: false,
                                kind: ConfirmationKind::DeleteBackend,
                                display_name: format!("{} ({})", backend_slug, tag),
                                detail: Some(format!("{}:{}", backend_slug, tag)),
                            };
                        }
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
