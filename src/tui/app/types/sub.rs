use crate::models::{
    BenchTuneConfig, BenchTuneProgress, BenchTuneResult, LoadProgress,
};
use crate::models::Backend;
use crate::backend::server::ServerHandle;
use crate::config::LogEntry;
use std::path::PathBuf;
use std::sync::Arc;
use std::collections::{VecDeque, BTreeMap};
use ratatui::widgets::{TableState, ListState};

use super::{ActivePanel, GlobalMode, LoadingPhase, SettingsRenderCache, ResizeState};

pub struct SettingsState {
    pub settings_selected_idx: usize,
    pub server_settings_selected_idx: usize,
    pub server_settings_scroll_offset: usize,
    pub settings_edit_buffer: String,
    pub settings_scroll_offset: usize,
    pub settings_render_cache: Option<SettingsRenderCache>,
}

pub struct PickerState {
    pub host_picker_entries: Vec<(String, String)>,
    pub host_picker_selected: usize,
    pub backend_picker_entries: Vec<(Backend, Option<String>)>,
    pub backend_picker_selected: usize,
    pub prompt_picker_entries: Vec<(String, String)>,
    pub prompt_picker_selected: usize,
    pub profile_picker_entries: Vec<(String, String)>,
    pub profile_picker_selected: usize,
    pub profiles_scroll_offset: usize,
    pub system_prompt_presets_scroll_offset: usize,
    pub rpc_workers_selected_idx: usize,
    pub editing_rpc_worker: Option<usize>,
    pub rpc_workers_scroll_offset: usize,
    pub readme_scroll_offset: usize,
}

pub struct DownloadState {
    pub download_progress: Vec<crate::models::DownloadState>,
    pub download_tx: Option<tokio::sync::broadcast::Sender<crate::models::DownloadState>>,
    pub download_rx: Option<tokio::sync::broadcast::Receiver<crate::models::DownloadState>>,
    pub download_scroll_state: TableState,
    pub downloading: bool,
}

pub struct ServerState {
    pub server_handle: Option<ServerHandle>,
    pub metrics_task_handle: Option<tokio::task::JoinHandle<()>>,
    pub sync_task_handle: Option<tokio::task::JoinHandle<()>>,
    pub spawn_task_handle: Option<tokio::task::JoinHandle<Result<(String, ServerHandle, String, crate::models::ModelSettings), String>>>,
    pub bench_tune_task_handle: Option<tokio::task::JoinHandle<(Result<Vec<BenchTuneResult>, String>, String, BenchTuneConfig)>>,
    pub server_log_rx: Option<tokio::sync::mpsc::Receiver<String>>,
    pub metrics_rx: Option<tokio::sync::mpsc::Receiver<crate::models::ServerMetrics>>,
    pub sync_rx: Option<tokio::sync::mpsc::Receiver<Vec<(String, String, Option<String>)>>>,
    pub spawn_log_tx: Option<tokio::sync::mpsc::Sender<String>>,
    pub metrics_model_name: Arc<std::sync::Mutex<Option<String>>>,
    pub loaded_model_names: Arc<std::sync::Mutex<Vec<String>>>,
    pub api_proxy_handle: Option<tokio::task::JoinHandle<()>>,
    pub metrics_tx: Option<tokio::sync::broadcast::Sender<crate::models::WsMetrics>>,
    pub cmd_display: Option<String>,
    pub spawned_settings: Option<crate::models::ModelSettings>,
    pub spawned_model_name: Option<String>,
    pub spawned_model_state: Option<String>,
    pub spawned_context_length: u32,
}

pub struct BenchTuneState {
    pub bench_tune_progress: Option<BenchTuneProgress>,
    pub bench_tune_results: Vec<BenchTuneResult>,
    pub bench_tune_running: bool,
    pub bench_tune_config: Option<BenchTuneConfig>,
    pub bench_tune_rx: Option<tokio::sync::mpsc::Receiver<crate::models::BenchTuneStatus>>,
    pub bench_tune_tx: Option<tokio::sync::mpsc::Sender<crate::models::BenchTuneStatus>>,
    pub bench_tune_output_view: Option<usize>,
    pub bench_tune_cancel_tx: Option<tokio::sync::watch::Sender<bool>>,
    pub bench_tune_output_scroll: usize,
    pub bench_tune_output_h_scroll: usize,
    pub bench_tune_result_row: usize,
    pub bench_tune_table_state: TableState,
    pub bench_tune_output_index: usize,
}

pub struct LogState {
    pub log_entries: VecDeque<LogEntry>,
    pub log_expanded: bool,
    pub log_scroll_offset: usize,
    pub log_follow: bool,
    pub log_total_lines: usize,
}

pub struct LoadingState {
    pub loading_phases: std::collections::HashSet<LoadingPhase>,
    pub last_active_phase: Option<LoadingPhase>,
    pub loading_progress: f32,
    pub progress_target: f32,
    pub load_progress: LoadProgress,
    pub last_spinner_time: Option<tokio::time::Instant>,
    pub loading_spinner: usize,
    pub model_total_layers: u32,
    pub model_hidden_size: u32,
    pub model_n_ctx_train: u32,
    pub model_n_head: u32,
    pub model_n_kv_head: u32,
    pub vram_estimate: u64,
    pub last_metadata_parse: (PathBuf, std::time::SystemTime),
}

pub struct PendingOperations {
    pub pending_download: Option<(String, String, String, u64)>,
    pub pending_deletion: Option<PathBuf>,
    pub pending_backend_deletion: Option<(Backend, String)>,
    pub pending_spawn: Option<(Option<crate::models::DiscoveredModel>, crate::models::ModelSettings)>,
    pub pending_api_load: Option<(String, Option<String>)>,
    pub pending_api_unload: Option<(String, Option<String>)>,
    pub pending_kill: Option<ServerHandle>,
    pub backend_resolving: bool,
    pub backend_resolve_handle: Option<tokio::task::JoinHandle<Result<PathBuf, String>>>,
}

pub struct SearchState {
    pub local_filter: String,
    pub filtering_local: bool,
    pub search_results_idx: Option<usize>,
    pub search_table_state: TableState,
    pub files_table_state: TableState,
    pub readme_cache: Option<(String, Vec<ratatui::text::Line<'static>>)>,
    pub gguf_metadata_cache: BTreeMap<String, crate::models::GgufMetadata>,
    pub pending_search_load: Option<(String, u32)>,
    pub search_loading: bool,
}

pub struct UIState {
    pub active_panel: ActivePanel,
    pub global_mode: GlobalMode,
    pub panel_visibility: u8,
    pub panel_help: bool,
    pub panel_help_offset: usize,
    pub last_error_message: Option<String>,
    pub needs_redraw: bool,
    pub list_state: ListState,
    pub resize_state: Option<ResizeState>,
    pub left_pct: u16,
}

pub struct EditState {
    pub edit_cursor_pos: usize,
    pub editing_n_predict: bool,
    pub n_predict_edit_buffer: String,
    pub editing_iters: bool,
    pub iters_edit_buffer: String,
    pub tags_editing: bool,
    pub tags_edit_buffer: String,
    pub tags_selected_idx: Option<usize>,
    pub tags_insert_mode: bool,
    pub editing_preset: Option<usize>,
}
