use crate::backend::server::ServerHandle;
use crate::backend::{hub, server, benchmark};
use crate::backend::hardware;
use crate::config::{Config, LogEntry, Profile};
use crate::models::{
    GPUBuffer, DiscoveredModel, LoadProgress, ModelSettings, ModelState, SearchResult, SearchSort, ServerMetrics,
    BenchTuneConfig, BenchTuneProgress, BenchTuneResult, BenchTuneStatus,
};
use crate::serve_api;
use crate::tui::format_size;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU8};
use chrono::Local;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{TableState, ListState};

use std::collections::VecDeque;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

/// Static cell for caching the API port string in help text.
static API_PORT_CACHE: Mutex<(u16, String)> = Mutex::new((0, String::new()));

use ratatui::text::Line;
use ratatui::layout::Rect;

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

// Maximum cache size for GGUF metadata to prevent unbounded memory growth


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
        entries: Vec<(crate::models::Backend, Option<String>)>,
        selected: usize,
    },
    Confirmation { selected: bool, kind: ConfirmationKind },
    RpcManager,
    About,
    MaxConcurrentPicker { value: String },
    BenchTuneSetup {
        config: crate::models::BenchTuneConfig,
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
    pub model_states: std::collections::HashMap<String, ModelState>,
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
    pub backend_picker_entries: Vec<(crate::models::Backend, Option<String>)>,
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
    pub gguf_metadata_cache: std::collections::HashMap<String, crate::models::GgufMetadata>,
    pub vram_estimate: u64, // estimated VRAM in MiB
    pub backend_resolving: bool,
    pub backend_resolve_handle: Option<tokio::task::JoinHandle<Result<std::path::PathBuf, String>>>,

    pub model_total_layers: u32, // total number of layers in the model
    pub model_hidden_size: u32, // hidden dimension size
    pub model_n_ctx_train: u32, // n_ctx_train from GGUF metadata
    pub model_n_head: u32, // attention head count (n_head)
    pub model_n_kv_head: u32, // KV head count (n_kv_head)
    pub max_threads: u32, // max threads = physical CPU cores
    pub pending_download: Option<(String, String, String, u64)>, // (model_id, filename, download_url, file_size)
    pub pending_deletion: Option<std::path::PathBuf>,
    pub pending_backend_deletion: Option<(crate::models::Backend, String)>,
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
    pub bench_tune_task_handle: Option<tokio::task::JoinHandle<(Result<Vec<crate::models::BenchTuneResult>, String>, String, crate::models::BenchTuneConfig)>>,
    pub spawn_log_tx: Option<tokio::sync::mpsc::Sender<String>>,
    pub metrics_model_name: Arc<std::sync::Mutex<Option<String>>>,
    pub loaded_model_names: Arc<std::sync::Mutex<Vec<String>>>,
    pub api_proxy_handle: Option<tokio::task::JoinHandle<()>>,
    /// Collection of background tasks for cleanup on shutdown
    pub background_tasks: std::collections::HashMap<String, tokio::task::JoinHandle<()>>,
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
    last_metadata_parse: (std::path::PathBuf, std::time::SystemTime),
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
    pub bench_tune_rx: Option<tokio::sync::mpsc::Receiver<BenchTuneStatus>>,
    /// Benchmark tuning channel sender
    pub bench_tune_tx: Option<tokio::sync::mpsc::Sender<BenchTuneStatus>>,
    /// Whether the benchmark output view modal is open
    pub bench_tune_output_view: Option<usize>,
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

impl App {
    pub fn new(config: Config) -> Self {
        let mut log = VecDeque::new();
        log.push_back(LogEntry::new("Starting llm-manager...", crate::config::LogLevel::Info));
        let default_params = config.default.clone();
        let settings: ModelSettings = default_params.into();
        let server_mode = config.default.server_mode.clone();
        let router_max_models = config.default.router_max_models;
        Self {
            running: true,
            config,
            models: Vec::new(),
            selected_model_idx: None,
            models_mode: ModelsMode::List,
            local_filter: String::new(),
            filtering_local: false,
            search_results_idx: None,
            model_settings_cache: settings.clone(),
            readme_cache: None,
            settings,
            model_states: Default::default(),
            metrics: Default::default(),
            download_progress: Vec::new(),
            download_tx: None,
            download_rx: None,
            download_scroll_state: TableState::default(),
            search_table_state: TableState::default(),
            files_table_state: TableState::default(),
            log_entries: log,
            active_panel: ActivePanel::Models,
            log_expanded: false,
            log_scroll_offset: 0,
            log_follow: true,
            log_total_lines: 0,
            settings_selected_idx: 0,
            server_settings_selected_idx: 0,
            server_settings_scroll_offset: 0,
            settings_edit_buffer: String::new(),
            settings_scroll_offset: 0,
            host_picker_entries: Vec::new(),
            host_picker_selected: 0,
            backend_picker_entries: Vec::new(),
            backend_picker_selected: 0,
            prompt_picker_entries: Vec::new(),
            prompt_picker_selected: 0,
            profile_picker_entries: Vec::new(),
            profile_picker_selected: 0,
            profiles_scroll_offset: 0,
            system_prompt_presets_scroll_offset: 0,
            readme_scroll_offset: 0,
            editing_preset: None,
            rpc_workers_selected_idx: 0,
            editing_rpc_worker: None,
            rpc_workers_scroll_offset: 0,
            edit_cursor_pos: 0,
            gguf_metadata_cache: Default::default(),
            vram_estimate: 0,
            backend_resolving: false,
            backend_resolve_handle: None,
            model_total_layers: 0,
            model_hidden_size: 0,
            model_n_ctx_train: 0,
            model_n_head: 0,
            model_n_kv_head: 0,
            max_threads: crate::config::physical_cores(),
            pending_download: None,
            pending_deletion: None,
            pending_backend_deletion: None,
            pending_spawn: None,
            pending_api_load: None,
            pending_api_unload: None,
            pending_kill: None,
            downloading: false,
            server_log_rx: None,
            metrics_rx: None,
            global_mode: GlobalMode::Normal,
            loading_phases: HashSet::new(),
            last_active_phase: None,
            loading_progress: 0.0,
            progress_target: 0.0,
            load_progress: Default::default(),
            last_spinner_time: None,
            loading_spinner: 0,
            cancelled: None,
            server_handle: None,
            metrics_task_handle: None,
            sync_task_handle: None,
            sync_rx: None,
            spawn_task_handle: None,
            bench_tune_task_handle: None,
            spawn_log_tx: None,
            metrics_model_name: Arc::new(std::sync::Mutex::new(None)),
            loaded_model_names: Arc::new(std::sync::Mutex::new(Vec::new())),
            api_proxy_handle: None,
            background_tasks: std::collections::HashMap::new(),
            needs_redraw: true,
            panel_visibility: 0b111111,
            panel_help: false,
            panel_help_offset: 0,
            last_error_message: None,
            last_metadata_parse: (std::path::PathBuf::new(), std::time::SystemTime::now()),
            pending_search_load: None,
            search_loading: false,
            server_mode,
            router_max_models,
            settings_render_cache: None,
            bench_tune_progress: None,
            bench_tune_results: Vec::new(),
            bench_tune_running: false,
            bench_tune_config: None,
            bench_tune_rx: None,
            bench_tune_tx: None,
            bench_tune_output_view: None,
            bench_tune_output_scroll: 0,
            bench_tune_output_h_scroll: 0,
            bench_tune_result_row: 0,
            list_state: ListState::default(),
            bench_tune_table_state: TableState::default(),
            bench_tune_output_index: 0,
            editing_n_predict: false,
            n_predict_edit_buffer: String::new(),
            editing_iters: false,
            iters_edit_buffer: String::new(),
            tags_editing: false,
            tags_edit_buffer: String::new(),
            tags_selected_idx: None,
            tags_insert_mode: false,
            left_pct: 55,
            resize_state: None,
        }
    }

    pub fn selected_model(&self) -> Option<&DiscoveredModel> {
        self.selected_model_idx.and_then(|i| self.models.get(i))
    }

    pub fn selected_model_settings(&self) -> ModelSettings {
        let model_name = self.selected_model().map(|m| m.name.as_str());
        // For the TUI, we don't currently support a separate profile_name 
        // in this method since it's already accounted for in overrides or the default settings.
        self.config.resolve_settings(model_name, None)
    }

  pub fn add_log(&mut self, message: impl Into<String>, level: crate::config::LogLevel) {
        let msg = message.into();
        self.log_message(&msg, level);
        self.update_spinner();
        self.detect_loading_phases(&msg);
        self.parse_loading_details(&msg);
        self.detect_load_state(&msg);
        let previous_progress = self.loading_progress;
        self.compute_progress();
        self.progress_target = self.loading_progress;
        self.loading_progress = previous_progress * 0.85 + self.progress_target * 0.15;
        self.handle_server_exit(&msg);
        self.trim_log();
        self.log_entries.push_back(LogEntry::new(msg, level));
        self.needs_redraw = true;
    }

    fn log_message(&mut self, msg: &str, level: crate::config::LogLevel) {
        match level {
            crate::config::LogLevel::Info => tracing::info!("{}", msg),
            crate::config::LogLevel::Warning => tracing::warn!("{}", msg),
            crate::config::LogLevel::Error => tracing::error!("{}", msg),
        }
    }

    fn update_spinner(&mut self) {
        self.last_spinner_time = Some(tokio::time::Instant::now());
        self.loading_spinner = 0;
    }

    fn detect_loading_phases(&mut self, msg: &str) {
        let upper = msg.to_uppercase();
        if self.loading_phases.is_empty() {
            // Detect server starting (first log line after spawn)
            if upper.contains("LLAMA") || upper.contains("SERVER") || upper.contains("GGML") {
                self.loading_phases.insert(LoadingPhase::ServerStarting);
                self.last_active_phase = Some(LoadingPhase::ServerStarting);
            }
        }
        if upper.contains("LLAMA_MODEL_LOADER") || upper.contains("LOADING MODEL") {
            self.last_error_message = None;
            self.loading_phases.insert(LoadingPhase::LoadingModel);
            self.last_active_phase = Some(LoadingPhase::LoadingModel);
        }
        if upper.contains("LOADED META") || upper.contains("META DATA") {
            self.last_error_message = None;
            self.loading_phases.insert(LoadingPhase::LoadingMeta);
            self.last_active_phase = Some(LoadingPhase::LoadingMeta);
        }
        if upper.contains("LOAD_TENSORS:") {
            self.last_error_message = None;
            self.loading_phases.insert(LoadingPhase::LoadingTensors);
            self.last_active_phase = Some(LoadingPhase::LoadingTensors);
        }
        if upper.contains("SERVER LISTENING") || upper.contains("HTTP SERVER LISTENING") {
            self.loading_phases.insert(LoadingPhase::ServerListening);
           self.last_active_phase = Some(LoadingPhase::ServerListening);
        }
    }

    fn parse_loading_details(&mut self, msg: &str) {
        let upper = msg.to_uppercase();
        if self.loading_phases.contains(&LoadingPhase::LoadingTensors) {
            // Parse "loading tensor X of Y" or "loading tensor X out of Y" pattern
            if upper.contains("LOADING TENSOR") {
                if let Some(pos) = msg.to_lowercase().find("loading tensor") {
                    let rest = &msg[pos + "loading tensor".len()..];
                    let parts: Vec<&str> = rest.split_whitespace().collect();
                    if parts.len() >= 3 {
                        if let Ok(n) = parts[0].parse::<u32>() {
                            self.load_progress.tensors_loaded = n;
                        }
                        // "of Y" or "out of Y" — Y is at index 2 or 4
                        let total_idx = if parts.len() >= 5 && parts[2].to_lowercase() == "out" {
                            4
                        } else if parts.len() >= 3 && parts[1].to_lowercase() == "of" {
                            2
                        } else {
                            usize::MAX
                        };
                        if total_idx != usize::MAX {
                            if let Ok(total) = parts[total_idx].trim_end_matches(|c: char| !c.is_ascii_digit()).parse::<u32>() {
                                self.load_progress.tensors_total = Some(total);
                            }
                        }
                    }
                }
            }
            // Count dots from progress lines like "................................"
            // Only use dot-counting as fallback when we haven't seen an explicit tensor count yet
            if self.load_progress.tensors_total.is_none() {
                let dot_count = msg.chars().filter(|&c| c == '.').count();
                if dot_count > 0 && dot_count <= 200 {
                    self.load_progress.tensors_loaded += dot_count as u32;
                }
            }

            // Offloading N repeating layers to GPU
            if upper.contains("OFFLOADING") && upper.contains("REPEATING LAYERS")
                && let Some(pos) = msg.find("offloading") {
                    let rest = &msg[pos + "offloading".len()..];
                    if let Some(colon_pos) = rest.find(':') {
                        let rest = rest[colon_pos + 1..].trim_start();
                        let end = rest.find(' ').unwrap_or(rest.len());
                        if let Ok(count) = rest[..end].trim().parse::<u32>() {
                            self.load_progress.layers_total = Some(count);
                        }
                    }
                }

            // Offloaded X/Y layers to GPU
            if upper.contains("OFFLOADED") && upper.contains("LAYERS")
                && let Some(pos) = msg.find("offloaded") {
                    let rest = &msg[pos + "offloaded".len()..];
                    if let Some(slash) = rest.find('/') {
                        let before = rest[..slash].trim();
                        let after = rest[slash + 1..].trim();
                        if let Ok(loaded) = before.parse::<u32>() {
                            self.load_progress.layers_loaded = Some(loaded);
                        }
                        if let Ok(total) = after.split_whitespace().next().unwrap_or("").parse::<u32>() {
                            self.load_progress.layers_total = Some(total);
                        }
                    }
                    // Also handle "offloaded N layers" without Y
                    if self.load_progress.layers_loaded.is_none() {
                        let rest = rest.trim_start();
                        let end = rest.find(' ').unwrap_or(rest.len());
                        if let Ok(count) = rest[..end].trim().parse::<u32>() {
                            self.load_progress.layers_loaded = Some(count);
                        }
                    }
                }

            // CPU_Mapped model buffer size = X MiB
            // Vulkan0 model buffer size = X MiB
            for keyword in &["model buffer size", "kv buffer size"] {
                if let Some(pos) = msg.to_lowercase().find(keyword) {
                    let before = &msg[..pos];
                    let device = before.split_whitespace().last().unwrap_or("").to_string();
                    if !device.is_empty() {
                        let rest = &msg[pos + keyword.len()..];
                        if let Some(eq_pos) = rest.find('=') {
                            let after = rest[eq_pos + 1..].trim();
                            let end = after.find(|c: char| !c.is_ascii_digit() && c != '.').unwrap_or(after.len());
                            if let Ok(mib) = after[..end].parse::<f64>() {
                                let exists = self.load_progress.buffers.iter_mut().find(|b| b.device == device);
                                if let Some(buf) = exists {
                                    buf.buffer_size_mib = mib;
                                } else {
                                    self.load_progress.buffers.push(GPUBuffer {
                                        device,
                                        buffer_size_mib: mib,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn detect_load_state(&mut self, msg: &str) {
        let upper = msg.to_uppercase();

        // Detect successful model load (including router mode)
        if upper.contains("LOADED SUCCESSFULLY")
            || upper.contains("LLAMA_NEW_CONTEXT_WITH_MODEL")
            || upper.contains("MAIN: MODEL LOADED")
            || upper.contains("UPDATE_SLOTS: ALL SLOTS ARE IDLE")
        {
            self.loading_phases.insert(LoadingPhase::Complete);
            self.last_active_phase = Some(LoadingPhase::Complete);
            self.loading_progress = 1.0;
            self.last_error_message = None;

            let mut to_update = Vec::new();
            if let Some(handle) = &self.server_handle {
                let port = handle.port;
                let pid = handle.pid;
                for (name, state) in &self.model_states {
                    if matches!(state, ModelState::Loading) {
                        to_update.push(name.clone());
                    }
                }
                for name in to_update {
                    self.model_states.insert(name.clone(), ModelState::Loaded { port, pid });
                    self.loaded_model_names.lock().unwrap_or_else(|e| e.into_inner()).push(name);
                }
            }
        }

        // Detect model load failure or crash
        let is_crash = upper.contains("LLAMA-SERVER") && (upper.contains("EXITED") || upper.contains("TERMINATED"));
        let is_error = is_crash
            || upper.contains("ERROR")
            || upper.contains("FAILED TO LOAD")
            || upper.contains("EXCEPTION")
            || upper.contains("VK::SYSTEMERROR")
            || upper.contains("OUTOFDEVICEMEMORY")
            || upper.contains("OUT OF MEMORY");

        if is_error {
            let is_loading = self.model_states.values().any(|s| matches!(s, ModelState::Loading));
            if is_crash || is_loading {
                let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
                let mut error_msg = if upper.contains("OUTOFDEVICEMEMORY") || upper.contains("OUT OF MEMORY") {
                    format!("Last Failed to load a model (OOM - {})", timestamp)
                } else {
                    format!("Last Failed to load a model ({})", timestamp)
                };

                if is_crash {
                    error_msg = format!("Last Failed to load a model (Router Crash - {})", timestamp);
                    if let Some(h) = self.server_handle.take() {
                        self.pending_kill = Some(h);
                    }
                }

                self.last_error_message = Some(error_msg);
                self.reset_loading_state(is_crash);
            }
        }
    }

    fn compute_progress(&mut self) {
        const PHASE_WEIGHTS: [(LoadingPhase, f32); 5] = [
            (LoadingPhase::ServerStarting, 0.08),
            (LoadingPhase::LoadingModel, 0.07),
            (LoadingPhase::LoadingMeta, 0.07),
            (LoadingPhase::LoadingTensors, 0.70),
            (LoadingPhase::ServerListening, 0.08),
        ];

        let mut phase_progress: f32 = 0.0;
        for (phase, weight) in &PHASE_WEIGHTS {
            if self.loading_phases.contains(phase) {
                phase_progress += weight;
            }
        }

        // Handle Complete phase separately — it means 100%
        if self.loading_phases.contains(&LoadingPhase::Complete) {
            self.loading_progress = 1.0;
            return;
        }

        // Spinner interpolation for ServerStarting (works even as the only active phase)
        if self.loading_phases.contains(&LoadingPhase::ServerStarting)
            && self.loading_phases.len() == 1
            && self.last_active_phase == Some(LoadingPhase::ServerStarting)
        {
            if let Some(last_spinner) = self.last_spinner_time {
                let elapsed = last_spinner.elapsed();
                phase_progress = (elapsed.as_millis() as f32 / 2000.0).min(1.0) * PHASE_WEIGHTS[0].1;
            }
        } else if self.loading_phases.len() > 1 {
            // Apply interpolation within the current active phase for smooth transitions
            if let Some(phase) = self.last_active_phase {
                let cumulative_before: f32 = PHASE_WEIGHTS.iter()
                    .filter(|(p, _)| *p != phase && self.loading_phases.contains(p))
                    .map(|(_, w)| w)
                    .sum();

                let phase_fraction = match phase {
                    LoadingPhase::LoadingModel => 0.5,
                    LoadingPhase::LoadingMeta => 0.5,
                     LoadingPhase::LoadingTensors => {
                        let mut tensor_fraction: f32 = 0.0;
                        if let (Some(loaded), Some(total)) = (self.load_progress.layers_loaded, self.load_progress.layers_total) {
                            let layer_fraction = loaded as f32 / total as f32;
                            tensor_fraction = layer_fraction.min(1.0);
                        }
                        if self.load_progress.tensors_loaded > 0 {
                            let estimated_total: f32 = match self.load_progress.tensors_total {
                                Some(total) => total as f32,
                                None => match self.load_progress.layers_total {
                                    Some(layers) => (layers as f32 * 12.0 + 10.0).max(100.0),
                                    None => 500.0,
                                },
                            };
                            tensor_fraction = (self.load_progress.tensors_loaded as f32 / estimated_total).min(0.95);
                        }
                        tensor_fraction
                    }
                    LoadingPhase::ServerListening => 0.8,
                    LoadingPhase::Complete => 1.0,
                    LoadingPhase::ServerStarting => 0.0,
                };

                phase_progress = cumulative_before + phase_fraction * PHASE_WEIGHTS.iter()
                    .find(|(p, _)| *p == phase)
                    .map(|(_, w)| *w)
                    .unwrap_or(0.0);
            }
        }

        if phase_progress > 0.0 {
            self.loading_progress = phase_progress;
        }
    }

    fn handle_server_exit(&mut self, msg: &str) {
        let upper = msg.to_uppercase();
        if upper.contains("LLAMA-SERVER EXITED") || upper.contains("LLAMA-BENCH EXITED") {
            self.server_handle = None;
            self.loading_phases.clear();
            self.last_active_phase = None;
            self.loading_progress = 0.0;
            self.load_progress = Default::default();
            self.needs_redraw = true;

            for state in self.model_states.values_mut() {
                *state = crate::models::ModelState::Available;
            }
        }
    }

    fn trim_log(&mut self) {
        if self.log_entries.len() >= 500 {
            self.log_entries.pop_front();
        }
    }

    /// Mark the app as needing a redraw in the next main loop iteration.
    pub fn set_redraw(&mut self) {
        self.needs_redraw = true;
    }

    pub fn is_model_loaded(&self, display_name: &str) -> bool {
        matches!(
            self.model_states.get(display_name),
            Some(ModelState::Loaded { .. })
        )
    }

    pub fn get_filtered_model_indices(&self) -> Vec<usize> {
        self.models
            .iter()
            .enumerate()
            .filter(|(_, m)| {
                self.local_filter.is_empty()
                    || m.display_name
                        .to_lowercase()
                        .contains(&self.local_filter.to_lowercase())
            })
            .map(|(i, _)| i)
            .collect()
    }

    /// Return the current number of search results.
    pub fn search_results_len(&self) -> usize {
        if let ModelsMode::Search { results, .. } = &self.models_mode {
            results.len()
        } else {
            0
        }
    }

    /// Check if a panel is visible.
    pub fn is_panel_visible(&self, index: u8) -> bool {
        self.panel_visibility & (1 << index) != 0
    }

    /// Toggle visibility of a panel.
    pub fn toggle_panel_visibility(&mut self, index: u8) {
        self.panel_visibility ^= 1 << index;
        // If hiding the log while expanded, collapse it.
        if index == 5 && !self.is_panel_visible(5) {
            self.log_expanded = false;
        }
    }

    pub fn on_model_selection_change(&mut self) {
        self.readme_cache = None;
        if let Some(idx) = self.selected_model_idx {
            let model = self.models[idx].clone();
            self.model_settings_cache = self.selected_model_settings();
            self.settings = self.model_settings_cache.clone();
            self.update_model_metadata();
            self.update_vram_estimate();

            // Sync loading progress with the newly selected model
            if self.is_model_loaded(&model.display_name) {
                self.loading_progress = 1.0;
                if !self.loading_phases.contains(&LoadingPhase::Complete) {
                    self.loading_phases.insert(LoadingPhase::Complete);
                }
            } else if matches!(self.model_states.get(&model.display_name), Some(ModelState::Loading) | Some(ModelState::Benchmarking)) {
                // Keep current loading/benchmarking progress
            } else {
                // Not loaded, loading, or benchmarking, reset progress
                self.loading_progress = 0.0;
          self.loading_phases.clear();
            self.last_active_phase = None;
                self.load_progress = Default::default();
            }
        } else {
            let default_params = self.config.default.clone();
            self.model_settings_cache = default_params.into();
            self.model_total_layers = 0;
            self.model_hidden_size = 0;
            self.model_n_ctx_train = 0;
            self.settings.is_mtp = false;
            self.settings.draft_tokens = 0;
            self.vram_estimate = 0;
            self.loading_progress = 0.0;
            self.loading_phases.clear();
            self.last_active_phase = None;
        }
        self.set_redraw();
    }

    /// Reset loading state (progress bar and model status) on failure.
    pub fn reset_loading_state(&mut self, is_crash: bool) {
        self.loading_phases.clear();
        self.last_active_phase = None;
        self.loading_progress = 0.0;
        self.load_progress = Default::default();
        self.last_spinner_time = None;
        self.loading_spinner = 0;
        
        // Models to fail: always any that were Loading. 
        // If it's a crash, also fail all that were Loaded.
        let mut to_fail = Vec::new();
        for (name, state) in &self.model_states {
            if matches!(state, ModelState::Loading) {
                to_fail.push(name.clone());
            } else if is_crash && matches!(state, ModelState::Loaded { .. }) {
                to_fail.push(name.clone());
            }
        }

        // Remove from loaded list and set to Failed
        for name in to_fail {
            self.loaded_model_names.lock().unwrap_or_else(|e| e.into_inner()).retain(|n| n != &name);
            let error = self.last_error_message.clone().unwrap_or_else(|| {
                let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
                format!("Last Failed to load a model ({})", timestamp)
            });
            self.model_states.insert(name.clone(), ModelState::Failed { error });
        }
        self.set_redraw();
    }

    /// Get the API port string, caching it to avoid re-allocating on every call.
    pub fn get_api_port_str(&self) -> String {
        let port = self.settings.api_endpoint_port;
        let mut cache = API_PORT_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        if cache.0 == port && !cache.1.is_empty() {
            return cache.1.clone();
        }
        cache.0 = port;
        cache.1 = port.to_string();
        cache.1.clone()
    }

    /// Compute VRAM estimate from model file size and current settings.
    pub fn update_vram_estimate(&mut self) {
        if let Some(model) = self.selected_model() {
            let model_mib = model.file_size / (1024 * 1024);
            let hidden = if self.model_hidden_size > 0 { Some(self.model_hidden_size) } else { None };
            let n_head = if self.model_n_head > 0 { Some(self.model_n_head) } else { None };
            let n_kv_head = if self.model_n_kv_head > 0 { Some(self.model_n_kv_head) } else { None };
            let gpu_mem_total_mib = self.metrics.gpu_mem_total / (1024 * 1024);
            self.vram_estimate = crate::models::estimate_vram_mib(
                model_mib, &self.settings, self.model_total_layers, hidden,
                n_head, n_kv_head, gpu_mem_total_mib
            );
            self.set_redraw();
        }
    }

    /// Read metadata (layers, hidden size) from the model's GGUF file.
    ///
    /// Uses a single cache keyed by the model's full path, so each unique
    /// model is parsed only once regardless of how many times it's selected.
    pub fn update_model_metadata(&mut self) {
        let model = match self.selected_model() {
            Some(m) => m.clone(),
            None => return,
        };
        let key = model.path.to_string_lossy().to_string();
        
        // Evict cache entries if it exceeds the maximum size
        const MAX_CACHE_SIZE: usize = 50;
        if self.gguf_metadata_cache.len() > MAX_CACHE_SIZE {
            // Remove the oldest entry (first inserted)
            if let Some(first_key) = self.gguf_metadata_cache.keys().next().cloned() {
                self.gguf_metadata_cache.remove(&first_key);
            }
        }
        
        // 1. Check persistent cache first
        if let Some(cached) = self.gguf_metadata_cache.get(&key) {
            self.model_total_layers = cached.layers;
            self.model_hidden_size = cached.hidden_size;
            self.model_n_ctx_train = cached.n_ctx_train;
            self.model_n_head = cached.n_head;
            self.model_n_kv_head = cached.n_kv_head;
        }

       // 2. Debounce logic: only skip if we tried this EXACT file (path + mtime) very recently
        // and it wasn't GGUF or we failed to parse it.
        if let Ok(meta) = std::fs::metadata(&model.path) {
            let mtime = meta.modified().unwrap_or(std::time::SystemTime::now());
            let (last_path, last_mtime) = &self.last_metadata_parse;
            if last_path == &model.path && mtime == *last_mtime {
                // Already tried this version of the file and it's not in cache (meaning it failed or is not GGUF)
                if self.model_hidden_size > 0 {
                    self.update_vram_estimate();
                }
                return;
            }
            self.last_metadata_parse = (model.path.clone(), mtime);
        }

        // 3. Perform the actual parse
        let path_str = model.path.to_string_lossy();
        match gguf_rs::get_gguf_container(&path_str) {
            Ok(mut container) => {
                match container.decode() {
                    Ok(model_data) => {
                        let mut layers = 0u32;
                        let mut hidden = 0u32;
                        let mut n_ctx_train = 0u32;
                        let mut n_head = 0u32;
                        let mut n_kv_head = 0u32;
                        let mut arch = String::new();
                        let mut file_type = String::new();
                        let mut quantization = String::new();
                        let mut model_parameters = String::new();
                        let mut domain = String::new();
                        let mut capabilities = Vec::new();
                        let mut tokenizer = String::new();
                        let mut vocab_size = 0u32;

                        if let Some(value) = model_data.metadata().get("general.architecture")
                            && let Some(v) = value.as_str() { arch = v.to_string(); }

                        // Detect MTP (Multi-Token Prediction)
                        if arch == "mtp" {
                            self.settings.is_mtp = true;
                            if let Some(value) = model_data.metadata().get("mtp.draft_tokens") {
                                self.settings.draft_tokens = value.as_u64()
                                    .or_else(|| value.as_i64().map(|x| x as u64))
                                    .or_else(|| value.as_f64().map(|x| x as u64))
                                    .unwrap_or(0) as u32;
                            }
                        }

                        // Capabilities
                        if model_data.metadata().contains_key("tokenizer.chat_template") {
                            capabilities.push("chat".to_string());
                        }
                        if let Some(value) = model_data.metadata().get("general.capabilities")
                            && let Some(arr) = value.as_array() {
                                for v in arr {
                                    if let Some(s) = v.as_str() {
                                        capabilities.push(s.to_string());
                                    }
                                }
                            }

                        let extract_num = |key: &str| -> Option<u64> {
                            model_data.metadata().get(key).and_then(|v| {
                                v.as_u64()
                                    .or_else(|| v.as_i64().map(|x| x as u64))
                                    .or_else(|| v.as_f64().map(|x| x as u64))
                            })
                        };

                        if let Some(v) = extract_num("general.file_type") {
                            quantization = match v {
                                0 => "F32".to_string(),
                                1 => "F16".to_string(),
                                2 => "Q4_0".to_string(),
                                3 => "Q4_1".to_string(),
                                7 => "Q8_0".to_string(),
                                8 => "Q5_0".to_string(),
                                9 => "Q5_1".to_string(),
                                10 => "Q2_K".to_string(),
                                11 => "Q3_K_S".to_string(),
                                12 => "Q3_K_M".to_string(),
                                13 => "Q3_K_L".to_string(),
                                14 => "Q4_K_S".to_string(),
                                15 => "Q4_K_M".to_string(),
                                16 => "Q5_K_S".to_string(),
                                17 => "Q5_K_M".to_string(),
                                18 => "Q6_K".to_string(),
                                19 => "IQ2_XXS".to_string(),
                                20 => "IQ2_XS".to_string(),
                                21 => "IQ3_XXS".to_string(),
                                22 => "IQ1_S".to_string(),
                                23 => "IQ4_NL".to_string(),
                                24 => "IQ3_S".to_string(),
                                25 => "IQ2_S".to_string(),
                                26 => "IQ4_XS".to_string(),
                                _ => format!("Unknown ({})", v),
                            };
                        }

                        let prefix = if arch.is_empty() { "llama" } else { &arch };

                        // Try architecture-specific prefix, fall back to "llama" if missing
                        let get_num_with_fallback = |suffix: &str| -> Option<u64> {
                            extract_num(&format!("{}.{}", prefix, suffix))
                                .or_else(|| {
                                    if prefix != "llama" {
                                        extract_num(&format!("llama.{}", suffix))
                                    } else {
                                        None
                                    }
                                })
                        };

                        if let Some(v) = get_num_with_fallback("block_count") {
                            layers = v as u32;
                        }
                        if let Some(v) = get_num_with_fallback("embedding_length") {
                            hidden = v as u32;
                        }
                        if let Some(v) = get_num_with_fallback("context_length") {
                            n_ctx_train = v as u32;
                        }
                        if let Some(v) = get_num_with_fallback("attention.head_count") {
                            n_head = v as u32;
                        }
                        if let Some(v) = get_num_with_fallback("attention.head_count_kv") {
                            n_kv_head = v as u32;
                        }

                        if layers == 0 && hidden == 0 {
                            let keys: Vec<String> = model_data.metadata().keys().take(10).cloned().collect();
                            self.add_log(format!("GGUF parse: found 0 layers/hidden. Arch: {}. Sample keys: {:?}", arch, keys), crate::config::LogLevel::Info);
                        }
                        if !model_data.get_version().is_empty() {
                            file_type = model_data.get_version();
                        }
                        if !model_data.model_parameters().is_empty() {
                            model_parameters = model_data.model_parameters();
                        }
                        if let Some(value) = model_data.metadata().get("general.domain")
                            && let Some(v) = value.as_str() { domain = v.to_string(); }
                        if let Some(value) = model_data.metadata().get("tokenizer.ggml.model")
                            && let Some(v) = value.as_str() { tokenizer = v.to_string(); }
                        if let Some(value) = model_data.metadata().get("tokenizer.ggml.tokens")
                            && let Some(arr) = value.as_array() {
                                vocab_size = arr.len() as u32;
                            }

                        self.model_total_layers = layers;
                        self.model_hidden_size = hidden;
                        self.model_n_ctx_train = n_ctx_train;
                        self.model_n_head = n_head;
                        self.model_n_kv_head = n_kv_head;

                        // Cache the parsed metadata
                        self.gguf_metadata_cache.insert(key, crate::models::GgufMetadata {
                                layers,
                                hidden_size: hidden,
                                n_ctx_train,
                                n_head,
                                n_kv_head,
                                arch,
                                file_type,
                                quantization,
                                model_parameters,
                                domain,
                                capabilities,
                                tokenizer,
                                vocab_size,
                                draft_tokens: self.settings.draft_tokens,
                            });
                        self.set_redraw();
                    }
                    Err(e) => {
                        self.add_log(format!("Failed to decode GGUF {}: {}", model.path.display(), e), crate::config::LogLevel::Error);
                    }
                }
            }
            Err(e) => {
                self.add_log(format!("Failed to parse GGUF {}: {}", model.path.display(), e), crate::config::LogLevel::Error);
            }
        }

        // Compute VRAM estimate once, after metadata fields are populated.
        if self.model_hidden_size > 0 {
            self.update_vram_estimate();
        }
    }

    /// Return a list of all currently visible and focusable panels in logical order.
    pub fn get_visible_panels(&self) -> Vec<ActivePanel> {
        let mut visible = Vec::new();

        // 1. Models (Left Top)
        if self.is_panel_visible(0) {
            visible.push(ActivePanel::Models);
        }

        // 2. Model Info (Left Bottom)
        if self.is_panel_visible(2) {
            visible.push(ActivePanel::ModelInfo);
        }

        // 3. Right Panel (README / Settings / Profiles / Presets)
        let is_search = matches!(self.models_mode, ModelsMode::Search { .. });
        let is_files = matches!(self.models_mode, ModelsMode::Files { .. });
        let show_readme = match &self.models_mode {
            ModelsMode::Search { show_readme, .. } => *show_readme,
            ModelsMode::Files { .. } => true,
            _ => false,
        };

        if self.active_panel == ActivePanel::Profiles {
            visible.push(ActivePanel::Profiles);
        } else if self.active_panel == ActivePanel::SystemPromptPresets {
            visible.push(ActivePanel::SystemPromptPresets);
        } else if show_readme && (is_search || is_files) {
            visible.push(ActivePanel::SearchReadme);
        } else {
            if self.is_panel_visible(1) && self.server_handle.is_none() {
                visible.push(ActivePanel::ServerSettings);
            }
            if self.is_panel_visible(3) {
                visible.push(ActivePanel::LlmSettings);
            }
        }

        // 4. Active Model (Bottom Middle) — read-only, not focusable

        // 5. Log (Bottom)
        if self.is_panel_visible(5) {
            visible.push(ActivePanel::Log);
        }

        // 6. Downloads (Bottom, shown when downloading)
        if self.downloading {
            visible.push(ActivePanel::Downloads);
        }

        visible
    }

    pub fn focus_next(&mut self) {
        let visible = self.get_visible_panels();
        if visible.is_empty() {
            return;
        }

        let current_idx = visible.iter().position(|&p| p == self.active_panel).unwrap_or(0);
        let next_idx = (current_idx + 1) % visible.len();
        self.active_panel = visible[next_idx];
        self.set_redraw();
    }

    pub fn focus_prev(&mut self) {
        let visible = self.get_visible_panels();
        if visible.is_empty() {
            return;
        }

        let current_idx = visible.iter().position(|&p| p == self.active_panel).unwrap_or(0);
        let prev_idx = (current_idx + visible.len() - 1) % visible.len();
        self.active_panel = visible[prev_idx];
        self.set_redraw();
    }

    /// Apply a profile's settings to the current settings.
    pub fn apply_profile(&mut self, profile: &Profile) {
        self.settings = profile.apply(self.settings.clone());
        self.resolve_system_prompt();
        self.settings_render_cache = None;
        self.add_log(format!("Applied profile: {}", profile.name), crate::config::LogLevel::Info);
        self.set_redraw();
    }

    /// Resolve system_prompt from the preset name.
    pub fn resolve_system_prompt(&mut self) {
        let presets = &self.config.system_prompt_presets;
        if let Some(preset) = presets.iter().find(|p| p.name == self.settings.system_prompt_preset_name) {
            self.settings.system_prompt = preset.content.clone();
        }
        self.set_redraw();
    }

    /// Save the current settings as a new profile.
    pub fn save_current_as_profile(&mut self, name: &str) {
        let profile = Profile {
            name: name.to_string(),
            description: format!("User-defined profile: {}", name),
            settings: crate::config::ModelOverride::from_settings(&self.settings),
        };
        self.config.profiles.push(profile);
        if let Err(e) = self.config.save() {
            self.add_log(format!("Failed to save profile: {}", e), crate::config::LogLevel::Error);
        } else {
            self.add_log(format!("Saved profile: {}", name), crate::config::LogLevel::Info);
        }
        self.set_redraw();
    }

    /// Save current settings as an override for the selected model.
    pub fn save_model_settings(&mut self) {
        if let Some(model) = self.selected_model() {
            let name = model.name.clone();
            let override_cfg = crate::config::ModelOverride::from_settings(&self.settings);
            self.config.model_overrides.insert(name.clone(), override_cfg);
            if let Err(e) = self.config.save() {
                self.add_log(format!("Failed to save settings for {}: {}", name, e), crate::config::LogLevel::Error);
            } else {
                self.add_log(format!("Saved settings for {}", name), crate::config::LogLevel::Info);
                // Update the cache so it reflects the newly saved settings
                self.model_settings_cache = self.settings.clone();
            }
        } else {
            self.add_log("No model selected to save settings for", crate::config::LogLevel::Warning);
        }
        self.settings_render_cache = None;
        self.set_redraw();
    }

    /// Check if any LLM settings have been modified since last save.
    pub fn is_settings_dirty(&self) -> bool {
        self.settings.is_dirty(&self.model_settings_cache)
    }

    /// Compute a fingerprint of the current settings for cache invalidation.
    pub fn settings_fingerprint(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        self.settings.context_length.hash(&mut h);
        self.settings.system_prompt_preset_name.hash(&mut h);
        self.settings.mlock.hash(&mut h);
        self.settings.gpu_layers_mode.hash(&mut h);
        self.settings.flash_attn.hash(&mut h);
        self.settings.kv_cache_offload.hash(&mut h);
        self.settings.cache_type_k.hash(&mut h);
        self.settings.cache_type_v.hash(&mut h);
        self.settings.expert_count.hash(&mut h);
        self.settings.batch_size.hash(&mut h);
        self.settings.uniform_cache.hash(&mut h);
        self.settings.max_concurrent_predictions.hash(&mut h);
        self.settings.seed.hash(&mut h);
        self.settings.temperature.to_bits().hash(&mut h);
        self.settings.top_k.hash(&mut h);
        self.settings.top_p.to_bits().hash(&mut h);
        self.settings.min_p.to_bits().hash(&mut h);
        self.settings.max_tokens.hash(&mut h);
        self.settings.repeat_penalty.to_bits().hash(&mut h);
        self.settings.repeat_last_n.hash(&mut h);
        self.settings.presence_penalty.map(|v| v.to_bits()).hash(&mut h);
        self.settings.frequency_penalty.map(|v| v.to_bits()).hash(&mut h);
        self.settings.keep.hash(&mut h);
        self.settings.mmap.hash(&mut h);
        self.settings.numa.hash(&mut h);
        self.settings.threads.hash(&mut h);
        self.settings.threads_batch.hash(&mut h);
        self.settings.get_active_backend_version().hash(&mut h);
        self.settings_edit_buffer.hash(&mut h);
        h.finish()
    }

    /// Delete a user profile by index in the merged display list.
    /// Returns true if a profile was deleted, false otherwise.
    pub fn delete_profile(&mut self, selected_idx: usize) -> bool {
        let builtin = crate::config::builtin_profiles();
        
        // Build the merged profile list (same as render logic)
        let mut all_profiles: Vec<crate::config::Profile> = builtin.to_vec();
        let mut user_profiles_displayed: Vec<(usize, crate::config::Profile)> = Vec::new();
        
        for (idx, p) in self.config.profiles.iter().enumerate() {
            if !builtin.iter().any(|b| b.name == p.name) {
                user_profiles_displayed.push((idx, p.clone()));
                all_profiles.push(p.clone());
            }
        }
        
        // Check if selection is valid
        if selected_idx >= all_profiles.len() {
            self.add_log("Invalid profile selection", crate::config::LogLevel::Info);
            return false;
        }
        
        // Check if it's a built-in profile
        if selected_idx < builtin.len() {
            self.add_log("Cannot delete built-in profiles", crate::config::LogLevel::Info);
            return false;
        }
        
        // Map from display index to actual config.profiles index
        let display_user_idx = selected_idx - builtin.len();
        if display_user_idx >= user_profiles_displayed.len() {
            self.add_log("Invalid profile selection", crate::config::LogLevel::Info);
            return false;
        }
        
        let (actual_idx, profile) = &user_profiles_displayed[display_user_idx];
        let profile_name = profile.name.clone();
        
        self.config.profiles.remove(*actual_idx);
        
        if let Err(e) = self.config.save() {
            self.add_log(format!("Failed to delete profile: {}", e), crate::config::LogLevel::Error);
            return false;
        }
        
        self.add_log(format!("Deleted profile: {}", profile_name), crate::config::LogLevel::Info);
        true
    }

    pub fn panel_help_lines(&self) -> Vec<ratatui::text::Line<'static>> {
        use ratatui::text::{Line, Span};
        let y = Style::default().fg(Color::Yellow);

        match self.active_panel {
            ActivePanel::Models => vec![
                Line::from(Span::styled("MODELS PANEL", y.add_modifier(Modifier::BOLD))),
                Line::from(""),
                Line::from("Displays your local GGUF models and their status."),
                Line::from(""),
                Line::from(vec![Span::styled("j / k / Arrow keys", y), Span::raw("  Navigate model list")]),
                Line::from(vec![Span::styled("Enter / l", y), Span::raw("  Load selected model into server")]),
                Line::from(vec![Span::styled("u", y), Span::raw("  Unload model from server")]),
                Line::from(vec![Span::styled("Ctrl+D", y), Span::raw("  Delete model (with confirmation)")]),
                Line::from(""),
                Line::from("In search mode (/):"),
                Line::from(vec![Span::styled("Enter", y), Span::raw("  Execute search")]),
                Line::from(vec![Span::styled("Esc", y), Span::raw("  Exit search")]),
                Line::from(vec![Span::styled("l", y), Span::raw("  View available GGUF files")]),
                Line::from(vec![Span::styled("S", y), Span::raw("  Cycle sort order (Relevance/Downloads/Likes/Trending/Created)")]),
                Line::from(vec![Span::styled("B", y), Span::raw("  Go back one page")]),
                Line::from(vec![Span::styled("Down at bottom", y), Span::raw("  Load more results (infinite scroll)")]),
                Line::from(vec![Span::styled("R", y), Span::raw("  Fetch and view README")]),
                Line::from(""),
                Line::from(vec![Span::styled("Shift+← / →", y), Span::raw("  Resize panel split (20%-80%)")]),
                Line::from(vec![Span::styled("Mouse drag on border", y), Span::raw("  Resize panel split")]),
                Line::from(""),
                Line::from(vec![Span::styled("Shift+A", y), Span::raw("  About box (GPLv3)")]),
            ],
            ActivePanel::Log => vec![
                Line::from(Span::styled("LOG PANEL", y.add_modifier(Modifier::BOLD))),
                Line::from(""),
                Line::from("Live output from the llama.cpp server."),
                Line::from(""),
                Line::from(vec![Span::styled("j / k / Arrow keys", y), Span::raw("  Scroll log (Manual mode)")]),
                Line::from(vec![Span::styled("f", y), Span::raw("  Toggle Follow mode")]),
                Line::from(vec![Span::styled("g", y), Span::raw("  Jump to top (Manual mode)")]),
                Line::from(vec![Span::styled("G", y), Span::raw("  Jump to bottom (Follow mode)")]),
                Line::from(vec![Span::styled("Enter", y), Span::raw("  Expand log (fills screen)")]),
                Line::from(vec![Span::styled("Esc", y), Span::raw("  Collapse log")]),
                Line::from(""),
                Line::from(vec![Span::styled("Shift+A", y), Span::raw("  About box (GPLv3)")]),
            ],
            ActivePanel::ServerSettings => {
                vec![
                    Line::from(Span::styled("SERVER SETTINGS", y.add_modifier(Modifier::BOLD))),
                    Line::from(""),
                    Line::from("Configuration for the llama.cpp server."),
                    Line::from(""),
                    Line::from(vec![Span::styled("j / k", y), Span::raw("  Select setting")]),
                    Line::from(vec![Span::styled("Enter", y), Span::raw("  Toggle value")]),
                    Line::from(vec![Span::styled("Left / Right", y), Span::raw("  Adjust value")]),
                    Line::from(""),
                    Line::from(vec![Span::styled("Host", y), Span::raw("  Bind address (127.0.0.1 or 0.0.0.0)")]),
                    Line::from(vec![Span::styled("Backend", y), Span::raw("  Acceleration backend (cpu / vulkan / rocm)")]),
                    Line::from(vec![Span::styled("Threads", y), Span::raw("  CPU threads for generation (1 to max)")]),
                    Line::from(vec![Span::styled("Threads Batch", y), Span::raw("  CPU threads for batch processing (1 to 32)")]),
                    Line::from(vec![Span::styled("Mode", y), Span::raw("  Server mode (Normal / Router)")]),
                    Line::from(vec![Span::styled("API Endpoint", y), Span::raw("  Enable API proxy (True/False)")]),
                    Line::from(vec![Span::styled("API Port", y), Span::raw(self.get_api_port_str())]),
                    Line::from(""),
                    Line::from(vec![Span::styled("Shift+A", y), Span::raw("  About box (GPLv3)")]),
                ]
            }
            ActivePanel::LlmSettings => vec![
                Line::from(Span::styled("LLM SETTINGS", y.add_modifier(Modifier::BOLD))),
                Line::from(""),
                Line::from("Fine-tuned settings for loading and running a model."),
                Line::from(""),
                Line::from(vec![Span::styled("j / k", y), Span::raw("  Navigate settings")]),
                Line::from(vec![Span::styled("Enter", y), Span::raw("  Apply value")]),
                Line::from(vec![Span::styled("Left / Right", y), Span::raw("  Adjust value")]),
                Line::from(vec![Span::styled("0-9, -, .", y), Span::raw("  Type numeric value  ·  Ctrl+F7/8/9 switch panels")]),
                Line::from(vec![Span::styled("Esc", y), Span::raw("  Cancel edit")]),
                Line::from(""),
                Line::from(vec![Span::styled("Ctrl+S", y), Span::raw("  Save settings for selected model")]),
                Line::from(vec![Span::styled("Ctrl+R", y), Span::raw("  Reset to defaults")]),
                Line::from(vec![Span::styled("Ctrl+E", y), Span::raw("  Toggle enabled/disabled")]),
                Line::from(""),
                Line::from(vec![Span::styled("--- Loading ---", y)]),
                Line::from(vec![Span::styled("Context", y), Span::raw("  Context window size in tokens. Determines how much of the conversation history is kept in memory. A larger context allows longer conversations but uses more RAM. Typical: 8192-65536 depending on model and RAM.")]),
                Line::from(vec![Span::styled("Prompt", y), Span::raw("  System prompt preset. Pre-configured prompts that shape how the model behaves (e.g., 'coder', 'assistant', 'creative'). Affects the model's personality and output style.")]),
                Line::from(vec![Span::styled("Keep in memory", y), Span::raw("  Lock model weights in RAM (mlock). Prevents the OS from swapping model weights to disk. Slows model load time but ensures faster inference once loaded. Useful for repeated use.")]),
                Line::from(""),
                Line::from(vec![Span::styled("--- GPU Offload ---", y)]),
                Line::from(vec![Span::styled("GPU Layers", y), Span::raw("  How many model layers to offload to GPU. Arrow keys cycle: Auto → 1 → 2 → ... → N → All → Auto. Auto lets llama.cpp decide based on VRAM. All loads every layer (999). Specific number sets exact offload count.")]),
                Line::from(vec![Span::styled("Flash Attention", y), Span::raw("  Enable Flash Attention (flash-attn) for faster inference. Requires compatible GPU (Ampere+ / Ada). Significantly speeds up long-context inference. Only works with certain GGUF formats.")]),
                Line::from(vec![Span::styled("KV Cache Offload", y), Span::raw("  Offload KV cache to RAM when GPU memory is full. Allows larger batch sizes and contexts at the cost of some speed. Useful when VRAM is limited but you still want longer conversations.")]),
                Line::from(vec![Span::styled("Cache Type K / V", y), Span::raw("  Quantization precision for KV cache (K = keys, V = values). Lower precision (e.g., Q4, Q8) saves VRAM but may slightly reduce quality. Default is usually FP16. Use lower values if running out of VRAM.")]),
                Line::from(vec![Span::styled("Active Experts", y), Span::raw("  Number of MoE (Mixture of Experts) experts to activate per token. -1 = auto (all active). Reducing this speeds up inference for MoE models like Mixtral but may reduce quality. Typical: 2-8 for Mixtral.")]),
                Line::from(""),
                Line::from(vec![Span::styled("--- Evaluation ---", y)]),
                Line::from(vec![Span::styled("Eval Batch", y), Span::raw("  Batch size for evaluation (inference). Larger batches use more VRAM but can improve throughput via parallelism. Small values (1-8) for low VRAM, larger (16-128) for high VRAM setups.")]),
                Line::from(vec![Span::styled("Unified KV", y), Span::raw("  Share KV cache across sequences. Reduces VRAM usage when running multiple requests by reusing allocated cache. May slightly reduce performance but enables more concurrent users.")]),
                Line::from(vec![Span::styled("Max Concurrent Pred", y), Span::raw("  Maximum number of models that can run simultaneously. Press Enter to open a picker that shows how context length divides per model. Each model needs its own VRAM/CPU resources.")]),
                Line::from(""),
                Line::from(vec![Span::styled("--- Sampling ---", y)]),
                Line::from(vec![Span::styled("Seed", y), Span::raw("  Random seed for reproducible outputs. -1 = random (default). Set to a fixed value for deterministic, repeatable responses — useful for debugging or testing prompts.")]),
                Line::from(vec![Span::styled("Temp", y), Span::raw("  Sampling temperature. Controls creativity: 0 = deterministic (most predictable), 0.7 = balanced, 1.0+ = creative. Lower values produce more focused, factual outputs. Typical: 0.7-0.9 for general use.")]),
                Line::from(vec![Span::styled("Top-k", y), Span::raw("  Only consider the top k most likely tokens at each step. Smaller top-k (e.g., 10-40) makes output more deterministic. Larger values allow more variety. Typical: 40-50. Set to 0 to disable.")]),
                Line::from(vec![Span::styled("Top-p", y), Span::raw("  Nucleus sampling: only consider tokens whose cumulative probability reaches p. Smaller top-p (e.g., 0.9) is more conservative, larger (e.g., 0.95-0.99) allows more variety. Often preferred over top-k. Typical: 0.9-0.95.")]),
                Line::from(vec![Span::styled("Min P", y), Span::raw("  Minimum probability threshold relative to the most likely token. Tokens below min_p * max_prob are excluded. A filter that's more principled than top-k/top-p for controlling diversity. Typical: 0.01-0.1.")]),
                         Line::from(vec![Span::styled("Max Tokens", y), Span::raw("  Maximum number of tokens to generate in the response. Prevents runaway responses. Set to 0 or Disabled for no limit. Typical: 4096-8192 for chat, higher for code generation.")]),
                         Line::from(""),
                         Line::from(vec![Span::styled("Shift+A", y), Span::raw("  About box (GPLv3)")]),
            ],
            ActivePanel::ActiveModel => vec![
                Line::from(Span::styled("ACTIVE MODEL PANEL", y.add_modifier(Modifier::BOLD))),
                Line::from(""),
                Line::from("Displays metrics for the currently loaded model."),
                Line::from(""),
                Line::from("Shows Tokens/s, context usage (progress bar), CPU, RAM, and VRAM."),
            ],
            ActivePanel::ModelInfo => vec![
                Line::from(Span::styled("MODEL INFO PANEL", y.add_modifier(Modifier::BOLD))),
                Line::from(""),
                Line::from("GGUF metadata for the selected model."),
                Line::from(""),
                Line::from("Displays file name, size, architecture, layers, and training context."),
            ],
            ActivePanel::Profiles => vec![
                Line::from(Span::styled("PROFILES PANEL", y.add_modifier(Modifier::BOLD))),
                Line::from(""),
                Line::from("Saved presets of settings for quick switching."),
                Line::from(""),
                Line::from(vec![Span::styled("j / k", y), Span::raw("  Select profile")]),
                Line::from(vec![Span::styled("Enter", y), Span::raw("  Apply profile settings")]),
                Line::from(vec![Span::styled("s", y), Span::raw("  Save current settings as new profile")]),
                Line::from(vec![Span::styled("d", y), Span::raw("  Delete user profile")]),
                Line::from(vec![Span::styled("Esc", y), Span::raw("  Back to settings")]),
            ],
            ActivePanel::SystemPromptPresets => vec![
                Line::from(Span::styled("SYSTEM PROMPT PRESETS", y.add_modifier(Modifier::BOLD))),
                Line::from(""),
                Line::from("Named system prompts for different use cases."),
                Line::from(""),
                Line::from(vec![Span::styled("j / k", y), Span::raw("  Select preset")]),
                Line::from(vec![Span::styled("Enter", y), Span::raw("  Apply preset")]),
                Line::from(vec![Span::styled("e", y), Span::raw("  Edit selected preset")]),
                Line::from(vec![Span::styled("n", y), Span::raw("  Create new preset")]),
                Line::from(vec![Span::styled("Esc", y), Span::raw("  Back to settings")]),
            ],
            ActivePanel::SearchReadme => vec![
                Line::from(Span::styled("README PANEL", y.add_modifier(Modifier::BOLD))),
                Line::from(""),
                Line::from("README documentation for the selected model."),
                Line::from(""),
                Line::from(vec![Span::styled("j / k / Arrow keys", y), Span::raw("  Scroll")]),
                Line::from(vec![Span::styled("h / l", y), Span::raw("  Scroll horizontally")]),
                Line::from(vec![Span::styled("Enter", y), Span::raw("  Expand to fullscreen")]),
                Line::from(vec![Span::styled("Esc", y), Span::raw("  Collapse / Exit")]),
            ],
            ActivePanel::Downloads => vec![
                Line::from(Span::styled("DOWNLOADS PANEL", y.add_modifier(Modifier::BOLD))),
                Line::from(""),
                Line::from("Active model downloads from HuggingFace."),
                Line::from(""),
                Line::from(vec![Span::styled("j / k / Arrow keys", y), Span::raw("  Select download")]),
                Line::from(vec![Span::styled("p", y), Span::raw("  Pause / Resume selected download")]),
                Line::from(vec![Span::styled("⌥C", y), Span::raw("  Cancel selected download")]),
            ],
            // Note: BenchTuneSetup is handled via GlobalMode logic in render/event,
            // but we can add a placeholder if we want dedicated help for it.
        }
    }

    pub fn fetch_host_picker_entries() -> Vec<(String, String)> {
        let mut entries = Vec::new();
        
        // Always include these two at the top
        entries.push(("127.0.0.1".to_string(), "localhost".to_string()));
        entries.push(("0.0.0.0".to_string(), "All interfaces".to_string()));
        
        // Add real network interfaces
        if let Ok(ifaces) = local_ip_address::list_afinet_netifas() {
            for (name, ip) in ifaces {
                let ip_str = ip.to_string();
                if ip_str != "127.0.0.1" && ip_str != "0.0.0.0" {
                    entries.push((ip_str, name));
                }
            }
        }
        
        entries
    }

    pub fn fetch_backend_picker_entries(&self) -> Vec<(crate::models::Backend, Option<String>)> {
        let platform = hardware::detect_platform();
        let mut entries = Vec::new();

        // 1. Add "latest" entries for backends supported on this platform
        match platform {
            crate::backend::hardware::Platform::Linux => {
                entries.push((crate::models::Backend::Cpu, None));
                entries.push((crate::models::Backend::Vulkan, None));
                if hardware::is_arm64() {
                    entries.push((crate::models::Backend::CpuArm64, None));
                }
                match hardware::detect_gpu_vendor() {
                    hardware::GpuVendor::Amd => {
                        entries.push((crate::models::Backend::Rocm, None));
                        entries.push((crate::models::Backend::RocmLemonade, None));
                    }
                    hardware::GpuVendor::Nvidia => {
                        entries.push((crate::models::Backend::Cuda, None));
                    }
                    _ => {}
                }
            }
            crate::backend::hardware::Platform::Windows => {
                entries.push((crate::models::Backend::CpuWindows, None));
                entries.push((crate::models::Backend::VulkanWindows, None));
                match hardware::detect_gpu_vendor() {
                    hardware::GpuVendor::Nvidia => {
                        entries.push((crate::models::Backend::CudaWindows12_4, None));
                        entries.push((crate::models::Backend::CudaWindows13_1, None));
                    }
                    hardware::GpuVendor::Amd => {
                        entries.push((crate::models::Backend::HipWindows, None));
                    }
                    _ => {}
                }
            }
            crate::backend::hardware::Platform::Macos => {
                if hardware::is_arm64() {
                    entries.push((crate::models::Backend::CpuMacosArm64, None));
                } else {
                    entries.push((crate::models::Backend::CpuMacosX64, None));
                }
            }
        }

        // 2. Add all installed versions (filtered by platform)
        let installed = crate::backend::hub::list_installed_backends();
        for (b, tag) in installed {
            if crate::backend::hardware::backend_supported(b, platform) {
                entries.push((b, Some(tag)));
            }
        }
        
        entries
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

    pub fn is_loading(&self) -> bool {
        self.model_states.values().any(|s| matches!(s, crate::models::ModelState::Loading))
    }

    pub fn tick_spinner(&mut self) {
        if self.is_loading() {
            let spinner_interval = std::time::Duration::from_millis(150);
            if self.last_spinner_time.is_none()
                || self.last_spinner_time.unwrap().elapsed() > spinner_interval
            {
                self.loading_spinner = (self.loading_spinner + 1) % 4;
                self.last_spinner_time = Some(tokio::time::Instant::now());
                self.set_redraw();
            }
        }
    }

    pub fn ensure_download_channel(&mut self) -> tokio::sync::broadcast::Sender<crate::models::DownloadState> {
        if self.download_rx.is_none() {
            let (tx, rx) = tokio::sync::broadcast::channel(10);
            self.download_tx = Some(tx);
            self.download_rx = Some(rx);
        }
        self.download_tx.as_ref().unwrap().clone()
    }

    pub async fn start_pending_download(&mut self) {
        if let Some((model_id, filename, download_url, file_size)) = self.pending_download.take() {
            let models_dir = self.config.models_dir.clone();
            let dest = models_dir.join(&filename);
            let free_space = hub::get_free_space_bytes(&models_dir);
            if file_size > free_space {
                self.add_log(
                    format!(
                        "Not enough disk space to download {}: need {} but only {} available",
                        filename,
                        format_size(file_size),
                        format_size(free_space)
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
                let result = hub::download_file(&model_id_clone, &filename_clone, &url_clone, &dest, &mut state, download_state_clone, tx_clone).await;
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
        let bin_dir = hub::get_backend_dir(backend, &tag);
        if bin_dir.exists() {
            if let Err(e) = std::fs::remove_dir_all(&bin_dir) {
                self.add_log(format!("Failed to delete backend: {}", e), crate::config::LogLevel::Error);
            } else {
                self.add_log(format!("Deleted backend {} ({})", backend, tag), crate::config::LogLevel::Info);
                let new_entries = self.fetch_backend_picker_entries();
                if let crate::tui::app::GlobalMode::BackendPicker { entries, selected } = &mut self.global_mode {
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
                            self.models = Self::discover_models(&self.config.models_dir);
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
                self.models_mode = crate::tui::app::ModelsMode::BenchTune;
                let bench_tune_config_clone = bench_tune_config.clone();
                let settings_clone = settings_clone.clone();
                let model_clone = model.clone();
                let tx_tune_clone = tx_tune.clone();
                let spawn_log_tx_clone = tx.clone();
                let handle = tokio::spawn(async move {
                    let results = benchmark::run_bench_tune(
                        &config_clone,
                        &bench_tune_config_clone,
                        &model_clone,
                        &settings_clone,
                        tx_tune_clone,
                        spawn_log_tx_clone,
                    ).await.map_err(|e| e.to_string());
                    (results, display_name, bench_tune_config_clone)
                });
                self.bench_tune_task_handle = Some(handle);
                self.spawn_log_tx = Some(tx);
                self.set_redraw();
                self.bench_tune_rx = Some(rx_tune);
            } else {
                let handle = tokio::spawn(async move {
                    server::spawn_server(&config_clone, model_clone.as_ref(), &settings_clone, tx_clone, download_tx_clone, server_mode_clone, router_max_models_clone).await
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
                            let _ = serve_api::start_api_server(
                                addr, None, server_port, model_name, pid, shutdown_rx
                            ).await;
                        });
                        self.api_proxy_handle = Some(handle);
                        self.add_log(format!("API proxy started on port {}", port), crate::config::LogLevel::Info);
                    }
                    self.loading_phases = std::iter::once(crate::tui::app::LoadingPhase::Complete).collect();
                    self.last_active_phase = Some(crate::tui::app::LoadingPhase::Complete);
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
            let mut m = match server::get_metrics(&host, port, None, Some(pid)).await {
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
                && let Ok(model_metrics) = server::get_metrics(&host, port, Some(&name), Some(pid)).await
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
            if let Ok(models) = server::list_models(&host, port).await
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
                                match benchmark::save_results(&bench_results, &output_dir, &bench_config).await {
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
                                let _ = server::unload_model(&host, port, &model_name, model_path_str.as_deref()).await;
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
                if self.loading_phases.contains(&crate::tui::app::LoadingPhase::Complete) || self.loading_phases.contains(&crate::tui::app::LoadingPhase::ServerListening) {
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
                        if let Err(e) = server::load_model(&host, port, &model_name_clone, model_path_clone.as_deref()).await {
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
        if !matches!(self.global_mode, crate::tui::app::GlobalMode::Confirmation { .. }) {
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
                            if let Err(e) = server::unload_model(&host, port, &model_name_clone, model_path_clone.as_deref()).await {
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
            match server::kill_server(handle).await {
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
                        self.model_states.insert(name, crate::models::ModelState::Available);
                    }
                    self.loaded_model_names.lock().unwrap().clear();
                    self.loading_phases = HashSet::new();
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
                    hub::search_models(&query_clone, search_limit, offset_clone).await
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
                            if let ModelsMode::Search { results, has_more, loading, .. } = &mut self.models_mode {
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
                            if let ModelsMode::Search { results, loading, has_more, .. } = &mut self.models_mode {
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
                        if let ModelsMode::Search { loading, .. } = &mut self.models_mode {
                            *loading = false;
                        }
                    }
                    Err(e) => {
                        self.add_log(format!("Search task error: {}", e), crate::config::LogLevel::Error);
                        if let ModelsMode::Search { loading, .. } = &mut self.models_mode {
                            *loading = false;
                        }
                    }
                }
            }
            self.search_loading = false;
            self.set_redraw();
        }
    }

    pub fn render<T: ratatui::backend::Backend>(&mut self, terminal: &mut ratatui::Terminal<T>) -> std::io::Result<()> {
        if self.needs_redraw {
            terminal.draw(|frame| crate::tui::render::render(frame, self))?;
            self.needs_redraw = false;
        }
        Ok(())
    }

    pub fn discover_models(dir: &std::path::Path) -> Vec<crate::models::DiscoveredModel> {
        let mut models = Vec::new();
        crate::backend::hub::walk_dir_recursive(dir, 0, 10, &mut |entry| {
            let path = entry.path();
            if path.is_file() && path.extension().map(|e| e == "gguf").unwrap_or(false) {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    let name = name.to_string();
                    let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                    let display_name = path
                        .strip_prefix(dir)
                        .ok()
                        .and_then(|p| p.to_str())
                        .unwrap_or(&name)
                        .to_string();
                    models.push(crate::models::DiscoveredModel {
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

    pub fn reset_to_defaults(&mut self) {
        let defaults = crate::models::ModelSettings::default();
        self.settings = defaults;
        // Clear dirty flag by updating the cache snapshot to match new settings
        self.model_settings_cache = self.settings.clone();
        // Reset model metadata to avoid stale values
        self.model_total_layers = 0;
        self.model_hidden_size = 0;
        self.model_n_ctx_train = 0;
        self.model_n_head = 0;
        self.model_n_kv_head = 0;
        self.vram_estimate = 0;
        self.settings.is_mtp = false;
        self.settings.draft_tokens = 0;
        self.settings_render_cache = None;
        self.add_log("Reset LLM Settings to defaults", crate::config::LogLevel::Info);
    }
}

#[cfg(test)]
mod tests {
        use super::*;

        fn make_app() -> App {
            let config = crate::config::Config {
                models_dir: std::path::PathBuf::new(),
                llama_server: std::path::PathBuf::new(),
                default: crate::config::DefaultParams::default(),
                model_overrides: std::collections::HashMap::new(),
                profiles: Vec::new(),
                system_prompt_presets: Vec::new(),
                rpc_workers: Vec::new(),
                search_limit: 50,
            };
            let mut app = App::new(config);
            app.loading_phases.clear();
            app.last_active_phase = None;
           app.loading_progress = 0.0;
            app.progress_target = 0.0;
            app.load_progress = LoadProgress {
                layers_total: None,
                layers_loaded: None,
                tensors_total: None,
                tensors_loaded: 0,
                buffers: vec![],
            };
            app.last_spinner_time = None;
            app
        }

     #[test]
        fn test_progress_server_starting() {
            let mut app = make_app();
            app.loading_phases.insert(LoadingPhase::ServerStarting);
            app.last_active_phase = Some(LoadingPhase::ServerStarting);
            app.compute_progress();
            // With only one phase, we get the full weight
            assert!((app.loading_progress - 0.08).abs() < 0.001);
        }

        #[test]
        fn test_progress_with_layers() {
            let mut app = make_app();
            app.loading_phases.insert(LoadingPhase::ServerStarting);
            app.loading_phases.insert(LoadingPhase::LoadingModel);
            app.loading_phases.insert(LoadingPhase::LoadingMeta);
            app.loading_phases.insert(LoadingPhase::LoadingTensors);
            app.last_active_phase = Some(LoadingPhase::LoadingTensors);
            app.load_progress.layers_loaded = Some(16);
            app.load_progress.layers_total = Some(32);
            app.compute_progress();
            // Should be ~0.22 + 0.5 * 0.70 = 0.57
            assert!((app.loading_progress - 0.57).abs() < 0.01);
        }

        #[test]
        fn test_progress_complete() {
            let mut app = make_app();
            app.loading_phases.insert(LoadingPhase::Complete);
            app.last_active_phase = Some(LoadingPhase::Complete);
            app.compute_progress();
            assert!((app.loading_progress - 1.0).abs() < 0.001);
        }

        #[test]
        fn test_progress_all_phases() {
            let mut app = make_app();
            app.loading_phases.insert(LoadingPhase::ServerStarting);
            app.loading_phases.insert(LoadingPhase::LoadingModel);
            app.loading_phases.insert(LoadingPhase::LoadingMeta);
            app.loading_phases.insert(LoadingPhase::LoadingTensors);
            app.loading_phases.insert(LoadingPhase::ServerListening);
            app.last_active_phase = Some(LoadingPhase::ServerListening);
            app.compute_progress();
            // All phases complete = 1.0 (ServerListening interpolates at 0.8)
            assert!((app.loading_progress - 0.98).abs() < 0.01);
        }
    }
