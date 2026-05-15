pub mod app;
pub mod routes;
pub mod ws;
pub mod upgrade;

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;
use tracing::{info, warn};
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use uuid::Uuid;

use crate::config::Config;
use crate::models::{DownloadState, ServerMetrics};

/// Shared application state for the HTTP server.
pub use app::AppState;

/// A llama.cpp server process managed by the HTTP server.
/// This is equivalent to `ServerHandle` in `backend/server.rs` but
/// owned by the HTTP server rather than the TUI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerProcess {
    pub port: u16,
    pub host: String,
    pub pid: u32,
}

/// State of the HTTP server itself.
#[allow(dead_code)]
#[derive(Debug, PartialEq, Eq)]
pub enum ServerState {
    Stopped,
    Starting,
    Running {
        process: Arc<ServerProcess>,
    },
    Error {
        message: String,
    },
}

impl Default for ServerState {
    fn default() -> Self {
        Self::Stopped
    }
}

/// Represents a model loaded/available on the HTTP server.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelState {
    Available,
    Loading,
    Loaded {
        port: u16,
        pid: u32,
    },
    Failed {
        error: String,
    },
}

impl Default for ModelState {
    fn default() -> Self {
        Self::Available
    }
}

/// A discovered model file, mirroring `DiscoveredModel` from `models.rs`.
#[derive(Debug, Clone, Serialize)]
pub struct DiscoveredModel {
    pub path: PathBuf,
    pub name: String,
    pub file_size: u64,
    pub display_name: String,
}

/// The HTTP server that exposes the management API.
pub struct Server {
    config: Config,
    bind_addr: SocketAddr,
    api_key: Option<String>,
    server_uuid: String,

    // Model registry
    models: std::sync::Arc<std::sync::Mutex<Vec<DiscoveredModel>>>,

    // llama-server process state
    server_state: std::sync::Arc<std::sync::RwLock<ServerState>>,

    // Model states
    model_states: std::sync::Arc<std::sync::RwLock<std::collections::HashMap<String, ModelState>>>,
    loaded_model_names: std::sync::Arc<std::sync::Mutex<Vec<String>>>,

    // Settings
    settings: std::sync::Arc<std::sync::RwLock<crate::models::ModelSettings>>,

    // Metrics
    metrics: std::sync::Arc<std::sync::RwLock<ServerMetrics>>,

    // Server logs (recent entries)
    log_entries: std::sync::Arc<std::sync::Mutex<VecDeque<String>>>,
    log_tx: std::sync::Arc<std::sync::Mutex<Option<tokio::sync::mpsc::Sender<String>>>>,

    // Downloads
    downloads: std::sync::Arc<std::sync::RwLock<std::collections::HashMap<String, DownloadState>>>,

    // WebSocket clients
    ws_clients: std::sync::Arc<std::sync::RwLock<std::collections::HashMap<String, tokio::sync::mpsc::Sender<serde_json::Value>>>>,
}

#[allow(dead_code)]
impl Server {
    pub fn new(config: Config, bind: SocketAddr, api_key: Option<String>) -> Self {
        // Discover models from models directory
        let models: Vec<DiscoveredModel> = crate::models::discover_models(&config.models_dir)
            .into_iter()
            .map(|m| DiscoveredModel {
                path: m.path,
                name: m.name,
                file_size: m.file_size,
                display_name: m.display_name,
            })
            .collect();
        info!("Discovered {} models for HTTP server", models.len());

        Self {
            config,
            bind_addr: bind,
            api_key,
            server_uuid: Uuid::new_v4().to_string(),
            models: std::sync::Arc::new(std::sync::Mutex::new(models)),
            server_state: std::sync::Arc::new(std::sync::RwLock::new(ServerState::Stopped)),
            model_states: std::sync::Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
            loaded_model_names: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            settings: std::sync::Arc::new(std::sync::RwLock::new(crate::models::ModelSettings::default())),
            metrics: std::sync::Arc::new(std::sync::RwLock::new(ServerMetrics::default())),
            log_entries: std::sync::Arc::new(std::sync::Mutex::new(VecDeque::new())),
            log_tx: std::sync::Arc::new(std::sync::Mutex::new(None)),
            downloads: std::sync::Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
            ws_clients: std::sync::Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        }
    }

    pub fn bind_addr(&self) -> SocketAddr {
        self.bind_addr
    }

    pub fn server_uuid(&self) -> &str {
        &self.server_uuid
    }

    /// Build the axum Router with all routes.
    pub fn router(&self) -> axum::Router {
        let state = AppState {
            config: Arc::new(self.config.clone()),
            server_uuid: self.server_uuid.clone(),
            models: Arc::clone(&self.models),
            server_state: Arc::clone(&self.server_state),
            model_states: Arc::clone(&self.model_states),
            loaded_model_names: Arc::clone(&self.loaded_model_names),
            settings: Arc::clone(&self.settings),
            metrics: Arc::clone(&self.metrics),
            log_entries: Arc::clone(&self.log_entries),
            log_tx: Arc::clone(&self.log_tx),
            downloads: Arc::clone(&self.downloads),
            ws_clients: Arc::clone(&self.ws_clients),
            api_key: self.api_key.clone(),
        };

        let ws_handler = ws::ws_handler;

        let middleware = ServiceBuilder::new()
            .layer(TraceLayer::new_for_http());

        axum::Router::new()
            .route("/api/v1/ws", axum::routing::get(ws_handler))
            .with_state(state.clone())
            .merge(routes::create_routes(state))
            .layer(middleware)
            .layer(tower_http::cors::CorsLayer::permissive())
    }

    /// Run the axum server (blocking).
    pub async fn run(&self) -> Result<(), std::io::Error> {
        let listener = tokio::net::TcpListener::bind(self.bind_addr).await?;
        info!("HTTP server listening on {}", self.bind_addr);
        axum::serve(listener, self.router()).await
    }

    async fn run_with_state(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let listener = tokio::net::TcpListener::bind(self.bind_addr).await?;
        info!("HTTP server listening on {}", self.bind_addr);
        if let Err(e) = axum::serve(listener, self.router()).await {
            warn!("HTTP server stopped with error: {}", e);
        }
        info!("HTTP server shut down");
        Ok(())
    }
}

/// Start the HTTP server as a background task.
/// Returns the server instance and a JoinHandle.
pub async fn start_server(
    config: Config,
    bind: Option<String>,
    api_key: Option<String>,
) -> Result<(Arc<Server>, tokio::task::JoinHandle<()>), anyhow::Error> {
    let bind_addr = match bind {
        Some(addr) => addr.parse()?,
        None => "127.0.0.1:49222".parse()?,
    };

    let server = Arc::new(Server::new(config, bind_addr, api_key));
    let has_key = server.api_key.is_some();
    info!("HTTP server starting on http://{}", bind_addr);
    if has_key {
        info!("HTTP server: API key authentication is ENABLED");
    } else {
        info!("HTTP server: no API key set (auth disabled)");
    }

    let server_clone = Arc::clone(&server);
    let handle = tokio::spawn(async move {
        if let Err(e) = server_clone.run().await {
            eprintln!("HTTP server error: {}", e);
        }
    });

    Ok((server, handle))
}
