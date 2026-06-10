use std::future::Future;
use std::pin::Pin;

use crossterm::event::{KeyCode, KeyEvent};

use crate::tui::app::{App, GlobalMode};

use super::OverlayHandler;

pub struct CmdLineHandler;

impl OverlayHandler for CmdLineHandler {
    fn can_handle(&self, mode: &GlobalMode) -> bool {
        matches!(mode, GlobalMode::CmdLine { .. })
    }

    fn handle<'a>(
        &'a self,
        app: &'a mut App,
        key: KeyEvent,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            if let GlobalMode::CmdLine { cmd_line: _ } = &app.ui.global_mode {
                match key.code {
                    KeyCode::Esc => {
                        app.ui.global_mode = GlobalMode::Normal;
                    }
                    KeyCode::Char('e') => {
                        if let GlobalMode::CmdLine { cmd_line } = &app.ui.global_mode {
                            let script = format!(
                                "#!/bin/bash\n# Exported from llm-manager\n\n{}\n",
                                cmd_line
                            );
                            if let Err(e) = std::fs::write("/tmp/test_llamaserver.sh", &script) {
                                app.add_log(
                                    format!("Failed to write script: {}", e),
                                    crate::config::LogLevel::Error,
                                );
                            } else {
                                app.add_log(
                                    "Wrote server command to /tmp/test_llamaserver.sh",
                                    crate::config::LogLevel::Info,
                                );
                            }
                        }
                    }
                    _ => {}
                }
            }
        })
    }
}
