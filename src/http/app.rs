use std::collections::VecDeque;
use std::sync::Arc;

use crate::models::{DownloadState, ServerMetrics};

/// Shared application state passed to all route handlers via `axum::Extension`.
#[allow(dead_code)]
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<crate::config::Config>,
    pub server_uuid: String,
    pub models: Arc<std::sync::Mutex<Vec<crate::http::DiscoveredModel>>>,
    pub server_state: Arc<std::sync::RwLock<crate::http::ServerState>>,
    pub model_states: Arc<std::sync::RwLock<std::collections::HashMap<String, crate::http::ModelState>>>,
    pub loaded_model_names: Arc<std::sync::Mutex<Vec<String>>>,
    pub settings: Arc<std::sync::RwLock<crate::models::ModelSettings>>,
    pub metrics: Arc<std::sync::RwLock<ServerMetrics>>,
    pub log_entries: Arc<std::sync::Mutex<VecDeque<String>>>,
    pub log_tx: Arc<std::sync::Mutex<Option<tokio::sync::mpsc::Sender<String>>>>,
    pub downloads: Arc<std::sync::RwLock<std::collections::HashMap<String, DownloadState>>>,
    pub ws_clients: Arc<std::sync::RwLock<std::collections::HashMap<String, tokio::sync::mpsc::Sender<serde_json::Value>>>>,
    pub api_key: Option<String>,
}

#[allow(dead_code)]
impl AppState {
    /// Broadcast an event to all connected WebSocket clients.
    pub fn broadcast(&self, event: &str, data: serde_json::Value) {
        let msg = serde_json::json!({
            "type": event,
            "data": data,
        });

        let count = {
            let mut clients = self.ws_clients.write().unwrap();
            let before = clients.len();
            clients.retain(|_id, tx| {
                let _ = tx.try_send(msg.clone());
                true
            });
            before
        };
        tracing::info!("WS broadcast {}: {} clients", event, count);
    }
}
