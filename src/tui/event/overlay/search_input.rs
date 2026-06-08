use std::pin::Pin;
use std::future::Future;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::widgets::TableState;

use super::super::helpers::TextEditor;
use crate::tui::app::pending_events::PendingEvent;
use crate::tui::app::{App, GlobalMode, ModelsMode};

use super::OverlayHandler;

pub struct SearchInputHandler;

impl OverlayHandler for SearchInputHandler {
    fn can_handle(&self, mode: &GlobalMode) -> bool {
        matches!(mode, GlobalMode::SearchInput { .. })
    }

    fn handle<'a>(
        &'a self,
        app: &'a mut App,
        key: KeyEvent,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            if let GlobalMode::SearchInput { buffer, cursor_pos } = &mut app.ui.global_mode {
                match key.code {
                    KeyCode::Esc => {
                        app.ui.global_mode = GlobalMode::Normal;
                        return;
                    }
                    KeyCode::Enter => {
                        let query = buffer.clone();
                        app.ui.global_mode = GlobalMode::Normal;
                        app.search.search_input = Some(query.clone());

                        if let ModelsMode::Search {
                            query: q,
                            page,
                            has_more,
                            ..
                        } = &mut app.models_mode
                        {
                            *q = query.clone();
                            *page = 0;
                            *has_more = true;
                        }

                        if query.is_empty() {
                            return;
                        }
                        app.add_log(
                            format!("Searching for '{}'...", query),
                            crate::config::LogLevel::Info,
                        );
                        let _ = app
                            .pending_tx
                            .send(PendingEvent::Search {
                                query: query.clone(),
                                offset: 0,
                            })
                            .await;
                        app.search.search_table_state = TableState::default();
                        app.search.search_results_idx = None;
                        return;
                    }
                    KeyCode::Char(c) => {
                        TextEditor {
                            buffer,
                            cursor: cursor_pos,
                        }
                        .insert_char(c);
                    }
                    KeyCode::Backspace => {
                        TextEditor {
                            buffer,
                            cursor: cursor_pos,
                        }
                        .backspace();
                    }
                    KeyCode::Delete => {
                        TextEditor {
                            buffer,
                            cursor: cursor_pos,
                        }
                        .delete();
                    }
                    KeyCode::Left => {
                        TextEditor {
                            buffer,
                            cursor: cursor_pos,
                        }
                        .move_left();
                    }
                    KeyCode::Right => {
                        TextEditor {
                            buffer,
                            cursor: cursor_pos,
                        }
                        .move_right();
                    }
                    KeyCode::Home => {
                        TextEditor {
                            buffer,
                            cursor: cursor_pos,
                        }
                        .home();
                    }
                    KeyCode::End => {
                        TextEditor {
                            buffer,
                            cursor: cursor_pos,
                        }
                        .end();
                    }
                    _ => {}
                }
                app.search.search_input = Some(buffer.clone());
            }
        })
    }
}
