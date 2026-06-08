use crossterm::event::{KeyCode, KeyEvent};
use std::pin::Pin;
use std::future::Future;

use super::super::helpers::mark_settings_dirty;
use crate::config::config_base_dir;
use crate::tui::app::{App, GlobalMode};

use super::OverlayHandler;

pub struct DirectoryPickerHandler;

impl OverlayHandler for DirectoryPickerHandler {
    fn name(&self) -> &'static str {
        "DirectoryPicker"
    }

    fn can_handle(&self, mode: &GlobalMode) -> bool {
        matches!(mode, GlobalMode::DirectoryPicker { .. })
    }

    fn handle<'a>(
        &'a self,
        app: &'a mut App,
        key: KeyEvent,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            if let GlobalMode::DirectoryPicker { entries, selected } = &mut app.ui.global_mode {
                match key.code {
                    KeyCode::Enter => {
                        if entries.is_empty() {
                            app.add_log(
                                crate::t!("log.no_directories"),
                                crate::config::LogLevel::Warning,
                            );
                            app.ui.global_mode = GlobalMode::Normal;
                            return;
                        }
                        let (_, path) = &entries[*selected];
                        let files = load_jinja_files_from_dir(path);
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
                        mark_settings_dirty(app, false);
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

pub fn load_jinja_files_from_dir(dir_path: &str) -> Vec<(String, String)> {
    let mut files = Vec::new();

    if let Ok(entries) = std::fs::read_dir(dir_path) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_file()
                && path
                    .extension()
                    .map(|e| e == "jinja")
                    .unwrap_or(false)
            {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    files.push((name.to_string(), path.to_string_lossy().to_string()));
                }
            }
        }
    }

    files.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    files
}

pub fn list_directories(base_path: &str) -> Vec<(String, String)> {
    let mut dirs = Vec::new();

    if let Ok(entries) = std::fs::read_dir(base_path) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    dirs.push((name.to_string(), path.to_string_lossy().to_string()));
                }
            }
        }
    }

    dirs.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    dirs
}

pub fn default_chat_templates_dir() -> String {
    config_base_dir()
        .join("llm-manager")
        .join("chat_templates")
        .to_string_lossy()
        .to_string()
}
