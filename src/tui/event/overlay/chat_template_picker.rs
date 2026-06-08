use std::pin::Pin;
use std::future::Future;

use crossterm::event::{KeyCode, KeyEvent};

use super::super::helpers::mark_settings_dirty;
use crate::tui::app::{App, GlobalMode};

use super::OverlayHandler;

pub struct ChatTemplatePickerHandler;

impl OverlayHandler for ChatTemplatePickerHandler {
    fn name(&self) -> &'static str {
        "ChatTemplatePicker"
    }

    fn can_handle(&self, mode: &GlobalMode) -> bool {
        matches!(mode, GlobalMode::ChatTemplatePicker { .. })
    }

    fn handle<'a>(
        &'a self,
        app: &'a mut App,
        key: KeyEvent,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            if let GlobalMode::ChatTemplatePicker {
                entries,
                selected,
                edit_buffer,
            } = &mut app.ui.global_mode
            {
                match key.code {
                    KeyCode::Enter => {
                        let entry = &entries[*selected];
                        match entry.as_str() {
                            "Auto (detect)" => {
                                app.settings.auto_chat_template = true;
                                app.settings.chat_template = None;
                            }
                            "None" => {
                                app.settings.auto_chat_template = false;
                                app.settings.chat_template = None;
                            }
                            "Custom..." => {
                                app.settings.auto_chat_template = false;
                                app.settings.chat_template =
                                    if edit_buffer.is_empty() {
                                        None
                                    } else {
                                        Some(edit_buffer.clone())
                                    };
                            }
                            _ => {
                                app.settings.auto_chat_template = false;
                                app.settings.chat_template = Some(entry.clone());
                            }
                        }
                        mark_settings_dirty(app, false);
                        app.ui.global_mode = GlobalMode::Normal;
                    }
                    KeyCode::Up => {
                        *selected = selected.saturating_sub(1);
                    }
                    KeyCode::Down => {
                        *selected = (*selected + 1).min(entries.len().saturating_sub(1));
                    }
                    KeyCode::Esc => {
                        app.ui.global_mode = GlobalMode::Normal;
                    }
                    _ => {}
                }
            }
        })
    }
}
