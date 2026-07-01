use crossterm::event::{KeyCode, KeyEvent};
use std::future::Future;
use std::pin::Pin;

use super::super::helpers::{mark_settings_dirty, picker_nav_down, picker_nav_up};
use crate::config::LogLevel;
use crate::tui::app::{App, GlobalMode};

use super::OverlayHandler;

pub struct ChatTemplateFilePickerHandler;

impl OverlayHandler for ChatTemplateFilePickerHandler {
    fn can_handle(&self, mode: &GlobalMode) -> bool {
        matches!(mode, GlobalMode::ChatTemplateFilePicker { .. })
    }

    fn handle<'a>(
        &'a self,
        app: &'a mut App,
        key: KeyEvent,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            if let GlobalMode::ChatTemplateFilePicker { entries, selected } =
                &mut app.ui.global_mode
            {
                match key.code {
                    KeyCode::Enter => {
                        if entries.is_empty() {
                            app.add_log(crate::t!("log.no_jinja_files"), LogLevel::Warning);
                            app.ui.global_mode = GlobalMode::ChatTemplatePicker {
                                entries: crate::models::get_available_chat_templates(),
                                selected: 0,
                            };
                            return;
                        }
                        let (_, path) = &entries[*selected];
                        app.settings.auto_chat_template = false;
                        app.settings.chat_template = Some(path.clone());
                        mark_settings_dirty(app, false);
                        app.ui.global_mode = GlobalMode::Normal;
                    }
                    KeyCode::Up | KeyCode::Char('k') => picker_nav_up(selected),
                    KeyCode::Down | KeyCode::Char('j') => picker_nav_down(selected, entries.len()),
                    KeyCode::Esc => {
                        app.ui.global_mode = GlobalMode::ChatTemplatePicker {
                            entries: crate::models::get_available_chat_templates(),
                            selected: 0,
                        };
                    }
                    _ => {}
                }
            }
        })
    }
}
