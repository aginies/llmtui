use super::types::App;
use std::path::Path;

impl App {
    pub fn render<T: ratatui::backend::Backend>(&mut self, terminal: &mut ratatui::Terminal<T>) -> std::io::Result<()> {
        if self.needs_redraw {
            terminal.draw(|frame| crate::tui::render::render(frame, self))?;
            self.needs_redraw = false;
        }
        Ok(())
    }

    pub fn discover_models(dir: &Path) -> Vec<crate::models::DiscoveredModel> {
        let mut models = Vec::new();
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
        models.sort_by(|a, b| a.name.cmp(&b.name));
        models
    }

    pub fn reset_to_defaults(&mut self) {
        let defaults = crate::models::ModelSettings::default();
        self.settings = defaults;
        // Clear dirty flag by updating the cache snapshot to match new settings
        self.model_settings_cache = self.settings.clone();
        // Reset model metadata to avoid stale values
        self.model_total_layers = 0;
        self.model_hidden_size = 0;
        self.model_n_ctx_train = 0;
        self.model_n_head = 0;
        self.model_n_kv_head = 0;
        self.vram_estimate = 0;
        self.settings.is_mtp = false;
        self.settings.draft_tokens = 0;
        self.settings_render_cache = None;
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
        self.readme_cache = None;
        if let Some(idx) = self.selected_model_idx {
            let model = self.models[idx].clone();
            self.model_settings_cache = self.selected_model_settings();
            self.settings = self.model_settings_cache.clone();
            self.update_model_metadata();
            self.update_vram_estimate();

            // Sync loading progress with the newly selected model
            if self.is_model_loaded(&model.display_name) {
                self.loading_progress = 1.0;
                if !self.loading_phases.contains(&super::types::LoadingPhase::Complete) {
                    self.loading_phases.insert(super::types::LoadingPhase::Complete);
                }
            } else if matches!(self.model_states.get(&model.display_name), Some(crate::models::ModelState::Loading) | Some(crate::models::ModelState::Benchmarking)) {
                // Keep current loading/benchmarking progress
            } else {
                // Not loaded, loading, or benchmarking, reset progress
                self.loading_progress = 0.0;
                self.loading_phases.clear();
                self.last_active_phase = None;
                self.load_progress = Default::default();
            }
        } else {
            let default_params = self.config.default.clone();
            self.model_settings_cache = default_params.into();
            self.model_total_layers = 0;
            self.model_hidden_size = 0;
            self.model_n_ctx_train = 0;
            self.settings.is_mtp = false;
            self.settings.draft_tokens = 0;
            self.vram_estimate = 0;
            self.loading_progress = 0.0;
            self.loading_phases.clear();
            self.last_active_phase = None;
        }
        self.set_redraw();
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
                self.local_filter.is_empty()
                    || m.display_name
                        .to_lowercase()
                        .contains(&self.local_filter.to_lowercase())
            })
            .map(|(i, _)| i)
            .collect()
    }
}
