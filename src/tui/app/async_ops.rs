use super::types::App;
use std::sync::Arc;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU8};

impl App {
    pub async fn start_pending_download(&mut self) {
        if let Some((model_id, filename, download_url, file_size)) = self.pending_download.take() {
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
                self.set_redraw();
                return;
            }
            let model_id_clone = model_id.clone();
            let filename_clone = filename.clone();
            let url_clone = download_url.clone();
            let cancelled = Arc::new(AtomicBool::new(false));
            let cancelled_clone = cancelled.clone();
            self.add_log(format!("Downloading {}...", model_id), crate::config::LogLevel::Info);
            let tx = self.ensure_download_channel();
            let tx_clone = tx.clone();
            let cancelled_for_state = cancelled_clone.clone();
            let download_state = Arc::new(AtomicU8::new(1));
            let download_state_clone = download_state.clone();
            let dest_path = dest.clone();
            self.download_progress.last_mut().and_then(|d| {
                d.dest = Some(dest_path.clone());
                None::<()>
            });

            tokio::spawn(async move {
                let mut state = crate::models::DownloadState::new(model_id_clone.clone(), filename_clone.clone(), 0);
                state.cancel_token = Some(cancelled_for_state);
                state.download_state = 1;
                state.dest = Some(dest_path);
                state.download_state_arc = Some(download_state_clone.clone());
                let result = crate::backend::hub::download_file(&model_id_clone, &filename_clone, &url_clone, &dest, &mut state, download_state_clone, tx_clone).await;
                if let Err(e) = result {
                    state.status = crate::models::DownloadStatus::Error(e.to_string());
                    let _ = tx.send(state);
                }
            });
            self.downloading = true;
            self.cancelled = Some(cancelled);
            self.download_scroll_state.select(Some(0));
            self.set_redraw();
        }
    }

    pub async fn start_pending_deletion(&mut self, path: PathBuf) {
        let path_clone = path.clone();
        tokio::spawn(async move {
            if let Err(e) = tokio::fs::remove_file(&path_clone).await {
                tracing::warn!("Failed to delete file: {}", e);
            }
        });
        let model_key = path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        self.config.model_overrides.remove(&model_key);
        if let Err(e) = self.config.save() {
            self.add_log(format!("Failed to save config after deletion: {}", e), crate::config::LogLevel::Error);
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
        self.add_log(format!("Model deleted: {:?}", path.file_name().unwrap_or_default()), crate::config::LogLevel::Info);
        self.set_redraw();
    }

    pub fn start_pending_backend_deletion(&mut self, backend: crate::models::Backend, tag: String) {
        let bin_dir = crate::backend::hub::get_backend_dir(backend, &tag);
        if bin_dir.exists() {
            if let Err(e) = std::fs::remove_dir_all(&bin_dir) {
                self.add_log(format!("Failed to delete backend: {}", e), crate::config::LogLevel::Error);
            } else {
                self.add_log(format!("Deleted backend {} ({})", backend, tag), crate::config::LogLevel::Info);
                let new_entries = self.fetch_backend_picker_entries();
                if let super::types::GlobalMode::BackendPicker { entries, selected } = &mut self.global_mode {
                    *entries = new_entries;
                    if *selected >= entries.len() {
                        *selected = entries.len().saturating_sub(1);
                    }
                }
            }
        }
        self.set_redraw();
    }

    pub async fn poll_backend_resolution(&mut self) {
        if let Some(handle) = &self.backend_resolve_handle {
            if handle.is_finished() {
                if let Some(handle) = self.backend_resolve_handle.take() {
                    match handle.await {
                        Ok(Ok(path)) => {
                            self.add_log(format!("Backend ready: {}", path.display()), crate::config::LogLevel::Info);
                        }
                        Ok(Err(e)) => {
                            self.add_log(format!("Backend installation failed: {}", e), crate::config::LogLevel::Error);
                        }
                        Err(e) => {
                            self.add_log(format!("Backend task panicked: {}", e), crate::config::LogLevel::Error);
                        }
                    }
                    self.backend_resolving = false;
                    self.set_redraw();
                }
            }
        }
    }

    pub fn poll_download_progress(&mut self) {
        let mut redraw = false;
        let mut download_logs = Vec::new();
        if let Some(rx) = &mut self.download_rx {
            while let Ok(state) = rx.try_recv() {
                if let Some(idx) = self.download_progress.iter().position(|d| {
                    d.model_id == state.model_id && d.filename == state.filename
                }) {
                    if state.total_bytes > 0 {
                        let old_pct = (self.download_progress[idx].downloaded_bytes as f32 / self.download_progress[idx].total_bytes as f32 * 100.0) as u32;
                        let new_pct = (state.downloaded_bytes as f32 / state.total_bytes as f32 * 100.0) as u32;
                        if new_pct / 5 > old_pct / 5 && new_pct < 100 {
                            let speed_mib = state.bytes_per_second / (1024.0 * 1024.0);
                            let total_mib = state.total_bytes as f64 / (1024.0 * 1024.0);
                            let name = if state.model_id == "llama-server" { "backend" } else { &state.filename };
                            download_logs.push(format!("Downloading {}: {}% of {:.1} MiB ({:.2} MiB/s)...", name, new_pct, total_mib, speed_mib));
                        }
                    }
                    self.download_progress[idx] = state;
                } else {
                    if state.model_id == "llama-server" {
                        download_logs.push("Starting backend download...".to_string());
                    } else {
                        download_logs.push(format!("Starting download: {}...", state.filename));
                    }
                    self.download_progress.push(state);
                }
                redraw = true;
            }
        }
        for log in download_logs {
            self.add_log(log, crate::config::LogLevel::Info);
        }
        if redraw {
            self.set_redraw();
        }
    }

    pub fn poll_bench_tune_progress(&mut self) {
        if let Some(mut rx) = self.bench_tune_rx.take() {
            while let Ok(status) = rx.try_recv() {
                self.bench_tune_progress = crate::models::BenchTuneProgress::from_status(&status);
                self.set_redraw();
            }
            self.bench_tune_rx = Some(rx);
        }
    }

    pub fn process_completed_downloads(&mut self) {
        let completed: Vec<crate::models::DownloadState> = self.download_progress.iter()
            .filter(|d| matches!(d.status, crate::models::DownloadStatus::Complete | crate::models::DownloadStatus::Error(_) | crate::models::DownloadStatus::Cancelled))
            .cloned()
            .collect();
        if !completed.is_empty() {
            for state in &completed {
                match &state.status {
                    crate::models::DownloadStatus::Complete => {
                        if state.model_id == "llama-server" {
                            self.add_log("Backend download complete", crate::config::LogLevel::Info);
                        } else {
                            self.add_log(format!("Download complete: {}", state.filename), crate::config::LogLevel::Info);
                            self.models = Self::discover_models(&self.config.models_dirs);
                        }
                    }
                    crate::models::DownloadStatus::Error(e) => {
                        let name = if state.model_id == "llama-server" { "Backend" } else { &state.filename };
                        self.add_log(format!("Download failed ({}): {}", name, e), crate::config::LogLevel::Error);
                    }
                    crate::models::DownloadStatus::Cancelled => {
                        let name = if state.model_id == "llama-server" { "Backend" } else { &state.filename };
                        self.add_log(format!("Download cancelled: {}", name), crate::config::LogLevel::Info);
                        if let Some(ref dest) = state.dest {
                            if dest.exists() {
                                if let Err(e) = std::fs::remove_file(dest) {
                                    self.add_log(format!("Failed to remove temp file {}: {}", dest.display(), e), crate::config::LogLevel::Warning);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            self.download_progress.retain(|d| {
                !matches!(d.status, crate::models::DownloadStatus::Complete | crate::models::DownloadStatus::Error(_) | crate::models::DownloadStatus::Cancelled)
            });
            self.downloading = !self.download_progress.is_empty();
            if !self.downloading {
                self.download_scroll_state.select(None);
            } else if let Some(idx) = self.download_scroll_state.selected()
                && idx >= self.download_progress.len()
            {
                self.download_scroll_state.select(Some(self.download_progress.len() - 1));
            }
            self.set_redraw();
        }
    }

    pub fn poll_server_logs(&mut self) {
        let mut server_logs = Vec::new();
        if let Some(rx) = &mut self.server_log_rx {
            while let Ok(line) = rx.try_recv() {
                if line.contains("tokens per second")
                    && let Some(tps_part) = line.split("tokens per second").next()
                        && let Some(val_str) = tps_part.split_whitespace().last()
                            && let Ok(tps) = val_str.parse::<f64>()
                {
                    if line.contains("prompt eval time =") {
                        self.metrics.prompt_tps = tps;
                        if tps > 0.0 {
                            self.metrics.prompt_latency_ms = 1000.0 / tps;
                        }
                    } else if line.contains("eval time =") {
                        self.metrics.tps = tps;
                        if tps > 0.0 {
                            self.metrics.latency_per_token_ms = 1000.0 / tps;
                        }
                    }
                }
                if line.contains("n_tokens =")
                    && let Some(tokens_part) = line.split("n_tokens =").last()
                {
                    let val_str = tokens_part.split(',').next().unwrap_or(tokens_part).trim();
                    if let Ok(tokens) = val_str.parse::<u32>() && tokens > 2048 {
                        self.metrics.ctx_used = tokens;
                    }
                }
                if line.contains("KV buffer size =")
                    && let Some(size_part) = line.split('=').next_back()
                {
                    let parts: Vec<&str> = size_part.split_whitespace().collect();
                    if !parts.is_empty()
                        && let Ok(mib) = parts[0].parse::<f64>()
                    {
                        self.metrics.gpu_mem_used = (mib * 1024.0 * 1024.0) as u64;
                    }
                }
                if line.contains("print_timing:")
                    && line.contains("n_decoded =")
                    && line.contains("t/s")
                {
                    if let Some(tokens_part) = line.split("n_decoded =").last() {
                        let tokens_str = tokens_part.split(',').next().unwrap_or(tokens_part).trim();
                        if let Ok(tokens) = tokens_str.parse::<u64>() {
                            self.metrics.decoded_tokens = tokens;
                        }
                    }
                    if let Some(tg_part) = line.split("tg =").last() {
                        let tg_str = tg_part.split_whitespace().next().unwrap_or("");
                        if let Ok(tg) = tg_str.parse::<f64>() {
                            self.metrics.throughput = tg;
                            if tg > 0.0 {
                                self.metrics.latency_per_token_ms = 1000.0 / tg;
                            }
                        }
                    }
                }
                server_logs.push(line);
                if server_logs.len() > 100 { break; }
            }
        }
        if !server_logs.is_empty() {
            for line in server_logs {
                self.add_log(line, crate::config::LogLevel::Info);
            }
            self.set_redraw();
        }
    }

    pub fn poll_sync(&mut self) {
        let mut sync_updated = false;
        if let Some(rx) = &mut self.sync_rx {
            while let Ok(models) = rx.try_recv() {
                if let Some(handle) = &self.server_handle {
                    let port = handle.port;
                    let pid = handle.pid;
                    for (id, status, path) in models {
                        let status_lower = status.to_lowercase();
                        let is_active = status_lower == "loaded" || status_lower == "loading" || status_lower == "ready";
                        let mut matched = false;
                        for model in &self.models {
                            let path_match = path.as_ref().map(|p| p == &model.path.to_string_lossy()).unwrap_or(false);
                            let id_match = id == model.display_name || id == model.name;
                            let filename_match = path.as_ref().and_then(|p| {
                                std::path::Path::new(p).file_name().map(|f| f.to_string_lossy().to_string())
                            }).map(|f| f == model.name).unwrap_or(false);
                            let id_filename_match = std::path::Path::new(&id)
                                .file_name()
                                .map(|f| f.to_string_lossy().to_string())
                                .map(|f| f == model.name || f == model.display_name)
                                .unwrap_or(false);
                            if path_match || id_match || filename_match || id_filename_match {
                                if is_active {
                                    if status_lower == "loading" {
                                        self.model_states.insert(model.display_name.clone(), crate::models::ModelState::Loading);
                                    } else {
                                        let mut loaded_names = self.loaded_model_names.lock().unwrap();
                                        if !loaded_names.contains(&model.display_name) {
                                            loaded_names.push(model.display_name.clone());
                                        }
                                        self.model_states.insert(model.display_name.clone(), crate::models::ModelState::Loaded { port, pid });
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
                                            let mut loaded_names = self.loaded_model_names.lock().unwrap();
                                            if !loaded_names.contains(&model.display_name) {
                                                loaded_names.push(model.display_name.clone());
                                            }
                                            self.model_states.insert(model.display_name.clone(), crate::models::ModelState::Loaded { port, pid });
                                        }
                                        matched = true;
                                        break;
                                    }
                                }
                                if matched { break; }
                            }
                        }
                    }
                    sync_updated = true;
                }
            }
        }
        if sync_updated {
            self.set_redraw();
        }
    }

    pub fn poll_metrics(&mut self) {
        if let Some(rx) = &mut self.metrics_rx {
            let mut received_metrics = false;
            while let Ok(mut m) = rx.try_recv() {
                if m.ctx_max == 0 {
                    m.ctx_max = self.settings.context_length;
                }
                if m.tps == 0.0 && self.metrics.tps > 0.0 {
                    m.tps = self.metrics.tps;
                }
                if m.prompt_tps == 0.0 && self.metrics.prompt_tps > 0.0 {
                    m.prompt_tps = self.metrics.prompt_tps;
                }
                if self.metrics.ctx_used > 0 {
                    m.ctx_used = self.metrics.ctx_used;
                }
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
                self.metrics = m;
                received_metrics = true;
            }
            if received_metrics {
                self.set_redraw();
            }
        }
    }

    pub async fn start_pending_spawn(&mut self) {
        if let Some((model_opt, settings)) = self.pending_spawn.take() {
            let (tx, rx) = tokio::sync::mpsc::channel(100);
            self.server_log_rx = Some(rx);
            let config_clone = self.config.clone();
            let model_clone = model_opt.clone();
            let settings_clone = settings.clone();
            let tx_clone = tx.clone();
            let server_mode_clone = self.server_mode.clone();
            let router_max_models_clone = self.router_max_models;
            let download_tx_clone = Some(self.ensure_download_channel());
            let display_name = model_opt.as_ref().map(|m| m.display_name.clone()).unwrap_or_else(|| "Router".to_string());
            if let Some(m) = &model_opt {
                let state = if server_mode_clone == crate::models::ServerMode::Bench {
                    crate::models::ModelState::Benchmarking
                } else if server_mode_clone == crate::models::ServerMode::BenchTune {
                    crate::models::ModelState::Benchmarking
                } else {
                    crate::models::ModelState::Loading
                };
                self.model_states.insert(m.display_name.clone(), state);
            }
            self.add_log(format!("Loading {}...", display_name), crate::config::LogLevel::Info);
            if server_mode_clone == crate::models::ServerMode::BenchTune {
                let model = match model_opt {
                    Some(m) => m,
                    None => {
                        self.add_log("Error: Benchmark tuning requires a selected model.", crate::config::LogLevel::Error);
                        return;
                    }
                };
                let bench_tune_config = self.bench_tune_config.take().unwrap_or_else(|| {
                    crate::models::BenchTuneConfig::new(
                        model.path.clone(),
                        3,
                        crate::models::BENCHMARK_PROMPT.to_string(),
                    )
                });
                let (tx_tune, rx_tune) = tokio::sync::mpsc::channel(100);
                self.bench_tune_tx = Some(tx_tune.clone());
                self.bench_tune_config = Some(bench_tune_config.clone());
                self.bench_tune_running = true;
                self.bench_tune_results.clear();
                self.bench_tune_result_row = 0;
                self.models_mode = super::types::ModelsMode::BenchTune;
                
                // Create cancellation channel for benchmark tuning
                let (cancel_tx, mut cancel_rx) = tokio::sync::watch::channel(false);
                self.bench_tune_cancel_tx = Some(cancel_tx);
                
                let bench_tune_config_clone = bench_tune_config.clone();
                let settings_clone = settings_clone.clone();
                let model_clone = model.clone();
                let tx_tune_clone = tx_tune.clone();
                let spawn_log_tx_clone = tx.clone();
                let handle = tokio::spawn(async move {
                    let results = crate::backend::benchmark::run_bench_tune(
                        &config_clone,
                        &bench_tune_config_clone,
                        &model_clone,
                        &settings_clone,
                        tx_tune_clone,
                        spawn_log_tx_clone,
                        &mut cancel_rx,
                    ).await.map_err(|e| e.to_string());
                    (results, display_name, bench_tune_config_clone)
                });
                self.bench_tune_task_handle = Some(handle);
                self.spawn_log_tx = Some(tx);
                self.set_redraw();
                self.bench_tune_rx = Some(rx_tune);
            } else {
                let handle = tokio::spawn(async move {
                    crate::backend::server::spawn_server(&config_clone, model_clone.as_ref(), &settings_clone, tx_clone, download_tx_clone, server_mode_clone, router_max_models_clone).await
                        .map(|(handle, cmd)| (display_name, handle, cmd))
                });
                self.spawn_task_handle = Some(handle);
                self.spawn_log_tx = Some(tx);
                self.set_redraw();
            }
        }
    }

    pub async fn poll_spawn_result(&mut self) {
        if let Some(handle) = &self.spawn_task_handle
            && handle.is_finished()
                && let Some(handle) = self.spawn_task_handle.take()
        {
            match handle.await {
                Ok(Ok((server_display_name, server_handle, _cmd))) => {
                    let port = server_handle.port;
                    let pid = server_handle.pid;
                    let host = server_handle.host.clone();
                    self.add_log(format!("Server started on port {port} (pid={pid})"), crate::config::LogLevel::Info);
                    self.server_handle = Some(server_handle);
                    if self.settings.api_endpoint_enabled {
                        let port = self.settings.api_endpoint_port;
                        let addr: std::net::SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap_or_else(|_| "127.0.0.1:49222".parse().unwrap());
                        let model_name = server_display_name.clone();
                        let server_port = self.server_handle.as_ref().map(|h| h.port).unwrap_or(8080);
                        let pid = self.server_handle.as_ref().map(|h| h.pid).unwrap_or(0);
                        let (_, shutdown_rx) = tokio::sync::watch::channel(false);
                        let handle = tokio::spawn(async move {
                            let _ = crate::serve_api::start_api_server(
                                addr, None, server_port, model_name, pid, shutdown_rx
                            ).await;
                        });
                        self.api_proxy_handle = Some(handle);
                        self.add_log(format!("API proxy started on port {}", port), crate::config::LogLevel::Info);
                    }
                    self.loading_phases = std::iter::once(super::types::LoadingPhase::Complete).collect();
                    self.last_active_phase = Some(super::types::LoadingPhase::Complete);
                    self.progress_target = 1.0;
                    let (metrics_tx, metrics_rx) = tokio::sync::mpsc::channel(10);
                    self.metrics_rx = Some(metrics_rx);
                    let task_host = host.clone();
                    let task_port = port;
                    let task_pid = pid;
                    let metrics_model_name = self.metrics_model_name.clone();
                    self.add_log("Starting metrics polling...", crate::config::LogLevel::Info);
                    let _task_handle = tokio::spawn(Self::metrics_polling_task(task_host, task_port, task_pid, metrics_model_name, metrics_tx));
                    self.metrics_task_handle = Some(_task_handle);
                    let sync_host = host.clone();
                    let sync_port = port;
                    let (sync_tx, sync_rx) = tokio::sync::mpsc::channel(1);
                    let _sync_task_handle = tokio::spawn(Self::sync_polling_task(sync_host, sync_port, sync_tx));
                    self.sync_rx = Some(sync_rx);
                    self.sync_task_handle = Some(_sync_task_handle);
                }
                Ok(Err(e)) => {
                    self.progress_target = 1.0;
                    self.add_log(format!("ERROR: Server failed: {}", e), crate::config::LogLevel::Error);
                    if let Some(mut rx) = self.server_log_rx.take() {
                        while let Ok(line) = rx.try_recv() {
                            self.add_log(line, crate::config::LogLevel::Info);
                        }
                    }
                    self.last_error_message = Some(e);
                    self.reset_loading_state(true);
                }
                Err(e) => {
                    self.progress_target = 1.0;
                    self.add_log(format!("ERROR: Spawn task panicked: {}", e), crate::config::LogLevel::Error);
                }
            }
            self.set_redraw();
        }
    }

    async fn metrics_polling_task(host: String, port: u16, pid: u32, metrics_model_name: Arc<std::sync::Mutex<Option<String>>>, metrics_tx: tokio::sync::mpsc::Sender<crate::models::ServerMetrics>) {
        let mut consecutive_failures: u32 = 0;
        let max_failures: u32 = 15;
        loop {
            let mut m = match crate::backend::server::get_metrics(&host, port, None, Some(pid)).await {
                Ok(metrics) => {
                    consecutive_failures = 0;
                    metrics
                }
                Err(_) => {
                    consecutive_failures += 1;
                    if consecutive_failures >= max_failures {
                        tracing::warn!("Metrics polling aborted after {} consecutive failures (server likely dead)", max_failures);
                        break;
                    }
                    if consecutive_failures % 5 == 1 {
                        tracing::warn!("Metrics polling: server unreachable (attempt {}/{})", consecutive_failures, max_failures);
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
                && let Ok(model_metrics) = crate::backend::server::get_metrics(&host, port, Some(&name), Some(pid)).await
            {
                let stotal = m.gpu_mem_total;
                let should_use_model_vram = if stotal > 0 {
                    model_metrics.gpu_mem_used >= stotal / 4
                } else {
                    true
                };
                m.ctx_used = model_metrics.ctx_used;
                m.ctx_max = model_metrics.ctx_max;
                m.tps = model_metrics.tps;
                if should_use_model_vram {
                    m.gpu_mem_used = model_metrics.gpu_mem_used;
                }
            }
            if metrics_tx.send(m).await.is_err() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    }

    async fn sync_polling_task(host: String, port: u16, sync_tx: tokio::sync::mpsc::Sender<Vec<(String, String, Option<String>)>>) {
        loop {
            if let Ok(models) = crate::backend::server::list_models(&host, port).await
                && sync_tx.send(models).await.is_err()
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    }

    pub async fn poll_bench_tune_result(&mut self) {
        if let Some(handle) = &self.bench_tune_task_handle
            && handle.is_finished()
            && let Some(handle) = self.bench_tune_task_handle.take()
        {
            match handle.await {
                Ok((results, display_name, bench_config)) => {
                    match results {
                        Ok(bench_results) => {
                            self.add_log(format!("Benchmark tuning completed for {} with {} results", display_name, bench_results.len()), crate::config::LogLevel::Info);
                            if bench_results.is_empty() {
                                self.add_log("No successful benchmark results were obtained. Check the Log (F6) for details on test failures.", crate::config::LogLevel::Warning);
                            } else {
                                let output_dir = crate::config::Config::config_path().parent().unwrap().join("benchmarks");
                                match crate::backend::benchmark::save_results(&bench_results, &output_dir, &bench_config).await {
                                    Ok(()) => self.add_log(format!("Results saved to {}/", output_dir.display()), crate::config::LogLevel::Info),
                                    Err(e) => self.add_log(format!("Failed to save benchmark results: {}", e), crate::config::LogLevel::Error),
                                }
                            }
                            let mut sorted_results = bench_results;
                            sorted_results.sort_by(|a, b| b.metrics.generation_tps.partial_cmp(&a.metrics.generation_tps).unwrap_or(std::cmp::Ordering::Equal));
                            self.bench_tune_results = sorted_results;
                            self.bench_tune_running = false;
                            let (host, port, model_name, model_path_str, task_name, model_display_name) = {
                                let model = match self.selected_model() {
                                    Some(m) => m,
                                    None => {
                                        self.set_redraw();
                                        return;
                                    }
                                };
                                let handle = match &self.server_handle {
                                    Some(h) => h,
                                    None => {
                                        self.set_redraw();
                                        return;
                                    }
                                };
                                (
                                    handle.host.clone(),
                                    handle.port,
                                    model.display_name.clone(),
                                    model.path.to_str().map(|s| s.to_string()),
                                    format!("bench_unload_{}", model.display_name),
                                    model.display_name.clone(),
                                )
                            };
                            let task_handle = tokio::spawn(async move {
                                let _ = crate::backend::server::unload_model(&host, port, &model_name, model_path_str.as_deref()).await;
                            });
                            self.background_tasks.insert(task_name, task_handle);
                            self.model_states.insert(model_display_name, crate::models::ModelState::Available);
                        }
                        Err(e) => {
                            self.add_log(format!("Benchmark tuning failed: {}", e), crate::config::LogLevel::Error);
                            self.bench_tune_running = false;
                            if let Some(model) = self.selected_model() {
                                self.model_states.insert(model.display_name.clone(), crate::models::ModelState::Failed { error: e.to_string() });
                            }
                        }
                    }
                }
                Err(e) => {
                    self.add_log(format!("Benchmark task panicked: {:?}", e), crate::config::LogLevel::Error);
                    self.bench_tune_running = false;
                }
            }
            self.set_redraw();
        }
    }

    pub fn handle_pending_api_load(&mut self) {
        if let Some((model_name, model_path)) = self.pending_api_load.clone() {
            if let Some(handle) = &self.server_handle {
                if self.loading_phases.contains(&super::types::LoadingPhase::Complete) || self.loading_phases.contains(&super::types::LoadingPhase::ServerListening) {
                    let host = handle.host.clone();
                    let port = handle.port;
                    let model_name_clone = model_name.clone();
                    let model_path_clone = model_path.clone();
                    self.pending_api_load = None;
                    self.add_log(format!("Sending load request for {}...", model_name_clone), crate::config::LogLevel::Info);
                    {
                        let mut lock = self.metrics_model_name.lock().unwrap();
                        *lock = Some(model_name_clone.clone());
                    }
                    let log_tx = self.spawn_log_tx.clone();
                    let model_name_err = model_name_clone.clone();
                    tokio::spawn(async move {
                        if let Err(e) = crate::backend::server::load_model(&host, port, &model_name_clone, model_path_clone.as_deref()).await {
                            let err_msg = format!("ERROR: Failed to load model {}: {}", model_name_err, e);
                            if let Some(tx) = log_tx {
                                let _ = tx.send(err_msg).await;
                            } else {
                                eprintln!("{}", err_msg);
                            }
                        }
                    });
                    self.model_states.insert(model_name, crate::models::ModelState::Loading);
                }
            } else if self.spawn_task_handle.is_none() && self.pending_spawn.is_none() {
                self.pending_api_load = None;
            }
        }
    }

    pub fn handle_pending_api_unload(&mut self) {
        if !matches!(self.global_mode, super::types::GlobalMode::Confirmation { .. }) {
            if let Some((model_name, model_path)) = self.pending_api_unload.take()
                && let Some(handle) = &self.server_handle
            {
                let server_mode = self.server_mode;
                let handle_clone = handle.clone();
                {
                    let mut lock = self.metrics_model_name.lock().unwrap();
                    if lock.as_deref() == Some(&model_name) {
                        *lock = None;
                    }
                }
                let host = handle.host.clone();
                let port = handle.port;
                let model_name_clone = model_name.clone();
                let model_path_clone = model_path.clone();
                if server_mode == crate::models::ServerMode::Normal {
                    self.add_log(format!("Unloading {} (killing server)...", model_name_clone), crate::config::LogLevel::Info);
                    self.pending_kill = Some(handle_clone);
                } else {
                    self.add_log(format!("Sending unload request for {}...", model_name_clone), crate::config::LogLevel::Info);
                    let kill_tx = self.spawn_log_tx.clone();
                    let kill_tx2 = kill_tx.clone();
                    let server_clone = self.server_handle.clone();
                    let host_clone = host.clone();
                    let port_clone = port;
                    let model_name_task = model_name_clone.clone();
                    self.background_tasks.insert(
                        format!("api_unload_{}", model_name_task),
                        tokio::spawn(async move {
                            if let Err(e) = crate::backend::server::unload_model(&host, port, &model_name_clone, model_path_clone.as_deref()).await {
                                if let Some(tx) = kill_tx {
                                    let _ = tx.send(format!("Failed to unload model via API: {}", e)).await;
                                }
                                return;
                            }
                            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                            if let Ok(loaded) = crate::backend::server::list_models(&host_clone, port_clone).await {
                                if loaded.is_empty() {
                                    if let Some(tx) = kill_tx {
                                        let _ = tx.send("No models left, stopping router...".to_string()).await;
                                    }
                                    if let Some(server) = server_clone {
                                        let _ = crate::backend::server::kill_server(server).await;
                                        if let Some(tx) = kill_tx2 {
                                            let _ = tx.send("Server stopped".to_string()).await;
                                        }
                                    }
                                } else {
                                    if let Some(tx) = kill_tx {
                                        let _ = tx.send(format!("{} models still loaded on server", loaded.len())).await;
                                    }
                                }
                            }
                        })
                    );
                }
                self.loaded_model_names.lock().unwrap().retain(|n| n != &model_name);
                self.model_states.insert(model_name, crate::models::ModelState::Available);
            }
        }
    }

    pub async fn start_pending_kill(&mut self) {
        if let Some(handle) = self.pending_kill.take() {
            match crate::backend::server::kill_server(handle).await {
                Ok(()) => {
                    self.add_log("Server stopped", crate::config::LogLevel::Info);
                    self.server_handle = None;
                    self.metrics_rx = None;
                    self.metrics = Default::default();
                    if let Some(task) = self.metrics_task_handle.take() {
                        task.abort();
                    }
                    if let Some(task) = self.sync_task_handle.take() {
                        task.abort();
                    }
                    self.sync_rx = None;
                    if let Some(proxy) = self.api_proxy_handle.take() {
                        proxy.abort();
                    }
                    let mut names_to_reset = Vec::new();
                    for (name, state) in &self.model_states {
                        if !matches!(state, crate::models::ModelState::Available) && !matches!(state, crate::models::ModelState::Failed { .. }) {
                            names_to_reset.push(name.clone());
                        }
                    }
                    for name in names_to_reset {
                        let n: String = name.clone();
                        self.model_states.insert(n, crate::models::ModelState::Available);
                    }
                    self.loaded_model_names.lock().unwrap().clear();
                    self.loading_phases = std::collections::HashSet::new();
                    self.loading_progress = 0.0;
                    self.progress_target = 0.0;
                }
                Err(e) => {
                    self.add_log(format!("Failed to stop server: {}", e), crate::config::LogLevel::Error);
                }
            }
            self.set_redraw();
        }
    }

    pub async fn handle_pending_search(&mut self) {
        if self.search_loading {
            if let Some((query, offset)) = self.pending_search_load.take() {
                let is_append = offset > 0;
                let query_clone = query.clone();
                let offset_clone = offset;
                let search_limit = self.config.search_limit;
                self.add_log(format!("Searching with limit={} offset={}...", search_limit, offset_clone), crate::config::LogLevel::Info);
                let search_handle = tokio::spawn(async move {
                    crate::backend::hub::search_models(&query_clone, search_limit, offset_clone).await
                });
                match search_handle.await {
                    Ok(Ok((res, _, raw_ids))) => {
                        let query_str = &query;
                        let mut buf = format!("Search complete: {} results for '{}'", res.len(), query_str);
                        buf.push_str(&format!("\n  RAW API returned: {}", raw_ids.join(", ")));
                        for r in &res {
                            let gguf_tags: Vec<String> = r.tags.iter().filter(|t| t.starts_with("gguf:")).cloned().collect();
                            buf.push_str(&format!("\n  {} quant={} tags={} params={} cap={} ctx={}", r.model_id, r.quantization.as_deref().unwrap_or("-"), gguf_tags.join(","), r.parameters.as_deref().unwrap_or("none"), r.capabilities.join(","), r.context_length.unwrap_or(0)));
                        }
                        let raw_len = raw_ids.len();
                        if is_append {
                            if let super::types::ModelsMode::Search { results, has_more, loading, .. } = &mut self.models_mode {
                                let models = self.models.clone();
                                for r in res {
                                    let downloaded = models.iter().any(|m| {
                                        m.name == r.model_id || m.name.starts_with(&r.model_id.rsplit('/').next().unwrap_or(""))
                                    });
                                    results.push(crate::models::SearchResult { downloaded, ..r });
                                }
                                if raw_len < self.config.search_limit as usize {
                                    *has_more = false;
                                }
                                *loading = false;
                            }
                        } else {
                            if let super::types::ModelsMode::Search { results, loading, has_more, .. } = &mut self.models_mode {
                                let models = self.models.clone();
                                *results = res.into_iter().map(|r| {
                                    let downloaded = models.iter().any(|m| {
                                        m.name == r.model_id || m.name.starts_with(&r.model_id.rsplit('/').next().unwrap_or(""))
                                    });
                                    crate::models::SearchResult { downloaded, ..r }
                                }).collect();
                                if !results.is_empty() {
                                    self.search_results_idx = Some(0);
                                } else {
                                    self.search_results_idx = None;
                                }
                                *has_more = raw_len >= self.config.search_limit as usize;
                                *loading = false;
                            }
                        }
                        self.add_log(buf, crate::config::LogLevel::Info);
                    }
                    Ok(Err(e)) => {
                        self.add_log(format!("Search failed: {}", e), crate::config::LogLevel::Error);
                        if let super::types::ModelsMode::Search { loading, .. } = &mut self.models_mode {
                            *loading = false;
                        }
                    }
                    Err(e) => {
                        self.add_log(format!("Search task error: {}", e), crate::config::LogLevel::Error);
                        if let super::types::ModelsMode::Search { loading, .. } = &mut self.models_mode {
                            *loading = false;
                        }
                    }
                }
            }
            self.search_loading = false;
            self.set_redraw();
        }
    }

    pub fn update_metrics_model_name(&mut self) {
        let active_loaded_model = if let Some(model) = self.selected_model() {
            if self.is_model_loaded(&model.name) {
                Some(model.name.clone())
            } else {
                None
            }
        } else {
            None
        };
        let mut lock = self.metrics_model_name.lock().unwrap();
        *lock = active_loaded_model;
    }

    pub fn ensure_download_channel(&mut self) -> tokio::sync::broadcast::Sender<crate::models::DownloadState> {
        if self.download_rx.is_none() {
            let (tx, rx) = tokio::sync::broadcast::channel(10);
            self.download_tx = Some(tx);
            self.download_rx = Some(rx);
        }
        self.download_tx.as_ref().unwrap().clone()
    }
}
