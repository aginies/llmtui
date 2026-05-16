mod backend;
mod config;
mod models;
mod serve;
mod serve_api;
mod tui;

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use tracing::info;

use crate::backend::hub;
use crate::backend::server;
use crate::config::Config;
use crate::tui::app::ModelsMode;
use crate::models::Backend;
use crate::models::{DiscoveredModel, DownloadState};
use crate::tui::app::App;
use std::sync::{Arc, atomic::AtomicBool};
use tracing_subscriber::prelude::*;

#[derive(Parser)]
#[command(name = "llm-manager", about = "Manage and chat with local LLMs")]
enum Cli {
    /// Manage and chat with local LLMs (TUI mode, default)
    #[command(name = "tui", about = "Start the terminal UI")]
    Tui {
        /// Path to models directory
        #[arg(short, long)]
        models_dir: Option<String>,

        /// Path to llama-server binary
        #[arg(short, long, default_value = "llama-server")]
        llama_server: String,

        /// Backend to use (cpu, vulkan)
        #[arg(short, long, default_value = "vulkan")]
        backend: String,

        /// Path to config file
        #[arg(short, long)]
        config: Option<String>,
    },

    /// Serve a model using llama-server with all config.yaml settings
    #[command(name = "serve", about = "Serve a model with llama-server")]
    Serve {
        /// Path to the model file (.gguf)
        #[arg(short, long)]
        model: String,

        /// Apply a settings profile (e.g. qwen, llama, mistral)
        #[arg(short, long)]
        profile: Option<String>,

        /// Path to config file
        #[arg(short, long)]
        config: Option<String>,

        /// Start an API proxy server on the given port
        #[arg(long)]
        api_port: Option<u16>,

        /// API key for authentication (Bearer token)
        #[arg(long)]
        api_key: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Redirect tracing to a file to avoid corrupting the TUI
    let data_dir = dirs::data_local_dir()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
        .join("llm-manager");
    std::fs::create_dir_all(&data_dir)?;
    let log_path = data_dir.join("llm-manager.log");
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_writer(log_file))
        .with(tracing_subscriber::EnvFilter::from_default_env().add_directive("llm_manager=info".parse().unwrap()))
        .init();

    info!("Logging to {}", log_path.display());

    match Cli::parse() {
        Cli::Serve { model, profile, config, api_port, api_key } => {
            serve::serve_model(&model, profile.as_deref(), config.as_deref(), api_port, api_key).await
        }
        Cli::Tui {
            models_dir,
            llama_server,
            backend,
            config,
        } => {
            let config_path = config.map(PathBuf::from).unwrap_or(Config::config_path());

            // Load or create config
            let config = if config_path.exists() {
                Config::load_from(config_path).map_err(|e| anyhow::anyhow!("Failed to load config: {}", e))?
            } else {
                let mut c = Config::default();
                c.models_dir = resolve_models_dir(&models_dir);
                c.llama_server = PathBuf::from(&llama_server);
                c.save().map_err(|e| anyhow::anyhow!("Failed to save config: {}", e))?;
                c
            };

            // Apply CLI backend override
            let backend = match backend.to_lowercase().as_str() {
                "vulkan" => Backend::Vulkan,
                _ => Backend::Cpu,
            };
            let mut config = config;
            config.default.backend = backend;

    // Ensure models directory exists
    std::fs::create_dir_all(&config.models_dir)?;

    // Discover models asynchronously
    let models_dir = config.models_dir.clone();
    let models = tokio::task::spawn_blocking(move || {
        discover_models(&models_dir)
    }).await.unwrap_or_default();
    
    info!("Discovered {} models", models.len());

    let mut app = App::new(config);
    app.models = models;
    if !app.models.is_empty() {
        app.selected_model_idx = Some(0);
        app.on_model_selection_change();
    }

    // Setup terminal
    crossterm::terminal::enable_raw_mode().map_err(|e| anyhow::anyhow!("Failed to enable raw terminal mode (are you running in a TTY?): {}", e))?;
    crossterm::execute!(
        std::io::stdout(),
        crossterm::terminal::EnterAlternateScreen,
        crossterm::event::EnableMouseCapture,
    )?;
    crossterm::execute!(std::io::stdout(), crossterm::terminal::Clear(crossterm::terminal::ClearType::All))?;

    let mut terminal = ratatui::Terminal::new(ratatui::backend::CrosstermBackend::new(std::io::stdout()))?;

    // Main event loop
    loop {
        // Update the active model name for metrics polling
        {
            let active_loaded_model = if let Some(model) = app.selected_model() {
                if app.is_model_loaded(&model.name) {
                    Some(model.name.clone())
                } else {
                    None
                }
            } else {
                None
            };
            let mut lock = app.metrics_model_name.lock().unwrap();
            *lock = active_loaded_model;
        }

        // Start pending download
        if let Some((model_id, filename, download_url)) = app.pending_download.take() {
            let models_dir = app.config.models_dir.clone();
            let dest = models_dir.join(&filename);
            let model_id_clone = model_id.clone();
            let filename_clone = filename.clone();
            let url_clone = download_url.clone();
            let cancelled = Arc::new(AtomicBool::new(false));
            let cancelled_clone = cancelled.clone();
            app.add_log(&format!("Downloading {}...", model_id), crate::config::LogLevel::Info);
            // Create broadcast channel if not already created (shared by all downloads)
            if app.download_rx.is_none() {
                let (tx, rx) = tokio::sync::broadcast::channel(10);
                app.download_tx = Some(tx);
                app.download_rx = Some(rx);
            }
            let tx = app.download_tx.as_ref().unwrap().clone();
            let tx_clone = tx.clone();
            let cancelled_for_state = cancelled_clone.clone();
            tokio::spawn(async move {
                let mut state = DownloadState::new(model_id_clone.clone(), filename_clone.clone(), 0);
                state.cancel_token = Some(cancelled_for_state);
                let result = hub::download_file(&model_id_clone, &filename_clone, &url_clone, &dest, &mut state, cancelled_clone, tx_clone).await;
                if let Err(e) = result {
                    state.status = crate::models::DownloadStatus::Error(e.to_string());
                    let _ = tx.send(state);
                }
            });
            app.downloading = true;
            app.cancelled = Some(cancelled);
            app.download_scroll_state.select(Some(0));
            app.set_redraw();
            // Don't switch models_mode to Download here anymore,
            // the side panel in render.rs will handle it using app.downloading and a temporary state.
        }

        // Start pending deletion
        if let Some(path) = app.pending_deletion.take() {
            let path_clone = path.clone();
            tokio::spawn(async move {
                if let Err(e) = tokio::fs::remove_file(&path_clone).await {
                    eprintln!("Failed to delete file: {}", e);
                }
            });
            // Remove this model's settings override from config
            let model_key = path.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            app.config.model_overrides.remove(&model_key);
            if let Err(e) = app.config.save() {
                eprintln!("Failed to save config after deletion: {}", e);
            }

            // Update UI list immediately
            app.models.retain(|m| m.path != path);
            if let Some(idx) = app.selected_model_idx {
                if idx >= app.models.len() && !app.models.is_empty() {
                    app.selected_model_idx = Some(app.models.len() - 1);
                    app.on_model_selection_change();
                } else if app.models.is_empty() {
                    app.selected_model_idx = None;
                    app.on_model_selection_change();
                } else {
                    app.on_model_selection_change();
                }
            }
            app.add_log(&format!("Model deleted: {:?}", path.file_name().unwrap_or_default()), crate::config::LogLevel::Info);
            app.set_redraw();
        }

        // Start pending server spawn
        if let Some((model_opt, settings)) = app.pending_spawn.take() {
            let (tx, rx) = tokio::sync::mpsc::channel(100);
            app.server_log_rx = Some(rx);

            let config_clone = app.config.clone();
            let model_clone = model_opt.clone();
            let settings_clone = settings.clone();
            let tx_clone = tx.clone();

            let display_name = model_opt.as_ref().map(|m| m.display_name.clone()).unwrap_or_else(|| "Router".to_string());
            if let Some(m) = &model_opt {
                app.model_states.insert(m.display_name.clone(), crate::models::ModelState::Loading);
            }
            app.add_log(&format!("Loading {}...", display_name), crate::config::LogLevel::Info);
            let handle = tokio::spawn(async move {
                server::spawn_server(&config_clone, model_clone.as_ref(), &settings_clone, tx_clone).await
                    .map(|(handle, cmd)| (display_name, handle, cmd))
            });
            app.spawn_task_handle = Some(handle);
            app.spawn_log_tx = Some(tx);
            app.set_redraw();
        }
        // Check if server spawn task is done
        if let Some(handle) = &app.spawn_task_handle {
            if handle.is_finished() {
                if let Some(handle) = app.spawn_task_handle.take() {
                    match handle.await {
Ok(Ok((_model_name, server_handle, _cmd))) => {
                            let port = server_handle.port;
                            let pid = server_handle.pid;
                            let host = server_handle.host.clone();
                            app.add_log(&format!("Server started on port {port} (pid={pid})"), crate::config::LogLevel::Info);
                            app.server_handle = Some(server_handle);
                            
// Start API proxy if enabled
                            if app.settings.api_endpoint_enabled {
                                let port = app.settings.api_endpoint_port;
                                let addr: std::net::SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap_or_else(|_| "127.0.0.1:49222".parse().unwrap());
                                let model_name = _model_name.clone();
                                let server_port = app.server_handle.as_ref().map(|h| h.port).unwrap_or(8080);
                                let pid = app.server_handle.as_ref().map(|h| h.pid).unwrap_or(0);
                                let handle = tokio::spawn(async move {
                                    let _ = crate::serve_api::start_api_server(
                                        addr, None, server_port, model_name, pid
                                    ).await;
                                });
                                app.api_proxy_handle = Some(handle);
                                app.add_log(&format!("API proxy started on port {}", port), crate::config::LogLevel::Info);
                            }
                            
                            app.loading_phases = vec![crate::tui::app::LoadingPhase::Complete];
                            app.loading_progress = 1.0;
                            
                            // Start continuous metrics polling task (runs until server stops).
                            let (metrics_tx, metrics_rx) = tokio::sync::mpsc::channel(10);
                            app.metrics_rx = Some(metrics_rx);
                            let task_host = host.clone();
                            let task_port = port;
                            let task_pid = pid;
                            let metrics_model_name = app.metrics_model_name.clone();
                            app.add_log("Starting metrics polling...", crate::config::LogLevel::Info);
                            let _task_handle = tokio::spawn(async move {
                                loop {
                                    // 1. Get system-wide / router-wide metrics (no model specified)
                                    // This gives us the accurate "Total VRAM" and system stats.
                                    let mut m = match server::get_metrics(&task_host, task_port, None, Some(task_pid)).await {
                                        Ok(metrics) => metrics,
                                        Err(_) => {
                                            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                                            continue;
                                        }
                                    };
                                    
                                    // The total VRAM is what we just got from the system-wide call
                                    m.total_vram_used = m.gpu_mem_used;

                                    // 2. If a specific model is active, get its specific metrics for the individual display
                                    let current_model = {
                                        let lock = metrics_model_name.lock().unwrap();
                                        lock.clone()
                                    };
                                    
                                    if let Some(name) = current_model {
                                        if let Ok(model_metrics) = server::get_metrics(&task_host, task_port, Some(&name), Some(task_pid)).await {
                                            // Only use model-specific VRAM if it's meaningful (e.g., >25% of total).
                                            // llama-server's kv_cache_usage is just the KV cache component and can be
                                            // much smaller than actual GPU usage. When system tools like nvidia-smi
                                            // are available, they give more accurate totals for single-model mode.
                                            let stotal = m.gpu_mem_total;
                                            let should_use_model_vram = if stotal > 0 {
                                                model_metrics.gpu_mem_used >= stotal / 4
                                            } else {
                                                true
                                            };

                                            // Override model-specific fields in the metrics struct
                                            m.ctx_used = model_metrics.ctx_used;
                                            m.ctx_max = model_metrics.ctx_max;
                                            m.tps = model_metrics.tps;
                                            if should_use_model_vram {
                                                m.gpu_mem_used = model_metrics.gpu_mem_used;
                                            }
                                        }
                                    }

                                    if metrics_tx.send(m).await.is_err() {
                                        break;
                                    }
                                    
                                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                                }
                            });
                            app.metrics_task_handle = Some(_task_handle);
                            
                            // Start model state synchronization task
                            let sync_host = host.clone();
                            let sync_port = port;
                            let (sync_tx, sync_rx) = tokio::sync::mpsc::channel(1);
                            let _sync_task_handle = tokio::spawn(async move {
                                loop {
                                    if let Ok(models) = server::list_models(&sync_host, sync_port).await {
                                        if sync_tx.send(models).await.is_err() {
                                            break;
                                        }
                                    }
                                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                                }
                            });
                            
                            app.sync_rx = Some(sync_rx);
                            app.sync_task_handle = Some(_sync_task_handle);
                        }
                        Ok(Err(e)) => {
                            app.loading_progress = 1.0;
                            app.add_log(&format!("ERROR: Server failed: {}", e), crate::config::LogLevel::Error);
                            // Drain any logs already in the channel
                            if let Some(mut rx) = app.server_log_rx.take() {
                                while let Ok(line) = rx.try_recv() {
                                    app.add_log(line, crate::config::LogLevel::Info);
                                }
                            }
                            // Mark the failed model so it's visible in the UI
                            app.last_error_message = Some(e);
                            app.reset_loading_state(true);
                        }
                        Err(e) => {
                            app.loading_progress = 1.0;
                            app.add_log(&format!("ERROR: Spawn task panicked: {}", e), crate::config::LogLevel::Error);
                        }
                    }
                    app.set_redraw();
                }
            }
        }

        // Handle pending API load
        if let Some((model_name, model_path)) = app.pending_api_load.clone() {
            if let Some(handle) = &app.server_handle {
                // Ensure server is listening (marked by Complete phase) before sending API load
                if app.loading_phases.contains(&crate::tui::app::LoadingPhase::Complete) || app.loading_phases.contains(&crate::tui::app::LoadingPhase::ServerListening) {
                    let host = handle.host.clone();
                    let port = handle.port;
                    let model_name_clone = model_name.clone();
                    let model_path_clone = model_path.clone();
                    
                    app.pending_api_load = None; // Clear it now that we are acting on it
                    app.add_log(&format!("Sending load request for {}...", model_name_clone), crate::config::LogLevel::Info);
                    
                    // Update metrics model name immediately so polling includes it
                    {
                        let mut lock = app.metrics_model_name.lock().unwrap();
                        *lock = Some(model_name_clone.clone());
                    }

                    let log_tx = app.spawn_log_tx.clone();
                    let model_name_err = model_name_clone.clone();
                    
                    tokio::spawn(async move {
                        if let Err(e) = server::load_model(&host, port, &model_name_clone, model_path_clone.as_deref()).await {
                            let err_msg = format!("ERROR: Failed to load model {}: {}", model_name_err, e);
                            if let Some(tx) = log_tx {
                                let _ = tx.send(err_msg).await;
                            } else {
                                eprintln!("{}", err_msg);
                            }
                        }
                    });

                    // Set initial state for this model
                    app.model_states.insert(model_name, crate::models::ModelState::Loading);
                }
            } else if app.spawn_task_handle.is_none() && app.pending_spawn.is_none() {
                // If no server and no server is being spawned, we can't load.
                // This might happen if server spawn failed.
                app.pending_api_load = None;
            }
        }

        // Handle pending API unload
        if let Some((model_name, model_path)) = app.pending_api_unload.take() {
            if let Some(handle) = &app.server_handle {
                let host = handle.host.clone();
                let port = handle.port;
                let model_name_clone = model_name.clone();
                let model_path_clone = model_path.clone();
                
                app.add_log(&format!("Sending unload request for {}...", model_name_clone), crate::config::LogLevel::Info);
                
                // Clear metrics model name
                {
                    let mut lock = app.metrics_model_name.lock().unwrap();
                    if lock.as_deref() == Some(&model_name_clone) {
                        *lock = None;
                    }
                }

                tokio::spawn(async move {
                    if let Err(e) = server::unload_model(&host, port, &model_name_clone, model_path_clone.as_deref()).await {
                        eprintln!("Failed to unload model via API: {}", e);
                    }
                });
                
                app.loaded_model_names.lock().unwrap().retain(|n| n != &model_name);
                app.model_states.insert(model_name, crate::models::ModelState::Available);
                
                // If no more models are loaded, kill the server
                let loaded_count = app.model_states.values().filter(|s| matches!(s, crate::models::ModelState::Loaded { .. })).count();
                if loaded_count == 0 {
                    app.add_log("No models left, stopping router...", crate::config::LogLevel::Info);
                    if let Some(h) = app.server_handle.take() {
                        app.pending_kill = Some(h);
                    }
                }
            }
        }

        // Start pending server kill
        if let Some(handle) = app.pending_kill.take() {
            match server::kill_server(handle).await {
                Ok(()) => {
                    app.add_log("Server stopped", crate::config::LogLevel::Info);
                    app.server_handle = None;
                    app.metrics_rx = None;
                    app.metrics = Default::default();
                    
                    // Abort the metrics task if it exists
                    if let Some(task) = app.metrics_task_handle.take() {
                        task.abort();
                    }

              // Abort the sync task if it exists
                    if let Some(task) = app.sync_task_handle.take() {
                        task.abort();
                    }
                    app.sync_rx = None;
                    
                    // Abort the API proxy if it exists
                    if let Some(proxy) = app.api_proxy_handle.take() {
                        proxy.abort();
                    }
                    
                    // Reset all model states to Available since the server is gone
                    // (But keep Failed states so the user sees the error message)
                    let mut names_to_reset = Vec::new();
                    for (name, state) in &app.model_states {
                        if !matches!(state, crate::models::ModelState::Available) && !matches!(state, crate::models::ModelState::Failed { .. }) {
                            names_to_reset.push(name.clone());
                        }
                    }
                    for name in names_to_reset {
                        app.model_states.insert(name, crate::models::ModelState::Available);
                    }
                    app.loaded_model_names.lock().unwrap().clear();

                    app.loading_phases = Vec::new();
                    app.loading_progress = 0.0;
                 }
                Err(e) => {
                    app.add_log(&format!("Failed to stop server: {}", e), crate::config::LogLevel::Error);
                }
            }
            app.set_redraw();
        }

        // Poll download channel for progress
        let mut redraw = false;
        if let Some(rx) = &mut app.download_rx {
            while let Ok(state) = rx.try_recv() {
                // Find matching download and update in-place, or append new
                if let Some(idx) = app.download_progress.iter().position(|d| {
                    d.model_id == state.model_id && d.filename == state.filename
                }) {
                    app.download_progress[idx] = state;
                } else {
                    app.download_progress.push(state);
                }
                redraw = true;
            }
        }
        if redraw {
            app.set_redraw();
        }

        // Process completed downloads (separate pass to avoid borrow issues)
        let completed: Vec<DownloadState> = app.download_progress.iter()
            .filter(|d| matches!(d.status, crate::models::DownloadStatus::Complete | crate::models::DownloadStatus::Error(_)))
            .cloned()
            .collect();
        if !completed.is_empty() {
            for state in &completed {
                match &state.status {
                    crate::models::DownloadStatus::Complete => {
                        app.add_log("Download complete!", crate::config::LogLevel::Info);
                        app.models = discover_models(&app.config.models_dir);
                    }
                    crate::models::DownloadStatus::Error(e) => {
                        app.add_log(&format!("Download failed: {}", e), crate::config::LogLevel::Error);
                    }
                    _ => {}
                }
            }
            app.download_progress.retain(|d| {
                !matches!(d.status, crate::models::DownloadStatus::Complete | crate::models::DownloadStatus::Error(_))
            });
            app.downloading = !app.download_progress.is_empty();
            if !app.downloading {
                app.download_scroll_state.select(None);
                if app.active_panel == crate::tui::app::ActivePanel::Downloads {
                    app.active_panel = crate::tui::app::ActivePanel::Log;
                }
            } else if let Some(idx) = app.download_scroll_state.selected() {
                if idx >= app.download_progress.len() {
                    app.download_scroll_state.select(Some(app.download_progress.len() - 1));
                }
            }
            app.set_redraw();
        }

        // Poll server log channel
        let mut server_logs = Vec::new();
        if let Some(rx) = &mut app.server_log_rx {
            while let Ok(line) = rx.try_recv() {
                // Parse TPS from logs if present
                if line.contains("tokens per second") {
                    if let Some(tps_part) = line.split("tokens per second").next() {
                        if let Some(val_str) = tps_part.split_whitespace().last() {
                            if let Ok(tps) = val_str.parse::<f64>() {
                                if line.contains("prompt eval time =") {
                                    app.metrics.prompt_tps = tps;
                                } else if line.contains("eval time =") {
                                    app.metrics.tps = tps;
                                }
                            }
                        }
                    }
                }
                // Parse Context Usage from logs: "n_tokens = 12667"
                // Don't use max() — after compaction the token count drops, and we
                // want the display to reflect the current state.
                if line.contains("n_tokens =") {
                    if let Some(tokens_part) = line.split("n_tokens =").last() {
                        let val_str = tokens_part.split(',').next().unwrap_or(tokens_part).trim();
                        if let Ok(tokens) = val_str.parse::<u32>() {
                            app.metrics.ctx_used = tokens;
                        }
                    }
                }
                // Parse VRAM (KV Cache) from logs: "Vulkan0 KV buffer size =  1008.00 MiB"
                if line.contains("KV buffer size =") {
                    if let Some(size_part) = line.split('=').last() {
                        let parts: Vec<&str> = size_part.split_whitespace().collect();
                        if !parts.is_empty() {
                            if let Ok(mib) = parts[0].parse::<f64>() {
                                app.metrics.gpu_mem_used = (mib * 1024.0 * 1024.0) as u64;
                            }
                        }
                    }
                }
                server_logs.push(line);
                if server_logs.len() > 100 { break; } // limit batch size
            }
        }
        if !server_logs.is_empty() {
            for line in server_logs {
                app.add_log(line, crate::config::LogLevel::Info);
            }
            app.set_redraw();
        }

        // Poll model state synchronization channel
        let mut sync_updated = false;
        if let Some(rx) = &mut app.sync_rx {
            while let Ok(models) = rx.try_recv() {
                if let Some(handle) = &app.server_handle {
                    let port = handle.port;
                    let pid = handle.pid;
                    for (id, status, path) in models {
                        // Robust matching: check path, display_name, or filename
                        let status_lower = status.to_lowercase();
                        let is_active = status_lower == "loaded" || status_lower == "loading" || status_lower == "ready";
                        
                        let mut matched = false;
                        for model in &app.models {
                            let path_match = path.as_ref().map(|p| p == &model.path.to_string_lossy()).unwrap_or(false);
                            let id_match = id == model.display_name || id == model.name;
                            
                            // Also try matching by filename extracted from server path/ID
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
                                             app.model_states.insert(model.display_name.clone(), crate::models::ModelState::Loading);
                                         } else {
                                             // Ensure it's in the loaded list
                                             let mut loaded_names = app.loaded_model_names.lock().unwrap();
                                             if !loaded_names.contains(&model.display_name) {
                                                 loaded_names.push(model.display_name.clone());
                                             }
                                             app.model_states.insert(model.display_name.clone(), crate::models::ModelState::Loaded { port, pid });
                                         }
                                    }
                                    // NEVER mark as Available here. 
                                    // Unloading is handled explicitly in the pending_api_unload block.
                                    matched = true;
                                }
                        }
                        
                        // If no direct match found, try fuzzy filename matching as last resort
                        if !matched {
                            let possible_names = vec![id.clone(), format!("{}.gguf", id)];
                            for name in possible_names {
                                for model in &app.models {
                                    if model.display_name == name || model.name == name {
                                         if is_active {
                                              // Ensure it's in the loaded list
                                              let mut loaded_names = app.loaded_model_names.lock().unwrap();
                                              if !loaded_names.contains(&model.display_name) {
                                                  loaded_names.push(model.display_name.clone());
                                              }
                                              app.model_states.insert(model.display_name.clone(), crate::models::ModelState::Loaded { port, pid });
                                          }
                                          // NEVER mark as Available here.
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
            app.set_redraw();
        }

        // Poll metrics channel
        if let Some(rx) = &mut app.metrics_rx {
            let mut received_metrics = false;
            while let Ok(mut m) = rx.try_recv() {
                // Fallback for ctx_max if server returns 0
                if m.ctx_max == 0 {
                    m.ctx_max = app.settings.context_length;
                }
                // Preserve log-parsed TPS if endpoint returns 0
                if m.tps == 0.0 && app.metrics.tps > 0.0 {
                    m.tps = app.metrics.tps;
                }
                // Prefer log-parsed context usage over endpoint when both are available,
                // so the display reflects actual inference state (including compaction drops).
                if app.metrics.ctx_used > 0 {
                    m.ctx_used = app.metrics.ctx_used;
                }
                // Preserve log-parsed VRAM if endpoint returns 0
                if m.gpu_mem_used == 0 && app.metrics.gpu_mem_used > 0 {
                    m.gpu_mem_used = app.metrics.gpu_mem_used;
                    if m.gpu_mem_total == 0 {
                        m.gpu_mem_total = app.metrics.gpu_mem_total;
                    }
                }
                app.metrics = m;
                received_metrics = true;
            }
            if received_metrics {
                app.set_redraw();
            }
        }

        // Handle pending search loading (pagination)
        if app.search_loading {
            if let Some((query, offset)) = app.pending_search_load.take() {
                let is_append = offset > 0;
                let query_clone = if is_append { Some(query.clone()) } else { None };
                let offset_clone = offset;
                let search_handle = tokio::spawn(async move {
                    hub::search_models(&query_clone.unwrap_or_default(), 70, offset_clone).await
                });

                match search_handle.await {
                    Ok(Ok((res, _))) => {
                        if is_append {
                            let res_len = res.len();
                            if let ModelsMode::Search { results, has_more, loading, .. } = &mut app.models_mode {
                                results.extend(res);
                                app.search_results_idx = Some(results.len().saturating_sub(1));
                                if res_len < 50 {
                                    *has_more = false;
                                }
                                *loading = false;
                            }
                        } else {
                            if let ModelsMode::Search { results, loading, .. } = &mut app.models_mode {
                                *results = res;
                                app.search_results_idx = Some(0);
                                *loading = false;
                            }
                        }
                        app.add_log("Search complete", crate::config::LogLevel::Info);
                    }
                    Ok(Err(e)) => {
                        app.add_log(&format!("Search failed: {}", e), crate::config::LogLevel::Error);
                        if let ModelsMode::Search { loading, .. } = &mut app.models_mode {
                            *loading = false;
                        }
                    }
                    Err(e) => {
                        app.add_log(&format!("Search task error: {}", e), crate::config::LogLevel::Error);
                        if let ModelsMode::Search { loading, .. } = &mut app.models_mode {
                            *loading = false;
                        }
                    }
                }
            }
            app.search_loading = false;
            app.set_redraw();
        }

        if app.needs_redraw {
            terminal.draw(|frame| tui::render::render(frame, &mut app))?;
            app.needs_redraw = false;
        }

        // Wait for an event with adaptive timeout
        // Use shorter timeout when actively downloading or server is running
        let poll_timeout = if app.downloading || app.server_handle.is_some() {
            std::time::Duration::from_millis(50)
        } else {
            std::time::Duration::from_millis(200)
        };
        
        if crossterm::event::poll(poll_timeout)? {
            if let Ok(event) = crossterm::event::read() {
                match event {
                    crossterm::event::Event::Key(key) => {
                        if key.kind != crossterm::event::KeyEventKind::Release {
                            tui::event::handle_key(&mut app, key).await;
                            app.set_redraw();
                        }
                    }
                    crossterm::event::Event::Mouse(mouse) => {
                        let size = terminal.size()?;
                        tui::event::handle_mouse(&mut app, mouse, ratatui::layout::Rect::new(0, 0, size.width, size.height));
                        // Only redraw on clicks or scrolls, not movement
                        match mouse.kind {
                            crossterm::event::MouseEventKind::Down(_) | 
                            crossterm::event::MouseEventKind::ScrollUp | 
                            crossterm::event::MouseEventKind::ScrollDown => {
                                app.set_redraw();
                            }
                            _ => {}
                        }
                    }
                    crossterm::event::Event::Resize(_, _) => {
                        app.set_redraw();
                    }
                    _ => {}
                }
            }
        }

        if !app.running {
            break;
        }
    }

 // Cleanup before exit: kill running server and background tasks
    println!("Shutting down all processes...");
    if let Some(handle) = app.server_handle.take() {
        let _ = server::kill_server(handle).await;
    }
    if let Some(task) = app.metrics_task_handle.take() {
        task.abort();
    }
   if let Some(task) = app.spawn_task_handle.take() {
        task.abort();
    }
   if let Some(task) = app.api_proxy_handle.take() {
        task.abort();
    }

    // Restore terminal
    crossterm::execute!(
        std::io::stdout(),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture,
    )?;
    crossterm::terminal::disable_raw_mode()?;

        Ok(())
        }
    }
}

/// Scan a directory (recursively) for .gguf model files.
fn discover_models(dir: &std::path::Path) -> Vec<DiscoveredModel> {
    let mut models = Vec::new();
    walk_dir(dir, dir, &mut models);
    models.sort_by(|a, b| a.name.cmp(&b.name));
    models
}

fn walk_dir(dir: &std::path::Path, base: &std::path::Path, models: &mut Vec<DiscoveredModel>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().map(|e| e == "gguf").unwrap_or(false) {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    let name = name.to_string();
                    let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                    // Compute display name: relative path from base directory.
                    let display_name = path
                        .strip_prefix(base)
                        .ok()
                        .and_then(|p| p.to_str())
                        .unwrap_or(&name)
                        .to_string();
                    models.push(DiscoveredModel {
                        path,
                        name,
                        file_size: size,
                        display_name,
                    });
                }
            } else if path.is_dir() {
                walk_dir(&path, base, models);
            }
        }
    }
}

fn resolve_models_dir(cli_value: &Option<String>) -> PathBuf {
    match cli_value {
        Some(p) => PathBuf::from(p),
        None => {
            let home = dirs::home_dir()
                .expect("could not determine home directory");
            home.join(".local/share/llm-manager/models")
        }
    }
}

