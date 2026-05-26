use crate::backend::server::ServerHandle;
use crate::config::{Config, LogEntry};
use crate::models::{
    DiscoveredModel, ModelSettings, ModelState, SearchResult, SearchSort, ServerMetrics,
    BenchTuneConfig, BenchTuneProgress, BenchTuneResult,
};
pub use crate::models::LoadProgress;
use crate::models::Backend;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::collections::{VecDeque, HashSet, BTreeMap, HashMap};
use ratatui::text::Line;
use ratatui::layout::Rect;
use ratatui::widgets::{TableState, ListState};

/// Static cell for caching the API port string in help text.
pub static API_PORT_CACHE: Mutex<(u16, String)> = Mutex::new((0, String::new()));

/// State for an in-progress panel resize drag.
pub struct ResizeState {
    /// Starting X position of the mouse when drag began.
    pub start_x: u16,
    /// Starting left_pct value when drag began.
    pub start_pct: u16,
    /// The area of the top panels container (for border detection).
    pub container: Rect,
}

/// Cache for the settings panel render output.
pub struct SettingsRenderCache {
    pub hash: u64,
    pub selected: usize,
    pub lines: Vec<Line<'static>>,
}

/// Which panel has focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivePanel {
    Models,
    Log,
    ServerSettings,
    LlmSettings,
    Profiles,
    SystemPromptPresets,
    SearchReadme,
    ActiveModel,
    ModelInfo,
    Downloads,
}

/// Mode for the models panel.
#[derive(Debug, Clone)]
pub enum ModelsMode {
    /// Normal mode: list of local models.
    List,
    /// Search mode: searching HuggingFace.
    Search {
        query: String,
        results: Vec<SearchResult>,
        sort_by: SearchSort,
        show_readme: bool,
        page: usize,
        /// Whether results are currently being loaded.
        loading: bool,
        /// Whether more results are available.
        has_more: bool,
    },
    /// Files mode: listing available GGUF files for a model.
    Files {
        model_id: String,
        files: Vec<(String, u64, String)>, // (filename, size, url)
        selected_idx: Option<usize>,
        previous_query: String,
        previous_results: Vec<SearchResult>,
        selected_result: Option<SearchResult>,
    },
    /// Benchmark tuning mode: running bench_tune on a model.
    BenchTune,
}

/// Global mode that overlays all panels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GlobalMode {
    Normal,
    CmdLine { cmd_line: String },
    HostPicker {
        entries: Vec<(String, String)>, // (ip, interface_name)
        selected: usize,
    },
    BackendPicker {
        entries: Vec<(Backend, Option<String>)>,
        selected: usize,
    },
    Confirmation { selected: bool, kind: ConfirmationKind },
    RpcManager,
    About,
    MaxConcurrentPicker { value: String },
    BenchTuneSetup {
        config: BenchTuneConfig,
        selected_idx: usize,
        bench_mode_selection: usize,
        editing_prompt: bool,
        editing_kwargs: bool,
    },
    PromptPicker {
        entries: Vec<(String, String)>, // (name, description)
        selected: usize,
        editing: bool,
        edit_buffer: String,
        edit_cursor_pos: usize,
        confirm_delete: bool,
    },
    ProfilePicker {
        entries: Vec<(String, String)>, // (name, description)
        selected: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmationKind {
    Exit,
    Reset,
    Delete,
    Unload,
    DeleteBackend,
}

/// Phase of model loading.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LoadingPhase {
    ServerStarting,
    LoadingModel,
    LoadingMeta,
    LoadingTensors,
    ServerListening,
    Complete,
}

impl LoadingPhase {
    pub fn label(&self) -> &'static str {
        match self {
            LoadingPhase::ServerStarting => "Server starting...",
            LoadingPhase::LoadingModel => "Loading model weights...",
            LoadingPhase::LoadingMeta => "Loading metadata...",
            LoadingPhase::LoadingTensors => "Loading tensors...",
            LoadingPhase::ServerListening => "Server listening...",
            LoadingPhase::Complete => "Ready",
        }
    }
}

/// The main application state.
pub struct App {
    pub running: bool,
    pub config: Config,
    pub models: Vec<DiscoveredModel>,
    pub selected_model_idx: Option<usize>,
    pub models_mode: ModelsMode,
    pub local_filter: String,
    pub filtering_local: bool,
    pub search_results_idx: Option<usize>,
    pub settings: ModelSettings,
    pub model_settings_cache: ModelSettings,
    pub readme_cache: Option<(String, Vec<ratatui::text::Line<'static>>)>,
    pub model_states: HashMap<String, ModelState>,
    pub metrics: ServerMetrics,
    pub download_progress: Vec<crate::models::DownloadState>,
    pub download_tx: Option<tokio::sync::broadcast::Sender<crate::models::DownloadState>>,
    pub download_rx: Option<tokio::sync::broadcast::Receiver<crate::models::DownloadState>>,
    pub download_scroll_state: TableState,
    pub search_table_state: TableState,
    pub files_table_state: TableState,
    pub log_entries: VecDeque<LogEntry>,
    pub active_panel: ActivePanel,
    pub log_expanded: bool,
    pub log_scroll_offset: usize,
    pub log_follow: bool,
    pub log_total_lines: usize,
    pub settings_selected_idx: usize,
    pub server_settings_selected_idx: usize, // 0=Host, 1=Backend
    pub server_settings_scroll_offset: usize,
    pub settings_edit_buffer: String,
    pub settings_scroll_offset: usize,
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
    pub readme_scroll_offset: usize,
    pub editing_preset: Option<usize>,
    pub rpc_workers_selected_idx: usize,
    pub editing_rpc_worker: Option<usize>,
    pub rpc_workers_scroll_offset: usize,
    pub edit_cursor_pos: usize,
    pub gguf_metadata_cache: BTreeMap<String, crate::models::GgufMetadata>,
    pub vram_estimate: u64, // estimated VRAM in MiB
    pub backend_resolving: bool,
    pub backend_resolve_handle: Option<tokio::task::JoinHandle<Result<PathBuf, String>>>,

    pub model_total_layers: u32, // total number of layers in the model
    pub model_hidden_size: u32, // hidden dimension size
    pub model_n_ctx_train: u32, // n_ctx_train from GGUF metadata
    pub model_n_head: u32, // attention head count (n_head)
    pub model_n_kv_head: u32, // KV head count (n_kv_head)
    pub max_threads: u32, // max threads = physical CPU cores
    pub pending_download: Option<(String, String, String, u64)>, // (model_id, filename, download_url, file_size)
    pub pending_deletion: Option<PathBuf>,
    pub pending_backend_deletion: Option<(Backend, String)>,
    pub pending_spawn: Option<(Option<DiscoveredModel>, ModelSettings)>,
    pub pending_api_load: Option<(String, Option<String>)>, // (id, absolute_path)
    pub pending_api_unload: Option<(String, Option<String>)>, // (id, absolute_path)
    pub pending_kill: Option<ServerHandle>,
    pub downloading: bool,
    pub server_log_rx: Option<tokio::sync::mpsc::Receiver<String>>,
    pub metrics_rx: Option<tokio::sync::mpsc::Receiver<crate::models::ServerMetrics>>,
    pub global_mode: GlobalMode,
    pub loading_phases: HashSet<LoadingPhase>,
    /// Tracks the most recently added phase for progress interpolation.
    pub last_active_phase: Option<LoadingPhase>,
    pub loading_progress: f32,
    /// Smoothed target for progress interpolation.
    pub progress_target: f32,
    pub load_progress: LoadProgress,
    /// Timestamp of the last spinner animation update.
    pub last_spinner_time: Option<tokio::time::Instant>,
    /// Current spinner frame index (0-3) for loading animation.
    pub loading_spinner: usize,
    pub cancelled: Option<Arc<AtomicBool>>,
    pub server_handle: Option<ServerHandle>,
    pub metrics_task_handle: Option<tokio::task::JoinHandle<()>>,
    pub sync_task_handle: Option<tokio::task::JoinHandle<()>>,
    pub sync_rx: Option<tokio::sync::mpsc::Receiver<Vec<(String, String, Option<String>)>>>,
    pub spawn_task_handle: Option<tokio::task::JoinHandle<Result<(String, ServerHandle, String), String>>>,
    pub bench_tune_task_handle: Option<tokio::task::JoinHandle<(Result<Vec<BenchTuneResult>, String>, String, BenchTuneConfig)>>,
    pub spawn_log_tx: Option<tokio::sync::mpsc::Sender<String>>,
    pub metrics_model_name: Arc<std::sync::Mutex<Option<String>>>,
    pub loaded_model_names: Arc<std::sync::Mutex<Vec<String>>>,
    pub api_proxy_handle: Option<tokio::task::JoinHandle<()>>,
    /// Collection of background tasks for cleanup on shutdown
    pub background_tasks: HashMap<String, tokio::task::JoinHandle<()>>,
    pub needs_redraw: bool,
    pub panel_help: bool,
    pub panel_visibility: u8,
    pub panel_help_offset: usize,
    /// Last error message captured from the log (used for Failed state display).
    pub last_error_message: Option<String>,
    /// Global server mode (Normal or Router) — persists across model switches.
    pub server_mode: crate::models::ServerMode,
    /// Global router max models setting.
    pub router_max_models: u32,
    /// Cached file modification time for debouncing metadata parsing.
    pub last_metadata_parse: (PathBuf, std::time::SystemTime),
    /// Cached settings panel render output.
    pub settings_render_cache: Option<SettingsRenderCache>,
    /// Pending search load (page) — set when user presses B or Down at bottom.
    pub pending_search_load: Option<(String, u32)>, // (query, offset)
    /// Whether search results are currently being loaded.
    pub search_loading: bool,
    /// Benchmark tuning progress
    pub bench_tune_progress: Option<BenchTuneProgress>,
    /// Benchmark tuning results
    pub bench_tune_results: Vec<BenchTuneResult>,
    /// Whether benchmark tuning is currently running
    pub bench_tune_running: bool,
    /// Benchmark tuning configuration
    pub bench_tune_config: Option<BenchTuneConfig>,
    /// Benchmark tuning channel receiver
    pub bench_tune_rx: Option<tokio::sync::mpsc::Receiver<crate::models::BenchTuneStatus>>,
    /// Benchmark tuning channel sender
    pub bench_tune_tx: Option<tokio::sync::mpsc::Sender<crate::models::BenchTuneStatus>>,
    /// Whether the benchmark output view modal is open
    pub bench_tune_output_view: Option<usize>,
    /// Cancellation channel for benchmark tuning
    pub bench_tune_cancel_tx: Option<tokio::sync::watch::Sender<bool>>,
    /// Scroll offset within the output view modal
    pub bench_tune_output_scroll: usize,
    /// Horizontal scroll offset within the output view modal
    pub bench_tune_output_h_scroll: usize,
    /// Index of the selected result row in the results table
    pub bench_tune_result_row: usize,
    /// Persistent scroll state for the model list in ModelsMode::List
    pub list_state: ListState,
    /// Persistent scroll state for the bench tune results table
    pub bench_tune_table_state: TableState,
    /// Index of the selected output within the current result
    pub bench_tune_output_index: usize,
    pub editing_n_predict: bool,
    pub n_predict_edit_buffer: String,
    pub editing_iters: bool,
    pub iters_edit_buffer: String,
    /// Tags editing state
    pub tags_editing: bool,
    pub tags_edit_buffer: String,
    pub tags_selected_idx: Option<usize>,
    pub tags_insert_mode: bool,
    /// Horizontal split percentage for left panel (20-80, default 55).
    pub left_pct: u16,
    /// State for an in-progress panel resize drag.
    pub resize_state: Option<ResizeState>,
}
