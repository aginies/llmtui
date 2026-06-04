use std::collections::HashMap;
use std::path::Path;

use serde::de::DeserializeOwned;
use tracing::warn;

/// Load all YAML configs from a directory into a HashMap keyed by filename stem.
pub(crate) fn load_all_from_dir<T: DeserializeOwned + std::fmt::Debug>(
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
                && let Ok(item) = serde_yaml::from_str::<T>(&content)
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
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            warn!(
                "Failed to create config directory {}: {}",
                parent.display(),
                e
            );
            return;
        }
    }
    let content = match serde_yaml::to_string(item) {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to serialize config '{}': {}", name, e);
            return;
        }
    };
    if let Err(e) = std::fs::write(&path, &content) {
        warn!("Failed to write config file {}: {}", path.display(), e);
        return;
    }

    let unused_path = unused_dir.join(format!("{}.yaml", name));
    if let Err(e) = std::fs::remove_file(&unused_path) {
        if e.kind() != std::io::ErrorKind::NotFound {
            warn!(
                "Failed to remove unused config {}: {}",
                unused_path.display(),
                e
            );
        }
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
