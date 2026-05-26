use super::types::App;

impl App {
    /// Compute VRAM estimate from model file size and current settings.
    pub fn update_vram_estimate(&mut self) {
        if let Some(model) = self.selected_model() {
            let model_mib = model.file_size / (1024 * 1024);
            let hidden = if self.model_hidden_size > 0 { Some(self.model_hidden_size) } else { None };
            let n_head = if self.model_n_head > 0 { Some(self.model_n_head) } else { None };
            let n_kv_head = if self.model_n_kv_head > 0 { Some(self.model_n_kv_head) } else { None };
            let gpu_mem_total_mib = self.metrics.gpu_mem_total / (1024 * 1024);
            self.vram_estimate = crate::models::estimate_vram_mib(
                model_mib, &self.settings, self.model_total_layers, hidden,
                n_head, n_kv_head, gpu_mem_total_mib
            );
            self.set_redraw();
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
        
        // Evict cache entries if it exceeds the maximum size
        const MAX_CACHE_SIZE: usize = 50;
        if self.gguf_metadata_cache.len() > MAX_CACHE_SIZE {
            // Remove the oldest entry (first inserted)
            if let Some(first_key) = self.gguf_metadata_cache.keys().next().cloned() {
                self.gguf_metadata_cache.remove(&first_key);
            }
        }
        
        // 1. Check persistent cache first
        if let Some(cached) = self.gguf_metadata_cache.get(&key) {
            self.model_total_layers = cached.layers;
            self.model_hidden_size = cached.hidden_size;
            self.model_n_ctx_train = cached.n_ctx_train;
            self.model_n_head = cached.n_head;
            self.model_n_kv_head = cached.n_kv_head;
        }

       // 2. Debounce logic: only skip if we tried this EXACT file (path + mtime) very recently
        // and it wasn't GGUF or we failed to parse it.
        if let Ok(meta) = std::fs::metadata(&model.path) {
            let mtime = meta.modified().unwrap_or(std::time::SystemTime::now());
            let (last_path, last_mtime) = &self.last_metadata_parse;
            if last_path == &model.path && mtime == *last_mtime {
                // Already tried this version of the file and it's not in cache (meaning it failed or is not GGUF)
                if self.model_hidden_size > 0 {
                    self.update_vram_estimate();
                }
                return;
            }
            self.last_metadata_parse = (model.path.clone(), mtime);
        }

        // 3. Perform the actual parse
        let path_str = model.path.to_string_lossy();
        match gguf_rs::get_gguf_container(&path_str) {
            Ok(mut container) => {
                match container.decode() {
                    Ok(model_data) => {
                        let mut layers = 0u32;
                        let mut hidden = 0u32;
                        let mut n_ctx_train = 0u32;
                        let mut n_head = 0u32;
                        let mut n_kv_head = 0u32;
                        let mut arch = String::new();
                        let mut file_type = String::new();
                        let mut quantization = String::new();
                        let mut model_parameters = String::new();
                        let mut domain = String::new();
                        let mut capabilities = Vec::new();
                        let mut tokenizer = String::new();
                        let mut vocab_size = 0u32;

                        if let Some(value) = model_data.metadata().get("general.architecture")
                            && let Some(v) = value.as_str() { arch = v.to_string(); }

                        // Detect MTP (Multi-Token Prediction)
                        if arch == "mtp" {
                            self.settings.is_mtp = true;
                            if let Some(value) = model_data.metadata().get("mtp.draft_tokens") {
                                self.settings.draft_tokens = value.as_u64()
                                    .or_else(|| value.as_i64().map(|x| x as u64))
                                    .or_else(|| value.as_f64().map(|x| x as u64))
                                    .unwrap_or(0) as u32;
                            }
                        }

                        // Capabilities
                        if model_data.metadata().contains_key("tokenizer.chat_template") {
                            capabilities.push("chat".to_string());
                        }
                        if let Some(value) = model_data.metadata().get("general.capabilities")
                            && let Some(arr) = value.as_array() {
                                for v in arr {
                                    if let Some(s) = v.as_str() {
                                        capabilities.push(s.to_string());
                                    }
                                }
                            }

                        let extract_num = |key: &str| -> Option<u64> {
                            model_data.metadata().get(key).and_then(|v| {
                                v.as_u64()
                                    .or_else(|| v.as_i64().map(|x| x as u64))
                                    .or_else(|| v.as_f64().map(|x| x as u64))
                            })
                        };

                        if let Some(v) = extract_num("general.file_type") {
                            quantization = match v {
                                0 => "F32".to_string(),
                                1 => "F16".to_string(),
                                2 => "Q4_0".to_string(),
                                3 => "Q4_1".to_string(),
                                7 => "Q8_0".to_string(),
                                8 => "Q5_0".to_string(),
                                9 => "Q5_1".to_string(),
                                10 => "Q2_K".to_string(),
                                11 => "Q3_K_S".to_string(),
                                12 => "Q3_K_M".to_string(),
                                13 => "Q3_K_L".to_string(),
                                14 => "Q4_K_S".to_string(),
                                15 => "Q4_K_M".to_string(),
                                16 => "Q5_K_S".to_string(),
                                17 => "Q5_K_M".to_string(),
                                18 => "Q6_K".to_string(),
                                19 => "IQ2_XXS".to_string(),
                                20 => "IQ2_XS".to_string(),
                                21 => "IQ3_XXS".to_string(),
                                22 => "IQ1_S".to_string(),
                                23 => "IQ4_NL".to_string(),
                                24 => "IQ3_S".to_string(),
                                25 => "IQ2_S".to_string(),
                                26 => "IQ4_XS".to_string(),
                                _ => format!("Unknown ({})", v),
                            };
                        }

                        let prefix = if arch.is_empty() { "llama" } else { &arch };

                        // Try architecture-specific prefix, fall back to "llama" if missing
                        let get_num_with_fallback = |suffix: &str| -> Option<u64> {
                            extract_num(&format!("{}.{}", prefix, suffix))
                                .or_else(|| {
                                    if prefix != "llama" {
                                        extract_num(&format!("llama.{}", suffix))
                                    } else {
                                        None
                                    }
                                })
                        };

                        if let Some(v) = get_num_with_fallback("block_count") {
                            layers = v as u32;
                        }
                        if let Some(v) = get_num_with_fallback("embedding_length") {
                            hidden = v as u32;
                        }
                        if let Some(v) = get_num_with_fallback("context_length") {
                            n_ctx_train = v as u32;
                        }
                        if let Some(v) = get_num_with_fallback("attention.head_count") {
                            n_head = v as u32;
                        }
                        if let Some(v) = get_num_with_fallback("attention.head_count_kv") {
                            n_kv_head = v as u32;
                        }

                        if layers == 0 && hidden == 0 {
                            let keys: Vec<String> = model_data.metadata().keys().take(10).cloned().collect();
                            self.add_log(format!("GGUF parse: found 0 layers/hidden. Arch: {}. Sample keys: {:?}", arch, keys), crate::config::LogLevel::Info);
                        }
                        if !model_data.get_version().is_empty() {
                            file_type = model_data.get_version();
                        }
                        if !model_data.model_parameters().is_empty() {
                            model_parameters = model_data.model_parameters();
                        }
                        if let Some(value) = model_data.metadata().get("general.domain")
                            && let Some(v) = value.as_str() { domain = v.to_string(); }
                        if let Some(value) = model_data.metadata().get("tokenizer.ggml.model")
                            && let Some(v) = value.as_str() { tokenizer = v.to_string(); }
                        if let Some(value) = model_data.metadata().get("tokenizer.ggml.tokens")
                            && let Some(arr) = value.as_array() {
                                vocab_size = arr.len() as u32;
                            }

                        self.model_total_layers = layers;
                        self.model_hidden_size = hidden;
                        self.model_n_ctx_train = n_ctx_train;
                        self.model_n_head = n_head;
                        self.model_n_kv_head = n_kv_head;

                        // Cache the parsed metadata
                        self.gguf_metadata_cache.insert(key, crate::models::GgufMetadata {
                                layers,
                                hidden_size: hidden,
                                n_ctx_train,
                                n_head,
                                n_kv_head,
                                arch,
                                file_type,
                                quantization,
                                model_parameters,
                                domain,
                                capabilities,
                                tokenizer,
                                vocab_size,
                                draft_tokens: self.settings.draft_tokens,
                            });
                        self.set_redraw();
                    }
                    Err(e) => {
                        self.add_log(format!("Failed to decode GGUF {}: {}", model.path.display(), e), crate::config::LogLevel::Error);
                    }
                }
            }
            Err(e) => {
                self.add_log(format!("Failed to parse GGUF {}: {}", model.path.display(), e), crate::config::LogLevel::Error);
            }
        }

        // Compute VRAM estimate once, after metadata fields are populated.
        if self.model_hidden_size > 0 {
            self.update_vram_estimate();
        }
    }
}
