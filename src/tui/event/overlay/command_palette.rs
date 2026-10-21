use std::future::Future;
use std::pin::Pin;

use crossterm::event::{KeyCode, KeyEvent};

use super::super::helpers::TextEditor;
use crate::tui::app::{App, GlobalMode};
use crate::tui::command_palette;

use super::OverlayHandler;

pub struct CommandPaletteHandler;

/// Snapshot the palette's query and selection without keeping a borrow of
/// the overlay state (callers need `app` again right after).
fn palette_state(app: &App) -> (String, usize) {
    match &app.ui.global_mode {
        GlobalMode::CommandPalette {
            buffer, selected, ..
        } => (buffer.clone(), *selected),
        _ => (String::new(), 0),
    }
}

/// Apply a text edit to the palette buffer, then clamp the selection to the
/// (possibly changed) filter results.
fn edit_buffer(app: &mut App, edit: impl FnOnce(&mut TextEditor)) {
    if let GlobalMode::CommandPalette {
        buffer, cursor_pos, ..
    } = &mut app.ui.global_mode
    {
        edit(&mut TextEditor {
            buffer,
            cursor: cursor_pos,
        });
    }
    let (query, sel) = palette_state(app);
    let entries = command_palette::available_actions(app);
    let matches = command_palette::filter_entries(&entries, &query);
    if sel >= matches.len()
        && let GlobalMode::CommandPalette { selected, .. } = &mut app.ui.global_mode
    {
        *selected = matches.len().saturating_sub(1);
    }
}

impl OverlayHandler for CommandPaletteHandler {
    fn can_handle(&self, mode: &GlobalMode) -> bool {
        matches!(mode, GlobalMode::CommandPalette { .. })
    }

    fn handle<'a>(
        &'a self,
        app: &'a mut App,
        key: KeyEvent,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            if !matches!(app.ui.global_mode, GlobalMode::CommandPalette { .. }) {
                return;
            }
            match key.code {
                KeyCode::Esc => {
                    app.ui.global_mode = GlobalMode::Normal;
                }
                KeyCode::Enter => {
                    let (query, sel) = palette_state(app);
                    let entries = command_palette::available_actions(app);
                    let matches = command_palette::filter_entries(&entries, &query);
                    let action = matches
                        .get(sel)
                        .and_then(|&i| entries.get(i))
                        .map(|e| e.action);
                    app.ui.global_mode = GlobalMode::Normal;
                    if let Some(action) = action {
                        command_palette::execute_action(app, action).await;
                    }
                }
                KeyCode::Down | KeyCode::Up => {
                    let (query, sel) = palette_state(app);
                    let entries = command_palette::available_actions(app);
                    let matches = command_palette::filter_entries(&entries, &query);
                    if !matches.is_empty() {
                        let new_sel = if key.code == KeyCode::Down {
                            (sel + 1) % matches.len()
                        } else if sel == 0 {
                            matches.len() - 1
                        } else {
                            sel - 1
                        };
                        if let GlobalMode::CommandPalette { selected, .. } = &mut app.ui.global_mode
                        {
                            *selected = new_sel;
                        }
                    }
                }
                KeyCode::Char(c) => edit_buffer(app, |ed| ed.insert_char(c)),
                KeyCode::Backspace => edit_buffer(app, |ed| ed.backspace()),
                KeyCode::Delete => edit_buffer(app, |ed| ed.delete()),
                KeyCode::Left => edit_buffer(app, |ed| ed.move_left()),
                KeyCode::Right => edit_buffer(app, |ed| ed.move_right()),
                KeyCode::Home => edit_buffer(app, |ed| ed.home()),
                KeyCode::End => edit_buffer(app, |ed| ed.end()),
                _ => {}
            }
        })
    }
}
