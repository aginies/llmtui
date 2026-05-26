use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::Profile;
use crate::config::builtin_profiles;

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

/// Load all profile configs from disk into a cache.
fn load_all_from_dir(dir: &Path) -> HashMap<String, Profile> {
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
                if let Ok(profile) = serde_yaml::from_str::<Profile>(&content) {
                    map.insert(name, profile);
                }
            }
        }
    }
    map
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
        // Remove from unused if it was there
        let unused_path = self.unused_dir.join(format!("{}.yaml", profile.name));
        let _ = std::fs::remove_file(&unused_path);

        // Write to profiles directory
        let path = self.profiles_dir.join(format!("{}.yaml", profile.name));
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(content) = serde_yaml::to_string(profile) {
            let _ = std::fs::write(&path, content);
        }
        self.cache.insert(profile.name.clone(), profile.clone());
    }

    /// Delete a profile by moving it to the unused directory.
    pub fn delete(&mut self, name: &str) -> bool {
        // Don't allow deleting built-ins
        let builtin = builtin_profiles();
        if builtin.iter().any(|b| b.name == name) {
            return false;
        }
        let src = self.profiles_dir.join(format!("{}.yaml", name));
        let dest = self.unused_dir.join(format!("{}.yaml", name));
        if src.exists() {
            let _ = std::fs::create_dir_all(&self.unused_dir);
            let _ = std::fs::rename(&src, &dest);
        }
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
