use crossterm::event::KeyCode;

use crate::tui::app::App;

pub fn handle_downloads_key(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            app.download.download_scroll_state.select_previous();
            app.set_redraw();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.download.download_scroll_state.select_next();
            app.set_redraw();
        }
        KeyCode::Char('c')
            if key.modifiers.contains(crossterm::event::KeyModifiers::ALT) =>
        {
            if let Some(idx) = app.download.download_scroll_state.selected() {
                         let name = app.download.download_progress[idx].filename.clone();
                         let dest = app.download.download_progress[idx].dest.take();
                         if let Some(token) = app.download.download_progress[idx].cancel_token.as_ref() {
                             token.store(true, std::sync::atomic::Ordering::Relaxed);
                         }
                         app.download.download_progress[idx].download_state = 3;
                         app.download.download_progress[idx].cancelled = true;
                         app.download.download_progress[idx].status = crate::models::DownloadStatus::Cancelled;
                         if let Some(ref path) = dest {
                             if path.exists() {
                                 if let Err(e) = std::fs::remove_file(path) {
                                     app.add_log(format!("Failed to remove temp file {}: {}", path.display(), e), crate::config::LogLevel::Warning);
                                 } else {
                                     app.add_log(format!("Removed temp file: {}", path.display()), crate::config::LogLevel::Info);
                                 }
                             }
                         }
                         app.download.download_progress.remove(idx);
                         app.download.downloading = !app.download.download_progress.is_empty();
                         if !app.download.downloading {
                             app.download.download_scroll_state.select(None);
                         } else if let Some(selected_idx) = app.download.download_scroll_state.selected()
                             && selected_idx >= app.download.download_progress.len() {
                                 app.download.download_scroll_state.select(Some(app.download.download_progress.len() - 1));
                         }
                         app.add_log(format!("Cancelled download of {}...", name), crate::config::LogLevel::Info);
                     }
                app.set_redraw();
            }
        _ => {}
    }
}
