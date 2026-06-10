use std::path::PathBuf;



use crate::config::SystemPromptPreset;
use crate::config::builtin_system_prompt_presets;
use crate::config::config_base_dir;
use crate::config::store::NamedStore;

/// Directory for per-preset YAML configs.
pub fn presets_config_dir() -> PathBuf {
    config_base_dir().join("llm-manager").join("presets")
}

/// Directory for unused (deleted) preset configs.
pub fn unused_presets_dir() -> PathBuf {
    config_base_dir().join("llm-manager").join("unused_presets")
}

/// System prompt preset store — manages per-preset YAML configs.
#[derive(Debug, Clone)]
pub struct PresetStore {
    inner: NamedStore<SystemPromptPreset>,
}

impl PresetStore {
    pub fn new() -> Self {
        let presets_dir = presets_config_dir();
        let unused_dir = unused_presets_dir();
        Self {
            inner: NamedStore::new(presets_dir, unused_dir),
        }
    }

    /// Get all user-defined presets (excluding built-ins).
    pub fn user_presets(&self) -> Vec<SystemPromptPreset> {
        let builtin = builtin_system_prompt_presets();
        let names: Vec<String> = builtin.iter().map(|b| b.name.clone()).collect();
        self.inner.user_items(&names)
    }

    /// Save (or update) a preset.
    pub fn save(&mut self, preset: &SystemPromptPreset) {
        self.inner.save(&preset.name, preset)
    }

    /// Insert a built-in preset into the in-memory cache only (no disk I/O).
    pub fn insert_builtin(&mut self, preset: SystemPromptPreset) {
        self.inner.insert_builtin(preset.name.clone(), preset)
    }

    /// Delete a preset by moving it to the unused directory.
    pub fn delete(&mut self, name: &str) -> bool {
        let builtin = builtin_system_prompt_presets();
        let names: Vec<String> = builtin.iter().map(|b| b.name.clone()).collect();
        self.inner.delete(name, &names)
    }

    /// Get a preset by name.
    pub fn get(&self, name: &str) -> Option<&SystemPromptPreset> {
        self.inner.get(name)
    }

    /// Get all presets (built-in + user).
    pub fn all(&self) -> Vec<SystemPromptPreset> {
        let builtin = builtin_system_prompt_presets();
        let names: Vec<String> = builtin.iter().map(|b| b.name.clone()).collect();
        self.inner.all(builtin, &names)
    }
}

impl Default for PresetStore {
    fn default() -> Self {
        Self::new()
    }
}
