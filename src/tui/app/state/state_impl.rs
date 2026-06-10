use super::parsing::*;
use crate::tui::app::types::LoadingPhase::*;
use crate::tui::app::types::{App, LoadingPhase};
use crate::config::LogLevel;
use crate::models::ModelState;
use chrono::Local;

impl App {
    pub fn add_log(&mut self, message: impl Into<String>, level: LogLevel) {
        let msg = message.into();
        self.log_message(&msg, level);
        self.detect_loading_phases(&msg);
        self.parse_loading_details(&msg);
        self.detect_load_state(&msg);
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

    fn detect_loading_phases(&mut self, msg: &str) {
        // All 5 regex-detected phases found — no new phases can ever be added
        if self.loading.loading_phases.len() >= 5 {
            return;
        }
        let old_phase = self.loading.last_active_phase;
        if self.loading.loading_phases.is_empty() && LLAMA_START.is_match(msg) {
            self.loading.loading_phases.insert(ServerStarting);
            self.loading.last_active_phase = Some(ServerStarting);
        }
        if LOADING_MODEL.is_match(msg) {
            self.ui.last_error_message = None;
            self.loading.loading_phases.insert(LoadingModel);
            self.loading.last_active_phase = Some(LoadingModel);
        }
        if LOADED_META.is_match(msg) {
            self.ui.last_error_message = None;
            self.loading.loading_phases.insert(LoadingMeta);
            self.loading.last_active_phase = Some(LoadingMeta);
        }
        if LOAD_TENSORS.is_match(msg) {
            self.ui.last_error_message = None;
            self.loading.loading_phases.insert(LoadingTensors);
            self.loading.last_active_phase = Some(LoadingTensors);
        }
        if SERVER_LISTENING.is_match(msg) {
            self.loading.loading_phases.insert(ServerListening);
            self.loading.last_active_phase = Some(ServerListening);
        }
        if self.loading.last_active_phase != old_phase {
            self.loading.phase_start_time = Some(tokio::time::Instant::now());
        }
    }

    fn parse_loading_details(&mut self, msg: &str) {
        if !self.loading.loading_phases.contains(&LoadingTensors) {
            return;
        }

        // Parse "loading tensor X of Y"
        if let Some(caps) = LOADING_TENSOR.captures(msg) {
            if let Ok(n) = caps.get(1).unwrap().as_str().parse::<u32>() {
                self.loading.load_progress.tensors_loaded = n;
            }
            if let Ok(total) = caps.get(2).unwrap().as_str().parse::<u32>() {
                self.loading.load_progress.tensors_total = Some(total);
            }
            return;
        }

        // Count dots from progress lines as fallback
        if self.loading.load_progress.tensors_total.is_none() {
            let dot_count = msg.chars().filter(|&c| c == '.').count();
            if dot_count > 0 && dot_count <= 200 {
                self.loading.load_progress.tensors_loaded += dot_count as u32;
            }
        }

        // Parse "offloading N repeating layers to GPU"
        if let Some(caps) = OFFLOADING_LAYERS.captures(msg)
            && let Ok(count) = caps.get(1).unwrap().as_str().parse::<u32>() {
                self.loading.load_progress.layers_total = Some(count);
            }

        // Parse "offloaded X/Y layers" or "offloaded X out of Y layers"
        if let Some(caps) = OFFLOADED_LAYERS.captures(msg) {
            if let Ok(loaded) = caps.get(1).unwrap().as_str().parse::<u32>() {
                self.loading.load_progress.layers_loaded = Some(loaded);
            }
            if let Ok(total) = caps.get(2).unwrap().as_str().parse::<u32>() {
                self.loading.load_progress.layers_total = Some(total);
            }
        }

        // Parse buffer sizes: "Vulkan0 model buffer size = X MiB"
        if let Some(caps) = MODEL_BUFFER_SIZE.captures(msg) {
            let device = caps.get(1).map(|m| m.as_str().trim().to_string()).unwrap_or_default();
            if let Ok(mib) = caps.get(2).unwrap().as_str().parse::<f64>() {
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

        // Parse: "kv buffer size = X MiB"
        if let Some(caps) = KV_BUFFER_SIZE.captures(msg)
            && let Ok(mib) = caps.get(1).unwrap().as_str().parse::<f64>() {
                let exists = self
                    .loading
                    .load_progress
                    .buffers
                    .iter_mut()
                    .find(|b| b.device == "kv");
                if let Some(buf) = exists {
                    buf.buffer_size_mib = mib;
                } else {
                    self.loading.load_progress.buffers.push(
                        crate::models::GPUBuffer {
                            device: "kv".to_string(),
                            buffer_size_mib: mib,
                        },
                    );
                }
            }
    }

    fn detect_load_state(&mut self, msg: &str) {
        if !is_loading_error(msg) {
            return;
        }

        let is_loading = self
            .model_states
            .values()
            .any(|s| matches!(s, ModelState::Loading));
        if !is_loading {
            return;
        }

        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
        let error_msg = if is_oom_error(msg) {
            format!("Last Failed to load a model (OOM - {})", timestamp)
        } else {
            format!("Last Failed to load a model ({})", timestamp)
        };

        self.ui.last_error_message = Some(error_msg);
        self.reset_loading_state(false);
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

        // Time interpolation for ServerStarting (works even as the only active phase)
        if self.loading.loading_phases.contains(&ServerStarting)
            && self.loading.loading_phases.len() == 1
            && self.loading.last_active_phase == Some(ServerStarting)
        {
            if let Some(start) = self.loading.phase_start_time {
                let elapsed = start.elapsed();
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

                // Time-based fallback: fraction increases over time so progress never stalls
                let time_fraction = if let Some(start) = self.loading.phase_start_time {
                    let elapsed = start.elapsed().as_secs_f32();
                    let duration = match phase {
                        LoadingModel | LoadingMeta => 1.0,
                        LoadingTensors => 30.0,
                        ServerListening => 2.0,
                        Complete | ServerStarting => f32::MAX,
                    };
                    (elapsed / duration).min(0.95)
                } else {
                    0.0
                };

                let data_fraction = match phase {
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

                let phase_fraction = time_fraction.max(data_fraction);

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
            self.loading.loading_progress = phase_progress.max(self.loading.loading_progress);
        }
    }

    pub fn tick_server_exit(&mut self) {
        if let Some(rx) = &mut self.server.server_exit_rx
            && let Ok(()) = rx.try_recv()
        {
            self.server.server_handle = None;
            self.loading.loading_phases.clear();
            self.loading.last_active_phase = None;
            self.loading.loading_progress = 0.0;
            self.loading.load_progress = Default::default();
            self.loading.last_spinner_time = None;
            self.loading.loading_spinner = 0;
            self.loading.phase_start_time = None;

            if !self.bench_tune.bench_tune_running {
                for state in self.model_states.values_mut() {
                    *state = crate::models::ModelState::Available;
                }
                self.ui.needs_redraw = true;
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
        self.loading.phase_start_time = None;
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
        self.pending.active_model_hint_dirty = true;
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

    pub fn tick_loading_progress(&mut self) {
        if !self.is_loading() {
            return;
        }
        let previous = self.loading.loading_progress;
        self.compute_progress();
        let target = self.loading.loading_progress;
        self.loading.loading_progress = previous * 0.8 + target * 0.2;
    }

    pub fn is_loading(&self) -> bool {
        self.model_states
            .values()
            .any(|s| matches!(s, ModelState::Loading))
    }

    pub fn tick_settings_help(&mut self) {
        if self.ui.active_panel != crate::tui::app::ActivePanel::LlmSettings {
            self.settings_state.help_focus_time = None;
            self.settings_state.help_visible = false;
            return;
        }
        if self.settings_state.help_focus_time.is_none() {
            self.settings_state.help_focus_time = Some(tokio::time::Instant::now());
        }
        if let Some(focus_time) = self.settings_state.help_focus_time
            && !self.settings_state.help_visible
            && focus_time.elapsed() >= std::time::Duration::from_millis(1500)
        {
            self.settings_state.help_visible = true;
        }
    }

    /// Returns true when the app is in a stable idle state with no pending
    /// work that requires a fast event loop. Used to increase the poll
    /// timeout and reduce idle CPU usage.
    pub fn is_truly_idle(&self) -> bool {
        // Active operations that require fast polling
        if self.download.downloading {
            return false;
        }
        if self.is_loading() {
            return false;
        }
        if self.bench_tune.bench_tune_running {
            return false;
        }
        if self.server.spawn_task_handle.is_some() {
            return false;
        }
        if self.server.bench_tune_task_handle.is_some() {
            return false;
        }
        if self.pending.pending_kill.is_some() {
            return false;
        }
        if self.pending.pending_api_load.is_some() {
            return false;
        }
        if self.pending.pending_api_unload.is_some() {
            return false;
        }
        if self.pending.backend_resolving {
            return false;
        }
        if self.pending.backend_resolve_handle.is_some() {
            return false;
        }
        // Active text scrolls need their tick interval
        if self.ui.text_scrolls.values().any(|s| s.visible && s.max_offset > 0) {
            return false;
        }
        // Confirmation dialogs need responsive input
        if matches!(self.ui.global_mode, crate::tui::app::GlobalMode::Confirmation { .. }) {
            return false;
        }
        true
    }
}
