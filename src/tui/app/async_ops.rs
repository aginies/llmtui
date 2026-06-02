use super::types::App;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8};

impl App {
    pub async fn start_pending_download(&mut self) {
        if let Some((model_id, filename, download_url, file_size)) =
            self.pending.pending_download.take()
        {
            let models_dirs = &self.config.models_dirs;
            // Use the first directory as the download destination
            let models_dir = models_dirs.first().cloned().unwrap_or_default();
            let dest = models_dir.join(&filename);
            let free_space = crate::backend::hub::get_free_space_bytes(&models_dir);
            if file_size > free_space {
                self.add_log(
                    format!(
                        "Not enough disk space to download {}: need {} but only {} available",
                        filename,
                        crate::tui::format_size(file_size),
                        crate::tui::format_size(free_space)
                    ),
                    crate::config::LogLevel::Warning,
                );
                return;
            }
            let model_id_clone = model_id.clone();
            let filename_clone = filename.clone();
            let url_clone = download_url.clone();
            let cancelled = Arc::new(AtomicBool::new(false));
            let cancelled_clone = cancelled.clone();
            self.add_log(
                format!("Downloading {}...", model_id),
                crate::config::LogLevel::Info,
            );
            let tx = self.ensure_download_channel();
            let tx_clone = tx.clone();
            let cancelled_for_state = cancelled_clone.clone();
            let download_state = Arc::new(AtomicU8::new(1));
            let download_state_clone = download_state.clone();
            let dest_path = dest.clone();
            self.download.download_progress.last_mut().and_then(|d| {
                d.dest = Some(dest_path.clone());
                None::<()>
            });

            tokio::spawn(async move {
                let mut state = crate::models::DownloadState::new(
                    model_id_clone.clone(),
                    filename_clone.clone(),
                    0,
                );
                state.cancel_token = Some(cancelled_for_state);
                state.download_state = 1;
                state.dest = Some(dest_path);
                state.download_state_arc = Some(download_state_clone.clone());
                let result = crate::backend::hub::download_file(
                    &model_id_clone,
                    &filename_clone,
                    &url_clone,
                    &dest,
                    &mut state,
                    download_state_clone,
                    tx_clone,
                )
                .await;
                if let Err(e) = result {
                    state.status = crate::models::DownloadStatus::Error(e.to_string());
                    let _ = tx.send(state);
                }
            });
            self.download.downloading = true;
            self.cancelled = Some(cancelled);
            self.download.download_scroll_state.select(Some(0));
            self.ui.needs_redraw = true;
        }
    }

    pub async fn start_pending_deletion(&mut self, path: PathBuf) {
        let path_clone = path.clone();
        tokio::spawn(async move {
            if let Err(e) = tokio::fs::remove_file(&path_clone).await {
                tracing::warn!("Failed to delete file: {}", e);
            }
        });
        let model_key = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        self.config.model_overrides.delete(&model_key);
        if let Err(e) = self.config.save() {
            self.add_log(
                format!("Failed to save config after deletion: {}", e),
                crate::config::LogLevel::Error,
            );
        }
        self.models.retain(|m| m.path != path);
        if let Some(idx) = self.selected_model_idx {
            if idx >= self.models.len() && !self.models.is_empty() {
                self.selected_model_idx = Some(self.models.len() - 1);
                self.on_model_selection_change();
            } else if self.models.is_empty() {
                self.selected_model_idx = None;
                self.on_model_selection_change();
            } else {
                self.on_model_selection_change();
            }
        }
        self.add_log(
            format!("Model deleted: {:?}", path.file_name().unwrap_or_default()),
            crate::config::LogLevel::Info,
        );
        self.ui.needs_redraw = true;
    }

    pub fn start_pending_backend_deletion(&mut self, backend: crate::models::Backend, tag: String) {
        let bin_dir = crate::backend::hub::get_backend_dir(backend, &tag);
        if bin_dir.exists() {
            if let Err(e) = std::fs::remove_dir_all(&bin_dir) {
                self.add_log(
                    format!("Failed to delete backend: {}", e),
                    crate::config::LogLevel::Error,
                );
            } else {
                self.add_log(
                    format!("Deleted backend {} ({})", backend, tag),
                    crate::config::LogLevel::Info,
                );
                let new_entries = self.fetch_backend_picker_entries();
                if let super::types::GlobalMode::BackendPicker { entries, selected } =
                    &mut self.ui.global_mode
                {
                    *entries = new_entries;
                    if *selected >= entries.len() {
                        *selected = entries.len().saturating_sub(1);
                    }
                }
                self.ui.needs_redraw = true;
            }
        }
    }

    pub async fn poll_backend_resolution(&mut self) {
        if let Some(handle) = &self.pending.backend_resolve_handle
            && handle.is_finished()
                && let Some(handle) = self.pending.backend_resolve_handle.take() {
                    match handle.await {
                        Ok(Ok(path)) => {
                            self.add_log(
                                format!("Backend ready: {}", path.display()),
                                crate::config::LogLevel::Info,
                            );
                        }
                        Ok(Err(e)) => {
                            self.add_log(
                                format!("Backend installation failed: {}", e),
                                crate::config::LogLevel::Error,
                            );
                        }
                        Err(e) => {
                            self.add_log(
                                format!("Backend task panicked: {}", e),
                                crate::config::LogLevel::Error,
                            );
                        }
                    }
                    self.pending.backend_resolving = false;
                    self.ui.needs_redraw = true;
                }
    }

    pub fn poll_download_progress(&mut self) {
        let mut redraw = false;
        let mut download_logs = Vec::new();
        if let Some(rx) = &mut self.download.download_rx {
            while let Ok(state) = rx.try_recv() {
                if let Some(idx) = self
                    .download
                    .download_progress
                    .iter()
                    .position(|d| d.model_id == state.model_id && d.filename == state.filename)
                {
                    if state.total_bytes > 0 {
                        let old_pct = (self.download.download_progress[idx].downloaded_bytes as f32
                            / self.download.download_progress[idx].total_bytes as f32
                            * 100.0) as u32;
                        let new_pct = (state.downloaded_bytes as f32 / state.total_bytes as f32
                            * 100.0) as u32;
                        if new_pct / 5 > old_pct / 5 && new_pct < 100 {
                            let speed_mib = state.bytes_per_second / (1024.0 * 1024.0);
                            let total_mib = state.total_bytes as f64 / (1024.0 * 1024.0);
                            let name = if state.model_id == "llama-server" {
                                "backend"
                            } else {
                                &state.filename
                            };
                            download_logs.push(format!(
                                "Downloading {}: {}% of {:.1} MiB ({:.2} MiB/s)...",
                                name, new_pct, total_mib, speed_mib
                            ));
                        }
                    }
                    self.download.download_progress[idx] = state;
                } else {
                    if state.model_id == "llama-server" {
                        download_logs.push("Starting backend download...".to_string());
                    } else {
                        download_logs.push(format!("Starting download: {}...", state.filename));
                    }
                    self.download.download_progress.push(state);
                }
                redraw = true;
            }
        }
        for log in download_logs {
            self.add_log(log, crate::config::LogLevel::Info);
        }
        if redraw {
            self.ui.needs_redraw = true;
        }
    }

    pub fn poll_bench_tune_progress(&mut self) {
        if let Some(mut rx) = self.bench_tune.bench_tune_rx.take() {
            while let Ok(status) = rx.try_recv() {
                self.bench_tune.bench_tune_progress =
                    crate::models::BenchTuneProgress::from_status(&status);
            }
            self.bench_tune.bench_tune_rx = Some(rx);
            self.ui.needs_redraw = true;
        }
    }

    pub fn process_completed_downloads(&mut self) {
        let completed: Vec<crate::models::DownloadState> = self
            .download
            .download_progress
            .iter()
            .filter(|d| {
                matches!(
                    d.status,
                    crate::models::DownloadStatus::Complete
                        | crate::models::DownloadStatus::Error(_)
                        | crate::models::DownloadStatus::Cancelled
                )
            })
            .cloned()
            .collect();
        if !completed.is_empty() {
            for state in &completed {
                match &state.status {
                    crate::models::DownloadStatus::Complete => {
                        if state.model_id == "llama-server" {
                            self.add_log(
                                "Backend download complete",
                                crate::config::LogLevel::Info,
                            );
                        } else {
                            self.add_log(
                                format!("Download complete: {}", state.filename),
                                crate::config::LogLevel::Info,
                            );
                            self.models = Self::discover_models(&self.config.models_dirs);
                        }
                    }
                    crate::models::DownloadStatus::Error(e) => {
                        let name = if state.model_id == "llama-server" {
                            "Backend"
                        } else {
                            &state.filename
                        };
                        self.add_log(
                            format!("Download failed ({}): {}", name, e),
                            crate::config::LogLevel::Error,
                        );
                    }
                    crate::models::DownloadStatus::Cancelled => {
                        let name = if state.model_id == "llama-server" {
                            "Backend"
                        } else {
                            &state.filename
                        };
                        self.add_log(
                            format!("Download cancelled: {}", name),
                            crate::config::LogLevel::Info,
                        );
                        if let Some(ref dest) = state.dest
                            && dest.exists()
                                && let Err(e) = std::fs::remove_file(dest) {
                                    self.add_log(
                                        format!(
                                            "Failed to remove temp file {}: {}",
                                            dest.display(),
                                            e
                                        ),
                                        crate::config::LogLevel::Warning,
                                    );
                                }
                    }
                    _ => {}
                }
            }
            self.download.download_progress.retain(|d| {
                !matches!(
                    d.status,
                    crate::models::DownloadStatus::Complete
                        | crate::models::DownloadStatus::Error(_)
                        | crate::models::DownloadStatus::Cancelled
                )
            });
              self.download.downloading = !self.download.download_progress.is_empty();
            if !self.download.downloading {
                self.download.download_scroll_state.select(None);
            } else if let Some(idx) = self.download.download_scroll_state.selected()
                && idx >= self.download.download_progress.len()
            {
                self.download
                    .download_scroll_state
                    .select(Some(self.download.download_progress.len() - 1));
            }
            self.ui.needs_redraw = true;
        }
    }

    pub fn poll_server_logs(&mut self) {
        let mut server_logs = Vec::new();
        if let Some(rx) = &mut self.server.server_log_rx {
            while let Ok(line) = rx.try_recv() {
                if line.contains("n_tokens =")
                    && let Some(tokens_part) = line.split("n_tokens =").last()
                {
                    let val_str = tokens_part.split(',').next().unwrap_or(tokens_part).trim();
                    if let Ok(tokens) = val_str.parse::<u32>() {
                        self.metrics.ctx_used = tokens;
                    }
                }
                if line.contains("n_decoded =")
                    && let Some(decoded_part) = line.split("n_decoded =").last()
                {
                    let val_str = decoded_part.split(',').next().unwrap_or(decoded_part).trim();
                    if let Ok(tokens) = val_str.parse::<u64>() {
                        self.metrics.decoded_tokens = tokens;
                    }
                }
                if line.contains("tg =")
                    && let Some(tg_part) = line.split("tg =").last()
                {
                    let val_str = tg_part.trim().split(' ').next().unwrap_or(tg_part).trim();
                    if let Ok(tg) = val_str.parse::<f64>() {
                        self.metrics.gen_tps = tg;
                    }
                }
                server_logs.push(line);
                if server_logs.len() > 100 {
                    break;
                }
            }
        }
        if !server_logs.is_empty() {
            for line in server_logs {
                self.add_log(line, crate::config::LogLevel::Info);
            }
            self.ui.needs_redraw = true;
        }
    }

    pub fn poll_sync(&mut self) {
        let mut sync_updated = false;
        if let Some(rx) = &mut self.server.sync_rx {
            while let Ok(models) = rx.try_recv() {
                if let Some(handle) = &self.server.server_handle {
                    let port = handle.port;
                    let pid = handle.pid;
                    for (id, status, path) in models {
                        let status_lower = status.to_lowercase();
                        let is_active = status_lower == "loaded"
                            || status_lower == "loading"
                            || status_lower == "ready";
                        let mut matched = false;
                        for model in &self.models {
                            let path_match = path
                                .as_ref()
                                .map(|p| p == &model.path.to_string_lossy())
                                .unwrap_or(false);
                            let id_match = id == model.display_name || id == model.name;
                            let filename_match = path
                                .as_ref()
                                .and_then(|p| {
                                    std::path::Path::new(p)
                                        .file_name()
                                        .map(|f| f.to_string_lossy().to_string())
                                })
                                .map(|f| f == model.name)
                                .unwrap_or(false);
                            let id_filename_match = std::path::Path::new(&id)
                                .file_name()
                                .map(|f| f.to_string_lossy().to_string())
                                .map(|f| f == model.name || f == model.display_name)
                                .unwrap_or(false);
                            if path_match || id_match || filename_match || id_filename_match {
                                if is_active {
                                    if status_lower == "loading" {
                                        self.model_states.insert(
                                            model.display_name.clone(),
                                            crate::models::ModelState::Loading,
                                        );
                                    } else {
                                        let mut loaded_names =
                                            self.server.loaded_model_names.lock().unwrap();
                                        if !loaded_names.contains(&model.display_name) {
                                            loaded_names.push(model.display_name.clone());
                                        }
                                        self.model_states.insert(
                                            model.display_name.clone(),
                                            crate::models::ModelState::Loaded { port, pid },
                                        );
                                    }
                                }
                                matched = true;
                            }
                        }
                        if !matched {
                            let possible_names = vec![id.clone(), format!("{}.gguf", id)];
                            for name in possible_names {
                                for model in &self.models {
                                    if model.display_name == name || model.name == name {
                                        if is_active {
                                            let mut loaded_names =
                                                self.server.loaded_model_names.lock().unwrap();
                                            if !loaded_names.contains(&model.display_name) {
                                                loaded_names.push(model.display_name.clone());
                                            }
                                            self.model_states.insert(
                                                model.display_name.clone(),
                                                crate::models::ModelState::Loaded { port, pid },
                                            );
                                        }
                                        matched = true;
                                        break;
                                    }
                                }
                                if matched {
                                    break;
                                }
                            }
                        }
                    }
                    sync_updated = true;
                }
            }
        }
        if sync_updated {
            self.ui.needs_redraw = true;
        }
    }

    pub fn poll_metrics(&mut self) {
        if let Some(rx) = &mut self.server.metrics_rx {
            let mut received_metrics = false;
            while let Ok(mut m) = rx.try_recv() {
                // ctx_max uses the effective context length (context_length * rope_scale).
                if self.server.spawned_context_length > 0 {
                    m.ctx_max = self.server.spawned_context_length;
                }

                // Only carry over values that are technically "stateful" or slow to update
                // but don't force them to stick if the API is clearly reporting something else.
                if m.gpu_mem_used == 0 && self.metrics.gpu_mem_used > 0 {
                    m.gpu_mem_used = self.metrics.gpu_mem_used;
                    if m.gpu_mem_total == 0 {
                        m.gpu_mem_total = self.metrics.gpu_mem_total;
                    }
                }

                if m.tps > 0.0 {
                    m.latency_per_token_ms = 1000.0 / m.tps;
                }
                if m.prompt_tps > 0.0 {
                    m.prompt_latency_ms = 1000.0 / m.prompt_tps;
                }

                // If log parsing gave us a value but API didn't (or hasn't yet), use the log value.
                if m.ctx_used == 0 && self.metrics.ctx_used > 0 {
                    m.ctx_used = self.metrics.ctx_used;
                }
                if m.decoded_tokens == 0 && self.metrics.decoded_tokens > 0 {
                    m.decoded_tokens = self.metrics.decoded_tokens;
                }
                if m.gen_tps == 0.0 && self.metrics.gen_tps > 0.0 {
                    m.gen_tps = self.metrics.gen_tps;
                }
                if m.cpu_usage == 0.0 && self.metrics.cpu_usage > 0.0 {
                    m.cpu_usage = self.metrics.cpu_usage;
                }

                self.metrics = m;
                received_metrics = true;
            }
            if received_metrics {
                self.ui.needs_redraw = true;
            }
        }
    }

    pub async fn poll_loading_completion(&mut self) {
        use super::types::LoadingPhase;

        if self
            .loading
            .loading_phases
            .contains(&LoadingPhase::Complete)
        {
            return;
        }

        if !self
            .loading
            .loading_phases
            .contains(&LoadingPhase::ServerListening)
        {
            return;
        }

        if self.loading.health_poll_handle.is_some() {
            if let Some(rx) = &mut self.loading.loading_completion_rx {
                let mut got_completion = false;
                while let Ok(()) = rx.try_recv() {
                    got_completion = true;
                }
                if got_completion {
                    // Clear all previous loading phases (Starting, Meta, Tensors) once complete
                    self.loading.loading_phases.clear();
                    self.loading.loading_phases.insert(LoadingPhase::Complete);
                    self.loading.last_active_phase = Some(LoadingPhase::Complete);
                    self.loading.loading_progress = 1.0;
                    if let Some(h) = self.loading.health_poll_handle.take() {
                        h.abort();
                    }
                    self.loading.loading_completion_rx = None;
                    self.server.spawned_model_state = Some("loaded".to_string());
                    self.loading.progress_target = 1.0;
                    self.ui.needs_full_redraw = true;
                    self.ui.needs_redraw = true;

                    if let Some(handle) = &self.server.server_handle {
                        let port = handle.port;
                        let pid = handle.pid;

                        // Cleanup stale "Loading" entries (like "llama-server") before updating
                        let to_update: Vec<String> = self
                            .model_states
                            .iter()
                            .filter(|(_, s)| matches!(s, crate::models::ModelState::Loading))
                            .map(|(n, _)| n.clone())
                            .collect();

                        // Explicitly remove all Loading entries first to ensure no duplicates or stale names persist
                        self.model_states
                            .retain(|_, s| !matches!(s, crate::models::ModelState::Loading));

                        for name in to_update {
                            // If it's a real model name (not a generic server process name), mark as Loaded
                            if name != "llama-server" && name != "Router" {
                                self.model_states.insert(
                                    name.clone(),
                                    crate::models::ModelState::Loaded { port, pid },
                                );
                                let mut loaded = self
                                    .server
                                    .loaded_model_names
                                    .lock()
                                    .unwrap_or_else(|e| e.into_inner());
                                if !loaded.contains(&name) {
                                    loaded.push(name);
                                }
                            }
                        }
                    }

                    self.metrics.ctx_used = 0;
                }
            }
            return;
        }

        if let Some(handle) = &self.server.server_handle {
            let host = handle.host.clone();
            let port = handle.port;
            let (tx, rx) = tokio::sync::mpsc::channel(1);
            self.loading.loading_completion_rx = Some(rx);

            // Abort any previous health poll task to prevent leaked tasks
            if let Some(prev) = self.loading.health_poll_handle.take() {
                prev.abort();
            }

            let handle = tokio::spawn(async move {
                let client = reqwest::Client::new();
                let url = format!("http://{}:{}/health", host, port);

                loop {
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    match client.get(&url).send().await {
                        Ok(resp) if resp.status().is_success() => {
                            if let Ok(json) = resp.json::<serde_json::Value>().await {
                                let status_ok =
                                    json.get("status").and_then(|v| v.as_str()) == Some("ok");
                                let slots_ready = json
                                    .get("slots")
                                    .and_then(|v| v.as_array())
                                    .map(|a| !a.is_empty())
                                    .unwrap_or(false);

                                if slots_ready || status_ok {
                                    let _ = tx.send(()).await;
                                    return;
                                }
                            }
                        }
                        _ => {}
                    }
                }
            });
            self.loading.health_poll_handle = Some(handle);
        }
    }

    pub async fn start_pending_spawn(&mut self) {
        if let Some((model_opt, settings)) = self.pending.pending_spawn.take() {
            let (tx, rx) = tokio::sync::mpsc::channel(100);
            self.server.server_log_rx = Some(rx);
            let (exit_tx, exit_rx) = tokio::sync::mpsc::channel(1);
            self.server.server_exit_tx = Some(exit_tx.clone());
            self.server.server_exit_rx = Some(exit_rx);
            let config_clone = self.config.clone();
            let model_clone = model_opt.clone();
            let settings_clone = settings.clone();
            let tx_clone = tx.clone();
            let server_mode_clone = self.server_mode;
            let router_max_models_clone = self.router_max_models;
            let download_tx_clone = Some(self.ensure_download_channel());
            let display_name = model_opt
                .as_ref()
                .map(|m| m.display_name.clone())
                .unwrap_or_else(|| "Router".to_string());
            if let Some(m) = &model_opt {
                let state = if server_mode_clone == crate::models::ServerMode::Bench
                    || server_mode_clone == crate::models::ServerMode::BenchTune
                {
                    crate::models::ModelState::Benchmarking
                } else {
                    crate::models::ModelState::Loading
                };
                self.model_states.insert(m.display_name.clone(), state);
            }
            self.add_log(
                format!("Loading {}...", display_name),
                crate::config::LogLevel::Info,
            );
            if server_mode_clone == crate::models::ServerMode::BenchTune {
                let model = match model_opt {
                    Some(m) => m,
                    None => {
                        self.add_log(
                            "Error: Benchmark tuning requires a selected model.",
                            crate::config::LogLevel::Error,
                        );
                        return;
                    }
                };
                let bench_tune_config =
                    self.bench_tune.bench_tune_config.take().unwrap_or_else(|| {
                        crate::models::BenchTuneConfig::new(
                            model.path.clone(),
                            3,
                            crate::models::BENCHMARK_PROMPT.to_string(),
                        )
                    });
                let (tx_tune, rx_tune) = tokio::sync::mpsc::channel(100);
                self.bench_tune.bench_tune_tx = Some(tx_tune.clone());
                self.bench_tune.bench_tune_config = Some(bench_tune_config.clone());
                self.bench_tune.bench_tune_running = true;
                self.bench_tune.bench_tune_results.clear();
                self.bench_tune.bench_tune_result_row = 0;
                self.models_mode = super::types::ModelsMode::BenchTune;

                // Create cancellation channel for benchmark tuning
                let (cancel_tx, mut cancel_rx) = tokio::sync::watch::channel(false);
                self.bench_tune.bench_tune_cancel_tx = Some(cancel_tx);

                let bench_tune_config_clone = bench_tune_config.clone();
                let settings_clone = settings_clone.clone();
                let model_clone = model.clone();
                let tx_tune_clone = tx_tune.clone();
                let spawn_log_tx_clone = tx.clone();
                let handle = tokio::spawn(async move {
                    let results = crate::backend::benchmark::run_bench_tune(
                        crate::backend::benchmark::BenchTuneRequest {
                            main_config: &config_clone,
                            config: &bench_tune_config_clone,
                            model: &model_clone,
                            settings: &settings_clone,
                            progress_tx: tx_tune_clone,
                            log_tx: spawn_log_tx_clone,
                            cancel_rx: &mut cancel_rx,
                        },
                    )
                    .await
                    .map_err(|e| e.to_string());
                    (results, display_name, bench_tune_config_clone)
                });
                self.server.bench_tune_task_handle = Some(handle);
                self.server.spawn_log_tx = Some(tx);
                self.bench_tune.bench_tune_rx = Some(rx_tune);
            } else {
                let settings_for_result = settings_clone.clone();
                let exit_tx_clone = exit_tx.clone();
                let handle = tokio::spawn(async move {
                    crate::backend::server::spawn_server(crate::backend::server::SpawnServerRequest {
                        config: &config_clone,
                        model: model_clone.as_ref(),
                        settings: &settings_clone,
                        log_tx: tx_clone,
                        progress_tx: download_tx_clone,
                        server_mode: server_mode_clone,
                        router_max_models: router_max_models_clone,
                        exit_tx: exit_tx_clone,
                    })
                    .await
                    .map(|(handle, cmd)| (display_name, handle, cmd, settings_for_result))
                });
                self.server.spawn_task_handle = Some(handle);
                self.server.spawn_log_tx = Some(tx);
                self.ui.needs_redraw = true;
            }
        }
    }

    pub async fn poll_spawn_result(&mut self) {
        if let Some(handle) = &self.server.spawn_task_handle
            && handle.is_finished()
            && let Some(handle) = self.server.spawn_task_handle.take()
        {
            match handle.await {
                Ok(Ok((server_display_name, server_handle, cmd, spawned_settings))) => {
                    let port = server_handle.port;
                    let pid = server_handle.pid;
                    let host = server_handle.host.clone();
                    self.add_log(
                        format!("Server started on port {port} (pid={pid})"),
                        crate::config::LogLevel::Info,
                    );
                    self.server.server_handle = Some(server_handle);
                    self.server.cmd_display = Some(cmd);
                    self.server.spawned_settings = Some(spawned_settings.clone());
                    self.server.spawned_model_name = Some(server_display_name.clone());
                    self.server.spawned_model_state = Some("loading".to_string());
                    self.server.spawned_context_length = (spawned_settings.context_length as f32
                        * spawned_settings.rope_scale)
                        as u32;
                    // API endpoint (proxy) is managed by update_api_endpoint(), which
                    // runs every loop iteration and (re)starts the proxy as needed
                    // when a new model becomes available.
                    self.loading.loading_phases =
                        std::iter::once(super::types::LoadingPhase::ServerListening).collect();
                    self.loading.last_active_phase =
                        Some(super::types::LoadingPhase::ServerListening);
                    self.server.spawned_model_state = Some("loading".to_string());
                    self.loading.progress_target = 1.0;
                    let (metrics_tx, metrics_rx) = tokio::sync::mpsc::channel(10);
                    self.server.metrics_rx = Some(metrics_rx);
                    let task_host = host.clone();
                    let task_port = port;
                    let task_pid = pid;
                    let metrics_model_name = self.server.metrics_model_name.clone();
                    self.add_log("Starting metrics polling...", crate::config::LogLevel::Info);
                    let _task_handle = tokio::spawn(Self::metrics_polling_task(
                        task_host,
                        task_port,
                        task_pid,
                        metrics_model_name,
                        metrics_tx,
                    ));
                    self.server.metrics_task_handle = Some(_task_handle);
                    let sync_host = host.clone();
                    let sync_port = port;
                    let (sync_tx, sync_rx) = tokio::sync::mpsc::channel(1);
                    let _sync_task_handle =
                        tokio::spawn(Self::sync_polling_task(sync_host, sync_port, sync_tx));
                    self.server.sync_rx = Some(sync_rx);
                    self.server.sync_task_handle = Some(_sync_task_handle);
                    self.ui.needs_redraw = true;
                }
                Ok(Err(e)) => {
                    self.loading.progress_target = 1.0;
                    self.add_log(
                        format!("ERROR: Server failed: {}", e),
                        crate::config::LogLevel::Error,
                    );
                    if let Some(mut rx) = self.server.server_log_rx.take() {
                        while let Ok(line) = rx.try_recv() {
                            self.add_log(line, crate::config::LogLevel::Info);
                        }
                    }
                    self.ui.last_error_message = Some(e);
                    self.reset_loading_state(true);
                    self.ui.needs_redraw = true;
                }
                Err(e) => {
                    self.loading.progress_target = 1.0;
                    self.add_log(
                        format!("ERROR: Spawn task panicked: {}", e),
                        crate::config::LogLevel::Error,
                    );
                    self.ui.needs_redraw = true;
                }
            }
        }
    }

    async fn metrics_polling_task(
        host: String,
        port: u16,
        pid: u32,
        metrics_model_name: Arc<std::sync::Mutex<Option<String>>>,
        metrics_tx: tokio::sync::mpsc::Sender<crate::models::ServerMetrics>,
    ) {
        let mut consecutive_failures: u32 = 0;
        let max_failures: u32 = 15;
        let mut prev_model_name: Option<String> = None;
        loop {
            let mut m = match crate::backend::server::get_metrics(&host, port, None, Some(pid))
                .await
            {
                Ok(metrics) => {
                    consecutive_failures = 0;
                    metrics
                }
                Err(_) => {
                    consecutive_failures += 1;
                    if consecutive_failures >= max_failures {
                        tracing::warn!(
                            "Metrics polling aborted after {} consecutive failures (server likely dead)",
                            max_failures
                        );
                        break;
                    }
                    if consecutive_failures % 5 == 1 {
                        tracing::warn!(
                            "Metrics polling: server unreachable (attempt {}/{})",
                            consecutive_failures,
                            max_failures
                        );
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    continue;
                }
            };
            m.total_vram_used = m.gpu_mem_used;
            let current_model = {
                let lock = metrics_model_name.lock().unwrap();
                lock.clone()
            };
            if let Some(name) = current_model
                && let Ok(model_metrics) =
                    crate::backend::server::get_metrics(&host, port, Some(&name), Some(pid)).await
            {
                let stotal = m.gpu_mem_total;
                let should_use_model_vram = if stotal > 0 {
                    model_metrics.gpu_mem_used >= stotal / 4
                } else {
                    true
                };
                // Reset ctx_used when model changes to avoid showing stale cumulative values.
                if prev_model_name.as_deref() != Some(&name) {
                    prev_model_name = Some(name.clone());
                    m.ctx_used = 0;
                }
                m.ctx_used = model_metrics.ctx_used;
                if model_metrics.ctx_max > 0 {
                    m.ctx_max = model_metrics.ctx_max;
                }
                if model_metrics.tps > 0.0 {
                    m.tps = model_metrics.tps;
                }
                if should_use_model_vram {
                    m.gpu_mem_used = model_metrics.gpu_mem_used;
                }
            }
            if metrics_tx.send(m).await.is_err() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    }

    async fn sync_polling_task(
        host: String,
        port: u16,
        sync_tx: tokio::sync::mpsc::Sender<Vec<(String, String, Option<String>)>>,
    ) {
        loop {
            if let Ok(models) = crate::backend::server::list_models(&host, port).await
                && sync_tx.send(models).await.is_err()
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        }
    }

    pub async fn poll_bench_tune_result(&mut self) {
        if let Some(handle) = &self.server.bench_tune_task_handle
            && handle.is_finished()
            && let Some(handle) = self.server.bench_tune_task_handle.take()
        {
            match handle.await {
                Ok((results, display_name, bench_config)) => match results {
                    Ok(bench_results) => {
                        self.add_log(
                            format!(
                                "Benchmark tuning completed for {} with {} results",
                                display_name,
                                bench_results.len()
                            ),
                            crate::config::LogLevel::Info,
                        );
                        if bench_results.is_empty() {
                            self.add_log("No successful benchmark results were obtained. Check the Log (F6) for details on test failures.", crate::config::LogLevel::Warning);
                        } else {
                            let output_dir = crate::config::Config::config_path()
                                .parent()
                                .unwrap()
                                .join("benchmarks");
                            match crate::backend::benchmark::save_results(
                                &bench_results,
                                &output_dir,
                                &bench_config,
                            )
                            .await
                            {
                                Ok(()) => self.add_log(
                                    format!("Results saved to {}/", output_dir.display()),
                                    crate::config::LogLevel::Info,
                                ),
                                Err(e) => self.add_log(
                                    format!("Failed to save benchmark results: {}", e),
                                    crate::config::LogLevel::Error,
                                ),
                            }
                        }
                        let mut sorted_results = bench_results;
                        sorted_results.sort_by(|a, b| {
                            b.metrics
                                .generation_tps
                                .partial_cmp(&a.metrics.generation_tps)
                                .unwrap_or(std::cmp::Ordering::Equal)
                        });
                        self.bench_tune.bench_tune_results = sorted_results;
                        self.bench_tune.bench_tune_running = false;

                        let model_display_name = self
                            .selected_model()
                            .map(|m| m.display_name.clone());

                        if let Some(model_display_name) = model_display_name {
                            self.model_states
                                .insert(model_display_name.clone(), crate::models::ModelState::Available);
                        }

                        if let Some(handle) = &self.server.server_handle {
                            if let Some(model) = self.selected_model() {
                                let host = handle.host.clone();
                                let port = handle.port;
                                let model_name = model.display_name.clone();
                                let model_path_str = model.path.to_str().map(|s| s.to_string());
                                let task_name = format!("bench_unload_{}", model.display_name);
                                let task_handle = tokio::spawn(async move {
                                    let _ = crate::backend::server::unload_model(
                                        &host,
                                        port,
                                        &model_name,
                                        model_path_str.as_deref(),
                                    )
                                    .await;
                                });
                                self.background_tasks.insert(task_name, task_handle);
                            }
                        }

                        self.ui.needs_redraw = true;
                    }
                    Err(e) => {
                        self.add_log(
                            format!("Benchmark tuning failed: {}", e),
                            crate::config::LogLevel::Error,
                        );
                        self.bench_tune.bench_tune_running = false;
                        if let Some(model) = self.selected_model() {
                            self.model_states.insert(
                                model.display_name.clone(),
                                crate::models::ModelState::Failed {
                                    error: e.to_string(),
                                },
                            );
                        }
                        self.ui.needs_redraw = true;
                    }
                },
                Err(e) => {
                    self.add_log(
                        format!("Benchmark task panicked: {:?}", e),
                        crate::config::LogLevel::Error,
                    );
                    self.bench_tune.bench_tune_running = false;
                    self.ui.needs_redraw = true;
                }
            }
        }
    }

    pub fn handle_pending_api_load(&mut self) {
        if let Some((model_name, model_path)) = self.pending.pending_api_load.clone() {
            if let Some(handle) = &self.server.server_handle {
                if self
                    .loading
                    .loading_phases
                    .contains(&super::types::LoadingPhase::Complete)
                    || self
                        .loading
                        .loading_phases
                        .contains(&super::types::LoadingPhase::ServerListening)
                {
                    let host = handle.host.clone();
                    let port = handle.port;
                    let model_name_clone = model_name.clone();
                    let model_path_clone = model_path.clone();
                    self.pending.pending_api_load = None;
                    self.add_log(
                        format!("Sending load request for {}...", model_name_clone),
                        crate::config::LogLevel::Info,
                    );
                    {
                        let mut lock = self.server.metrics_model_name.lock().unwrap();
                        *lock = Some(model_name_clone.clone());
                    }
                    let log_tx = self.server.spawn_log_tx.clone();
                    let model_name_err = model_name_clone.clone();
                    self.metrics.ctx_used = 0;
                    tokio::spawn(async move {
                        if let Err(e) = crate::backend::server::load_model(
                            &host,
                            port,
                            &model_name_clone,
                            model_path_clone.as_deref(),
                        )
                        .await
                        {
                            let err_msg =
                                format!("ERROR: Failed to load model {}: {}", model_name_err, e);
                            if let Some(tx) = log_tx {
                                let _ = tx.send(err_msg.clone()).await;
                            } else {
                                tracing::error!("{}", err_msg);
                            }
                        }
                    });
                    self.model_states
                        .insert(model_name, crate::models::ModelState::Loading);
                    self.ui.needs_redraw = true;
                }
            } else if self.server.spawn_task_handle.is_none()
                && self.pending.pending_spawn.is_none()
            {
                self.pending.pending_api_load = None;
            }
        }
    }

    pub fn handle_pending_api_unload(&mut self) {
        if !matches!(
            self.ui.global_mode,
            super::types::GlobalMode::Confirmation { .. }
        )
            && let Some((model_name, model_path)) = self.pending.pending_api_unload.take()
                && let Some(handle) = &self.server.server_handle
            {
                let server_mode = self.server_mode;
                let handle_clone = handle.clone();
                {
                    let mut lock = self.server.metrics_model_name.lock().unwrap();
                    if lock.as_deref() == Some(&model_name) {
                        *lock = None;
                    }
                }
                let host = handle.host.clone();
                let port = handle.port;
                let model_name_clone = model_name.clone();
                let model_path_clone = model_path.clone();
                if server_mode == crate::models::ServerMode::Normal {
                    self.add_log(
                        format!("Unloading {} (killing server)...", model_name_clone),
                        crate::config::LogLevel::Info,
                    );
                    self.pending.pending_kill = Some(handle_clone);
                } else {
                    self.add_log(
                        format!("Sending unload request for {}...", model_name_clone),
                        crate::config::LogLevel::Info,
                    );
                    let kill_tx = self.server.spawn_log_tx.clone();
                    let kill_tx2 = kill_tx.clone();
                    let server_clone = self.server.server_handle.clone();
                    let host_clone = host.clone();
                    let port_clone = port;
                    let model_name_task = model_name_clone.clone();
                    let loaded_names_clone = self.server.loaded_model_names.clone();
                    self.background_tasks.insert(
                        format!("api_unload_{}", model_name_task),
                        tokio::spawn(async move {
                            if let Err(e) = crate::backend::server::unload_model(
                                &host,
                                port,
                                &model_name_clone,
                                model_path_clone.as_deref(),
                            )
                            .await
                            {
                                if let Some(tx) = kill_tx {
                                    let _ = tx
                                        .send(format!("Failed to unload model via API: {}", e))
                                        .await;
                                }
                                return;
                            }
                            tokio::time::sleep(std::time::Duration::from_millis(500)).await;

                            let mut should_stop = false;

                            if let Ok(loaded) =
                                crate::backend::server::list_models(&host_clone, port_clone).await
                            {
                                if loaded.is_empty() {
                                    should_stop = true;
                                } else {
                                    if let Some(tx) = kill_tx.clone() {
                                        let _ = tx
                                            .send(format!(
                                                "{} models still loaded on server",
                                                loaded.len()
                                            ))
                                            .await;
                                    }
                                }
                            }

                            if !should_stop {
                                let loaded_names =
                                    loaded_names_clone.lock().unwrap_or_else(|e| e.into_inner());
                                if loaded_names.is_empty() {
                                    should_stop = true;
                                }
                            }

                            if should_stop {
                                if let Some(tx) = kill_tx {
                                    let _ = tx
                                        .send("No models left, stopping router...".to_string())
                                        .await;
                                }
                                if let Some(server) = server_clone {
                                    let _ = crate::backend::server::kill_server(server).await;
                                    if let Some(tx) = kill_tx2 {
                                        let _ = tx.send("Server stopped".to_string()).await;
                                    }
                                }
                            }
                        }),
                    );
                }
                self.server
                    .loaded_model_names
                    .lock()
                    .unwrap()
                    .retain(|n| n != &model_name);
                self.metrics.ctx_used = 0;
                self.model_states
                    .insert(model_name, crate::models::ModelState::Available);
                self.ui.needs_redraw = true;
            }
    }

    pub async fn start_pending_kill(&mut self) {
        if let Some(handle) = self.pending.pending_kill.take() {
            match crate::backend::server::kill_server(handle).await {
                Ok(()) => {
                    self.add_log("Server stopped", crate::config::LogLevel::Info);
                    self.server.server_handle = None;
                    self.server.metrics_rx = None;
                    self.metrics = Default::default();
                    if let Some(task) = self.server.metrics_task_handle.take() {
                        task.abort();
                    }
                    if let Some(task) = self.server.sync_task_handle.take() {
                        task.abort();
                    }
                    self.server.sync_rx = None;
                    if let Some(tx) = self.server.api_shutdown_tx.take() {
                        let _ = tx.send(true);
                    }
                    if let Some(proxy) = self.server.api_proxy_handle.take() {
                        proxy.abort();
                    }
                    let mut names_to_reset = Vec::new();
                    for (name, state) in &self.model_states {
                        if !matches!(state, crate::models::ModelState::Available)
                            && !matches!(state, crate::models::ModelState::Failed { .. })
                        {
                            names_to_reset.push(name.clone());
                        }
                    }
                    for name in names_to_reset {
                        let n: String = name.clone();
                        self.model_states
                            .insert(n, crate::models::ModelState::Available);
                    }
                    self.server.loaded_model_names.lock().unwrap().clear();
                    self.loading.loading_phases = std::collections::HashSet::new();
                    self.loading.loading_progress = 0.0;
                    self.loading.progress_target = 0.0;
                    self.ui.needs_full_redraw = true;
                    self.ui.needs_redraw = true;
                }
                Err(e) => {
                    self.add_log(
                        format!("Failed to stop server: {}", e),
                        crate::config::LogLevel::Error,
                    );
                }
            }
        }
    }

    pub async fn handle_pending_search(&mut self) {
        if self.search.search_loading {
            if let Some((query, offset)) = self.search.pending_search_load.take() {
                let is_append = offset > 0;
                let query_clone = query.clone();
                let offset_clone = offset;
                let search_limit = self.config.search_limit;
                self.add_log(
                    format!(
                        "Searching with limit={} offset={}...",
                        search_limit, offset_clone
                    ),
                    crate::config::LogLevel::Info,
                );
                let search_handle = tokio::spawn(async move {
                    crate::backend::hub::search_models(&query_clone, search_limit, offset_clone)
                        .await
                });
                match search_handle.await {
                    Ok(Ok((res, _, raw_ids))) => {
                        let query_str = &query;
                        let mut buf =
                            format!("Search complete: {} results for '{}'", res.len(), query_str);
                        buf.push_str(&format!("\n  RAW API returned: {}", raw_ids.join(", ")));
                        for r in &res {
                            let gguf_tags: Vec<String> = r
                                .tags
                                .iter()
                                .filter(|t| t.starts_with("gguf:"))
                                .cloned()
                                .collect();
                            buf.push_str(&format!(
                                "\n  {} quant={} tags={} params={} cap={} ctx={}",
                                r.model_id,
                                r.quantization.as_deref().unwrap_or("-"),
                                gguf_tags.join(","),
                                r.parameters.as_deref().unwrap_or("none"),
                                r.capabilities.join(","),
                                r.context_length.unwrap_or(0)
                            ));
                        }
                        let raw_len = raw_ids.len();
                        if is_append {
                            if let super::types::ModelsMode::Search {
                                results,
                                has_more,
                                loading,
                                ..
                            } = &mut self.models_mode
                            {
                                let models = self.models.clone();
                                for r in res {
                                    let downloaded =
                                        super::sync_ops::model_is_downloaded(&models, &r.model_id);
                                    results.push(crate::models::SearchResult { downloaded, ..r });
                                }
                                if raw_len < self.config.search_limit as usize {
                                    *has_more = false;
                                }
                                *loading = false;
                            }
                        } else {
                            if let super::types::ModelsMode::Search {
                                results,
                                loading,
                                has_more,
                                ..
                            } = &mut self.models_mode
                            {
                                let models = self.models.clone();
                                *results = res
                                    .into_iter()
                                    .map(|r| {
                                        let downloaded = super::sync_ops::model_is_downloaded(
                                            &models,
                                            &r.model_id,
                                        );
                                        crate::models::SearchResult { downloaded, ..r }
                                    })
                                    .collect();
                                if !results.is_empty() {
                                    self.search.search_results_idx = Some(0);
                                } else {
                                    self.search.search_results_idx = None;
                                }
                                *has_more = raw_len >= self.config.search_limit as usize;
                                *loading = false;
                            }
                        }
                        self.add_log(buf, crate::config::LogLevel::Info);
                    }
                    Ok(Err(e)) => {
                        self.add_log(
                            format!("Search failed: {}", e),
                            crate::config::LogLevel::Error,
                        );
                        if let super::types::ModelsMode::Search { loading, .. } =
                            &mut self.models_mode
                        {
                            *loading = false;
                        }
                    }
                    Err(e) => {
                        self.add_log(
                            format!("Search task error: {}", e),
                            crate::config::LogLevel::Error,
                        );
                        if let super::types::ModelsMode::Search { loading, .. } =
                            &mut self.models_mode
                        {
                            *loading = false;
                        }
                    }
                }
            }
            self.search.search_loading = false;
        }
    }

    pub fn update_metrics_model_name(&mut self) {
        let active_loaded_model = if let Some(model) = self.selected_model() {
            if self.is_model_loaded(&model.display_name) {
                Some(model.display_name.clone())
            } else {
                // Fallback to the first actually loaded model
                let lock = self
                    .server
                    .loaded_model_names
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                lock.first().cloned()
            }
        } else {
            // No selection, fallback to the first actually loaded model
            let lock = self
                .server
                .loaded_model_names
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            lock.first().cloned()
        };
        let mut lock = self
            .server
            .metrics_model_name
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        *lock = active_loaded_model;
    }

    pub fn ensure_download_channel(
        &mut self,
    ) -> tokio::sync::broadcast::Sender<crate::models::DownloadState> {
        if self.download.download_rx.is_none() {
            let (tx, rx) = tokio::sync::broadcast::channel(10);
            self.download.download_tx = Some(tx);
            self.download.download_rx = Some(rx);
        }
        self.download.download_tx.as_ref().unwrap().clone()
    }

    pub async fn update_ws_server(&mut self) {
        let enabled = self.settings.ws_server_enabled;
        let port = self.settings.ws_server_port;
        let auth_key = self.settings.ws_server_auth_key.clone();
        let tls_enabled = self.settings.ws_server_tls_enabled;
        let tls_cert = self.settings.ws_server_tls_cert.clone();
        let tls_key = self.settings.ws_server_tls_key.clone();

        // Load TLS config only if paths changed since last load, or if not yet cached.
        let tls_cfg = if tls_enabled {
            let needs_reload = match (&tls_cert, &tls_key) {
                (Some(cert), Some(key)) => {
                    Some(cert.as_str()) != self.server.running_ws_tls_cert_path.as_deref()
                        || Some(key.as_str()) != self.server.running_ws_tls_key_path.as_deref()
                }
                _ => true,
            };
            if needs_reload {
                if let (Some(cert), Some(key)) = (&tls_cert, &tls_key) {
                    crate::backend::tls::load_tls_config(cert, key).await.ok()
                } else {
                    None
                }
            } else {
                self.server.running_ws_tls_cfg.clone()
            }
        } else {
            self.server.running_ws_tls_cfg = None;
            self.server.running_ws_tls_cert_path = None;
            self.server.running_ws_tls_key_path = None;
            None
        };

        // Cache the TLS config and the paths used to load it.
        if let (Some(cert), Some(key)) = (&tls_cert, &tls_key) {
            self.server.running_ws_tls_cert_path = Some(cert.clone());
            self.server.running_ws_tls_key_path = Some(key.clone());
        }
        self.server.running_ws_tls_cfg = tls_cfg.clone();

        // Check if settings have changed since last start
        let settings_changed = self.server.running_ws_port != Some(port)
            || self.server.running_ws_auth != auth_key
            || self.server.running_ws_tls != Some(tls_enabled);

        if self.ws_server_handle.is_some() && (!enabled || settings_changed) {
            let handle = self.ws_server_handle.take().unwrap();
            crate::backend::ws_server::stop_ws_server(handle);
            self.server.running_ws_port = None;
            self.server.running_ws_auth = None;
            self.server.running_ws_tls = None;
            if !enabled {
                self.add_log("Dashboard disabled", crate::config::LogLevel::Info);
            }
        }

        if enabled && self.ws_server_handle.is_none() {
            let (tx, rx) = tokio::sync::broadcast::channel(64);
            let ws_rx = std::sync::Arc::new(rx);
            let _host = self.settings.host.clone();
            match crate::backend::ws_server::start_ws_server(
                port,
                ws_rx,
                auth_key.clone(),
                tls_cfg,
                _host,
            )
            .await
            {
                Ok(handle) => {
                    self.server.metrics_tx = Some(tx);
                    self.ws_server_handle = Some(handle);
                    self.server.running_ws_port = Some(port);
                    self.server.running_ws_auth = auth_key.clone();
                    self.server.running_ws_tls = Some(tls_enabled);
                    let protocol = if tls_enabled { "https" } else { "http" };
                    let auth_param = match &auth_key {
                        Some(a) => format!("?auth={}", urlencoding::encode(a)),
                        None => String::new(),
                    };
                    self.add_log(
                        format!(
                            "Dashboard enabled: {protocol}://{}:{}/dashboard{}",
                            self.settings.host, port, auth_param
                        ),
                        crate::config::LogLevel::Info,
                    );
                }
                Err(e) => {
                    // Bind failed (port in use, invalid address, etc.). Surface the
                    // error to the user and flip the toggle back so the loop does
                    // not busy-retry every iteration.
                    self.add_log(
                        format!("Dashboard failed to start on port {}: {}", port, e),
                        crate::config::LogLevel::Error,
                    );
                    self.settings.ws_server_enabled = false;
                    self.config.default.ws_server_enabled = false;
                    self.config.ws_server.enabled = false;
                    if let Err(e) = self.config.save() {
                        self.add_log(
                            format!("Failed to persist dashboard-disabled state: {}", e),
                            crate::config::LogLevel::Error,
                        );
                    }
                }
            }
        }
    }

    /// Start/stop the API endpoint proxy based on settings.
    ///
    /// The proxy can run before any model is loaded (it will accept connections
    /// for `/api/status` and serve a proxy that returns errors until a model is
    /// loaded). When a model is loaded later, or the loaded model changes, the
    /// proxy is restarted so it points at the right llama-server port.
    pub async fn update_api_endpoint(&mut self) {
        let enabled = self.settings.api_endpoint_enabled;
        let port = self.settings.api_endpoint_port;
        let host = self.settings.host.clone();
        let server_port = self
            .server
            .server_handle
            .as_ref()
            .map(|h| h.port)
            .unwrap_or(0);
        let pid = self
            .server
            .server_handle
            .as_ref()
            .map(|h| h.pid)
            .unwrap_or(0);
        let model_name = self.server.spawned_model_name.clone().unwrap_or_default();

        // No backend server and API proxy is not running — nothing to do.
        // This prevents a busy loop where settings_changed is always true
        // because running_api_server_port holds the old port while server_port is 0.
        if self.server.server_handle.is_none() && self.server.api_proxy_handle.is_none() {
            return;
        }

        let settings_changed = self.server.running_api_port != Some(port)
            || self.server.running_api_server_port != Some(server_port)
            || self.server.running_api_model.as_deref() != Some(model_name.as_str());

        // Stop if disabled or settings/model changed.
        if self.server.api_proxy_handle.is_some() && (!enabled || settings_changed) {
            if let Some(tx) = self.server.api_shutdown_tx.take() {
                let _ = tx.send(true);
            }
            if let Some(handle) = self.server.api_proxy_handle.take() {
                handle.abort();
            }
            self.server.running_api_port = None;
            self.server.running_api_server_port = None;
            self.server.running_api_model = None;
            if !enabled {
                self.add_log("API endpoint disabled", crate::config::LogLevel::Info);
            }
        }

        // Start if enabled and not running.
        if enabled && self.server.api_proxy_handle.is_none() {
            let addr: std::net::SocketAddr = match format!("{}:{}", host, port).parse() {
                Ok(a) => a,
                Err(e) => {
                    self.add_log(
                        format!(
                            "API endpoint failed to start: invalid address {}:{}: {}",
                            host, port, e
                        ),
                        crate::config::LogLevel::Error,
                    );
                    self.settings.api_endpoint_enabled = false;
                    self.config.default.api_endpoint_enabled = false;
                    let _ = self.config.save();
                    return;
                }
            };

            // Pre-bind to detect port-in-use before spawning.
            match tokio::net::TcpListener::bind(addr).await {
                Ok(listener) => drop(listener),
                Err(e) => {
                    self.add_log(
                        format!("API endpoint failed to start on {}:{}: {}", host, port, e),
                        crate::config::LogLevel::Error,
                    );
                    self.settings.api_endpoint_enabled = false;
                    self.config.default.api_endpoint_enabled = false;
                    let _ = self.config.save();
                    return;
                }
            }

            let (api_shutdown_tx, api_shutdown_rx) = tokio::sync::watch::channel(false);
            self.server.api_shutdown_tx = Some(api_shutdown_tx);
            let host_clone = host.clone();
            let model_name_clone = model_name.clone();
            let handle = tokio::spawn(async move {
                let _ = crate::serve_api::start_api_server(
                    addr,
                    None,
                    server_port,
                    model_name_clone,
                    pid,
                    api_shutdown_rx,
                    host_clone,
                    None,
                )
                .await;
            });
            self.server.api_proxy_handle = Some(handle);
            self.server.running_api_port = Some(port);
            self.server.running_api_server_port = Some(server_port);
            self.server.running_api_model = Some(model_name);
            let status = if server_port == 0 {
                " (no model loaded yet)"
            } else {
                ""
            };
            self.add_log(
                format!("API endpoint started on {}:{}{}", host, port, status),
                crate::config::LogLevel::Info,
            );
        }
    }
}
