use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config::config_base_dir;
use crate::config::store::{load_all_from_dir, move_to_unused, save_yaml};
use crate::models::ModelSettingsProfile;

/// Directory for per-model settings profile YAML configs.
pub fn model_settings_profiles_dir() -> PathBuf {
    config_base_dir().join("llm-manager").join("model_settings")
}

/// Directory for unused (deleted) model settings profile configs.
pub fn unused_model_settings_profiles_dir() -> PathBuf {
    config_base_dir()
        .join("llm-manager")
        .join("unused_model_settings")
}

/// Per-model settings profile store.
///
/// Each model can have multiple named settings profiles stored as YAML files
/// in `~/.config/llm-manager/model_settings/`.
/// Files are named `<model_key>__<profile_name>.yaml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelSettingsStore {
    profiles_dir: PathBuf,
    unused_dir: PathBuf,
    /// Keyed by "<model_key>__<profile_name>".
    cache: HashMap<String, ModelSettingsProfile>,
}

impl ModelSettingsStore {
    pub fn new() -> Self {
        let profiles_dir = model_settings_profiles_dir();
        let unused_dir = unused_model_settings_profiles_dir();
        let cache = load_all_from_dir(&profiles_dir);

        Self {
            profiles_dir,
            unused_dir,
            cache,
        }
    }

    /// Build the storage key for a profile.
    fn profile_key(model_key: &str, profile_name: &str) -> String {
        format!("{}__{}", model_key, profile_name)
    }

    /// Parse a storage key back into (model_key, profile_name).
    fn parse_profile_key(key: &str) -> Option<(String, String)> {
        // Find the last "__" separator
        if let Some(pos) = key.rfind("__") {
            let model_key = key[..pos].to_string();
            let profile_name = key[pos + 2..].to_string();
            Some((model_key, profile_name))
        } else {
            None
        }
    }

    /// Get all profiles for a model (keyed by display_name).
    #[allow(dead_code)]
    pub fn get_for_model(&self, display_name: &str) -> Vec<ModelSettingsProfile> {
        let model_key = crate::config::model_config::key_from_display(display_name);
        let prefix = format!("{}__", model_key);
        self.cache
            .values()
            .filter(|p| {
                let key = Self::profile_key(&model_key, &p.name);
                key.starts_with(&prefix)
            })
            .cloned()
            .collect()
    }

    /// Get a specific profile for a model by name.
    pub fn get(&self, display_name: &str, profile_name: &str) -> Option<ModelSettingsProfile> {
        let model_key = crate::config::model_config::key_from_display(display_name);
        let key = Self::profile_key(&model_key, profile_name);
        self.cache.get(&key).cloned()
    }

    /// Save (or update) a profile for a model.
    pub fn save(&mut self, display_name: &str, profile: &ModelSettingsProfile) {
        let model_key = crate::config::model_config::key_from_display(display_name);
        let key = Self::profile_key(&model_key, &profile.name);
        save_yaml(&key, profile, &self.profiles_dir, &self.unused_dir);
        self.cache.insert(key, profile.clone());
    }

    /// Delete a profile for a model.
    pub fn delete(&mut self, display_name: &str, profile_name: &str) {
        let model_key = crate::config::model_config::key_from_display(display_name);
        let key = Self::profile_key(&model_key, profile_name);
        move_to_unused(&key, &self.profiles_dir, &self.unused_dir);
        self.cache.remove(&key);
    }

    /// Get all profile names for a model (sorted).
    pub fn profile_names(&self, display_name: &str) -> Vec<String> {
        let model_key = crate::config::model_config::key_from_display(display_name);
        let prefix = format!("{}__", model_key);
        let mut names: Vec<String> = self
            .cache
            .keys()
            .filter(|k| k.starts_with(&prefix))
            .map(|k| {
                let (_, name) =
                    Self::parse_profile_key(k).unwrap_or((String::new(), String::new()));
                name
            })
            .collect();
        names.sort();
        names
    }
}

impl Default for ModelSettingsStore {
    fn default() -> Self {
        Self::new()
    }
}
