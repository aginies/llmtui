use std::pin::Pin;
use std::future::Future;

use crossterm::event::{KeyCode, KeyEvent};
use arboard;

use crate::tui::app::{App, GlobalMode};

use super::OverlayHandler;

pub struct DashboardUrlHandler;

impl OverlayHandler for DashboardUrlHandler {
    fn can_handle(&self, mode: &GlobalMode) -> bool {
        matches!(mode, GlobalMode::DashboardUrl { .. })
    }

    fn handle<'a>(
        &'a self,
        app: &'a mut App,
        key: KeyEvent,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            if let GlobalMode::DashboardUrl {
                host,
                port,
                auth_key,
                tls_enabled,
                ..
            } = &app.ui.global_mode
            {
                match key.code {
                    KeyCode::Esc => {
                        app.ui.global_mode = GlobalMode::Normal;
                    }
                    KeyCode::Enter => {
                        let host_val = crate::models::format_host(host);
                        let mut url = format!(
                            "{}://{}:{}/dashboard",
                            if *tls_enabled { "https" } else { "http" },
                            host_val,
                            port
                        );
                        if !auth_key.is_empty() {
                            url.push_str(&format!("?auth={}", auth_key));
                        }
                        let cb = arboard::Clipboard::new();
                        if let Ok(mut cb) = cb {
                            let _ = cb.set().text(&url);
                        }
                        app.ui.global_mode = GlobalMode::Normal;
                    }
                    _ => {}
                }
            }
        })
    }
}
