use std::future::Future;
use std::pin::Pin;

use crossterm::event::{KeyCode, KeyEvent};

use super::super::helpers::{mark_settings_dirty, picker_nav_down, picker_nav_up, sync_global_settings};
use crate::tui::app::{App, GlobalMode};

use super::OverlayHandler;

/// F8 proposal picker: choose one of the feasible ctx / KV-cache / MTP
/// proposals and apply it to the selected model's settings.
pub struct CtxProposalPickerHandler;

impl OverlayHandler for CtxProposalPickerHandler {
    fn can_handle(&self, mode: &GlobalMode) -> bool {
        matches!(mode, GlobalMode::CtxProposalPicker { .. })
    }

    fn handle<'a>(
        &'a self,
        app: &'a mut App,
        key: KeyEvent,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            if let GlobalMode::CtxProposalPicker {
                proposals,
                selected,
                ..
            } = &mut app.ui.global_mode
            {
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => picker_nav_up(selected),
                    KeyCode::Down | KeyCode::Char('j') => {
                        picker_nav_down(selected, proposals.len())
                    }
                    KeyCode::Home => *selected = 0,
                    KeyCode::End => *selected = proposals.len().saturating_sub(1),
                    KeyCode::Enter => {
                        if let Some(p) = proposals.get(*selected).copied() {
                            app.settings.context_length = p.ctx;
                            app.settings.cache_type_k = Some(p.cache_k);
                            app.settings.cache_type_v = Some(p.cache_v);
                            if let Some(dt) = p.mtp_draft_tokens {
                                app.settings.spec_type = "draft-mtp".to_string();
                                app.settings.draft_tokens = dt;
                            }
                            app.ui.global_mode = GlobalMode::Normal;
                            sync_global_settings(app);
                            mark_settings_dirty(app, true);
                            app.add_log(
                                crate::t_fmt!(
                                    "dialog.ctx_proposal.applied",
                                    crate::tui::format_number(u64::from(p.ctx)),
                                    p.cache_k.to_string(),
                                    p.cache_v.to_string()
                                ),
                                crate::config::LogLevel::Info,
                            );
                        }
                    }
                    KeyCode::Esc | KeyCode::Char('q') => {
                        app.ui.global_mode = GlobalMode::Normal;
                    }
                    _ => {}
                }
            }
        })
    }
}
