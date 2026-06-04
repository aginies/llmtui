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

/// Per-model configuration store.
///
/// Each model config is a YAML file stored in `~/.config/llm-manager/models/`.
/// Files are named `<model_name>.yaml` where `model_name` is the filename
/// without the `.gguf` extension.
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

    /// Get the config for a model by name.
    pub fn get(&self, name: &str) -> Option<&ModelOverride> {
        self.cache.get(name)
    }

    /// Save (or update) a model config.
    pub fn save(&mut self, name: &str, config: &ModelOverride) {
        save_yaml(name, config, &self.models_dir, &self.unused_dir);
        self.cache.insert(name.to_string(), config.clone());
    }

    /// Delete a model config by moving it to the unused directory.
    pub fn delete(&mut self, name: &str) {
        move_to_unused(name, &self.models_dir, &self.unused_dir);
        self.cache.remove(name);
    }

    /// Get all model config keys.
    pub fn keys(&self) -> Vec<String> {
        let mut keys: Vec<String> = self.cache.keys().cloned().collect();
        keys.sort();
        keys
    }
}

impl Default for ModelConfigStore {
    fn default() -> Self {
        Self::new()
    }
}
