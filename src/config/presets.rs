use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::SystemPromptPreset;
use crate::config::builtin_system_prompt_presets;

/// Directory for per-preset YAML configs.
pub fn presets_config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_default()
        .join("llm-manager")
        .join("presets")
}

/// Directory for unused (deleted) preset configs.
pub fn unused_presets_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_default()
        .join("llm-manager")
        .join("unused_presets")
}

/// Load all preset configs from disk into a cache.
fn load_all_from_dir(dir: &Path) -> HashMap<String, SystemPromptPreset> {
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
                if let Ok(preset) = serde_yaml::from_str::<SystemPromptPreset>(&content) {
                    map.insert(name, preset);
                }
            }
        }
    }
    map
}

/// System prompt preset store — manages per-preset YAML configs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetStore {
    presets_dir: PathBuf,
    unused_dir: PathBuf,
    cache: HashMap<String, SystemPromptPreset>,
}

impl PresetStore {
    pub fn new() -> Self {
        let presets_dir = presets_config_dir();
        let unused_dir = unused_presets_dir();
        let cache = load_all_from_dir(&presets_dir);
        Self {
            presets_dir,
            unused_dir,
            cache,
        }
    }

    /// Get all user-defined presets (excluding built-ins).
    pub fn user_presets(&self) -> Vec<&SystemPromptPreset> {
        let builtin = builtin_system_prompt_presets();
        self.cache
            .values()
            .filter(|p| !builtin.iter().any(|b| b.name == p.name))
            .collect()
    }

    /// Save (or update) a preset.
    pub fn save(&mut self, preset: &SystemPromptPreset) {
        // Remove from unused if it was there
        let unused_path = self.unused_dir.join(format!("{}.yaml", preset.name));
        let _ = std::fs::remove_file(&unused_path);

        // Write to presets directory
        let path = self.presets_dir.join(format!("{}.yaml", preset.name));
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(content) = serde_yaml::to_string(preset) {
            let _ = std::fs::write(&path, content);
        }
        self.cache.insert(preset.name.clone(), preset.clone());
    }

    /// Delete a preset by moving it to the unused directory.
    pub fn delete(&mut self, name: &str) -> bool {
        // Don't allow deleting built-ins
        let builtin = builtin_system_prompt_presets();
        if builtin.iter().any(|b| b.name == name) {
            return false;
        }
        let src = self.presets_dir.join(format!("{}.yaml", name));
        let dest = self.unused_dir.join(format!("{}.yaml", name));
        if src.exists() {
            let _ = std::fs::create_dir_all(&self.unused_dir);
            let _ = std::fs::rename(&src, &dest);
        }
        self.cache.remove(name);
        true
    }

    /// Get a preset by name.
    pub fn get(&self, name: &str) -> Option<&SystemPromptPreset> {
        self.cache.get(name)
    }

    /// Get all presets (built-in + user).
    pub fn all(&self) -> Vec<SystemPromptPreset> {
        let builtin = builtin_system_prompt_presets();
        let mut all: Vec<SystemPromptPreset> = builtin.clone();
        for p in self.cache.values() {
            if !builtin.iter().any(|b| b.name == p.name) {
                all.push(p.clone());
            }
        }
        all
    }

    }

impl Default for PresetStore {
    fn default() -> Self {
        Self::new()
    }
}
