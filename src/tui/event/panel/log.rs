use crossterm::event::KeyCode;

use crate::tui::app::App;

pub fn handle_log_key(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Enter if !app.log_expanded => {
            app.log_expanded = true;
            app.set_redraw();
        }
        KeyCode::Esc if app.log_expanded => {
            app.log_expanded = false;
            app.set_redraw();
        }
        KeyCode::Char('f') => {
            app.log_follow = !app.log_follow;
            app.set_redraw();
        }
        KeyCode::Char('g') | KeyCode::Home => {
            app.log_scroll_offset = 0;
            app.log_follow = false;
            app.set_redraw();
        }
        KeyCode::Char('G') | KeyCode::End => {
            app.log_follow = true;
            app.set_redraw();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.log_scroll_offset = app.log_scroll_offset.saturating_sub(1);
            app.log_follow = false;
            app.set_redraw();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.log_scroll_offset = app.log_scroll_offset + 1;
            // Get inner height (approximate, since we don't have layout here)
            // But we can just use the total lines check
            if app.log_scroll_offset >= app.log_total_lines.saturating_sub(5) {
                 app.log_follow = true;
            } else {
                 app.log_follow = false;
            }
            app.set_redraw();
        }
        KeyCode::PageUp => {
            app.log_scroll_offset = app.log_scroll_offset.saturating_sub(15);
            app.log_follow = false;
            app.set_redraw();
        }
        KeyCode::PageDown => {
            app.log_scroll_offset = app.log_scroll_offset + 15;
            if app.log_scroll_offset >= app.log_total_lines.saturating_sub(5) {
                app.log_follow = true;
            }
            app.set_redraw();
        }
        _ => {}
    }
}
