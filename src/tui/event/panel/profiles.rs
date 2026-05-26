use crossterm::event::KeyCode;

use crate::tui::app::App;

pub fn handle_profiles_key(app: &mut App, key: crossterm::event::KeyEvent) {
    let all_profiles = app.config.merged_profiles();
    let total = all_profiles.len();

    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            app.settings_state.settings_selected_idx = app.settings_state.settings_selected_idx.saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if total > 0 {
                app.settings_state.settings_selected_idx = (app.settings_state.settings_selected_idx + 1).min(total - 1);
            }
        }
        KeyCode::PageUp => {
            app.picker.profiles_scroll_offset = app.picker.profiles_scroll_offset.saturating_sub(5);
        }
        KeyCode::PageDown => {
            app.picker.profiles_scroll_offset = app.picker.profiles_scroll_offset.saturating_add(5);
        }
        KeyCode::Enter => {
            // Use the merged list for applying profiles
            if let Some(profile) = all_profiles.get(app.settings_state.settings_selected_idx) {
                let profile = profile.clone();
                app.apply_profile(&profile);
                app.ui.active_panel = crate::tui::app::ActivePanel::LlmSettings;
            }
        }
        KeyCode::Char('s') => {
            // Save current settings as a new profile
            app.save_current_as_profile("New Profile");
            app.ui.active_panel = crate::tui::app::ActivePanel::LlmSettings;
        }
        KeyCode::Char('d') => {
            // Delete the selected user profile (not built-in)
            if app.delete_profile(app.settings_state.settings_selected_idx) {
                let new_total = app.config.merged_profiles().len();
                if new_total > 0 && app.settings_state.settings_selected_idx >= new_total {
                    app.settings_state.settings_selected_idx = new_total - 1;
                }
            }
        }
        KeyCode::Esc => {
            app.ui.active_panel = crate::tui::app::ActivePanel::LlmSettings;
        }
        _ => {}
    }
}
