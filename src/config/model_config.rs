use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::config::ModelOverride;
use crate::config::config_base_dir;
use crate::config::store::{load_all_from_dir, move_to_unused, save_yaml};

/// Directory for per-model YAML configs.
pub fn models_config_dir() -> PathBuf {
    config_base_dir().join("llm-manager").join("models")
}

/// Directory for unused (deleted) model configs.
pub fn unused_config_dir() -> PathBuf {
    config_base_dir().join("llm-manager").join("unused")
}

/// Convert a display_name (relative path) to a filesystem-safe key.
/// Replaces path separators with "__" to create unique keys.
/// E.g. "qwen/model-v2" → "qwen__model-v2"
pub fn key_from_display(display_name: &str) -> String {
    display_name.replace(std::path::MAIN_SEPARATOR, "__")
}

/// Convert a filesystem-safe key back to a display_name.
/// Reverses the key_from_display transformation.
/// E.g. "qwen__model-v2" → "qwen/model-v2"
pub fn display_from_key(key: &str) -> String {
    key.replace("__", std::path::MAIN_SEPARATOR_STR)
}

/// Per-model configuration store.
///
/// Each model config is a YAML file stored in `~/.config/llm-manager/models/`.
/// Files are named `<key>.yaml` where `key` is derived from the model's
/// display_name (path relative to its model directory) with path separators
/// replaced by "__".
///
/// Example: model at "models/qwen/model-v2.gguf" has display_name "qwen/model-v2"
/// and config file "qwen__model-v2.yaml".
///
/// Deleting a model moves its config file to `~/.config/llm-manager/unused/`
/// instead of removing it, allowing recovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfigStore {
    models_dir: PathBuf,
    unused_dir: PathBuf,
    model_dirs: Vec<PathBuf>,
    cache: HashMap<String, ModelOverride>,
}

impl ModelConfigStore {
    pub fn new(model_dirs: Vec<PathBuf>) -> Self {
        let models_dir = models_config_dir();
        let unused_dir = unused_config_dir();
        let mut cache = load_all_from_dir(&models_dir);

        // Migrate old filename-only keys to new display_name-based keys
        Self::migrate_old_keys(&mut cache, &models_dir, &model_dirs);

        Self {
            models_dir,
            unused_dir,
            model_dirs,
            cache,
        }
    }

    /// Migrate old config keys (filename-only) to new keys (display_name-based).
    /// For each old key, scans model_dirs for matching .gguf files.
    /// If exactly one match found, renames the config file to the new key.
    fn migrate_old_keys(
        cache: &mut HashMap<String, ModelOverride>,
        models_dir: &Path,
        model_dirs: &[PathBuf],
    ) {
        let old_keys: Vec<String> = cache.keys().cloned().collect();
        let mut renamed = Vec::new();

        for old_key in &old_keys {
            // Skip keys that already contain "__" (already migrated)
            if old_key.contains("__") {
                continue;
            }

            // Find matching .gguf files in model_dirs
            let mut matches: Vec<(PathBuf, PathBuf)> = Vec::new();
            for model_dir in model_dirs {
                if let Ok(entries) = std::fs::read_dir(model_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_file()
                            && path.extension().map(|e| e == "gguf").unwrap_or(false)
                        {
                            if let Some(name) = path.file_name() {
                                let filename = name.to_string_lossy();
                                let stem = filename
                                    .strip_suffix(".gguf")
                                    .or_else(|| filename.strip_suffix(".GGUF"))
                                    .unwrap_or(&filename);
                                if stem == old_key {
                                    matches.push((path.clone(), path.parent().unwrap().to_path_buf()));
                                }
                            }
                        }
                    }
                }

                // Also search recursively
                Self::find_matching_files(model_dir, old_key, &mut matches);
            }

            if matches.len() == 1 {
                let (model_path, model_dir) = &matches[0];
                let display_name = model_path
                    .strip_prefix(model_dir)
                    .ok()
                    .and_then(|p| p.to_str())
                    .unwrap_or(old_key);

                let new_key = key_from_display(display_name);
                if new_key != *old_key {
                    let old_file = models_dir.join(format!("{}.yaml", old_key));
                    let new_file = models_dir.join(format!("{}.yaml", new_key));

                    if old_file.exists() && !new_file.exists() {
                        if let Err(e) = std::fs::rename(&old_file, &new_file) {
                            warn!(
                                "Failed to migrate config '{}' to '{}': {}",
                                old_file.display(),
                                new_file.display(),
                                e
                            );
                        } else {
                            // Update cache key
                            if let Some(config) = cache.remove(old_key) {
                                cache.insert(new_key.clone(), config);
                            }
                            renamed.push((old_key.clone(), new_key));
                        }
                    }
                }
            } else if matches.len() > 1 {
                warn!(
                    "Skipping migration for '{}' - {} matching models found (ambiguous)",
                    old_key,
                    matches.len()
                );
            }
        }

        if !renamed.is_empty() {
            warn!("Migrated {} model config(s) to new key format", renamed.len());
            for (old, new) in &renamed {
                warn!("  '{}' → '{}'", old, new);
            }
        }
    }

    /// Recursively search for .gguf files matching a filename stem.
    fn find_matching_files(
        dir: &Path,
        stem: &str,
        matches: &mut Vec<(PathBuf, PathBuf)>,
    ) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    Self::find_matching_files(&path, stem, matches);
                } else if path.is_file()
                    && path.extension().map(|e| e == "gguf").unwrap_or(false)
                {
                    if let Some(name) = path.file_name() {
                        let filename = name.to_string_lossy();
                        let file_stem = filename
                            .strip_suffix(".gguf")
                            .or_else(|| filename.strip_suffix(".GGUF"))
                            .unwrap_or(&filename);
                        if file_stem == stem {
                            matches.push((
                                path.clone(),
                                path.parent().unwrap().to_path_buf(),
                            ));
                        }
                    }
                }
            }
        }
    }

    /// Get the config for a model by its display_name.
    pub fn get(&self, display_name: &str) -> Option<&ModelOverride> {
        let key = key_from_display(display_name);
        self.cache.get(&key)
    }

    /// Save (or update) a model config by its display_name.
    pub fn save(&mut self, display_name: &str, config: &ModelOverride) {
        let key = key_from_display(display_name);
        save_yaml(&key, config, &self.models_dir, &self.unused_dir);
        self.cache.insert(key, config.clone());
    }

    /// Delete a model config by its display_name.
    pub fn delete(&mut self, display_name: &str) {
        let key = key_from_display(display_name);
        move_to_unused(&key, &self.models_dir, &self.unused_dir);
        self.cache.remove(&key);
    }

    /// Get all model config display names (keys transformed back to display form).
    pub fn keys(&self) -> Vec<String> {
        let mut keys: Vec<String> = self
            .cache
            .keys()
            .map(|k| display_from_key(k))
            .collect();
        keys.sort();
        keys
    }
}

impl Default for ModelConfigStore {
    fn default() -> Self {
        Self::new(vec![])
    }
}
