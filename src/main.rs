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
use std::sync::{Arc, atomic::{AtomicBool, AtomicU8}};
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

        /// Backend to use (cpu, vulkan, rocm, rocm-lemonade, cuda)
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
            let backend = Backend::from_str(&backend);
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
                    app.add_log(format!("Downloading {}...", model_id), crate::config::LogLevel::Info);
                    // Create broadcast channel if not already created (shared by all downloads)
                    if app.download_rx.is_none() {
                        let (tx, rx) = tokio::sync::broadcast::channel(10);
                        app.download_tx = Some(tx);
                        app.download_rx = Some(rx);
                    }
                    let tx = app.download_tx.as_ref().unwrap().clone();
                    let tx_clone = tx.clone();
                    let cancelled_for_state = cancelled_clone.clone();
                    let download_state = Arc::new(AtomicU8::new(1));
                    let download_state_clone = download_state.clone();

                    tokio::spawn(async move {
                        let mut state = DownloadState::new(model_id_clone.clone(), filename_clone.clone(), 0);
                        state.cancel_token = Some(cancelled_for_state);
                        state.download_state = 1;
                        state.download_state_arc = Some(download_state_clone.clone());
                        let result = hub::download_file(&model_id_clone, &filename_clone, &url_clone, &dest, &mut state, download_state_clone, tx_clone).await;
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
        if !matches!(app.global_mode, crate::tui::app::GlobalMode::Confirmation { .. }) {
            if let Some(path) = app.pending_deletion.take() {
                let path_clone = path.clone();
                tokio::spawn(async move {
                    if let Err(e) = tokio::fs::remove_file(&path_clone).await {
                        tracing::warn!("Failed to delete file: {}", e);
                    }
                });
                // Remove this model's settings override from config
                let model_key = path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                app.config.model_overrides.remove(&model_key);
                if let Err(e) = app.config.save() {
                    app.add_log(format!("Failed to save config after deletion: {}", e), crate::config::LogLevel::Error);
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
                app.add_log(format!("Model deleted: {:?}", path.file_name().unwrap_or_default()), crate::config::LogLevel::Info);
                app.set_redraw();
            }

            if let Some((backend, tag)) = app.pending_backend_deletion.take() {
                let bin_dir = crate::backend::hub::get_backend_dir(backend, &tag);
                
                if bin_dir.exists() {
                    if let Err(e) = std::fs::remove_dir_all(&bin_dir) {
                        app.add_log(format!("Failed to delete backend: {}", e), crate::config::LogLevel::Error);
                    } else {
                        app.add_log(format!("Deleted backend {} ({})", backend, tag), crate::config::LogLevel::Info);
                        
                        // Fetch new entries before borrowing global_mode as mutable
                        let new_entries = app.fetch_backend_picker_entries();
                        
                        // If we are currently in BackendPicker mode, we need to refresh the entries
                        if let crate::tui::app::GlobalMode::BackendPicker { entries, selected } = &mut app.global_mode {
                            *entries = new_entries;
                            if *selected >= entries.len() {
                                *selected = entries.len().saturating_sub(1);
                            }
                        }
                    }
                }
                app.set_redraw();
            }
        }

                // Poll backend resolution task
                if let Some(handle) = &app.backend_resolve_handle {
                    if handle.is_finished() {
                        if let Some(handle) = app.backend_resolve_handle.take() {
                            match handle.await {
                                Ok(Ok(path)) => {
                                    app.add_log(format!("Backend ready: {}", path.display()), crate::config::LogLevel::Info);
                                }
                                Ok(Err(e)) => {
                                    app.add_log(format!("Backend installation failed: {}", e), crate::config::LogLevel::Error);
                                }
                                Err(e) => {
                                    app.add_log(format!("Backend task panicked: {}", e), crate::config::LogLevel::Error);
                                }
                            }
                            app.backend_resolving = false;
                            app.set_redraw();
                        }
                    }
                }

        // Start pending server spawn
        if let Some((model_opt, settings)) = app.pending_spawn.take() {
            let (tx, rx) = tokio::sync::mpsc::channel(100);
            app.server_log_rx = Some(rx);

            let config_clone = app.config.clone();
            let model_clone = model_opt.clone();
            let settings_clone = settings.clone();
            let tx_clone = tx.clone();
            let server_mode_clone = app.server_mode.clone();
            let router_max_models_clone = app.router_max_models;
            
            // Ensure download channel exists so progress reporting works for backend binaries
            if app.download_rx.is_none() {
                let (tx, rx) = tokio::sync::broadcast::channel(10);
                app.download_tx = Some(tx);
                app.download_rx = Some(rx);
            }
            let download_tx_clone = app.download_tx.clone();

            let display_name = model_opt.as_ref().map(|m| m.display_name.clone()).unwrap_or_else(|| "Router".to_string());
            if let Some(m) = &model_opt {
                let state = if server_mode_clone == crate::models::ServerMode::Bench {
                    crate::models::ModelState::Benchmarking
                } else if server_mode_clone == crate::models::ServerMode::BenchTune {
                    crate::models::ModelState::Benchmarking
                } else {
                    crate::models::ModelState::Loading
                };
                app.model_states.insert(m.display_name.clone(), state);
            }
            app.add_log(format!("Loading {}...", display_name), crate::config::LogLevel::Info);

            if server_mode_clone == crate::models::ServerMode::BenchTune {
                let model = match model_opt {
                    Some(m) => m,
                    None => {
                        app.add_log("Error: Benchmark tuning requires a selected model.", crate::config::LogLevel::Error);
                        continue;
                    }
                };

               let bench_tune_config = app.bench_tune_config.take().unwrap_or_else(|| {
                    crate::models::BenchTuneConfig::new(
                        model.path.clone(),
                        3, // Default iterations
                        "Create Mona Lisa image in ascii art using text, number, symbol, everything possible. this should be the perfect painting.".to_string(),
                    )
                });
                
                let (tx_tune, rx_tune) = tokio::sync::mpsc::channel(100);
                app.bench_tune_tx = Some(tx_tune.clone());
                app.bench_tune_config = Some(bench_tune_config.clone());
                app.bench_tune_running = true;
                app.bench_tune_results.clear();
                app.bench_tune_result_row = 0;
                app.models_mode = crate::tui::app::ModelsMode::BenchTune;
                
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
                    ).await.map_err(|e| e.to_string());
                    
                    (results, display_name, bench_tune_config_clone)
                });
                
                app.bench_tune_task_handle = Some(handle);
                app.spawn_log_tx = Some(tx); // Keep using the original tx for other logs
                app.set_redraw();
                
                // Actually app will poll rx_tune in main loop
                app.bench_tune_rx = Some(rx_tune);
            } else {
                let handle = tokio::spawn(async move {
                    server::spawn_server(&config_clone, model_clone.as_ref(), &settings_clone, tx_clone, download_tx_clone, server_mode_clone, router_max_models_clone).await
                        .map(|(handle, cmd)| (display_name, handle, cmd))
                });
                app.spawn_task_handle = Some(handle);
                app.spawn_log_tx = Some(tx);
                app.set_redraw();
            }
        }
        // Check if server spawn task is done
        if let Some(handle) = &app.spawn_task_handle
            && handle.is_finished()
                && let Some(handle) = app.spawn_task_handle.take() {
                    match handle.await {
Ok(Ok((server_display_name, server_handle, _cmd))) => {
                            let port = server_handle.port;
                            let pid = server_handle.pid;
                            let host = server_handle.host.clone();
                            app.add_log(format!("Server started on port {port} (pid={pid})"), crate::config::LogLevel::Info);
                            app.server_handle = Some(server_handle);
                            
                            // Start API proxy if enabled
                            if app.settings.api_endpoint_enabled {
                                let port = app.settings.api_endpoint_port;
                                let addr: std::net::SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap_or_else(|_| "127.0.0.1:49222".parse().unwrap());
                                let model_name = server_display_name.clone();
                                let server_port = app.server_handle.as_ref().map(|h| h.port).unwrap_or(8080);
                                let pid = app.server_handle.as_ref().map(|h| h.pid).unwrap_or(0);
                                let handle = tokio::spawn(async move {
                                    let _ = crate::serve_api::start_api_server(
                                        addr, None, server_port, model_name, pid
                                    ).await;
                                });
                                app.api_proxy_handle = Some(handle);
                                app.add_log(format!("API proxy started on port {}", port), crate::config::LogLevel::Info);
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
                                    
                                    if let Some(name) = current_model
                                        && let Ok(model_metrics) = server::get_metrics(&task_host, task_port, Some(&name), Some(task_pid)).await {
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
                                    if let Ok(models) = server::list_models(&sync_host, sync_port).await
                                        && sync_tx.send(models).await.is_err() {
                                            break;
                                        }
                                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                                }
                            });
                            
                            app.sync_rx = Some(sync_rx);
                            app.sync_task_handle = Some(_sync_task_handle);
                        }
                        Ok(Err(e)) => {
                            app.loading_progress = 1.0;
                            app.add_log(format!("ERROR: Server failed: {}", e), crate::config::LogLevel::Error);
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
                            app.add_log(format!("ERROR: Spawn task panicked: {}", e), crate::config::LogLevel::Error);
                        }
                    }
                    app.set_redraw();
                }

        // Check if benchmark tuning task is done
        if let Some(handle) = &app.bench_tune_task_handle
            && handle.is_finished()
            && let Some(handle) = app.bench_tune_task_handle.take() {
                match handle.await {
                    Ok((results, display_name, bench_config)) => {
                        match results {
                            Ok(bench_results) => {
                                app.add_log(format!("Benchmark tuning completed for {} with {} results", display_name, bench_results.len()), crate::config::LogLevel::Info);
                                
                                if bench_results.is_empty() {
                                    app.add_log("No successful benchmark results were obtained. Check the Log (F6) for details on test failures.", crate::config::LogLevel::Warning);
                                } else {
                                    // Save results to file
                                    let output_dir = crate::config::Config::config_path().parent().unwrap().join("benchmarks");
                                    match crate::backend::benchmark::save_results(&bench_results, &output_dir, &bench_config).await {
                                        Ok(()) => app.add_log(format!("Results saved to {}/", output_dir.display()), crate::config::LogLevel::Info),
                                        Err(e) => app.add_log(format!("Failed to save benchmark results: {}", e), crate::config::LogLevel::Error),
                                    }
                                }

                                    // Sort results by generation TPS (descending)
                                    let mut sorted_results = bench_results;
                                    sorted_results.sort_by(|a, b| b.metrics.generation_tps.partial_cmp(&a.metrics.generation_tps).unwrap_or(std::cmp::Ordering::Equal));
                                    app.bench_tune_results = sorted_results;
                                    app.bench_tune_running = false;
                                
// Unload the model after benchmarking
                                    {
                                        // Clone data from model to release immutable borrow
                                        let (host, port, model_name, model_path_str, task_name, model_display_name) = {
                                            let model = match app.selected_model() {
                                                Some(m) => m,
                                                None => continue,
                                            };
                                            let handle = match &app.server_handle {
                                                Some(h) => h,
                                                None => continue,
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
                                        // Model reference dropped here, now we can do mutable operations
                                        let task_handle = tokio::spawn(async move {
                                            let _ = server::unload_model(&host, port, &model_name, model_path_str.as_deref()).await;
                                        });
                                        app.background_tasks.insert(task_name, task_handle);
                                        app.model_states.insert(model_display_name, crate::models::ModelState::Available);
                                    }
                            }
                            Err(e) => {
                                app.add_log(format!("Benchmark tuning failed: {}", e), crate::config::LogLevel::Error);
                                app.bench_tune_running = false;
                                
                                // Update model state to Failed
                                if let Some(model) = app.selected_model() {
                                    app.model_states.insert(model.display_name.clone(), crate::models::ModelState::Failed { error: e.to_string() });
                                }
                            }
                        }
                    }
                    Err(e) => {
                        app.add_log(format!("Benchmark task panicked: {:?}", e), crate::config::LogLevel::Error);
                        app.bench_tune_running = false;
                    }
                }
                app.set_redraw();
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
                    app.add_log(format!("Sending load request for {}...", model_name_clone), crate::config::LogLevel::Info);
                    
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
        if !matches!(app.global_mode, crate::tui::app::GlobalMode::Confirmation { .. }) {
            if let Some((model_name, model_path)) = app.pending_api_unload.take()
                && let Some(handle) = &app.server_handle {
                let host = handle.host.clone();
                let port = handle.port;
                let model_name_clone = model_name.clone();
                let model_path_clone = model_path.clone();
                let server_mode = app.server_mode;
                let handle_clone = handle.clone();
                
                // Clear metrics model name
                {
                    let mut lock = app.metrics_model_name.lock().unwrap();
                    if lock.as_deref() == Some(&model_name_clone) {
                        *lock = None;
                    }
                }

                // In normal mode, the model was loaded via CLI -m flag, so the API
                // /models/unload endpoint cannot unload it. Kill the server instead.
                if server_mode == crate::models::ServerMode::Normal {
                    app.add_log(format!("Unloading {} (killing server)...", model_name_clone), crate::config::LogLevel::Info);
                    app.pending_kill = Some(handle_clone);
                } else {
                    app.add_log(format!("Sending unload request for {}...", model_name_clone), crate::config::LogLevel::Info);
                    
                    let kill_tx = app.spawn_log_tx.clone();
                    let kill_tx2 = kill_tx.clone();
                    let server_clone = app.server_handle.clone();
                    let host_clone = host.clone();
                    let port_clone = port;
                    let model_name_task = model_name_clone.clone();
                    
                    app.background_tasks.insert(
                        format!("api_unload_{}", model_name_task),
                        tokio::spawn(async move {
                            if let Err(e) = server::unload_model(&host, port, &model_name_clone, model_path_clone.as_deref()).await {
                                if let Some(tx) = kill_tx {
                                    let _ = tx.send(format!("Failed to unload model via API: {}", e)).await;
                                }
                                return;
                            }
                            
                            // Wait for the server to finish unloading
                            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                            
                            // Check what's actually loaded on the server
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
                
                app.loaded_model_names.lock().unwrap().retain(|n| n != &model_name);
                app.model_states.insert(model_name, crate::models::ModelState::Available);
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
                    app.add_log(format!("Failed to stop server: {}", e), crate::config::LogLevel::Error);
                }
            }
            app.set_redraw();
        }

        // Poll download channel for progress
        let mut redraw = false;
        let mut download_logs = Vec::new();
        if let Some(rx) = &mut app.download_rx {
            while let Ok(state) = rx.try_recv() {
                if let Some(idx) = app.download_progress.iter().position(|d| {
                    d.model_id == state.model_id && d.filename == state.filename
                }) {
                    // Log download progress if total_bytes is known
                    if state.total_bytes > 0 {
                        let old_pct = (app.download_progress[idx].downloaded_bytes as f32 / app.download_progress[idx].total_bytes as f32 * 100.0) as u32;
                        let new_pct = (state.downloaded_bytes as f32 / state.total_bytes as f32 * 100.0) as u32;
                        if new_pct / 5 > old_pct / 5 && new_pct < 100 {
                            let speed_mib = state.bytes_per_second / (1024.0 * 1024.0);
                            let total_mib = state.total_bytes as f64 / (1024.0 * 1024.0);
                            let name = if state.model_id == "llama-server" { "backend" } else { &state.filename };
                            download_logs.push(format!("Downloading {}: {}% of {:.1} MiB ({:.2} MiB/s)...", name, new_pct, total_mib, speed_mib));
                        }
                    }

                    app.download_progress[idx] = state;
                } else {
                    if state.model_id == "llama-server" {
                        download_logs.push("Starting backend download...".to_string());
                    } else {
                        download_logs.push(format!("Starting download: {}...", state.filename));
                    }
                    app.download_progress.push(state);
                }
                redraw = true;
            }
        }
        for log in download_logs {
            app.add_log(log, crate::config::LogLevel::Info);
        }
        if redraw {
            app.set_redraw();
        }

        // Poll benchmark tuning progress
        if let Some(mut rx) = app.bench_tune_rx.take() {
            while let Ok(status) = rx.try_recv() {
                app.bench_tune_progress = crate::models::BenchTuneProgress::from_status(&status);
                app.set_redraw();
            }
            app.bench_tune_rx = Some(rx);
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
                        if state.model_id == "llama-server" {
                            app.add_log("Backend download complete", crate::config::LogLevel::Info);
                        } else {
                            app.add_log(format!("Download complete: {}", state.filename), crate::config::LogLevel::Info);
                            app.models = discover_models(&app.config.models_dir);
                        }
                    }
                    crate::models::DownloadStatus::Error(e) => {
                        let name = if state.model_id == "llama-server" { "Backend" } else { &state.filename };
                        app.add_log(format!("Download failed ({}): {}", name, e), crate::config::LogLevel::Error);
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
            } else if let Some(idx) = app.download_scroll_state.selected()
                && idx >= app.download_progress.len() {
                    app.download_scroll_state.select(Some(app.download_progress.len() - 1));
                }
            app.set_redraw();
        }

        // Poll server log channel
        let mut server_logs = Vec::new();
        if let Some(rx) = &mut app.server_log_rx {
            while let Ok(line) = rx.try_recv() {
                // Parse TPS from logs if present
                if line.contains("tokens per second")
                    && let Some(tps_part) = line.split("tokens per second").next()
                        && let Some(val_str) = tps_part.split_whitespace().last()
                            && let Ok(tps) = val_str.parse::<f64>() {
                                if line.contains("prompt eval time =") {
                                    app.metrics.prompt_tps = tps;
                                } else if line.contains("eval time =") {
                                    app.metrics.tps = tps;
                                }
                            }
                // Parse Context Usage from logs: "n_tokens = 12667"
                // Don't use max() — after compaction the token count drops, and we
                // want the display to reflect the current state.
                if line.contains("n_tokens =")
                    && let Some(tokens_part) = line.split("n_tokens =").last() {
                        let val_str = tokens_part.split(',').next().unwrap_or(tokens_part).trim();
                        if let Ok(tokens) = val_str.parse::<u32>() && tokens > 2048 {
                            app.metrics.ctx_used = tokens;
                        }
                    }
                // Parse VRAM (KV Cache) from logs: "Vulkan0 KV buffer size =  1008.00 MiB"
                if line.contains("KV buffer size =")
                    && let Some(size_part) = line.split('=').next_back() {
                        let parts: Vec<&str> = size_part.split_whitespace().collect();
                        if !parts.is_empty()
                            && let Ok(mib) = parts[0].parse::<f64>() {
                                app.metrics.gpu_mem_used = (mib * 1024.0 * 1024.0) as u64;
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
                // ctx_used stays 0 when the model is idle, so the display shows 0/{ctx_max}.
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
                let query_clone = query.clone();
                let offset_clone = offset;
                app.add_log(format!("Searching with limit={} offset={}...", app.config.search_limit, offset_clone), crate::config::LogLevel::Info);
                let search_handle = tokio::spawn(async move {
                    hub::search_models(&query_clone, app.config.search_limit, offset_clone).await
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
                            if let ModelsMode::Search { results, has_more, loading, .. } = &mut app.models_mode {
                                results.extend(res);
                                if raw_len < app.config.search_limit as usize {
                                    *has_more = false;
                                }
                                *loading = false;
                            }
                        } else {
                            if let ModelsMode::Search { results, loading, has_more, .. } = &mut app.models_mode {
                                *results = res;
                                if !results.is_empty() {
                                    app.search_results_idx = Some(0);
                                } else {
                                    app.search_results_idx = None;
                                }
                                *has_more = raw_len >= app.config.search_limit as usize;
                                *loading = false;
                            }
                        }
                        app.add_log(buf, crate::config::LogLevel::Info);
                    }
                    Ok(Err(e)) => {
                        app.add_log(format!("Search failed: {}", e), crate::config::LogLevel::Error);
                        if let ModelsMode::Search { loading, .. } = &mut app.models_mode {
                            *loading = false;
                        }
                    }
                    Err(e) => {
                        app.add_log(format!("Search task error: {}", e), crate::config::LogLevel::Error);
                        if let ModelsMode::Search { loading, .. } = &mut app.models_mode {
                            *loading = false;
                        }
                    }
                }
            }
            app.search_loading = false;
            app.set_redraw();
        }

        // Animate spinner when model is loading but no log messages arrive
        let is_loading = app.model_states.values().any(|s| matches!(s, crate::models::ModelState::Loading));
        if is_loading {
            let spinner_interval = std::time::Duration::from_millis(150);
            
            if app.last_spinner_time.is_none() 
                || app.last_spinner_time.unwrap().elapsed() > spinner_interval {
                app.loading_spinner = (app.loading_spinner + 1) % 4;
                app.last_spinner_time = Some(tokio::time::Instant::now());
                app.set_redraw();
            }
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
        
        if crossterm::event::poll(poll_timeout)?
            && let Ok(event) = crossterm::event::read() {
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

        if !app.running {
            break;
        }
    }

    // Cleanup before exit: kill running server and background tasks
    tracing::info!("Shutting down all processes...");
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

    // Abort all background tasks
    for (_, task) in app.background_tasks.drain() {
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
    crate::backend::hub::walk_dir_recursive(dir, 0, 10, &mut |entry| {
        let path = entry.path();
        if path.is_file() && path.extension().map(|e| e == "gguf").unwrap_or(false) {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                let name = name.to_string();
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                // Compute display name: relative path from base directory.
                let display_name = path
                    .strip_prefix(dir)
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
        }
    });
    models.sort_by(|a, b| a.name.cmp(&b.name));
    models
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

