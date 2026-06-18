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

    /// Check if global settings match config defaults.
    /// Returns true when all global-scoped fields equal their config.default values.
    pub fn is_global_settings_same_as_config(&self) -> bool {
        let d = &self.config.default;
        self.settings.host == d.host
            && self.settings.port == d.port
            && self.settings.backend == d.backend
            && self.settings.parallel == d.parallel
            && self.settings.max_concurrent_predictions == d.max_concurrent_predictions
            && self.settings.threads == d.threads
            && self.settings.threads_batch == d.threads_batch
            && self.settings.api_endpoint_enabled == d.api_endpoint_enabled
            && self.settings.api_endpoint_port == d.api_endpoint_port
            && self.settings.api_endpoint_key == d.api_endpoint_key
            && self.server_mode == d.server_mode
            && self.router_max_models == d.router_max_models
            && self.settings.llama_cpp_version_cpu == d.llama_cpp_version_cpu
            && self.settings.llama_cpp_version_vulkan == d.llama_cpp_version_vulkan
            && self.settings.llama_cpp_version_rocm == d.llama_cpp_version_rocm
            && self.settings.llama_cpp_version_rocm_lemonade == d.llama_cpp_version_rocm_lemonade
            && self.settings.llama_cpp_version_cuda == d.llama_cpp_version_cuda
    }

    /// Check if any LLM settings have been modified since last save.
    pub fn is_settings_dirty(&self) -> bool {
        self.settings.is_dirty(&self.model_settings_cache)
            || !self.is_global_settings_same_as_config()
    }

    /// Compute a fingerprint of the current settings for cache invalidation.
    pub fn settings_fingerprint(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        self.settings.hash(&mut h);
        self.settings_state.settings_edit_buffer.hash(&mut h);
        self.settings_state.expert_mode.hash(&mut h);
        h.finish()
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
