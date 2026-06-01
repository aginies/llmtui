use std::collections::HashMap;
use std::path::Path;

use serde::de::DeserializeOwned;

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
                if let Ok(item) = serde_yaml::from_str::<T>(&content) {
                    map.insert(name, item);
                }
            }
        }
    }
    map
}

/// Save an item as a YAML file. Removes from unused dir if present.
pub(crate) fn save_yaml<T: serde::Serialize + std::fmt::Debug>(
    name: &str,
    item: &T,
    active_dir: &Path,
    unused_dir: &Path,
) {
    let unused_path = unused_dir.join(format!("{}.yaml", name));
    let _ = std::fs::remove_file(&unused_path);

    let path = active_dir.join(format!("{}.yaml", name));
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(content) = serde_yaml::to_string(item) {
        let _ = std::fs::write(&path, content);
    }
}

/// Delete an item by moving from active dir to unused dir.
pub(crate) fn move_to_unused(name: &str, active_dir: &Path, unused_dir: &Path) {
    let src = active_dir.join(format!("{}.yaml", name));
    let dest = unused_dir.join(format!("{}.yaml", name));
    if src.exists() {
        let _ = std::fs::create_dir_all(unused_dir);
        let _ = std::fs::rename(&src, &dest);
    }
}
