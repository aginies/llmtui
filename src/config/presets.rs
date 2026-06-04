use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config::SystemPromptPreset;
use crate::config::builtin_system_prompt_presets;
use crate::config::config_base_dir;
use crate::config::store::{load_all_from_dir, move_to_unused, save_yaml};

/// Directory for per-preset YAML configs.
pub fn presets_config_dir() -> PathBuf {
    config_base_dir().join("llm-manager").join("presets")
}

/// Directory for unused (deleted) preset configs.
pub fn unused_presets_dir() -> PathBuf {
    config_base_dir().join("llm-manager").join("unused_presets")
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
    pub fn user_presets(&self) -> Vec<SystemPromptPreset> {
        let builtin = builtin_system_prompt_presets();
        self.cache
            .values()
            .filter(|p| !builtin.iter().any(|b| b.name == p.name))
            .cloned()
            .collect()
    }

    /// Save (or update) a preset.
    pub fn save(&mut self, preset: &SystemPromptPreset) {
        save_yaml(&preset.name, preset, &self.presets_dir, &self.unused_dir);
        self.cache.insert(preset.name.clone(), preset.clone());
    }

    /// Insert a built-in preset into the in-memory cache only (no disk I/O).
    pub fn insert_builtin(&mut self, preset: SystemPromptPreset) {
        self.cache.insert(preset.name.clone(), preset);
    }

    /// Delete a preset by moving it to the unused directory.
    pub fn delete(&mut self, name: &str) -> bool {
        let builtin = builtin_system_prompt_presets();
        if builtin.iter().any(|b| b.name == name) {
            return false;
        }
        move_to_unused(name, &self.presets_dir, &self.unused_dir);
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
