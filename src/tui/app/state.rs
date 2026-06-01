use super::types::LoadingPhase::*;
use super::types::{App, LoadingPhase};
use crate::config::LogLevel;
use crate::models::ModelState;
use chrono::Local;

impl App {
    pub fn add_log(&mut self, message: impl Into<String>, level: LogLevel) {
        let msg = message.into();
        self.log_message(&msg, level);
        self.update_spinner();
        self.detect_loading_phases(&msg);
        self.parse_loading_details(&msg);
        self.detect_load_state(&msg);
        let previous_progress = self.loading.loading_progress;
        self.compute_progress();
        self.loading.progress_target = self.loading.loading_progress;
        self.loading.loading_progress =
            previous_progress * 0.85 + self.loading.progress_target * 0.15;
        self.trim_log();
        self.log
            .log_entries
            .push_back(crate::config::LogEntry::new(msg, level));
    }

    fn log_message(&mut self, msg: &str, level: LogLevel) {
        match level {
            LogLevel::Info => tracing::info!("{}", msg),
            LogLevel::Warning => tracing::warn!("{}", msg),
            LogLevel::Error => tracing::error!("{}", msg),
        }
    }

    fn update_spinner(&mut self) {
        self.loading.last_spinner_time = Some(tokio::time::Instant::now());
        self.loading.loading_spinner = 0;
    }

    fn detect_loading_phases(&mut self, msg: &str) {
        let upper = msg.to_uppercase();
        if self.loading.loading_phases.is_empty() {
            // Detect server starting (first log line after spawn)
            if upper.contains("LLAMA") || upper.contains("SERVER") || upper.contains("GGML") {
                self.loading.loading_phases.insert(ServerStarting);
                self.loading.last_active_phase = Some(ServerStarting);
            }
        }
        if upper.contains("LLAMA_MODEL_LOADER") || upper.contains("LOADING MODEL") {
            self.ui.last_error_message = None;
            self.loading.loading_phases.insert(LoadingModel);
            self.loading.last_active_phase = Some(LoadingModel);
        }
        if upper.contains("LOADED META") || upper.contains("META DATA") {
            self.ui.last_error_message = None;
            self.loading.loading_phases.insert(LoadingMeta);
            self.loading.last_active_phase = Some(LoadingMeta);
        }
        if upper.contains("LOAD_TENSORS:") {
            self.ui.last_error_message = None;
            self.loading.loading_phases.insert(LoadingTensors);
            self.loading.last_active_phase = Some(LoadingTensors);
        }
        if upper.contains("SERVER LISTENING")
            || upper.contains("HTTP SERVER LISTENING")
            || upper.contains("LOAD_MODEL: INITIALIZING SLOTS")
            || (upper.contains("SRV")
                && upper.contains("LOAD_MODEL")
                && upper.contains("INITIALIZING"))
        {
            self.loading.loading_phases.insert(ServerListening);
            self.loading.last_active_phase = Some(ServerListening);
        }
    }

    fn parse_loading_details(&mut self, msg: &str) {
        let upper = msg.to_uppercase();
        if self.loading.loading_phases.contains(&LoadingTensors) {
            // Parse "loading tensor X of Y" or "loading tensor X out of Y" pattern
            if upper.contains("LOADING TENSOR") {
                if let Some(pos) = msg.to_lowercase().find("loading tensor") {
                    let rest = &msg[pos + "loading tensor".len()..];
                    let parts: Vec<&str> = rest.split_whitespace().collect();
                    if parts.len() >= 3 {
                        if let Ok(n) = parts[0].parse::<u32>() {
                            self.loading.load_progress.tensors_loaded = n;
                        }
                        // "of Y" or "out of Y" — Y is at index 2 or 4
                        let total_idx = if parts.len() >= 5 && parts[2].to_lowercase() == "out" {
                            4
                        } else if parts.len() >= 3 && parts[1].to_lowercase() == "of" {
                            2
                        } else {
                            usize::MAX
                        };
                        if total_idx != usize::MAX {
                            if let Ok(total) = parts[total_idx]
                                .trim_end_matches(|c: char| !c.is_ascii_digit())
                                .parse::<u32>()
                            {
                                self.loading.load_progress.tensors_total = Some(total);
                            }
                        }
                    }
                }
            }
            // Count dots from progress lines like "................................"
            // Only use dot-counting as fallback when we haven't seen an explicit tensor count yet
            if self.loading.load_progress.tensors_total.is_none() {
                let dot_count = msg.chars().filter(|&c| c == '.').count();
                if dot_count > 0 && dot_count <= 200 {
                    self.loading.load_progress.tensors_loaded += dot_count as u32;
                }
            }

            // Offloading N repeating layers to GPU
            if upper.contains("OFFLOADING")
                && upper.contains("REPEATING LAYERS")
                && let Some(pos) = msg.find("offloading")
            {
                let rest = &msg[pos + "offloading".len()..];
                if let Some(colon_pos) = rest.find(':') {
                    let rest = rest[colon_pos + 1..].trim_start();
                    let end = rest.find(' ').unwrap_or(rest.len());
                    if let Ok(count) = rest[..end].trim().parse::<u32>() {
                        self.loading.load_progress.layers_total = Some(count);
                    }
                }
            }

            // Offloaded X/Y layers to GPU
            if upper.contains("OFFLOADED")
                && upper.contains("LAYERS")
                && let Some(pos) = msg.find("offloaded")
            {
                let rest = &msg[pos + "offloaded".len()..];
                if let Some(slash) = rest.find('/') {
                    let before = rest[..slash].trim();
                    let after = rest[slash + 1..].trim();
                    if let Ok(loaded) = before.parse::<u32>() {
                        self.loading.load_progress.layers_loaded = Some(loaded);
                    }
                    if let Ok(total) = after.split_whitespace().next().unwrap_or("").parse::<u32>()
                    {
                        self.loading.load_progress.layers_total = Some(total);
                    }
                }
                // Also handle "offloaded N layers" without Y
                if self.loading.load_progress.layers_loaded.is_none() {
                    let rest = rest.trim_start();
                    let end = rest.find(' ').unwrap_or(rest.len());
                    if let Ok(count) = rest[..end].trim().parse::<u32>() {
                        self.loading.load_progress.layers_loaded = Some(count);
                    }
                }
            }

            // CPU_Mapped model buffer size = X MiB
            // Vulkan0 model buffer size = X MiB
            for keyword in &["model buffer size", "kv buffer size"] {
                if let Some(pos) = msg.to_lowercase().find(keyword) {
                    let before = &msg[..pos];
                    let device = before.split_whitespace().last().unwrap_or("").to_string();
                    if !device.is_empty() {
                        let rest = &msg[pos + keyword.len()..];
                        if let Some(eq_pos) = rest.find('=') {
                            let after = rest[eq_pos + 1..].trim();
                            let end = after
                                .find(|c: char| !c.is_ascii_digit() && c != '.')
                                .unwrap_or(after.len());
                            if let Ok(mib) = after[..end].parse::<f64>() {
                                let exists = self
                                    .loading
                                    .load_progress
                                    .buffers
                                    .iter_mut()
                                    .find(|b| b.device == device);
                                if let Some(buf) = exists {
                                    buf.buffer_size_mib = mib;
                                } else {
                                    self.loading.load_progress.buffers.push(
                                        crate::models::GPUBuffer {
                                            device,
                                            buffer_size_mib: mib,
                                        },
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn detect_load_state(&mut self, _msg: &str) {
        // Log-based load state detection removed.
        // Loading completion is now detected via /health API polling.
        // Server exit is now detected via channel-based signaling.
        // Error detection still uses log parsing for OOM/crash detection.
        let upper = _msg.to_uppercase();

        let is_error = upper.contains("ERROR")
            || upper.contains("FAILED TO LOAD")
            || upper.contains("EXCEPTION")
            || upper.contains("VK::SYSTEMERROR")
            || upper.contains("OUTOFDEVICEMEMORY")
            || upper.contains("OUT OF MEMORY");

        if is_error {
            let is_loading = self
                .model_states
                .values()
                .any(|s| matches!(s, ModelState::Loading));
            if is_loading {
                let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
                let error_msg =
                    if upper.contains("OUTOFDEVICEMEMORY") || upper.contains("OUT OF MEMORY") {
                        format!("Last Failed to load a model (OOM - {})", timestamp)
                    } else {
                        format!("Last Failed to load a model ({})", timestamp)
                    };

                self.ui.last_error_message = Some(error_msg);
                self.reset_loading_state(false);
            }
        }
    }

    pub(crate) fn compute_progress(&mut self) {
        const PHASE_WEIGHTS: [(LoadingPhase, f32); 5] = [
            (ServerStarting, 0.08),
            (LoadingModel, 0.07),
            (LoadingMeta, 0.07),
            (LoadingTensors, 0.70),
            (ServerListening, 0.08),
        ];

        let mut phase_progress: f32 = 0.0;
        for (phase, weight) in &PHASE_WEIGHTS {
            if self.loading.loading_phases.contains(phase) {
                phase_progress += weight;
            }
        }

        // Handle Complete phase separately — it means 100%
        if self.loading.loading_phases.contains(&Complete) {
            self.loading.loading_progress = 1.0;
            return;
        }

        // Spinner interpolation for ServerStarting (works even as the only active phase)
        if self.loading.loading_phases.contains(&ServerStarting)
            && self.loading.loading_phases.len() == 1
            && self.loading.last_active_phase == Some(ServerStarting)
        {
            if let Some(last_spinner) = self.loading.last_spinner_time {
                let elapsed = last_spinner.elapsed();
                phase_progress =
                    (elapsed.as_millis() as f32 / 2000.0).min(1.0) * PHASE_WEIGHTS[0].1;
            }
        } else if self.loading.loading_phases.len() > 1 {
            // Apply interpolation within the current active phase for smooth transitions
            if let Some(phase) = self.loading.last_active_phase {
                let cumulative_before: f32 = PHASE_WEIGHTS
                    .iter()
                    .filter(|(p, _)| *p != phase && self.loading.loading_phases.contains(p))
                    .map(|(_, w)| w)
                    .sum();

                let phase_fraction = match phase {
                    LoadingModel => 0.5,
                    LoadingMeta => 0.5,
                    LoadingTensors => {
                        let mut tensor_fraction: f32 = 0.0;
                        if let (Some(loaded), Some(total)) = (
                            self.loading.load_progress.layers_loaded,
                            self.loading.load_progress.layers_total,
                        ) {
                            let layer_fraction = loaded as f32 / total as f32;
                            tensor_fraction = layer_fraction.min(1.0);
                        }
                        if self.loading.load_progress.tensors_loaded > 0 {
                            let estimated_total: f32 =
                                match self.loading.load_progress.tensors_total {
                                    Some(total) => total as f32,
                                    None => match self.loading.load_progress.layers_total {
                                        Some(layers) => (layers as f32 * 12.0 + 10.0).max(100.0),
                                        None => 500.0,
                                    },
                                };
                            tensor_fraction = (self.loading.load_progress.tensors_loaded as f32
                                / estimated_total)
                                .min(0.95);
                        }
                        tensor_fraction
                    }
                    ServerListening => 0.8,
                    Complete => 1.0,
                    ServerStarting => 0.0,
                };

                phase_progress = cumulative_before
                    + phase_fraction
                        * PHASE_WEIGHTS
                            .iter()
                            .find(|(p, _)| *p == phase)
                            .map(|(_, w)| *w)
                            .unwrap_or(0.0);
            }
        }

        if phase_progress > 0.0 {
            self.loading.loading_progress = phase_progress;
        }
    }

    pub fn handle_server_exit(&mut self) {
        if let Some(rx) = &mut self.server.server_exit_rx {
            if let Ok(()) = rx.try_recv() {
                self.server.server_handle = None;
                self.loading.loading_phases.clear();
                self.loading.last_active_phase = None;
                self.loading.loading_progress = 0.0;
                self.loading.load_progress = Default::default();

                for state in self.model_states.values_mut() {
                    *state = crate::models::ModelState::Available;
                }
            }
        }
    }

    fn trim_log(&mut self) {
        if self.log.log_entries.len() >= 500 {
            self.log.log_entries.pop_front();
        }
    }

    pub fn is_model_loaded(&self, display_name: &str) -> bool {
        matches!(
            self.model_states.get(display_name),
            Some(ModelState::Loaded { .. })
        )
    }

    /// Reset loading state (progress bar and model status) on failure.
    pub fn reset_loading_state(&mut self, is_crash: bool) {
        self.loading.loading_phases.clear();
        self.loading.last_active_phase = None;
        self.loading.loading_progress = 0.0;
        self.loading.load_progress = Default::default();
        self.loading.last_spinner_time = None;
        self.loading.loading_spinner = 0;
        if let Some(h) = self.loading.health_poll_handle.take() {
            h.abort();
        }

        // Models to fail: always any that were Loading.
        // If it's a crash, also fail all that were Loaded.
        let to_fail: Vec<String> = self
            .model_states
            .iter()
            .filter(|(_, state)| {
                matches!(state, ModelState::Loading)
                    || (is_crash && matches!(state, ModelState::Loaded { .. }))
            })
            .map(|(name, _)| name.clone())
            .collect();

        // Remove from loaded list and set to Failed
        for name in &to_fail {
            self.server
                .loaded_model_names
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .retain(|n| n != name);
            let error = self.ui.last_error_message.clone().unwrap_or_else(|| {
                let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
                format!("Last Failed to load a model ({})", timestamp)
            });
            self.model_states
                .insert(name.clone(), ModelState::Failed { error });
        }
    }

    pub fn tick_spinner(&mut self) {
        if self.is_loading() {
            let spinner_interval = std::time::Duration::from_millis(150);
            if self.loading.last_spinner_time.is_none()
                || self.loading.last_spinner_time.unwrap().elapsed() > spinner_interval
            {
                self.loading.loading_spinner = (self.loading.loading_spinner + 1) % 4;
                self.loading.last_spinner_time = Some(tokio::time::Instant::now());
            }
        }
    }

    pub fn is_loading(&self) -> bool {
        self.model_states
            .values()
            .any(|s| matches!(s, ModelState::Loading))
    }
}
