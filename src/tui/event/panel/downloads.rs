use crossterm::event::KeyCode;

use crate::tui::app::App;

pub fn handle_downloads_key(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            app.download.download_scroll_state.select_previous();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.download.download_scroll_state.select_next();
        }
        KeyCode::Char('c') if key.modifiers.contains(crossterm::event::KeyModifiers::ALT) => {
            if let Some(idx) = app.download.download_scroll_state.selected() {
                app.cancel_download(idx);
            }
        }
        _ => {}
    }
}
