use std::pin::Pin;
use std::future::Future;

use crossterm::event::{KeyCode, KeyEvent};

use super::super::helpers::mark_settings_dirty;
use crate::tui::app::{App, GlobalMode};

use super::OverlayHandler;

pub struct ChatTemplatePickerHandler;

impl OverlayHandler for ChatTemplatePickerHandler {
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
                ..
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
                            "Browse directory..." => {
                                let base_path = super::directory_picker::default_chat_templates_dir();
                                let files = super::directory_picker::load_jinja_files_recursive(&base_path);
                                if files.is_empty() {
                                    app.add_log(
                                        crate::t!("log.no_jinja_files"),
                                        crate::config::LogLevel::Warning,
                                    );
                                    app.ui.global_mode = GlobalMode::Normal;
                                    return;
                                }
                                app.ui.global_mode = GlobalMode::ChatTemplateFilePicker {
                                    entries: files,
                                    selected: 0,
                                };
                                return;
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
