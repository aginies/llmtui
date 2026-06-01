use super::types::App;
use std::path::PathBuf;

impl App {
    pub fn render<T: ratatui::backend::Backend>(&mut self, terminal: &mut ratatui::Terminal<T>) -> std::io::Result<()> {
        if self.ui.needs_full_redraw {
            terminal.clear()?;
            self.ui.needs_full_redraw = false;
        }
        terminal.draw(|frame| crate::tui::render::render(frame, self))?;
        Ok(())
    }

    pub fn discover_models(dirs: &[PathBuf]) -> Vec<crate::models::DiscoveredModel> {
        let mut models = Vec::new();
        for dir in dirs {
            crate::backend::hub::walk_dir_recursive(dir, 0, 10, &mut |entry| {
                let path = entry.path();
                if path.is_file() && path.extension().map(|e| e == "gguf").unwrap_or(false) {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        let name = name.to_string();
                        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
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
        self.add_log("Reset LLM Settings to defaults", crate::config::LogLevel::Info);
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
                if !self.loading.loading_phases.contains(&super::types::LoadingPhase::Complete) {
                    self.loading.loading_phases.insert(super::types::LoadingPhase::Complete);
                }
            } else if matches!(self.model_states.get(&model.display_name), Some(crate::models::ModelState::Loading) | Some(crate::models::ModelState::Benchmarking)) {
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
pub fn model_is_downloaded(models: &[crate::models::DiscoveredModel], model_id: &str) -> bool {
    let repo_name = model_id.rsplit('/').next().unwrap_or(model_id).to_lowercase();
    let repo_normalized = normalize_separators(&repo_name);
    models.iter().any(|m| {
        let mut local = m.name.to_lowercase();
        if let Some(stripped) = local.strip_suffix(".gguf") {
            local = stripped.to_string();
        }
        let local_normalized = normalize_separators(&local);
        local_normalized == repo_normalized
            || local_normalized.starts_with(&format!("{}-", repo_normalized))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::DiscoveredModel;

    fn make_discovered(name: &str) -> DiscoveredModel {
        DiscoveredModel {
            path: PathBuf::from(format!("/models/{}", name)),
            name: name.to_string(),
            file_size: 0,
            display_name: name.to_string(),
        }
    }

    #[test]
    fn exact_match() {
        let models = vec![make_discovered("Qwen2.5-7B-Instruct.gguf")];
        assert!(model_is_downloaded(&models, "Qwen/Qwen2.5-7B-Instruct"));
    }

    #[test]
    fn hyphen_separator_match() {
        let models = vec![make_discovered("Qwen2.5-7B-Instruct-Q4_K_M.gguf")];
        assert!(model_is_downloaded(&models, "Qwen/Qwen2.5-7B-Instruct"));
    }

    #[test]
    fn underscore_separator_match() {
        let models = vec![make_discovered("Qwen2.5_7B_Instruct.gguf")];
        assert!(model_is_downloaded(&models, "Qwen/Qwen2.5-7B-Instruct"));
    }

    #[test]
    fn underscore_separator_match_with_quant() {
        let models = vec![make_discovered("Qwen2.5_7B_Instruct-Q4_K_M.gguf")];
        assert!(model_is_downloaded(&models, "Qwen/Qwen2.5-7B-Instruct"));
    }

    #[test]
    fn case_insensitive_match() {
        let models = vec![make_discovered("qwen2.5-7b-instruct-q4_k_m.gguf")];
        assert!(model_is_downloaded(&models, "Qwen/Qwen2.5-7B-Instruct"));
    }

    #[test]
    fn no_match_different_model() {
        let models = vec![make_discovered("Llama-3.1-8B-Instruct-Q4_K_M.gguf")];
        assert!(!model_is_downloaded(&models, "Qwen/Qwen2.5-7B-Instruct"));
    }

    #[test]
    fn no_match_partial_prefix_false_positive() {
        let models = vec![make_discovered("Qwen2.5-Mistral-Q4_K_M.gguf")];
        assert!(!model_is_downloaded(&models, "Qwen/Qwen2.5-7B-Instruct"));
    }

    #[test]
    fn no_match_empty_models() {
        let models: Vec<DiscoveredModel> = vec![];
        assert!(!model_is_downloaded(&models, "Qwen/Qwen2.5-7B-Instruct"));
    }

    #[test]
    fn strip_gguf_extension() {
        let models = vec![make_discovered("Qwen2.5-7B-Instruct-Q4_K_M.GGUF")];
        assert!(model_is_downloaded(&models, "Qwen/Qwen2.5-7B-Instruct"));
    }

    #[test]
    fn no_gguf_extension() {
        let models = vec![make_discovered("Qwen2.5-7B-Instruct-Q4_K_M")];
        assert!(model_is_downloaded(&models, "Qwen/Qwen2.5-7B-Instruct"));
    }

    #[test]
    fn multiple_models() {
        let models = vec![
            make_discovered("Llama-3.1-8B-Instruct-Q4_K_M.gguf"),
            make_discovered("Qwen2.5-7B-Instruct-Q4_K_M.gguf"),
            make_discovered("Mistral-7B-v0.3-Q8_0.gguf"),
        ];
        assert!(model_is_downloaded(&models, "Qwen/Qwen2.5-7B-Instruct"));
        assert!(model_is_downloaded(&models, "meta-llama/Llama-3.1-8B-Instruct"));
        assert!(model_is_downloaded(&models, "mistralai/Mistral-7B-v0.3"));
        assert!(!model_is_downloaded(&models, "google/gemma-2-9b"));
    }
}
