use crossterm::event::KeyCode;

use crate::tui::app::{App, ModelsMode};

pub async fn fetch_and_store_readme(app: &mut App, model_id: String) {
    match crate::backend::hub::fetch_readme(&model_id).await {
        Ok(readme) => {
            if let ModelsMode::Search { results, .. } = &mut app.models_mode
                && let Some(idx) = app.search_results_idx
                && let Some(r) = results.get_mut(idx)
            {
                r.readme = Some(readme);
            }
            app.add_log("README loaded.", crate::config::LogLevel::Info);
        }
        Err(e) => {
            app.add_log(format!("Failed to fetch README: {}", e), crate::config::LogLevel::Error);
        }
    }
}

pub async fn fetch_readme_for_selected(app: &mut App, model_id: String) {
    if let ModelsMode::Search { results, show_readme, .. } = &app.models_mode
        && *show_readme
            && let Some(idx) = app.search_results_idx
                && let Some(r) = results.get(idx)
                    && r.readme.is_none() {
                        app.add_log(format!("Fetching README for {}...", model_id), crate::config::LogLevel::Info);
                        fetch_and_store_readme(app, model_id).await;
                    }
}

pub fn handle_readme_key(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            if let ModelsMode::Search { show_readme, .. } = &mut app.models_mode {
                *show_readme = false;
                app.active_panel = crate::tui::app::ActivePanel::Models;
            }
            if let ModelsMode::Files { .. } = &app.models_mode {
                app.active_panel = crate::tui::app::ActivePanel::Models;
            }
            app.set_redraw();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.readme_scroll_offset = app.readme_scroll_offset.saturating_sub(1);
            app.set_redraw();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.readme_scroll_offset = app.readme_scroll_offset.saturating_add(1);
            app.set_redraw();
        }
        _ => {}
    }
}
