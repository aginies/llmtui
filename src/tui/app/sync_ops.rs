use super::types::App;
use std::path::PathBuf;

impl App {
    pub fn render<T: ratatui::backend::Backend>(
        &mut self,
        terminal: &mut ratatui::Terminal<T>,
    ) -> std::io::Result<()> {
        if self.ui.needs_full_redraw {
            terminal.clear()?;
            self.ui.needs_full_redraw = false;
        }
        terminal.draw(|frame| crate::tui::render::render(frame, self))?;
        Ok(())
    }

    pub fn discover_models(
        dirs: &[PathBuf],
        downloads: &[crate::models::DownloadState],
    ) -> Vec<crate::models::DiscoveredModel> {
        let mut models = Vec::new();

        // Build a map of filename -> expected total_bytes from active downloads
        let expected_sizes: std::collections::HashMap<&str, u64> = downloads
            .iter()
            .filter(|d| d.status == crate::models::DownloadStatus::Downloading)
            .map(|d| (d.filename.as_str(), d.total_bytes))
            .collect();

        for dir in dirs {
            crate::backend::hub::walk_dir_recursive(dir, 0, 10, &mut |entry| {
                let path = entry.path();
                if path.is_file()
                    && path.extension().map(|e| e == "gguf").unwrap_or(false)
                    && let Some(name) = path.file_name().and_then(|n| n.to_str())
                {
                    let name = name.to_string();
                    let size = entry.metadata().map(|m| m.len()).unwrap_or(0);

                    // Skip files that are currently being downloaded (partial downloads)
                    if let Some(&expected) = expected_sizes.get(name.as_str()) {
                        if size != expected {
                            return;
                        }
                    }

                    let display_name = path
                        .strip_prefix(dir)
                        .ok()
                        .and_then(|p| p.to_str())
                        .unwrap_or(&name)
                        .to_string();
                    models.push(crate::models::DiscoveredModel {
                        path,
                        name,
                        file_size: size,
                        display_name,
                    });
                }
            });
        }
        models.sort_by(|a, b| a.name.cmp(&b.name));
        models
    }

    pub fn reset_to_defaults(&mut self) {
        let defaults = crate::models::ModelSettings::default();
        self.settings = defaults;
        // Clear dirty flag by updating the cache snapshot to match new settings
        self.model_settings_cache = self.settings.clone();
        // Reset model metadata to avoid stale values
        self.loading.model_total_layers = 0;
        self.loading.model_hidden_size = 0;
        self.loading.model_n_ctx_train = 0;
        self.loading.model_n_head = 0;
        self.loading.model_n_kv_head = 0;
        self.loading.vram_estimate = 0;
        self.settings.spec_type = String::new();
        self.settings.draft_tokens = 0;
        self.settings_state.settings_render_cache = None;
        self.add_log(
            "Reset LLM Settings to defaults",
            crate::config::LogLevel::Info,
        );
        self.ui.needs_redraw = true;
    }

    pub fn selected_model(&self) -> Option<&crate::models::DiscoveredModel> {
        self.selected_model_idx.and_then(|i| self.models.get(i))
    }

    pub fn selected_model_settings(&self) -> crate::models::ModelSettings {
        let model_name = self.selected_model().map(|m| m.name.as_str());
        // For the TUI, we don't currently support a separate profile_name
        // in this method since it's already accounted for in overrides or the default settings.
        self.config.resolve_settings(model_name, None)
    }

    pub fn on_model_selection_change(&mut self) {
        self.search.readme_cache = None;
        if let Some(idx) = self.selected_model_idx {
            let model = self.models[idx].clone();
            self.model_settings_cache = self.selected_model_settings();
            self.settings = self.model_settings_cache.clone();
            self.update_model_metadata();
            self.update_vram_estimate();

            // Sync loading progress with the newly selected model
            if self.is_model_loaded(&model.display_name) {
                self.loading.loading_progress = 1.0;
                if !self
                    .loading
                    .loading_phases
                    .contains(&super::types::LoadingPhase::Complete)
                {
                    self.loading
                        .loading_phases
                        .insert(super::types::LoadingPhase::Complete);
                }
            } else if matches!(
                self.model_states.get(&model.display_name),
                Some(crate::models::ModelState::Loading)
                    | Some(crate::models::ModelState::Benchmarking)
            ) {
                // Keep current loading/benchmarking progress
            } else {
                // Not loaded, loading, or benchmarking, reset progress
                self.loading.loading_progress = 0.0;
                self.loading.loading_phases.clear();
                self.loading.last_active_phase = None;
                self.loading.load_progress = Default::default();
            }
        } else {
            let default_params = self.config.default.clone();
            self.model_settings_cache = default_params.into();
            self.loading.model_total_layers = 0;
            self.loading.model_hidden_size = 0;
            self.loading.model_n_ctx_train = 0;
            self.settings.spec_type = String::new();
            self.settings.draft_tokens = 0;
            self.loading.vram_estimate = 0;
            self.loading.loading_progress = 0.0;
            self.loading.loading_phases.clear();
            self.loading.last_active_phase = None;
        }
        self.ui.needs_redraw = true;
    }

    /// Return the current number of search results.
    pub fn search_results_len(&self) -> usize {
        if let super::types::ModelsMode::Search { results, .. } = &self.models_mode {
            results.len()
        } else {
            0
        }
    }

    pub fn get_filtered_model_indices(&self) -> Vec<usize> {
        self.models
            .iter()
            .enumerate()
            .filter(|(_, m)| {
                self.search.local_filter.is_empty()
                    || m.display_name
                        .to_lowercase()
                        .contains(&self.search.local_filter.to_lowercase())
            })
            .map(|(i, _)| i)
            .collect()
    }
}

/// Normalize separator characters for comparison (dashes and underscores).
fn normalize_separators(s: &str) -> String {
    s.replace('_', "-")
}

/// Check if a model (identified by HF model_id) is already downloaded locally.
/// Matches by comparing the HF repo name against local filenames, case-insensitively.
/// Checks all prefixes of the repo name to handle cases where the repo name has
/// extra components not present in the local filename (e.g. "Qwen3.6-27B-MTP-GGUF"
/// vs local "Qwen3.6-27B-Q3_K_S.gguf").
pub fn model_is_downloaded(models: &[crate::models::DiscoveredModel], model_id: &str) -> bool {
    let repo_name = model_id
        .rsplit('/')
        .next()
        .unwrap_or(model_id)
        .to_lowercase();
    let repo_normalized = normalize_separators(&repo_name);
    let repo_parts: Vec<&str> = repo_normalized.split('-').collect();

    models.iter().any(|m| {
        let mut local = m.name.to_lowercase();
        if let Some(stripped) = local.strip_suffix(".gguf") {
            local = stripped.to_string();
        }
        let local_normalized = normalize_separators(&local);

        // Check exact match first
        if local_normalized == repo_normalized {
            return true;
        }

        // Check if local starts with any prefix of the repo name (minimum 8 chars to avoid false positives)
        for i in 1..=repo_parts.len() {
            let prefix = repo_parts[..i].join("-");
            if prefix.len() >= 8 && local_normalized.starts_with(&format!("{}-", prefix)) {
                return true;
            }
        }
        false
    })
}

/// Check if a model has been downloaded by verifying that
/// models_dir/<model_id>/ exists and is non-empty.
pub fn model_dir_has_contents(models_dirs: &[PathBuf], model_id: &str) -> bool {
    for dir in models_dirs {
        let model_dir = dir.join(model_id);
        if let Ok(mut entries) = std::fs::read_dir(&model_dir) {
            return entries.next().is_some();
        }
    }
    false
}
