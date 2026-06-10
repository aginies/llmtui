use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tracing::warn;

/// Load all YAML configs from a directory into a HashMap keyed by filename stem.
pub(crate) fn load_all_from_dir<T: DeserializeOwned + std::fmt::Debug + 'static>(
    dir: &Path,
) -> HashMap<String, T> {
    let mut map = HashMap::new();
    if !dir.is_dir() {
        return map;
    }
    for entry in match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            warn!("Failed to read config directory {}: {}", dir.display(), e);
            return map;
        }
    } {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                warn!("Failed to read config entry: {}", e);
                continue;
            }
        };
        let path = entry.path();
        if path.extension().map(|e| e == "yaml").unwrap_or(false) {
            let name = match path.file_stem().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            if let Ok(content) = std::fs::read_to_string(&path)
                && let Ok(item) = serde_yml::from_str::<T>(&content)
            {
                map.insert(name, item);
            }
        }
    }
    map
}

/// Save an item as a YAML file. Writes to active first, then removes from unused dir.
/// Order matters: if crash occurs after write, data is safely persisted.
pub(crate) fn save_yaml<T: serde::Serialize + std::fmt::Debug>(
    name: &str,
    item: &T,
    active_dir: &Path,
    unused_dir: &Path,
) {
    let path = active_dir.join(format!("{}.yaml", name));
    if let Some(parent) = path.parent()
        && let Err(e) = std::fs::create_dir_all(parent) {
            warn!(
                "Failed to create config directory {}: {}",
                parent.display(),
                e
            );
            return;
        }
    let content = match serde_yml::to_string(item) {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to serialize config '{}': {}", name, e);
            return;
        }
    };
    let tmp_path = path.with_extension("yaml.tmp");
    if let Err(e) = std::fs::write(&tmp_path, &content) {
        warn!("Failed to write config file {}: {}", tmp_path.display(), e);
        return;
    }
    if let Err(e) = std::fs::rename(&tmp_path, &path) {
        warn!("Failed to rename config file {} -> {}: {}", tmp_path.display(), path.display(), e);
        return;
    }

    let unused_path = unused_dir.join(format!("{}.yaml", name));
    if let Err(e) = std::fs::remove_file(&unused_path)
        && e.kind() != std::io::ErrorKind::NotFound {
            warn!(
                "Failed to remove unused config {}: {}",
                unused_path.display(),
                e
            );
        }
}

/// Trait for items that have a name field (used for store keying).
pub(crate) trait NamedItem {
    fn name(&self) -> &str;
}

/// Generic store for items keyed by name with built-in/user distinction.
#[derive(Clone, Debug)]
pub(crate) struct NamedStore<T: Clone + Serialize + for<'de> Deserialize<'de> + std::fmt::Debug + NamedItem> {
    dir: PathBuf,
    unused_dir: PathBuf,
    cache: HashMap<String, T>,
}

impl<T: Clone + Serialize + for<'de> Deserialize<'de> + std::fmt::Debug + NamedItem + 'static> NamedStore<T> {
    pub fn new(dir: PathBuf, unused_dir: PathBuf) -> Self {
        let cache = load_all_from_dir(&dir);
        Self { dir, unused_dir, cache }
    }

    pub fn save(&mut self, name: &str, item: &T) {
        save_yaml(name, item, &self.dir, &self.unused_dir);
        self.cache.insert(name.to_string(), item.clone());
    }

    pub fn insert_builtin(&mut self, name: String, item: T) {
        self.cache.insert(name, item);
    }

    pub fn delete(&mut self, name: &str, builtin_names: &[String]) -> bool {
        if builtin_names.iter().any(|b| b == name) {
            return false;
        }
        move_to_unused(name, &self.dir, &self.unused_dir);
        self.cache.remove(name);
        true
    }

    pub fn get(&self, name: &str) -> Option<&T> {
        self.cache.get(name)
    }

    pub fn user_items(&self, builtin_names: &[String]) -> Vec<T> {
        self.cache
            .values()
            .filter(|p| !builtin_names.iter().any(|b| b == p.name()))
            .cloned()
            .collect()
    }

    pub fn all(&self, builtin: Vec<T>, builtin_names: &[String]) -> Vec<T> {
        let mut all = builtin;
        for p in self.cache.values() {
            if !builtin_names.iter().any(|b| b == p.name()) {
                all.push(p.clone());
            }
        }
        all
    }
}

/// Delete an item by moving from active dir to unused dir.
pub(crate) fn move_to_unused(name: &str, active_dir: &Path, unused_dir: &Path) {
    let src = active_dir.join(format!("{}.yaml", name));
    let dest = unused_dir.join(format!("{}.yaml", name));
    if src.exists() {
        if let Err(e) = std::fs::create_dir_all(unused_dir) {
            warn!(
                "Failed to create unused config directory {}: {}",
                unused_dir.display(),
                e
            );
            return;
        }
        if let Err(e) = std::fs::rename(&src, &dest) {
            warn!(
                "Failed to move config to unused ({} -> {}): {}",
                src.display(),
                dest.display(),
                e
            );
        }
    }
}
