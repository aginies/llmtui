use super::types::App;
use super::types::ModelsMode;
use std::collections::HashMap;
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
        search_results: &[crate::models::SearchResult],
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
                    if let Some(&expected) = expected_sizes.get(name.as_str())
                        && size != expected {
                            return;
                        }

                    let display_name = path
                        .strip_prefix(dir)
                        .ok()
                        .and_then(|p| p.to_str())
                        .unwrap_or(&name)
                        .to_string();

                    // Try to match with search results to get pipeline_tag and capabilities
                    let (pipeline_tag, capabilities) = search_results
                        .iter()
                        .find(|r| display_name.starts_with(&r.model_id))
                        .map(|r| {
                            (r.pipeline_tag.clone(), r.capabilities.clone())
                        })
                        .unwrap_or((None, Vec::new()));

                    models.push(crate::models::DiscoveredModel {
                        path,
                        name,
                        file_size: size,
                        display_name,
                        pipeline_tag,
                        capabilities,
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
        let model_name = self.selected_model().map(|m| m.display_name.as_str());
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
                self.loading.phase_start_time = None;
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
            self.loading.phase_start_time = None;
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
        let filter = self.search.local_filter.to_lowercase();
        if filter.is_empty() {
            return (0..self.models.len()).collect();
        }
        self.models.iter().enumerate()
            .filter(|(_, m)| m.display_name.to_lowercase().contains(&filter))
            .map(|(i, _)| i)
            .collect()
    }

    pub fn get_sorted_model_indices(&mut self) -> &[usize] {
        // Ensure ctx_cache is populated (needed for ListSort::Context)
        let _ = self.get_ctx_cache();
        let sort_by = match &self.models_mode {
            ModelsMode::List { sort_by } => *sort_by,
            _ => return &[],
        };
        let filter = self.search.local_filter.to_lowercase();

        if self.search.list_sort_version == 0
            || self.search.last_list_sort_by != sort_by
            || self.search.last_list_filter != filter
        {
            let mut sorted: Vec<usize> = self.get_filtered_model_indices();
            self.sort_model_indices(&mut sorted, sort_by);
            self.search.list_sorted_indices = sorted;
            self.search.list_sort_version = 1;
            self.search.last_list_sort_by = sort_by;
            self.search.last_list_filter = filter;
        }
        &self.search.list_sorted_indices
    }

    fn sort_model_indices(&self, indices: &mut [usize], sort_by: crate::models::ListSort) {
        indices.sort_by(|&a, &b| {
            let model_a = &self.models[a];
            let model_b = &self.models[b];
            match sort_by {
                crate::models::ListSort::Name => {
                    model_a.display_name.cmp(&model_b.display_name)
                }
                crate::models::ListSort::Status => {
                    let state_a = self.model_states.get(&model_a.display_name);
                    let state_b = self.model_states.get(&model_b.display_name);
                    let prio_a = match state_a {
                        Some(crate::models::ModelState::Loaded { .. }) => 3,
                        Some(crate::models::ModelState::Loading) => 2,
                        Some(crate::models::ModelState::Benchmarking) => 1,
                        _ => 0,
                    };
                    let prio_b = match state_b {
                        Some(crate::models::ModelState::Loaded { .. }) => 3,
                        Some(crate::models::ModelState::Loading) => 2,
                        Some(crate::models::ModelState::Benchmarking) => 1,
                        _ => 0,
                    };
                    prio_b.cmp(&prio_a)
                }
                crate::models::ListSort::Params => {
                    let ka = &*model_a.path.to_string_lossy();
                    let kb = &*model_b.path.to_string_lossy();
                    let meta_a = self.search.gguf_metadata_cache.get(ka);
                    let meta_b = self.search.gguf_metadata_cache.get(kb);
                    let val_a = meta_a.map(|m| {
                        let trimmed = m.model_parameters.trim();
                        let num_str = trimmed.trim_end_matches(|c: char| c == 'B' || c == 'b').trim();
                        num_str.parse::<f64>().unwrap_or(0.0)
                    }).unwrap_or(0.0);
                    let val_b = meta_b.map(|m| {
                        let trimmed = m.model_parameters.trim();
                        let num_str = trimmed.trim_end_matches(|c: char| c == 'B' || c == 'b').trim();
                        num_str.parse::<f64>().unwrap_or(0.0)
                    }).unwrap_or(0.0);
                    val_b.partial_cmp(&val_a).unwrap_or(std::cmp::Ordering::Equal)
                }
                crate::models::ListSort::Qual => {
                    let ka = &*model_a.path.to_string_lossy();
                    let kb = &*model_b.path.to_string_lossy();
                    let meta_a = self.search.gguf_metadata_cache.get(ka);
                    let meta_b = self.search.gguf_metadata_cache.get(kb);
                    let rank_a = meta_a.map(|m| m.quality_rank).unwrap_or(0);
                    let rank_b = meta_b.map(|m| m.quality_rank).unwrap_or(0);
                    rank_b.cmp(&rank_a)
                }
                crate::models::ListSort::Context => {
                    let ctx_a = self.search.ctx_cache.get(&model_a.display_name)
                        .map(|(c, _, _)| *c)
                        .unwrap_or(0);
                    let ctx_b = self.search.ctx_cache.get(&model_b.display_name)
                        .map(|(c, _, _)| *c)
                        .unwrap_or(0);
                    ctx_b.cmp(&ctx_a)
                }
            }
        });
    }

    pub fn get_ctx_cache(&mut self) -> HashMap<String, (u32, bool, f32)> {
        if self.search.ctx_cache_version == 0 {
            let mut cache: HashMap<String, (u32, bool, f32)> =
                HashMap::with_capacity(self.models.len());
            for model in &self.models {
                let s = self.config.resolve_settings(Some(model.display_name.as_str()), None);
                cache.insert(
                    model.display_name.clone(),
                    (s.context_length, s.rope_yarn_enabled, s.rope_scale),
                );
            }
            self.search.ctx_cache = cache.clone();
            self.search.ctx_cache_version = 1;
        }
        self.search.ctx_cache.clone()
    }

    pub fn invalidate_list_caches(&mut self) {
        self.search.list_sort_version = 0;
        self.search.ctx_cache_version = 0;
    }

    pub fn rebuild_downloaded_set(&mut self) {
        self.search.downloaded_filenames = self.models.iter()
            .map(|m| m.name.to_lowercase())
            .collect();
    }
}

/// Check if a given suffix on a normalized GGUF filename corresponds to a valid quantization format
/// (e.g. "-q4_k_m", "-iq3_xxs", "-fp16", "-bf16", "-f32").
fn is_quantization_suffix(suffix: &str) -> bool {
    if !suffix.starts_with('-') {
        return false;
    }
    let rest = &suffix[1..];
    
    if rest.starts_with('q') {
        let after_q = &rest[1..];
        if after_q.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            return true;
        }
    }
    if rest.starts_with("iq") {
        let after_iq = &rest[2..];
        if after_iq.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            return true;
        }
    }
    if rest.starts_with("fp") {
        let after_fp = &rest[2..];
        if after_fp.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            return true;
        }
    }
    if rest.starts_with("bf") {
        let after_bf = &rest[2..];
        if after_bf.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            return true;
        }
    }
    if rest.starts_with('f') {
        let after_f = &rest[1..];
        if after_f.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            return true;
        }
    }
    false
}

/// Check if a model (identified by HF model_id like "unsloth/Qwen3.5-4B-MTP-GGUF")
/// is already downloaded locally by comparing it against our list of discovered models
/// using normalized names, handling separators and common HF repository suffixes.
pub fn model_is_downloaded(models: &[crate::models::DiscoveredModel], model_id: &str) -> bool {
    let repo_parts: Vec<&str> = model_id.split('/').collect();
    let (repo_author, repo_model_name) = if repo_parts.len() > 1 {
        (Some(repo_parts[0]), repo_parts[1])
    } else {
        (None, model_id)
    };

    let mut expected_prefix = repo_model_name.to_lowercase().replace('_', "-");
    
    if expected_prefix.ends_with("-gguf") {
        expected_prefix = expected_prefix.strip_suffix("-gguf").unwrap().to_string();
    }
    if expected_prefix.ends_with("-mtp") {
        expected_prefix = expected_prefix.strip_suffix("-mtp").unwrap().to_string();
    }

    for model in models {
        // Direct match: if the file is in a subdirectory that exactly starts with model_id,
        // it was downloaded specifically for this model.
        let display_name_lower = model.display_name.to_lowercase().replace('_', "-");
        let model_id_lower = model_id.to_lowercase().replace('_', "-");
        if display_name_lower.starts_with(&model_id_lower) {
            if display_name_lower.len() == model_id_lower.len() {
                return true;
            }
            if let Some(next_char) = display_name_lower[model_id_lower.len()..].chars().next() {
                if next_char == '/' {
                    return true;
                }
            }
        }

        let local_parts: Vec<&str> = model.display_name.split('/').collect();
        let local_author = if local_parts.len() > 1 {
            Some(local_parts[0])
        } else {
            None
        };

        // If both authors are specified, they must match (case-insensitively)
        if let (Some(ra), Some(la)) = (repo_author, local_author) {
            if ra.to_lowercase() != la.to_lowercase() {
                continue;
            }
        }

        let mut local_name = model.name.to_lowercase().replace('_', "-");
        if local_name.ends_with(".gguf") {
            local_name = local_name.strip_suffix(".gguf").unwrap().to_string();
        }
        
        if local_name.starts_with(&expected_prefix) {
            if local_name.len() == expected_prefix.len() {
                return true;
            }
            let suffix = &local_name[expected_prefix.len()..];
            if is_quantization_suffix(suffix) {
                return true;
            }
        }
    }
    false
}

