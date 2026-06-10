use std::path::PathBuf;



use crate::config::Profile;
use crate::config::builtin_profiles;
use crate::config::config_base_dir;
use crate::config::store::NamedStore;

/// Directory for per-profile YAML configs.
pub fn profiles_config_dir() -> PathBuf {
    config_base_dir().join("llm-manager").join("profiles")
}

/// Directory for unused (deleted) profile configs.
pub fn unused_profiles_dir() -> PathBuf {
    config_base_dir()
        .join("llm-manager")
        .join("unused_profiles")
}

/// Profile store — manages per-profile YAML configs.
#[derive(Debug, Clone)]
pub struct ProfileStore {
    inner: NamedStore<Profile>,
}

impl ProfileStore {
    pub fn new() -> Self {
        let profiles_dir = profiles_config_dir();
        let unused_dir = unused_profiles_dir();
        Self {
            inner: NamedStore::new(profiles_dir, unused_dir),
        }
    }

    /// Save (or update) a profile.
    pub fn save(&mut self, profile: &Profile) {
        self.inner.save(&profile.name, profile)
    }

    /// Insert a built-in profile into the in-memory cache only (no disk I/O).
    pub fn insert_builtin(&mut self, profile: Profile) {
        self.inner.insert_builtin(profile.name.clone(), profile)
    }

    /// Delete a profile by moving it to the unused directory.
    pub fn delete(&mut self, name: &str) -> bool {
        let builtin = builtin_profiles();
        let names: Vec<String> = builtin.iter().map(|b| b.name.clone()).collect();
        self.inner.delete(name, &names)
    }

    /// Get a profile by name.
    pub fn get(&self, name: &str) -> Option<&Profile> {
        self.inner.get(name)
    }

    /// Get all user-defined profiles (excluding built-ins).
    pub fn user_profiles(&self) -> Vec<Profile> {
        let builtin = builtin_profiles();
        let names: Vec<String> = builtin.iter().map(|b| b.name.clone()).collect();
        self.inner.user_items(&names)
    }

    /// Get all profiles (built-in + user).
    pub fn all(&self) -> Vec<Profile> {
        let builtin = builtin_profiles();
        let names: Vec<String> = builtin.iter().map(|b| b.name.clone()).collect();
        self.inner.all(builtin, &names)
    }
}

impl Default for ProfileStore {
    fn default() -> Self {
        Self::new()
    }
}
