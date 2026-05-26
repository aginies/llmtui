use super::types::App;

impl App {
    /// Apply a profile's settings to the current settings.
    pub fn apply_profile(&mut self, profile: &crate::config::Profile) {
        self.settings = profile.apply(self.settings.clone());
        self.resolve_system_prompt();
        self.settings_state.settings_render_cache = None;
        self.add_log(format!("Applied profile: {}", profile.name), crate::config::LogLevel::Info);
        self.set_redraw();
    }

    /// Resolve system_prompt from the preset name.
    pub fn resolve_system_prompt(&mut self) {
        if let Some(content) = self.config.get_preset_content(&self.settings.system_prompt_preset_name) {
            self.settings.system_prompt = content;
        }
        self.set_redraw();
    }

    /// Save the current settings as a new profile.
    pub fn save_current_as_profile(&mut self, name: &str) {
        let profile = crate::config::Profile {
            name: name.to_string(),
            description: format!("User-defined profile: {}", name),
            settings: crate::config::ModelOverride::from_settings(&self.settings),
        };
        self.config.profiles.save(&profile);
        if let Err(e) = self.config.save() {
            self.add_log(format!("Failed to save profile: {}", e), crate::config::LogLevel::Error);
        } else {
            self.add_log(format!("Saved profile: {}", name), crate::config::LogLevel::Info);
        }
        self.set_redraw();
    }

    /// Save current settings as an override for the selected model.
    pub fn save_model_settings(&mut self) {
        if let Some(model) = self.selected_model() {
            let name = model.name.clone();
            let override_cfg = crate::config::ModelOverride::from_settings(&self.settings);
            self.config.model_overrides.save(&name, &override_cfg);
            if let Err(e) = self.config.save() {
                self.add_log(format!("Failed to save settings for {}: {}", name, e), crate::config::LogLevel::Error);
            } else {
                self.add_log(format!("Saved settings for {}", name), crate::config::LogLevel::Info);
                // Update the cache so it reflects the newly saved settings
                self.model_settings_cache = self.settings.clone();
            }
        } else {
            self.add_log("No model selected to save settings for", crate::config::LogLevel::Warning);
        }
        self.settings_state.settings_render_cache = None;
        self.set_redraw();
    }

    /// Check if any LLM settings have been modified since last save.
    pub fn is_settings_dirty(&self) -> bool {
        self.settings.is_dirty(&self.model_settings_cache)
    }

    /// Compute a fingerprint of the current settings for cache invalidation.
    pub fn settings_fingerprint(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        self.settings.context_length.hash(&mut h);
        self.settings.system_prompt_preset_name.hash(&mut h);
        self.settings.mlock.hash(&mut h);
        self.settings.gpu_layers_mode.hash(&mut h);
        self.settings.flash_attn.hash(&mut h);
        self.settings.kv_cache_offload.hash(&mut h);
        self.settings.cache_type_k.hash(&mut h);
        self.settings.cache_type_v.hash(&mut h);
        self.settings.expert_count.hash(&mut h);
        self.settings.batch_size.hash(&mut h);
        self.settings.uniform_cache.hash(&mut h);
        self.settings.max_concurrent_predictions.hash(&mut h);
        self.settings.seed.hash(&mut h);
        self.settings.temperature.to_bits().hash(&mut h);
        self.settings.top_k.hash(&mut h);
        self.settings.top_p.to_bits().hash(&mut h);
        self.settings.min_p.to_bits().hash(&mut h);
        self.settings.max_tokens.hash(&mut h);
        self.settings.repeat_penalty.to_bits().hash(&mut h);
        self.settings.repeat_last_n.hash(&mut h);
        self.settings.presence_penalty.map(|v| v.to_bits()).hash(&mut h);
        self.settings.frequency_penalty.map(|v| v.to_bits()).hash(&mut h);
        self.settings.keep.hash(&mut h);
        self.settings.mmap.hash(&mut h);
        self.settings.numa.hash(&mut h);
        self.settings.threads.hash(&mut h);
        self.settings.threads_batch.hash(&mut h);
        self.settings.get_active_backend_version().hash(&mut h);
        self.settings_state.settings_edit_buffer.hash(&mut h);
        h.finish()
    }

    /// Delete a user profile by index in the merged display list.
    /// Returns true if a profile was deleted, false otherwise.
    pub fn delete_profile(&mut self, selected_idx: usize) -> bool {
        let builtin = crate::config::builtin_profiles();
        let all_profiles = self.config.profiles.all();
        
        // Check if selection is valid
        if selected_idx >= all_profiles.len() {
            self.add_log("Invalid profile selection", crate::config::LogLevel::Info);
            return false;
        }
        
        // Check if it's a built-in profile
        if selected_idx < builtin.len() {
            self.add_log("Cannot delete built-in profiles", crate::config::LogLevel::Info);
            return false;
        }
        
        let profile = all_profiles[selected_idx].clone();
        let profile_name = profile.name.clone();
        
        self.config.profiles.delete(&profile_name);
        
        if let Err(e) = self.config.save() {
            self.add_log(format!("Failed to delete profile: {}", e), crate::config::LogLevel::Error);
            return false;
        }
        
        self.add_log(format!("Deleted profile: {}", profile_name), crate::config::LogLevel::Info);
        true
    }

    pub fn get_api_port_str(&self) -> String {
        let port = self.settings.api_endpoint_port;
        let mut cache = super::types::API_PORT_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        if cache.0 == port && !cache.1.is_empty() {
            return cache.1.clone();
        }
        cache.0 = port;
        cache.1 = port.to_string();
        cache.1.clone()
    }
}
