use super::types::App;

impl App {
    /// Compute VRAM estimate from model file size and current settings.
    pub fn update_vram_estimate(&mut self) {
        if let Some(model) = self.selected_model() {
            let model_mib = model.file_size / (1024 * 1024);
            let hidden = if self.loading.model_hidden_size > 0 { Some(self.loading.model_hidden_size) } else { None };
            let n_head = if self.loading.model_n_head > 0 { Some(self.loading.model_n_head) } else { None };
            let n_kv_head = if self.loading.model_n_kv_head > 0 { Some(self.loading.model_n_kv_head) } else { None };
            let gpu_mem_total_mib = self.metrics.gpu_mem_total / (1024 * 1024);
            self.loading.vram_estimate = crate::models::estimate_vram_mib(
                model_mib, &self.settings, self.loading.model_total_layers, hidden,
                n_head, n_kv_head, gpu_mem_total_mib
            );
        }
    }

    /// Read metadata (layers, hidden size) from the model's GGUF file.
    ///
    /// Uses a single cache keyed by the model's full path, so each unique
    /// model is parsed only once regardless of how many times it's selected.
    pub fn update_model_metadata(&mut self) {
        let model = match self.selected_model() {
            Some(m) => m.clone(),
            None => return,
        };
        let key = model.path.to_string_lossy().to_string();

        // 1. Debounce: skip re-parse if file hasn't changed.
        // This must run before the cache lookup so file changes are detected
        // even when a stale cache entry exists.
        if let Ok(meta) = std::fs::metadata(&model.path) {
            let mtime = meta.modified().unwrap_or(std::time::SystemTime::now());
            let (last_path, last_mtime) = &self.loading.last_metadata_parse;
            if last_path == &model.path && mtime == *last_mtime {
                // File unchanged — use cached values if available.
                if let Some(cached) = self.search.gguf_metadata_cache.get(&key) {
                    self.loading.model_total_layers = cached.layers;
                    self.loading.model_hidden_size = cached.hidden_size;
                    self.loading.model_n_ctx_train = cached.n_ctx_train;
                    self.loading.model_n_head = cached.n_head;
                    self.loading.model_n_kv_head = cached.n_kv_head;
                }
                if self.loading.model_hidden_size > 0 {
                    self.update_vram_estimate();
                }
                return;
            }
            self.loading.last_metadata_parse = (model.path.clone(), mtime);
        }

        // 2. Evict cache entries if it exceeds the maximum size.
        // BTreeMap keys are sorted, so `next()` returns the smallest (oldest) key.
        const MAX_CACHE_SIZE: usize = 50;
        if self.search.gguf_metadata_cache.len() > MAX_CACHE_SIZE {
            if let Some(first_key) = self.search.gguf_metadata_cache.keys().next().cloned() {
                self.search.gguf_metadata_cache.remove(&first_key);
            }
        }

        // 3. Perform the actual parse
        if let Ok(meta) = crate::models::GgufMetadata::from_path(&model.path) {
            self.loading.model_total_layers = meta.layers;
            self.loading.model_hidden_size = meta.hidden_size;
            self.loading.model_n_ctx_train = meta.n_ctx_train;
            self.loading.model_n_head = meta.n_head;
            self.loading.model_n_kv_head = meta.n_kv_head;

            if meta.arch == "mtp" {
                self.settings.is_mtp = true;
                self.settings.draft_tokens = meta.draft_tokens;
            }

            // Cache the parsed metadata
            self.search.gguf_metadata_cache.insert(key, meta);
        } else {
            self.add_log(format!("Failed to parse GGUF metadata for {}", model.path.display()), crate::config::LogLevel::Error);
        }

        // Compute VRAM estimate once, after metadata fields are populated.
        if self.loading.model_hidden_size > 0 {
            self.update_vram_estimate();
        }
    }
}
