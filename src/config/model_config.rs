use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config::ModelOverride;
use crate::config::config_base_dir;
use crate::config::store::{load_all_from_dir, move_to_unused, save_yaml};

/// Directory for per-model YAML configs.
pub fn models_config_dir() -> PathBuf {
    config_base_dir().join("llm-manager").join("models")
}

/// Directory for unused (deleted) model configs.
pub fn unused_config_dir() -> PathBuf {
    config_base_dir().join("llm-manager").join("unused")
}

/// Convert a display_name (relative path) to a filesystem-safe key.
/// Replaces path separators with "__" to create unique keys.
/// E.g. "qwen/model-v2" → "qwen__model-v2"
pub fn key_from_display(display_name: &str) -> String {
    display_name.replace(std::path::MAIN_SEPARATOR, "__")
}

/// Convert a filesystem-safe key back to a display_name.
/// Reverses the key_from_display transformation.
/// E.g. "qwen__model-v2" → "qwen/model-v2"
pub fn display_from_key(key: &str) -> String {
    key.replace("__", std::path::MAIN_SEPARATOR_STR)
}

/// Per-model configuration store.
///
/// Each model config is a YAML file stored in `~/.config/llm-manager/models/`.
/// Files are named `<key>.yaml` where `key` is derived from the model's
/// display_name (path relative to its model directory) with path separators
/// replaced by "__".
///
/// Example: model at "models/qwen/model-v2.gguf" has display_name "qwen/model-v2"
/// and config file "qwen__model-v2.yaml".
///
/// Deleting a model moves its config file to `~/.config/llm-manager/unused/`
/// instead of removing it, allowing recovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfigStore {
    models_dir: PathBuf,
    unused_dir: PathBuf,
    cache: HashMap<String, ModelOverride>,
}

impl ModelConfigStore {
    pub fn new() -> Self {
        let models_dir = models_config_dir();
        let unused_dir = unused_config_dir();
        let cache = load_all_from_dir(&models_dir);

        Self {
            models_dir,
            unused_dir,
            cache,
        }
    }

    /// Get the config for a model by its display_name.
    pub fn get(&self, display_name: &str) -> Option<&ModelOverride> {
        let key = key_from_display(display_name);
        self.cache.get(&key)
    }

    /// Save (or update) a model config by its display_name.
    pub fn save(&mut self, display_name: &str, config: &ModelOverride) {
        let key = key_from_display(display_name);
        save_yaml(&key, config, &self.models_dir, &self.unused_dir);
        self.cache.insert(key, config.clone());
    }

    /// Delete a model config by its display_name.
    pub fn delete(&mut self, display_name: &str) {
        let key = key_from_display(display_name);
        move_to_unused(&key, &self.models_dir, &self.unused_dir);
        self.cache.remove(&key);
    }

    /// Get all model config display names (keys transformed back to display form).
    pub fn keys(&self) -> Vec<String> {
        let mut keys: Vec<String> = self.cache.keys().map(|k| display_from_key(k)).collect();
        keys.sort();
        keys
    }
}

impl Default for ModelConfigStore {
    fn default() -> Self {
        Self::new()
    }
}
