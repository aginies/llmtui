use std::future::Future;
use std::pin::Pin;

use arboard;
use crossterm::event::{KeyCode, KeyEvent};

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
                ws_port,
                api_port,
                llm_port,
                auth_key,
                ws_enabled: _,
                tls_enabled,
            } = &app.ui.global_mode
            {
                match key.code {
                    KeyCode::Esc => {
                        app.ui.global_mode = GlobalMode::Normal;
                    }
                    KeyCode::Enter => {
                        let host_val = crate::models::format_host(host);
                        let api_url = format!(
                            "{}://{}:{}",
                            if *tls_enabled { "https" } else { "http" },
                            host_val,
                            api_port
                        );
                        let metrics_url = format!(
                            "http://{}:{}",
                            host_val,
                            llm_port
                        );
                        let mut dashboard_url = format!(
                            "{}://{}:{}/dashboard",
                            if *tls_enabled { "https" } else { "http" },
                            host,
                            ws_port
                        );
                        if !auth_key.is_empty() {
                            dashboard_url.push_str(&format!("?auth={}", auth_key));
                        }
                        let all_urls = format!("{}\n{}\n{}", api_url, metrics_url, dashboard_url);
                        let cb = arboard::Clipboard::new();
                        if let Ok(mut cb) = cb {
                            let _ = cb.set().text(&all_urls);
                        }
                        app.ui.global_mode = GlobalMode::Normal;
                    }
                    _ => {}
                }
            }
        })
    }
}
