use crossterm::event::KeyCode;

use crate::config::builtin_profiles;
use crate::tui::app::App;

pub fn handle_profiles_key(app: &mut App, key: crossterm::event::KeyEvent) {
    let builtin = builtin_profiles();
    
    // Build merged profile list (same as render logic)
    let mut all_profiles: Vec<crate::config::Profile> = builtin.to_vec();
    for p in &app.config.profiles {
        if !builtin.iter().any(|b| b.name == p.name) {
            all_profiles.push(p.clone());
        }
    }
    let total = all_profiles.len();

    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            app.settings_selected_idx = app.settings_selected_idx.saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if total > 0 {
                app.settings_selected_idx = (app.settings_selected_idx + 1).min(total - 1);
            }
        }
        KeyCode::PageUp => {
            app.profiles_scroll_offset = app.profiles_scroll_offset.saturating_sub(5);
        }
        KeyCode::PageDown => {
            app.profiles_scroll_offset = app.profiles_scroll_offset.saturating_add(5);
        }
        KeyCode::Enter => {
            // Use the merged list for applying profiles
            if let Some(profile) = all_profiles.get(app.settings_selected_idx) {
                let profile = profile.clone();
                app.apply_profile(&profile);
                app.active_panel = crate::tui::app::ActivePanel::LlmSettings;
            }
        }
        KeyCode::Char('s') => {
            // Save current settings as a new profile
            app.save_current_as_profile("New Profile");
            app.active_panel = crate::tui::app::ActivePanel::LlmSettings;
        }
        KeyCode::Char('d') => {
            // Delete the selected user profile (not built-in)
            if app.delete_profile(app.settings_selected_idx) {
                let new_total = app.config.merged_profiles().len();
                if new_total > 0 && app.settings_selected_idx >= new_total {
                    app.settings_selected_idx = new_total - 1;
                }
            }
        }
        KeyCode::Esc => {
            app.active_panel = crate::tui::app::ActivePanel::LlmSettings;
        }
        _ => {}
    }
}
