use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config::Profile;
use crate::config::builtin_profiles;
use crate::config::store::{load_all_from_dir, move_to_unused, save_yaml};

/// Directory for per-profile YAML configs.
pub fn profiles_config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_default()
        .join("llm-manager")
        .join("profiles")
}

/// Directory for unused (deleted) profile configs.
pub fn unused_profiles_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_default()
        .join("llm-manager")
        .join("unused_profiles")
}

/// Profile store — manages per-profile YAML configs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileStore {
    profiles_dir: PathBuf,
    unused_dir: PathBuf,
    cache: HashMap<String, Profile>,
}

impl ProfileStore {
    pub fn new() -> Self {
        let profiles_dir = profiles_config_dir();
        let unused_dir = unused_profiles_dir();
        let cache = load_all_from_dir(&profiles_dir);
        Self {
            profiles_dir,
            unused_dir,
            cache,
        }
    }

    /// Save (or update) a profile.
    pub fn save(&mut self, profile: &Profile) {
        save_yaml(&profile.name, profile, &self.profiles_dir, &self.unused_dir);
        self.cache.insert(profile.name.clone(), profile.clone());
    }

    /// Delete a profile by moving it to the unused directory.
    pub fn delete(&mut self, name: &str) -> bool {
        let builtin = builtin_profiles();
        if builtin.iter().any(|b| b.name == name) {
            return false;
        }
        move_to_unused(name, &self.profiles_dir, &self.unused_dir);
        self.cache.remove(name);
        true
    }

    /// Get a profile by name.
    pub fn get(&self, name: &str) -> Option<&Profile> {
        self.cache.get(name)
    }

    /// Get all profiles (built-in + user).
    pub fn all(&self) -> Vec<Profile> {
        let builtin = builtin_profiles();
        let mut all: Vec<Profile> = builtin.clone();
        for p in self.cache.values() {
            if !builtin.iter().any(|b| b.name == p.name) {
                all.push(p.clone());
            }
        }
        all
    }
}

impl Default for ProfileStore {
    fn default() -> Self {
        Self::new()
    }
}
