use std::future::Future;
use std::pin::Pin;

use crossterm::event::{KeyCode, KeyEvent};
use reqwest;

use super::super::helpers::{TextEditor, sync_global_settings};
use crate::tui::app::{App, GlobalMode, WebSearchCheckStatus};

use super::OverlayHandler;

pub struct WebSearchPickerHandler;

impl OverlayHandler for WebSearchPickerHandler {
    fn can_handle(&self, mode: &GlobalMode) -> bool {
        matches!(mode, GlobalMode::WebSearchPicker { .. })
    }

    fn handle<'a>(
        &'a self,
        app: &'a mut App,
        key: KeyEvent,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            if let GlobalMode::WebSearchPicker {
                enabled,
                engine,
                engine_url,
                api_key,
                selected_field,
                engine_picker_selected,
                editing,
                edit_buffer,
                edit_cursor_pos,
                check_status,
            } = &mut app.ui.global_mode
            {
                // ── Engine sub-picker ──────────────────────────────────
                if *selected_field < -1 {
                    let engines = ["searxng"];
                    match key.code {
                        KeyCode::Enter | KeyCode::Char(' ') => {
                            if *engine_picker_selected < engines.len() {
                                let chosen = engines[*engine_picker_selected].to_string();
                                *engine = chosen.clone();
                                app.config.default.web_search_engine = chosen;
                                let _ = app.config.save();
                            }
                            *selected_field = 0;
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            *engine_picker_selected = engine_picker_selected.saturating_sub(1);
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            *engine_picker_selected = (*engine_picker_selected + 1).min(engines.len() - 1);
                        }
                        KeyCode::Char(c) if c.is_ascii_digit() => {
                            let idx = c.to_digit(10).unwrap() as usize;
                            if idx < engines.len() {
                                *engine_picker_selected = idx;
                            }
                        }
                        KeyCode::Esc => {
                            *selected_field = 0;
                        }
                        _ => {}
                    }
                    return;
                }

                match key.code {
                    // ── Main picker Enter ──────────────────────────────
                    KeyCode::Enter => {
                        if *editing {
                            // Commit text edit
                            if *selected_field == 1 {
                                app.config.default.web_search_engine_url = edit_buffer.clone();
                                engine_url.clone_from(edit_buffer);
                            }
                            if *selected_field == 2 {
                                app.config.default.web_search_api_key = if edit_buffer.is_empty() {
                                    None
                                } else {
                                    Some(edit_buffer.clone())
                                };
                                *api_key = if edit_buffer.is_empty() {
                                    None
                                } else {
                                    Some(edit_buffer.clone())
                                };
                            }
                            *editing = false;
                            sync_global_settings(app);
                            return;
                        }
                        match *selected_field {
                             -1 => {
                                 // Toggle enabled
                                 *enabled = !*enabled;
                                 app.config.default.web_search_enabled = *enabled;
                                 if *enabled && !engine_url.is_empty() {
                                     let engine = engine.clone();
                                     let engine_url = engine_url.clone();
                                     let api_key = api_key.clone();
                                     *check_status = Some(WebSearchCheckStatus::Checking);
                                     app.ui.needs_redraw = true;
                                     let handle = tokio::spawn(async move {
                                         check_web_search_health(&engine, &engine_url, api_key.as_deref().unwrap_or("")).await
                                     });
                                     app.pending.web_search_check_handle = Some(handle);
                                 } else if *enabled {
                                     *check_status = None;
                                 }
                             }
                            0 => {
                                // Open engine picker
                                let current = engine.as_str();
                                let engines = ["searxng"];
                                *engine_picker_selected = engines.iter().position(|e| *e == current).unwrap_or(0);
                                *selected_field = -2; // sentinel for engine picker
                            }
                            1 => {
                                 // Edit URL
                                 edit_buffer.clone_from(engine_url);
                                 *editing = true;
                                 *edit_cursor_pos = edit_buffer.chars().count();
                                 *check_status = None;
                             }
                             2 => {
                                 // Edit API key
                                 edit_buffer.clear();
                                 if let Some(ref key) = *api_key {
                                     edit_buffer.push_str(key);
                                 }
                                 *editing = true;
                                 *edit_cursor_pos = edit_buffer.chars().count();
                                 *check_status = None;
                             }
                             _ => {}
                        }
                        sync_global_settings(app);
                    }
                    // ── Navigation ─────────────────────────────────────
                    KeyCode::Up | KeyCode::Char('k') => {
                        if !*editing {
                            *selected_field = if *selected_field <= -1 {
                                2
                            } else {
                                *selected_field - 1
                            };
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if !*editing {
                            *selected_field = if *selected_field >= 2 {
                                -1
                            } else {
                                *selected_field + 1
                            };
                        }
                    }
                    // ── Esc ────────────────────────────────────────────
                    KeyCode::Esc => {
                        if *editing {
                            *editing = false;
                            edit_buffer.clear();
                        } else {
                            app.ui.global_mode = GlobalMode::Normal;
                        }
                    }
                    // ── Text editing ───────────────────────────────────
                    KeyCode::Char(c) if *editing => {
                        TextEditor { buffer: edit_buffer, cursor: edit_cursor_pos }.insert_char(c);
                    }
                    KeyCode::Backspace if *editing => {
                        TextEditor { buffer: edit_buffer, cursor: edit_cursor_pos }.backspace();
                    }
                    KeyCode::Left if *editing => {
                        TextEditor { buffer: edit_buffer, cursor: edit_cursor_pos }.move_left();
                    }
                    KeyCode::Right if *editing => {
                        TextEditor { buffer: edit_buffer, cursor: edit_cursor_pos }.move_right();
                    }
                    KeyCode::Home if *editing => {
                        TextEditor { buffer: edit_buffer, cursor: edit_cursor_pos }.home();
                    }
                    KeyCode::End if *editing => {
                         TextEditor { buffer: edit_buffer, cursor: edit_cursor_pos }.end();
                     }
                     // ── Manual check ───────────────────────────────────
                     KeyCode::Char('c') if !*editing => {
                         if *selected_field == -1 && *enabled && !engine_url.is_empty() {
                             let engine = engine.clone();
                             let engine_url = engine_url.clone();
                             let api_key = api_key.clone();
                             *check_status = Some(WebSearchCheckStatus::Checking);
                             app.ui.needs_redraw = true;
                             let handle = tokio::spawn(async move {
                                 check_web_search_health(&engine, &engine_url, api_key.as_deref().unwrap_or("")).await
                             });
                             app.pending.web_search_check_handle = Some(handle);
                         }
                     }
                     _ => {}
                }
            }
        })
    }
}

pub async fn check_web_search_health(_engine: &str, engine_url: &str, api_key: &str) -> Result<(), String> {
    let client = reqwest::Client::new();
    let url = format!(
        "{}/search?q=test&format=json",
        engine_url.trim_end_matches('/')
    );

    let mut request = client
        .get(&url)
        .header(
            "User-Agent",
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        )
        .timeout(std::time::Duration::from_secs(10));

    if !api_key.is_empty() {
        request = request.header("Authorization", format!("Bearer {}", api_key));
    }

    let response = request.send().await.map_err(|e| format!("Connection failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("HTTP {}: {}", response.status(), response.status().canonical_reason().unwrap_or("Unknown")));
    }

    let body = response.text().await.map_err(|e| format!("Failed to read response: {}", e))?;

    match body.parse::<serde_json::Value>() {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("Invalid JSON response: {}", e)),
    }
}
