use crossterm::event::KeyCode;

use crate::tui::app::App;

pub fn handle_log_key(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Enter if !app.log.log_expanded => {
            app.log.log_expanded = true;
        }
        KeyCode::Esc if app.log.log_expanded => {
            app.log.log_expanded = false;
        }
        KeyCode::Char('f') => {
            app.log.log_follow = !app.log.log_follow;
        }
        KeyCode::Char('g') | KeyCode::Home => {
            app.log.log_scroll_offset = 0;
            app.log.log_follow = false;
        }
        KeyCode::Char('G') | KeyCode::End => {
            app.log.log_follow = true;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.log.log_scroll_offset = app.log.log_scroll_offset.saturating_sub(1);
            app.log.log_follow = false;
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.log.log_scroll_offset += 1;
            // Get inner height (approximate, since we don't have layout here)
            // But we can just use the total lines check
            app.log.log_follow =
                app.log.log_scroll_offset >= app.log.log_total_lines.saturating_sub(5);
        }
        KeyCode::PageUp => {
            app.log.log_scroll_offset = app.log.log_scroll_offset.saturating_sub(15);
            app.log.log_follow = false;
        }
        KeyCode::PageDown => {
            app.log.log_scroll_offset += 15;
            if app.log.log_scroll_offset >= app.log.log_total_lines.saturating_sub(5) {
                app.log.log_follow = true;
            }
        }
        _ => {}
    }
}
