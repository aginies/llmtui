use crate::backend::server::ServerHandle;
use crate::config::LogEntry;
use crate::models::Backend;
use crate::models::{BenchTuneConfig, BenchTuneProgress, BenchTuneResult, LoadProgress};

use ratatui::widgets::TableState;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::Arc;

use super::{
    ActivePanel, GlobalMode, LoadingPhase, ResizeState, SettingsRenderCache, TextScrollState,
};

pub type SpawnTaskHandle = tokio::task::JoinHandle<
    Result<(String, ServerHandle, String, crate::models::ModelSettings), String>,
>;
pub type BenchTuneTaskHandle = tokio::task::JoinHandle<(
    Result<Vec<BenchTuneResult>, String>,
    String,
    BenchTuneConfig,
)>;
type SyncRx = tokio::sync::mpsc::Receiver<Vec<(String, String, Option<String>)>>;

pub struct SettingsState {
    pub settings_selected_idx: usize,
    pub server_settings_selected_idx: usize,
    pub server_settings_scroll_offset: usize,
    pub settings_edit_buffer: String,
    pub settings_scroll_offset: usize,
    pub settings_render_cache: Option<SettingsRenderCache>,
    pub expert_mode: bool,
    pub help_focus_time: Option<tokio::time::Instant>,
    pub help_visible: bool,
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
    pub last_progress_update: std::time::Instant,
}

pub struct ServerState {
    pub server_handle: Option<ServerHandle>,
    pub metrics_task_handle: Option<tokio::task::JoinHandle<()>>,
    pub sync_task_handle: Option<tokio::task::JoinHandle<()>>,
    pub spawn_task_handle: Option<SpawnTaskHandle>,
    pub bench_tune_task_handle: Option<BenchTuneTaskHandle>,
    pub server_log_rx: Option<tokio::sync::mpsc::Receiver<String>>,
    pub metrics_rx: Option<tokio::sync::mpsc::Receiver<crate::models::ServerMetrics>>,
    pub sync_rx: Option<SyncRx>,
    pub spawn_log_tx: Option<tokio::sync::mpsc::Sender<String>>,
    pub metrics_model_name: Arc<std::sync::Mutex<Option<String>>>,
    pub loaded_model_names: Arc<std::sync::Mutex<Vec<String>>>,
    pub api_proxy_handle: Option<tokio::task::JoinHandle<()>>,
    pub metrics_tx: Option<tokio::sync::broadcast::Sender<crate::models::WsMetrics>>,
    pub running_ws_port: Option<u16>,
    pub running_ws_auth: Option<String>,
    pub running_server_tls: Option<bool>,
    pub running_api_port: Option<u16>,
    pub running_api_server_port: Option<u16>,
    pub running_api_model: Option<String>,
    pub running_server_tls_cfg: Option<axum_server::tls_rustls::RustlsConfig>,
    pub running_server_tls_cert_path: Option<String>,
    pub running_server_tls_key_path: Option<String>,
    pub cmd_display: Option<String>,
    pub spawned_settings: Option<crate::models::ModelSettings>,
    pub spawned_model_name: Option<String>,
    pub spawned_model_state: Option<String>,
    pub spawned_context_length: u32,
    pub server_exit_rx: Option<tokio::sync::mpsc::Receiver<()>>,
    pub server_exit_tx: Option<tokio::sync::mpsc::Sender<()>>,
    pub api_shutdown_tx: Option<tokio::sync::watch::Sender<bool>>,
    /// Last time tick_server_logs ran (throttled to ~500ms).
    pub last_server_logs_tick: Option<std::time::Instant>,
    /// Last time tick_sync ran (throttled to ~1000ms).
    pub last_sync_tick: Option<std::time::Instant>,
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
    pub phase_start_time: Option<tokio::time::Instant>,
    pub model_total_layers: u32,
    pub model_hidden_size: u32,
    pub model_n_ctx_train: u32,
    pub model_n_head: u32,
    pub model_n_kv_head: u32,
    pub vram_estimate: u64,
    pub health_poll_handle: Option<tokio::task::JoinHandle<()>>,
    pub loading_completion_rx: Option<tokio::sync::mpsc::Receiver<()>>,
}

pub struct PendingOperations {
    /// API load pending — kept as field because `try_execute_api_load` uses
    /// a clone-check pattern (must survive across ticks until server is ready).
    pub pending_api_load: Option<String>,
    /// API unload pending — kept as field because confirmation dialog reads it.
    pub pending_api_unload: Option<String>,
    /// Kill handle — kept as field because the main loop needs to take() it
    /// for the async kill operation.
    pub pending_kill: Option<ServerHandle>,
    /// Backend resolution state — moved here from old PendingOperations.
   pub backend_resolving: bool,
    pub backend_resolve_handle: Option<tokio::task::JoinHandle<Result<std::path::PathBuf, String>>>,
    /// Web search health check handle.
    pub web_search_check_handle: Option<tokio::task::JoinHandle<Result<(), String>>>,
    /// Dirty flag for active_model_hint — set to true when model_states changes.
    pub active_model_hint_dirty: bool,
    /// Cached metrics model name for debouncing.
    pub metrics_model_name_cache: Option<String>,
    /// Last time metrics model name was updated.
    pub metrics_model_name_last: Option<std::time::Instant>,
}

pub struct SearchState {
    pub local_filter: String,
    pub filtering_local: bool,
    pub search_results_idx: Option<usize>,
    pub search_table_state: TableState,
    pub files_table_state: TableState,
    pub readme_cache: Option<(String, Vec<ratatui::text::Line<'static>>)>,
    pub gguf_metadata_cache: BTreeMap<String, crate::models::GgufMetadata>,
    pub search_input: Option<String>,
    pub gguf_naming_cache:
        std::collections::HashMap<String, crate::tui::gguf_naming::GgufExplanation>,
    // ── Cache for sorted model indices (list mode) ──
    pub list_sorted_indices: Vec<usize>,
    pub list_sort_version: u64,
    pub last_list_sort_by: crate::models::ListSort,
    pub last_list_filter: String,
    // ── Cache for context settings map (list mode) ──
    pub ctx_cache: HashMap<String, (u32, bool, f32)>,
    pub ctx_cache_version: u64,
    // ── Cache for downloaded filenames (files mode) ──
    pub downloaded_filenames: std::collections::HashSet<String>,
}

pub struct UIState {
    pub active_panel: ActivePanel,
    pub global_mode: GlobalMode,
    pub panel_visibility: u8,
    pub panel_help: bool,
    pub panel_help_offset: usize,
    pub last_error_message: Option<String>,
    pub models_table_state: TableState,
    pub resize_state: Option<ResizeState>,
    pub left_pct: u16,
    pub needs_full_redraw: bool,
    pub needs_redraw: bool,
    pub text_scrolls: HashMap<String, TextScrollState>,
    /// Flag set by tick_metrics when metrics changed, consumed by WS broadcast.
    pub metrics_changed: bool,
    /// Last time WS metrics were broadcast.
    pub last_ws_broadcast: Option<std::time::Instant>,
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
