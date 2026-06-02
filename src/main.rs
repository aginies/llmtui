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

use crate::backend::server;
use crate::config::Config;
use crate::models::Backend;
use crate::tui::app::App;
use tracing_subscriber::prelude::*;

#[derive(Parser)]
#[command(name = "llm-manager", about = "Manage and chat with local LLMs")]
enum Cli {
    /// Manage and chat with local LLMs (TUI mode, default)
    #[command(name = "tui", about = "Start the terminal UI")]
    Tui {
        /// Path to models directory (can be specified multiple times)
        #[arg(short, long)]
        models_dirs: Option<Vec<String>>,

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

        /// Enable the WebSocket dashboard server
        #[arg(long)]
        ws_enable: bool,

        /// Port for the WebSocket dashboard server
        #[arg(long)]
        ws_port: Option<u16>,

        /// Auth key for the WebSocket dashboard server
        #[arg(long)]
        ws_auth: Option<String>,

        /// Path to a custom llama-server binary to use instead of auto-resolved
        #[arg(long)]
        backend_binary: Option<String>,

        /// Host to bind the API proxy and WebSocket servers to (default: 127.0.0.1)
        #[arg(long)]
        host: Option<String>,

        /// Log file path (default: stdout, useful for systemd)
        #[arg(long)]
        log_file: Option<String>,

        /// Enable TLS for the WebSocket dashboard and API servers (auto-generates self-signed certs)
        #[arg(long)]
        tls_enable: bool,

        /// Path to TLS certificate PEM file
        #[arg(long)]
        tls_cert: Option<String>,

        /// Path to TLS private key PEM file
        #[arg(long)]
        tls_key: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli {
        Cli::Serve {
            model,
            profile,
            config,
            api_port,
            api_key,
            ws_enable,
            ws_port,
            ws_auth,
            backend_binary,
            host,
            log_file,
            tls_enable,
            tls_cert,
            tls_key,
        } => {
            // For serve mode, log to stdout or file
            if let Some(path) = &log_file {
                let path = PathBuf::from(path);
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent).ok();
                }
                let file = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&path)
                    .expect("Failed to open log file");
                tracing_subscriber::registry()
                    .with(tracing_subscriber::fmt::layer().with_writer(file))
                    .with(
                        tracing_subscriber::EnvFilter::from_default_env()
                            .add_directive("llm_manager=info".parse().unwrap()),
                    )
                    .init();
            } else {
                tracing_subscriber::registry()
                    .with(tracing_subscriber::fmt::layer())
                    .with(
                        tracing_subscriber::EnvFilter::from_default_env()
                            .add_directive("llm_manager=info".parse().unwrap()),
                    )
                    .init();
            }

            serve::serve_model(serve::ServeOptions {
                model_path: model,
                profile_name: profile,
                config_path: config,
                api_port,
                api_key,
                ws_enable,
                ws_port,
                ws_auth,
                backend_binary,
                host,
                tls_enable,
                tls_cert,
                tls_key,
            })
            .await
            .map_err(|e| {
                tracing::error!("{}", e);
                e
            })
        }
        Cli::Tui {
            models_dirs: cli_models_dirs,
            llama_server,
            backend,
            config,
        } => {
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
                .with(
                    tracing_subscriber::EnvFilter::from_default_env()
                        .add_directive("llm_manager=info".parse().unwrap()),
                )
                .init();

            info!("Logging to {}", log_path.display());

            let config_path = config.map(PathBuf::from).unwrap_or(Config::config_path());

            // Load or create config
            let mut config = if config_path.exists() {
                Config::load_from(config_path)
                    .map_err(|e| anyhow::anyhow!("Failed to load config: {}", e))?
            } else {
                Config { llama_server: PathBuf::from(&llama_server), ..Default::default() }
            };

            // If CLI models_dirs are provided, override the config ones
            if let Some(dirs) = cli_models_dirs {
                config.models_dirs = resolve_models_dirs(&Some(dirs));
            }

            // Apply CLI backend override
            let backend = Backend::from_str(&backend);
            config.default.backend = backend;

            // Ensure models directories exist
            for dir in &config.models_dirs {
                std::fs::create_dir_all(dir)?;
            }

            // Discover models asynchronously
            let models_dirs = config.models_dirs.clone();
            let models = tokio::task::spawn_blocking(move || App::discover_models(&models_dirs))
                .await
                .unwrap_or_default();

            info!("Discovered {} models", models.len());

            let mut app = App::new(config);
            app.models = models;
            if !app.models.is_empty() {
                app.selected_model_idx = Some(0);
                app.on_model_selection_change();
            }

            // WebSocket metrics channel
            let (ws_metrics_tx, _) = tokio::sync::broadcast::channel(64);
            app.server.metrics_tx = Some(ws_metrics_tx);

            // Setup terminal
            crossterm::terminal::enable_raw_mode().map_err(|e| {
                anyhow::anyhow!(
                    "Failed to enable raw terminal mode (are you running in a TTY?): {}",
                    e
                )
            })?;
            crossterm::execute!(
                std::io::stdout(),
                crossterm::terminal::EnterAlternateScreen,
                crossterm::event::EnableMouseCapture,
            )?;
            crossterm::execute!(
                std::io::stdout(),
                crossterm::terminal::Clear(crossterm::terminal::ClearType::All)
            )?;

            let mut terminal =
                ratatui::Terminal::new(ratatui::backend::CrosstermBackend::new(std::io::stdout()))?;

            // Main event loop
            loop {
                app.update_metrics_model_name();
                app.start_pending_download().await;
                if let Some(path) = app.pending.pending_deletion.take() {
                    app.start_pending_deletion(path).await;
                }
                if let Some((backend, tag)) = app.pending.pending_backend_deletion.take() {
                    app.start_pending_backend_deletion(backend, tag);
                }
                app.poll_backend_resolution().await;
                app.start_pending_spawn().await;
                app.poll_spawn_result().await;
                app.poll_bench_tune_result().await;
                app.handle_server_exit();
                app.handle_pending_api_load();
                app.handle_pending_api_unload();
                app.start_pending_kill().await;
                app.poll_download_progress();
                app.poll_bench_tune_progress();
                app.process_completed_downloads();
                app.poll_server_logs();
                app.poll_loading_completion().await;
                app.poll_sync();
                app.poll_metrics();

                // Send metrics snapshot to WebSocket clients
                if let Some(tx) = &app.server.metrics_tx {
                    // Try to get the first actually loaded model name
                    let loaded_model_name = {
                        let names = app
                            .server
                            .loaded_model_names
                            .lock()
                            .unwrap_or_else(|e| e.into_inner());
                        names.first().cloned()
                    };

                    let model_name = loaded_model_name
                        .as_deref()
                        .or(app.server.spawned_model_name.as_deref())
                        .unwrap_or("");

                    let state = if !model_name.is_empty() {
                        if app.is_model_loaded(model_name) {
                            "loaded"
                        } else if app.is_loading() {
                            "loading"
                        } else {
                            "unloaded"
                        }
                    } else {
                        "unloaded"
                    };

                    let settings = app
                        .server
                        .spawned_settings
                        .as_ref()
                        .unwrap_or(&app.settings);

                    if let Err(e) = tx.send(crate::models::WsMetrics::from_metrics(
                        &app.metrics,
                        model_name,
                        state,
                        settings,
                        app.server.cmd_display.as_deref(),
                    )) {
                        tracing::debug!("Failed to send metrics to ws: {e}");
                    }
                }

                app.handle_pending_search().await;
                app.update_ws_server().await;
                app.update_api_endpoint().await;
                app.tick_spinner();
                app.render(&mut terminal)?;

                let poll_timeout = if app.download.downloading || app.server.server_handle.is_some()
                {
                    std::time::Duration::from_millis(50)
                } else {
                    std::time::Duration::from_millis(200)
                };

                if crossterm::event::poll(poll_timeout)?
                    && let Ok(event) = crossterm::event::read()
                {
                    match event {
                        crossterm::event::Event::Key(key) => {
                            if key.kind != crossterm::event::KeyEventKind::Release {
                                tui::event::handle_key(&mut app, key).await;
                            }
                        }
                        crossterm::event::Event::Mouse(mouse) => {
                            let size = terminal.size()?;
                            tui::event::handle_mouse(
                                &mut app,
                                mouse,
                                ratatui::layout::Rect::new(0, 0, size.width, size.height),
                            );
                            match mouse.kind {
                                crossterm::event::MouseEventKind::Down(_)
                                | crossterm::event::MouseEventKind::ScrollUp
                                | crossterm::event::MouseEventKind::ScrollDown => {}
                                _ => {}
                            }
                        }
                        crossterm::event::Event::Resize(_, _) => {}
                        _ => {}
                    }
                }

                if !app.running {
                    break;
                }
            }
            // Cleanup before exit: kill running server and background tasks
            tracing::info!("Shutting down all processes...");
            if let Some(handle) = app.server.server_handle.take() {
                let _ = server::kill_server(handle).await;
            }
            if let Some(task) = app.server.metrics_task_handle.take() {
                task.abort();
            }
            if let Some(task) = app.server.spawn_task_handle.take() {
                task.abort();
            }
            if let Some(task) = app.server.api_proxy_handle.take() {
                task.abort();
            }
            if let Some(handle) = app.ws_server_handle.take() {
                backend::ws_server::stop_ws_server(handle);
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

fn resolve_models_dirs(cli_value: &Option<Vec<String>>) -> Vec<PathBuf> {
    match cli_value {
        Some(dirs) => dirs.iter().map(PathBuf::from).collect(),
        None => {
            let home = dirs::home_dir()
                .or_else(|| std::env::current_dir().ok())
                .unwrap_or_default();
            vec![home.join(".local/share/llm-manager/models")]
        }
    }
}
