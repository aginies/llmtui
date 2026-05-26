use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::ModelOverride;

/// Directory for per-model YAML configs.
pub fn models_config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_default()
        .join("llm-manager")
        .join("models")
}

/// Directory for unused (deleted) model configs.
pub fn unused_config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_default()
        .join("llm-manager")
        .join("unused")
}

/// Load all model configs from disk into a cache.
fn load_all_from_dir(dir: &Path) -> HashMap<String, ModelOverride> {
    let mut map = HashMap::new();
    if !dir.is_dir() {
        return map;
    }
    for entry in match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return map,
    } {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();
        if path.extension().map(|e| e == "yaml").unwrap_or(false) {
            let name = match path.file_stem().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(cfg) = serde_yaml::from_str::<ModelOverride>(&content) {
                    map.insert(name, cfg);
                }
            }
        }
    }
    map
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

    /// Create a new empty store without loading from disk (for tests).
    #[allow(dead_code)]
    pub fn new_empty() -> Self {
        Self {
            models_dir: models_config_dir(),
            unused_dir: unused_config_dir(),
            cache: HashMap::new(),
        }
    }

    /// Get the config for a model by name.
    pub fn get(&self, name: &str) -> Option<&ModelOverride> {
        self.cache.get(name)
    }

    /// Save (or update) a model config.
    pub fn save(&mut self, name: &str, config: &ModelOverride) {
        // Remove from unused if it was there
        let unused_path = self.unused_dir.join(format!("{}.yaml", name));
        let _ = std::fs::remove_file(&unused_path);

        // Write to models directory
        let path = self.models_dir.join(format!("{}.yaml", name));
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(content) = serde_yaml::to_string(config) {
            let _ = std::fs::write(&path, content);
        }
        self.cache.insert(name.to_string(), config.clone());
    }

    /// Delete a model config by moving it to the unused directory.
    pub fn delete(&mut self, name: &str) {
        let src = self.models_dir.join(format!("{}.yaml", name));
        let dest = self.unused_dir.join(format!("{}.yaml", name));
        if src.exists() {
            let _ = std::fs::create_dir_all(&self.unused_dir);
            let _ = std::fs::rename(&src, &dest);
        }
        self.cache.remove(name);
    }

    /// Check if there are any model configs.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// Get all model config keys.
    pub fn keys(&self) -> Vec<String> {
        let mut keys: Vec<String> = self.cache.keys().cloned().collect();
        keys.sort();
        keys
    }

    /// Reload from disk (e.g., after restoring from unused).
    #[allow(dead_code)]
    pub fn reload(&mut self) {
        self.cache = load_all_from_dir(&self.models_dir);
    }

    /// Restore a deleted config from unused directory.
    #[allow(dead_code)]
    pub fn restore(&mut self, name: &str) -> bool {
        let unused_path = self.unused_dir.join(format!("{}.yaml", name));
        let models_path = self.models_dir.join(format!("{}.yaml", name));
        if unused_path.exists() && !models_path.exists() {
            let _ = std::fs::create_dir_all(&self.models_dir);
            if let Ok(content) = std::fs::read_to_string(&unused_path) {
                if let Ok(config) = serde_yaml::from_str::<ModelOverride>(&content) {
                    let _ = std::fs::remove_file(&unused_path);
                    let _ = std::fs::write(&models_path, content);
                    self.cache.insert(name.to_string(), config);
                    return true;
                }
            }
        }
        false
    }
}

impl Default for ModelConfigStore {
    fn default() -> Self {
        Self::new()
    }
}
