use super::types::App;
use crate::tui::event::helpers::sync_global_settings;

impl App {
    /// Apply a profile's settings to the current settings.
    pub fn apply_profile(&mut self, profile: &crate::config::Profile) {
        self.settings = profile.apply(self.settings.clone());
        self.resolve_system_prompt();
        self.settings_state.settings_render_cache = None;
        self.add_log(
            format!("Applied profile: {}", profile.name),
            crate::config::LogLevel::Info,
        );
    }

    /// Resolve system_prompt from the preset name.
    pub fn resolve_system_prompt(&mut self) {
        if let Some(content) = self
            .config
            .get_preset_content(&self.settings.system_prompt_preset_name)
        {
            self.settings.system_prompt = content;
        }
    }

    /// Save current settings as an override for the selected model.
    pub fn save_model_settings(&mut self) {
        if let Some(model) = self.selected_model() {
            let name = model.display_name.clone();
            let override_cfg = crate::config::ModelOverride::from_settings(&self.settings);
            self.config.model_overrides.save(&name, &override_cfg);
            if let Err(e) = self.config.save() {
                self.add_log(
                    format!("Failed to save settings for {}: {}", name, e),
                    crate::config::LogLevel::Error,
                );
            } else {
                self.add_log(
                    format!("Saved settings for {}", name),
                    crate::config::LogLevel::Info,
                );
                // Update the cache so it reflects the newly saved settings
                self.model_settings_cache = self.settings.clone();
                // Also sync global settings so is_settings_dirty() returns false
                sync_global_settings(self);
            }
        } else {
            self.add_log(
                "No model selected to save settings for",
                crate::config::LogLevel::Warning,
            );
        }
        self.settings_state.settings_render_cache = None;
    }

    /// Save current settings into the given per-model profile (overwrite).
    pub fn save_to_profile(&mut self, profile_name: &str) {
        let model = match self.selected_model() {
            Some(m) => m.display_name.clone(),
            None => return,
        };

        let description = self
            .config
            .model_settings_profiles
            .get(&model, profile_name)
            .map(|p| p.description.clone())
            .unwrap_or_default();

        let profile = crate::models::ModelSettingsProfile {
            name: profile_name.to_string(),
            description,
            settings: self.settings.clone(),
        };
        self.config.model_settings_profiles.save(&model, &profile);
        if let Err(e) = self.config.save() {
            self.add_log(
                format!("Failed to save profile '{}': {}", profile_name, e),
                crate::config::LogLevel::Error,
            );
        } else {
            self.model_settings_cache = self.settings.clone();
            self.settings_state.settings_render_cache = None;
            self.add_log(
                format!("Saved settings to profile: {}", profile_name),
                crate::config::LogLevel::Info,
            );
        }
    }

    /// Save current settings: to the active per-model profile if one is
    /// selected, otherwise to the model's base settings (override).
    pub fn save_current_settings(&mut self) {
        if let Some(profile) = self.active_settings_profile() {
            self.save_to_profile(&profile.name);
        } else {
            self.save_model_settings();
        }
    }

    /// Check if any LLM settings have been modified since last save.
    /// If a per-model profile is active, compare against it; otherwise compare
    /// against the model's base settings cache.
    pub fn is_settings_dirty(&self) -> bool {
        if let Some(profile) = self.active_settings_profile() {
            return self.settings != profile.settings;
        }
        self.settings.is_dirty(&self.model_settings_cache)
    }

    // ── Per-model LLM settings profiles ──────────────────────────

    /// Get the active per-model settings profile for the selected model.
    pub fn active_settings_profile(&self) -> Option<crate::models::ModelSettingsProfile> {
        let model = self.selected_model()?;
        let profile_name = self.active_settings_profiles.get(&model.display_name)?;
        self.config
            .model_settings_profiles
            .get(&model.display_name, profile_name)
    }

    /// Open the per-model settings profile picker.
    /// A "+ New profile" pseudo-entry is prepended at index 0; selecting it
    /// opens the save-as-profile dialog.
    pub fn open_model_settings_picker(&mut self) {
        let Some(model) = self.selected_model() else {
            return;
        };
        let model_name = model.display_name.clone();
        let profile_names = self
            .config
            .model_settings_profiles
            .profile_names(&model_name);

        let mut entries: Vec<(String, String)> = Vec::with_capacity(profile_names.len() + 1);
        entries.push((
            crate::t!("dialog.model_settings_picker.new_entry").to_string(),
            String::new(),
        ));
        entries.extend(profile_names.iter().map(|name| {
            let desc = self
                .config
                .model_settings_profiles
                .get(&model_name, name)
                .map(|p| p.description.clone())
                .unwrap_or_default();
            (name.clone(), desc)
        }));

        let selected = self
            .active_settings_profile()
            .map(|p| p.name)
            .and_then(|active| entries.iter().position(|(n, _)| n == &active))
            .unwrap_or(0);

        self.ui.global_mode =
            crate::tui::app::types::GlobalMode::ModelSettingsPicker { entries, selected };
    }

    /// Load settings from the active per-model profile (or base settings if none).
    pub fn load_settings_for_model(&mut self) {
        let model = match self.selected_model() {
            Some(m) => m.display_name.clone(),
            None => return,
        };

        let settings = self.config.resolve_settings_with_profile(
            Some(&model),
            self.active_settings_profiles
                .get(&model)
                .map(|s| s.as_str()),
        );

        self.model_settings_cache = settings.clone();
        self.settings = settings;
        self.update_model_metadata();
        self.update_vram_estimate();
        self.settings_state.settings_render_cache = None;
    }

    /// Save current LLM settings as a new per-model profile.
    /// The name and description are read from the ProfileCreate global mode.
    pub fn save_as_profile(&mut self) -> bool {
        let model = match self.selected_model() {
            Some(m) => m.display_name.clone(),
            None => return false,
        };

        let (name, description) = match &self.ui.global_mode {
            crate::tui::app::types::GlobalMode::ProfileCreate {
                name, description, ..
            } => (name.clone(), description.clone()),
            _ => return false,
        };

        if name.trim().is_empty() {
            self.add_log("Profile name is required", crate::config::LogLevel::Warning);
            return false;
        }

        // Check for duplicate name
        let existing = self.config.model_settings_profiles.profile_names(&model);
        if existing.contains(&name) {
            self.add_log(
                format!("Profile '{}' already exists", name),
                crate::config::LogLevel::Warning,
            );
            return false;
        }

        let profile = crate::models::ModelSettingsProfile {
            name: name.clone(),
            description,
            settings: self.settings.clone(),
        };
        self.config.model_settings_profiles.save(&model, &profile);
        if let Err(e) = self.config.save() {
            self.add_log(
                format!("Failed to save profile: {}", e),
                crate::config::LogLevel::Error,
            );
            false
        } else {
            self.active_settings_profiles
                .insert(model.clone(), name.clone());
            self.model_settings_cache = self.settings.clone();
            self.settings_state.settings_render_cache = None;
            self.add_log(
                format!("Saved settings as profile: {}", name),
                crate::config::LogLevel::Info,
            );
            true
        }
    }

    /// Apply a per-model profile by name to the current model.
    pub fn apply_profile_by_name(&mut self, profile_name: &str) {
        let model = match self.selected_model() {
            Some(m) => m.display_name.clone(),
            None => return,
        };

        self.active_settings_profiles
            .insert(model, profile_name.to_string());
        self.load_settings_for_model();
        self.add_log(
            format!("Applied profile: {}", profile_name),
            crate::config::LogLevel::Info,
        );
    }

    /// Delete a per-model settings profile for the selected model.
    pub fn delete_settings_profile(&mut self, profile_name: &str) -> bool {
        let model = match self.selected_model() {
            Some(m) => m.display_name.clone(),
            None => return false,
        };

        if profile_name == "Default" {
            self.add_log(
                "Cannot delete the Default profile",
                crate::config::LogLevel::Info,
            );
            return false;
        }

        self.config
            .model_settings_profiles
            .delete(&model, profile_name);
        if let Err(e) = self.config.save() {
            self.add_log(
                format!("Failed to delete profile: {}", e),
                crate::config::LogLevel::Error,
            );
            return false;
        }

        // If the deleted profile was active for this model, clear it and reload base settings
        if self
            .active_settings_profiles
            .get(&model)
            .map(|s| s.as_str())
            == Some(profile_name)
        {
            self.active_settings_profiles.remove(&model);
            self.load_settings_for_model();
        }

        self.add_log(
            format!("Deleted profile: {}", profile_name),
            crate::config::LogLevel::Info,
        );
        true
    }

    /// Return the settings version counter for cache invalidation.
    pub fn settings_fingerprint(&self) -> u64 {
        self.settings_state.settings_version
    }

    /// Delete a user profile by index in the merged display list.
    /// Returns true if a profile was deleted, false otherwise.
    pub fn delete_profile(&mut self, selected_idx: usize) -> bool {
        let builtin = crate::config::builtin_profiles();
        let all_profiles = self.config.profiles.all();

        // Check if selection is valid
        if selected_idx >= all_profiles.len() {
            self.add_log(crate::t!("profiles.invalid"), crate::config::LogLevel::Info);
            return false;
        }

        // Check if it's a built-in profile
        if selected_idx < builtin.len() {
            self.add_log(
                "Cannot delete built-in profiles",
                crate::config::LogLevel::Info,
            );
            return false;
        }

        let profile = all_profiles[selected_idx].clone();
        let profile_name = profile.name.clone();

        self.config.profiles.delete(&profile_name);

        if let Err(e) = self.config.save() {
            self.add_log(
                format!("Failed to delete profile: {}", e),
                crate::config::LogLevel::Error,
            );
            return false;
        }

        self.add_log(
            format!("Deleted profile: {}", profile_name),
            crate::config::LogLevel::Info,
        );
        true
    }

    pub fn get_api_port_str(&self) -> String {
        let port = self.settings.api_endpoint_port;
        let mut cache = super::types::API_PORT_CACHE
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if cache.0 == port && !cache.1.is_empty() {
            return cache.1.clone();
        }
        cache.0 = port;
        cache.1 = port.to_string();
        cache.1.clone()
    }
}
