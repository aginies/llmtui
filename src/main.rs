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
                App::discover_models(&models_dir)
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
                app.update_metrics_model_name();
                app.start_pending_download().await;
                if let Some(path) = app.pending_deletion.take() {
                    app.start_pending_deletion(path).await;
                }
                if let Some((backend, tag)) = app.pending_backend_deletion.take() {
                    app.start_pending_backend_deletion(backend, tag);
                }
                app.poll_backend_resolution().await;
                app.start_pending_spawn().await;
                app.poll_spawn_result().await;
                app.poll_bench_tune_result().await;
                app.handle_pending_api_load();
                app.handle_pending_api_unload();
                app.start_pending_kill().await;
                app.poll_download_progress();
                app.poll_bench_tune_progress();
                app.process_completed_downloads();
                app.poll_server_logs();
                app.poll_sync();
                app.poll_metrics();
                app.handle_pending_search().await;
                app.tick_spinner();
                app.render(&mut terminal)?;

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

fn resolve_models_dir(cli_value: &Option<String>) -> PathBuf {
    match cli_value {
        Some(p) => PathBuf::from(p),
        None => {
            let home = dirs::home_dir()
                .or_else(|| std::env::current_dir().ok())
                .unwrap_or_default();
            home.join(".local/share/llm-manager/models")
        }
    }
}

