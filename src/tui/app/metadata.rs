use super::types::App;

impl App {
    /// Compute VRAM estimate from model file size and current settings.
    pub fn update_vram_estimate(&mut self) {
        if let Some(model) = self.selected_model() {
            let model_mib = model.file_size / (1024 * 1024);
            let hidden = if self.loading.model_hidden_size > 0 {
                Some(self.loading.model_hidden_size)
            } else {
                None
            };
            let n_head = if self.loading.model_n_head > 0 {
                Some(self.loading.model_n_head)
            } else {
                None
            };
            let n_kv_head = if self.loading.model_n_kv_head > 0 {
                Some(self.loading.model_n_kv_head)
            } else {
                None
            };
            let gpu_mem_total_mib = self.metrics.gpu_mem_total / (1024 * 1024);
            self.loading.vram_estimate = crate::models::estimate_vram_mib(
                model_mib,
                &self.settings,
                self.loading.model_total_layers,
                hidden,
                n_head,
                n_kv_head,
                gpu_mem_total_mib,
            );
        }
    }

   /// Read metadata (layers, hidden size) from the model's GGUF file.
    ///
    /// Uses a cache keyed by the model's full path, so each unique model
    /// is parsed only once regardless of how many times it's selected.
    pub fn update_model_metadata(&mut self) {
        let model = match self.selected_model() {
            Some(m) => m.clone(),
            None => return,
        };
        let key = model.path.to_string_lossy().to_string();

        // Cache hit — use stored values immediately without re-parsing.
        if let Some(cached) = self.search.gguf_metadata_cache.get(&key) {
            self.loading.model_total_layers = cached.layers;
            self.loading.model_hidden_size = cached.hidden_size;
            self.loading.model_n_ctx_train = cached.n_ctx_train;
            self.loading.model_n_head = cached.n_head;
            self.loading.model_n_kv_head = cached.n_kv_head;
            if self.loading.model_hidden_size > 0 {
                self.update_vram_estimate();
            }
            return;
        }

        // Cache miss — parse the GGUF file and store in cache.
        if let Ok(meta) = crate::models::GgufMetadata::from_path(&model.path) {
            self.loading.model_total_layers = meta.layers;
            self.loading.model_hidden_size = meta.hidden_size;
            self.loading.model_n_ctx_train = meta.n_ctx_train;
            self.loading.model_n_head = meta.n_head;
            self.loading.model_n_kv_head = meta.n_kv_head;

            if meta.arch == "mtp" {
                self.settings.spec_type = "draft-mtp".to_string();
                self.settings.draft_tokens = meta.draft_tokens;
            }

            self.search.gguf_metadata_cache.insert(key, meta);
        } else {
            self.add_log(
                format!("Failed to parse GGUF metadata for {}", model.path.display()),
                crate::config::LogLevel::Error,
            );
        }

        // Compute VRAM estimate once, after metadata fields are populated.
        if self.loading.model_hidden_size > 0 {
            self.update_vram_estimate();
        }
    }
}
