use crate::backend::server::ServerHandle;
use crate::config::{Config, LogEntry, Profile};
use crate::models::{
    DiscoveredModel, ModelSettings, ModelState, SearchResult, SearchSort, ServerMetrics,
};
use chrono::Local;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::TableState;

use std::collections::VecDeque;
use std::sync::{Arc, atomic::AtomicBool};

use gguf_rs;

// Maximum cache size for GGUF metadata to prevent unbounded memory growth


/// Which panel has focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivePanel {
    Models,
    Log,
    Downloads,
    ServerSettings,
    LlmSettings,
    Profiles,
    SystemPromptPresets,
    SearchReadme,
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
   /// Version picker mode: selecting llama.cpp release version.
    VersionPicker {
        releases: Vec<crate::models::LlamaCppRelease>,
        selected_idx: usize,
        previous_mode: Box<ModelsMode>,
    },
}

/// Global mode that overlays all panels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GlobalMode {
    Normal,
    Help,
    DeleteConfirmation,
    ResetConfirmation,
    ExitConfirmation,
    CmdLine { cmd_line: String },
}

/// Phase of model loading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    pub log_entries: VecDeque<LogEntry>,
    pub active_panel: ActivePanel,
    pub log_expanded: bool,
    pub log_scroll_offset: u16,
    pub settings_selected_idx: usize,
    pub server_settings_selected_idx: usize, // 0=Host, 1=Backend
    pub settings_edit_buffer: String,
    pub settings_scroll_offset: u16,

    pub profiles_scroll_offset: u16,
    pub system_prompt_presets_scroll_offset: u16,
    pub readme_scroll_offset: u16,
    pub readme_scroll_offset_x: u16,
    pub readme_expanded: bool,
    pub editing_preset: Option<usize>,
    pub edit_cursor_pos: usize,
    pub gguf_metadata_cache: std::collections::HashMap<String, crate::models::GgufMetadata>,
    pub vram_estimate: u64, // estimated VRAM in MiB
    pub model_total_layers: u32, // total number of layers in the model
    pub model_hidden_size: u32, // hidden dimension size
    pub model_n_ctx_train: u32, // n_ctx_train from GGUF metadata
    pub model_n_head: u32, // attention head count (n_head)
    pub model_n_kv_head: u32, // KV head count (n_kv_head)
    pub max_threads: u32, // max threads = physical CPU cores
    pub pending_download: Option<(String, String, String)>, // (model_id, filename, download_url)
    pub pending_deletion: Option<std::path::PathBuf>,
    pub pending_spawn: Option<(Option<DiscoveredModel>, ModelSettings)>,
    pub pending_api_load: Option<(String, Option<String>)>, // (id, absolute_path)
    pub pending_api_unload: Option<(String, Option<String>)>, // (id, absolute_path)
    pub pending_kill: Option<ServerHandle>,
    pub downloading: bool,
    pub server_log_rx: Option<tokio::sync::mpsc::Receiver<String>>,
    pub metrics_rx: Option<tokio::sync::mpsc::Receiver<crate::models::ServerMetrics>>,
    pub global_mode: GlobalMode,
    pub loading_phases: Vec<LoadingPhase>,
    pub loading_progress: f32,
    pub cancelled: Option<Arc<AtomicBool>>,
    pub server_handle: Option<ServerHandle>,
    pub metrics_task_handle: Option<tokio::task::JoinHandle<()>>,
    pub sync_task_handle: Option<tokio::task::JoinHandle<()>>,
    pub sync_rx: Option<tokio::sync::mpsc::Receiver<Vec<(String, String, Option<String>)>>>,
    pub spawn_task_handle: Option<tokio::task::JoinHandle<Result<(String, ServerHandle, String), String>>>,
    pub spawn_log_tx: Option<tokio::sync::mpsc::Sender<String>>,
    pub metrics_model_name: Arc<std::sync::Mutex<Option<String>>>,
    pub loaded_model_names: Arc<std::sync::Mutex<Vec<String>>>,
    pub needs_redraw: bool,
   pub panel_help: bool,
    pub panel_help_offset: u16,
    /// Last error message captured from the log (used for Failed state display).
    pub last_error_message: Option<String>,
    /// Cached file modification time for debouncing metadata parsing.
    last_metadata_parse: (std::path::PathBuf, std::time::SystemTime),
    pub cached_cpu_versions: Vec<String>,
    pub cached_vulkan_versions: Vec<String>,
    pub cached_rocm_versions: Vec<String>,
    pub picker_backend: crate::models::Backend,
    pub version_picker_scroll_offset: u16,
    pub version_picker_show_cached: bool,
    pub pending_version_picker: bool,
    /// Pending search load (page) — set when user presses B or Down at bottom.
    pub pending_search_load: Option<(String, u32)>, // (query, offset)
     /// Whether search results are currently being loaded.
    pub search_loading: bool,
}

impl App {
    pub fn new(config: Config) -> Self {
        let mut log = VecDeque::new();
        log.push_back(LogEntry::new("Starting llm-manager...", crate::config::LogLevel::Info));
        let default_params = config.default.clone();
        let settings: ModelSettings = default_params.into();
        Self {
            running: true,
            config,
            models: Vec::new(),
            selected_model_idx: None,
            models_mode: ModelsMode::List,
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
            log_entries: log,
            active_panel: ActivePanel::Models,
            log_expanded: false,
            log_scroll_offset: 0,
            settings_selected_idx: 0,
            server_settings_selected_idx: 0,
            settings_edit_buffer: String::new(),
            settings_scroll_offset: 0,
            profiles_scroll_offset: 0,
            system_prompt_presets_scroll_offset: 0,
            readme_scroll_offset: 0,
            readme_scroll_offset_x: 0,
            readme_expanded: false,
            editing_preset: None,
            edit_cursor_pos: 0,
            gguf_metadata_cache: Default::default(),
            vram_estimate: 0,
            model_total_layers: 0,
            model_hidden_size: 0,
            model_n_ctx_train: 0,
            model_n_head: 0,
            model_n_kv_head: 0,
            max_threads: crate::config::physical_cores(),
            pending_download: None,
            pending_deletion: None,
            pending_spawn: None,
            pending_api_load: None,
            pending_api_unload: None,
            pending_kill: None,
            downloading: false,
            server_log_rx: None,
            metrics_rx: None,
            global_mode: GlobalMode::Normal,
            loading_phases: Vec::new(),
            loading_progress: 0.0,
            cancelled: None,
            server_handle: None,
            metrics_task_handle: None,
            sync_task_handle: None,
            sync_rx: None,
            spawn_task_handle: None,
            spawn_log_tx: None,
            metrics_model_name: Arc::new(std::sync::Mutex::new(None)),
            loaded_model_names: Arc::new(std::sync::Mutex::new(Vec::new())),
            needs_redraw: true,
            panel_help: false,
            panel_help_offset: 0,
            last_error_message: None,
last_metadata_parse: (std::path::PathBuf::new(), std::time::SystemTime::now()),
            pending_version_picker: false,
            cached_cpu_versions: Vec::new(),
            cached_vulkan_versions: Vec::new(),
            cached_rocm_versions: Vec::new(),
            picker_backend: crate::models::Backend::Cpu,
            version_picker_scroll_offset: 0,
            version_picker_show_cached: false,
            pending_search_load: None,
            search_loading: false,
        }
    }

    pub fn selected_model(&self) -> Option<&DiscoveredModel> {
        self.selected_model_idx.and_then(|i| self.models.get(i))
    }

   pub fn selected_model_settings(&self) -> ModelSettings {
        let mut base: ModelSettings = self.config.default.clone().into();
        // Check for per-model overrides
        if let Some(model) = self.selected_model() {
            if let Some(override_cfg) = self.config.model_overrides.get(&model.name) {
                override_cfg.apply(&mut base);
            }
        }
        base
    }

  pub fn refresh_cached_versions(&mut self) {
        let bin_dir = dirs::data_local_dir()
            .unwrap_or_default()
            .join("llm-manager")
            .join("bin");
        if let Ok(entries) = std::fs::read_dir(&bin_dir) {
            let mut cpu_versions: Vec<String> = Vec::new();
            let mut vulkan_versions: Vec<String> = Vec::new();
            let mut rocm_versions: Vec<String> = Vec::new();
            for entry in entries.flatten() {
                if let Ok(meta) = entry.metadata() {
                    if meta.is_dir() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        // Extract version from llama-server-{cpu|vulkan|rocm}-{version}
                        if let Some(pos) = name.rfind('-') {
                            let version = &name[pos + 1..];
                            if !version.is_empty() {
                                if name.starts_with("llama-server-cpu-") && !cpu_versions.contains(&version.to_string()) {
                                    cpu_versions.push(version.to_string());
                                } else if name.starts_with("llama-server-vulkan-") && !vulkan_versions.contains(&version.to_string()) {
                                    vulkan_versions.push(version.to_string());
                                } else if name.starts_with("llama-server-rocm-") && !rocm_versions.contains(&version.to_string()) {
                                    rocm_versions.push(version.to_string());
                                }
                            }
                        }
                    }
                }
            }
            cpu_versions.sort();
            vulkan_versions.sort();
            rocm_versions.sort();
            self.cached_cpu_versions = cpu_versions;
            self.cached_vulkan_versions = vulkan_versions;
            self.cached_rocm_versions = rocm_versions;
        }
    }

    pub fn add_log(&mut self, message: impl Into<String>, level: crate::config::LogLevel) {
        let msg = message.into();
        match level {
            crate::config::LogLevel::Info => tracing::info!("{}", msg),
            crate::config::LogLevel::Warning => tracing::warn!("{}", msg),
            crate::config::LogLevel::Error => tracing::error!("{}", msg),
        }

        // Detect loading phases from llama-server log output
        let upper = msg.to_uppercase();
        if upper.contains("LLAMA_MODEL_LOADER") || upper.contains("LOADING MODEL") {
            self.last_error_message = None;
            if !self.loading_phases.contains(&LoadingPhase::LoadingModel) {
                self.loading_phases.push(LoadingPhase::LoadingModel);
            }
        }
        if upper.contains("LOADED META") || upper.contains("META DATA") {
            self.last_error_message = None;
            if !self.loading_phases.contains(&LoadingPhase::LoadingMeta) {
                self.loading_phases.push(LoadingPhase::LoadingMeta);
            }
        }
        if upper.contains("LOADED TENSORS") || upper.contains("TENSORS") {
            self.last_error_message = None;
            if !self.loading_phases.contains(&LoadingPhase::LoadingTensors) {
                self.loading_phases.push(LoadingPhase::LoadingTensors);
            }
        }
        if upper.contains("SERVER LISTENING") || upper.contains("HTTP SERVER LISTENING") {
            if !self.loading_phases.contains(&LoadingPhase::ServerListening) {
                self.loading_phases.push(LoadingPhase::ServerListening);
            }
        }

        // Detect successful model load (including router mode)
        if upper.contains("LOADED SUCCESSFULLY")
            || upper.contains("LLAMA_NEW_CONTEXT_WITH_MODEL")
            || upper.contains("MAIN: MODEL LOADED")
            || upper.contains("UPDATE_SLOTS: ALL SLOTS ARE IDLE")
        {
            self.loading_phases.push(LoadingPhase::Complete);
            self.loading_progress = 1.0;
            self.last_error_message = None;

            // Transition any Loading models to Loaded
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
                    self.loaded_model_names.lock().unwrap().push(name);
                }
            }
        }

        // Detect model load failure or crash
        let is_crash =
            upper.contains("LLAMA-SERVER") && (upper.contains("EXITED") || upper.contains("TERMINATED"));
        let is_error = is_crash
            || upper.contains("ERROR")
            || upper.contains("FAILED TO LOAD")
            || upper.contains("EXCEPTION")
            || upper.contains("VK::SYSTEMERROR")
            || upper.contains("OUTOFDEVICEMEMORY")
            || upper.contains("OUT OF MEMORY");

        if is_error {
            let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
            let mut error_msg = if upper.contains("OUTOFDEVICEMEMORY") || upper.contains("OUT OF MEMORY") {
                format!("Last Failed to load a model (OOM - {})", timestamp)
            } else {
                format!("Last Failed to load a model ({})", timestamp)
            };

            // If the server itself exited, make the message more specific
            if is_crash {
                error_msg = format!("Last Failed to load a model (Router Crash - {})", timestamp);
                if let Some(h) = self.server_handle.take() {
                    self.pending_kill = Some(h);
                }
            }

            self.last_error_message = Some(error_msg);
            self.reset_loading_state();
        }

        // Update progress based on observed phases
        let total_phases = 4; // ServerStarting, Model, Meta, Tensors
        let seen = self.loading_phases.len();
        if seen > 0 {
            self.loading_progress = (seen as f32) / (total_phases as f32);
        }

        // Trim before pushing to prevent memory spikes
        if self.log_entries.len() >= 500 {
            self.log_entries.pop_front();
        }
        self.log_entries.push_back(LogEntry::new(msg, level));
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

    /// Return the current number of search results.
    pub fn search_results_len(&self) -> usize {
        if let ModelsMode::Search { results, .. } = &self.models_mode {
            results.len()
        } else {
            0
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
                    self.loading_phases.push(LoadingPhase::Complete);
                }
            } else if matches!(self.model_states.get(&model.display_name), Some(ModelState::Loading)) {
                // Keep current loading progress if we are already loading this model
            } else {
                // Not loaded and not loading, reset progress
                self.loading_progress = 0.0;
                self.loading_phases.clear();
            }
        } else {
            let default_params = self.config.default.clone();
            self.model_settings_cache = default_params.into();
            self.model_total_layers = 0;
            self.model_hidden_size = 0;
            self.model_n_ctx_train = 0;
            self.vram_estimate = 0;
            self.loading_progress = 0.0;
            self.loading_phases.clear();
        }
        self.set_redraw();
    }

    /// Reset loading state (progress bar and model status) on failure.
    pub fn reset_loading_state(&mut self) {
        self.loading_phases.clear();
        self.loading_progress = 0.0;
        
        // Revert any Loading models to Failed
        let mut to_revert = Vec::new();
        for (name, state) in &self.model_states {
            if matches!(state, ModelState::Loading) {
                to_revert.push(name.clone());
            }
        }
        // Downgrade any Loaded models to Failed
        let mut to_fail = Vec::new();
        for (name, state) in &self.model_states {
            if matches!(state, ModelState::Loaded { .. }) {
                to_fail.push(name.clone());
            }
        }
        // Remove from loaded list
        for name in to_revert.iter().chain(to_fail.iter()) {
            self.loaded_model_names.lock().unwrap().retain(|n| n != name);
            let error = self.last_error_message.clone().unwrap_or_else(|| {
                let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
                format!("Last Failed to load a model ({})", timestamp)
            });
            self.model_states.insert(name.clone(), ModelState::Failed { error });
        }
        self.set_redraw();
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
        
        // 1. Check persistent cache first
        if let Some(cached) = self.gguf_metadata_cache.get(&key) {
            self.model_total_layers = cached.layers;
            self.model_hidden_size = cached.hidden_size;
            self.model_n_ctx_train = cached.n_ctx_train;
            self.model_n_head = cached.n_head;
            self.model_n_kv_head = cached.n_kv_head;
        }

        // Compute VRAM estimate now that metadata fields are populated.
        // Doing it here ensures the estimate is available immediately when
        // the model is selected, rather than waiting for a separate call to
        // `update_vram_estimate()` which may be skipped if hidden_size is
        // already set.
        if self.model_hidden_size > 0 {
            self.update_vram_estimate();
        }

        // 2. Debounce logic: only skip if we tried this EXACT file (path + mtime) very recently
        // and it wasn't GGUF or we failed to parse it.
        if let Ok(meta) = std::fs::metadata(&model.path) {
            let mtime = meta.modified().unwrap_or(std::time::SystemTime::now());
            let (last_path, last_mtime) = &self.last_metadata_parse;
            if last_path == &model.path && mtime == *last_mtime {
                // Already tried this version of the file and it's not in cache (meaning it failed or is not GGUF)
                return;
            }
            self.last_metadata_parse = (model.path.clone(), mtime);
        }

        // 3. Perform the actual parse
        let path_str = model.path.to_string_lossy();
        if let Ok(mut container) = gguf_rs::get_gguf_container(&path_str) {
            if let Ok(model_data) = container.decode() {
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

                if let Some(value) = model_data.metadata().get("general.architecture") {
                    if let Some(v) = value.as_str() { arch = v.to_string(); }
                }

                // Capabilities
                if model_data.metadata().contains_key("tokenizer.chat_template") {
                    capabilities.push("chat".to_string());
                }
                if let Some(value) = model_data.metadata().get("general.capabilities") {
                    if let Some(arr) = value.as_array() {
                        for v in arr {
                            if let Some(s) = v.as_str() {
                                capabilities.push(s.to_string());
                            }
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
                    self.add_log(&format!("GGUF parse: found 0 layers/hidden. Arch: {}. Sample keys: {:?}", arch, keys), crate::config::LogLevel::Info);
                }
                if !model_data.get_version().is_empty() {
                    file_type = model_data.get_version();
                }
                if !model_data.model_parameters().is_empty() {
                    model_parameters = model_data.model_parameters();
                }
                if let Some(value) = model_data.metadata().get("general.domain") {
                    if let Some(v) = value.as_str() { domain = v.to_string(); }
                }
                if let Some(value) = model_data.metadata().get("tokenizer.ggml.model") {
                    if let Some(v) = value.as_str() { tokenizer = v.to_string(); }
                }
                if let Some(value) = model_data.metadata().get("tokenizer.ggml.tokens") {
                    if let Some(arr) = value.as_array() {
                        vocab_size = arr.len() as u32;
                    }
                }

                self.model_total_layers = layers;
                self.model_hidden_size = hidden;
                self.model_n_ctx_train = n_ctx_train;
                self.model_n_head = n_head;
                self.model_n_kv_head = n_kv_head;

                // Cache the parsed metadata
                if layers > 0 || hidden > 0 {
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
                    });
                }
                self.set_redraw();

                // Compute VRAM estimate now that metadata is loaded
                if hidden > 0 {
                    self.update_vram_estimate();
                }
            }
        } else {
            // Log failure so user knows why metadata is missing
            if let Err(e) = gguf_rs::get_gguf_container(&path_str) {
                self.add_log(&format!("Failed to parse GGUF {}: {}", model.path.display(), e), crate::config::LogLevel::Error);
            } else {
                self.add_log(&format!("Failed to decode GGUF: {}", model.path.display()), crate::config::LogLevel::Error);
            }
        }
    }

    pub fn focus_next(&mut self) {
        self.active_panel = match self.active_panel {
            ActivePanel::Models => ActivePanel::Log,
            ActivePanel::Log => {
                if !self.download_progress.is_empty() {
                    ActivePanel::Downloads
                } else {
                    ActivePanel::ServerSettings
                }
            }
            ActivePanel::Downloads => ActivePanel::ServerSettings,
            ActivePanel::ServerSettings => ActivePanel::LlmSettings,
            ActivePanel::LlmSettings => ActivePanel::Models,
            _ => ActivePanel::Models,
        };
        self.set_redraw();
    }

    pub fn focus_prev(&mut self) {
        self.active_panel = match self.active_panel {
            ActivePanel::Models => ActivePanel::LlmSettings,
            ActivePanel::LlmSettings => ActivePanel::ServerSettings,
            ActivePanel::ServerSettings => {
                if !self.download_progress.is_empty() {
                    ActivePanel::Downloads
                } else {
                    ActivePanel::Log
                }
            }
            ActivePanel::Downloads => ActivePanel::Log,
            ActivePanel::Log => ActivePanel::Models,
            _ => ActivePanel::Models,
        };
        self.set_redraw();
    }

    /// Apply a profile's settings to the current settings.
    pub fn apply_profile(&mut self, profile: &Profile) {
        self.settings = profile.apply(self.settings.clone());
        self.resolve_system_prompt();
        self.add_log(&format!("Applied profile: {}", profile.name), crate::config::LogLevel::Info);
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
            self.add_log(&format!("Failed to save profile: {}", e), crate::config::LogLevel::Error);
        } else {
            self.add_log(&format!("Saved profile: {}", name), crate::config::LogLevel::Info);
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
                self.add_log(&format!("Failed to save settings for {}: {}", name, e), crate::config::LogLevel::Error);
            } else {
                self.add_log(&format!("Saved settings for {}", name), crate::config::LogLevel::Info);
                // Update the cache so it reflects the newly saved settings
                self.model_settings_cache = self.settings.clone();
            }
        } else {
            self.add_log("No model selected to save settings for", crate::config::LogLevel::Warning);
        }
        self.set_redraw();
    }

    /// Check if any LLM settings have been modified since last save.
    pub fn is_settings_dirty(&self) -> bool {
        let s = &self.settings;
        let c = &self.model_settings_cache;

        let f32_dirty = |a: Option<f32>, b: Option<f32>| match (a, b) {
            (Some(v1), Some(v2)) => (v1 - v2).abs() > 0.001,
            (None, None) => false,
            _ => true,
        };

        s.context_length != c.context_length
            || s.threads != c.threads
            || s.threads_batch != c.threads_batch
            || s.mlock != c.mlock
            || s.system_prompt_preset_name != c.system_prompt_preset_name
            || s.gpu_layers != c.gpu_layers
            || s.flash_attn != c.flash_attn
            || s.kv_cache_offload != c.kv_cache_offload
            || s.cache_type_k != c.cache_type_k
            || s.cache_type_v != c.cache_type_v
            || s.batch_size != c.batch_size
            || s.ubatch_size != c.ubatch_size
            || s.uniform_cache != c.uniform_cache
            || s.max_concurrent_predictions != c.max_concurrent_predictions
            || s.seed != c.seed
            || (s.temperature - c.temperature).abs() > 0.001
            || s.top_k != c.top_k
            || (s.top_p - c.top_p).abs() > 0.001
            || (s.min_p - c.min_p).abs() > 0.001
            || s.max_tokens != c.max_tokens
            || (s.repeat_penalty - c.repeat_penalty).abs() > 0.001
            || s.repeat_last_n != c.repeat_last_n
            || f32_dirty(s.presence_penalty, c.presence_penalty)
            || f32_dirty(s.frequency_penalty, c.frequency_penalty)
            || s.keep != c.keep
            || s.mmap != c.mmap
            || s.numa != c.numa
            || s.expert_count != c.expert_count
            || s.llama_cpp_version_cpu != c.llama_cpp_version_cpu
            || s.llama_cpp_version_vulkan != c.llama_cpp_version_vulkan
            || s.llama_cpp_version_rocm != c.llama_cpp_version_rocm
    }

    /// Delete a user profile by index in the merged display list.
    /// Returns true if a profile was deleted, false otherwise.
    pub fn delete_profile(&mut self, selected_idx: usize) -> bool {
        let builtin = crate::config::builtin_profiles();
        
        // Build the merged profile list (same as render logic)
        let mut all_profiles: Vec<crate::config::Profile> = builtin.iter().cloned().collect();
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
            self.add_log(&format!("Failed to delete profile: {}", e), crate::config::LogLevel::Error);
            return false;
        }
        
        self.add_log(&format!("Deleted profile: {}", profile_name), crate::config::LogLevel::Info);
        true
    }

    pub fn panel_help_lines(&self) -> Vec<ratatui::text::Line<'static>> {
        use ratatui::text::{Line, Span};
        let y = Style::default().fg(Color::Yellow);

        match self.active_panel {
            ActivePanel::Models => vec![
                Line::from(Span::styled("MODELS PANEL", y.clone().add_modifier(Modifier::BOLD))),
                Line::from(""),
                Line::from("Displays your local GGUF models and their status."),
                Line::from(""),
                Line::from(vec![Span::styled("j / k / Arrow keys", y.clone()), Span::raw("  Navigate model list")]),
                Line::from(vec![Span::styled("Enter / l", y.clone()), Span::raw("  Load selected model into server")]),
                Line::from(vec![Span::styled("u", y.clone()), Span::raw("  Unload model from server")]),
                Line::from(vec![Span::styled("Ctrl+D", y.clone()), Span::raw("  Delete model (with confirmation)")]),
                Line::from(""),
                Line::from("In search mode (/):"),
                Line::from(vec![Span::styled("Enter", y.clone()), Span::raw("  Execute search")]),
                Line::from(vec![Span::styled("Esc", y.clone()), Span::raw("  Exit search")]),
                Line::from(vec![Span::styled("l", y.clone()), Span::raw("  View available GGUF files")]),
                Line::from(vec![Span::styled("S", y.clone()), Span::raw("  Cycle sort order (Relevance/Downloads/Likes/Trending/Created)")]),
                Line::from(vec![Span::styled("B", y.clone()), Span::raw("  Go back one page")]),
                Line::from(vec![Span::styled("Down at bottom", y.clone()), Span::raw("  Load more results (infinite scroll)")]),
                Line::from(vec![Span::styled("R", y.clone()), Span::raw("  Fetch and view README")]),
            ],
            ActivePanel::Log => vec![
                Line::from(Span::styled("LOG PANEL", y.clone().add_modifier(Modifier::BOLD))),
                Line::from(""),
                Line::from("Live output from the llama.cpp server."),
                Line::from(""),
                Line::from(vec![Span::styled("j / k / Arrow keys", y.clone()), Span::raw("  Scroll log")]),
                Line::from(vec![Span::styled("g", y.clone()), Span::raw("  Jump to bottom")]),
                Line::from(vec![Span::styled("G", y.clone()), Span::raw("  Jump to top")]),
                Line::from(vec![Span::styled("Enter", y.clone()), Span::raw("  Expand log (fills screen)")]),
                Line::from(vec![Span::styled("Esc", y.clone()), Span::raw("  Collapse log")]),
            ],
            ActivePanel::Downloads => vec![
                Line::from(Span::styled("DOWNLOADS PANEL", y.clone().add_modifier(Modifier::BOLD))),
                Line::from(""),
                Line::from("Shows active downloads from HuggingFace."),
                Line::from(""),
                Line::from(vec![Span::styled("j / k / Arrow keys", y.clone()), Span::raw("  Select download")]),
                Line::from(vec![Span::styled("c", y.clone()), Span::raw("  Cancel selected download")]),
            ],
            ActivePanel::ServerSettings => vec![
                Line::from(Span::styled("SERVER SETTINGS", y.clone().add_modifier(Modifier::BOLD))),
                Line::from(""),
                Line::from("Configuration for the llama.cpp server."),
                Line::from(""),
                Line::from(vec![Span::styled("j / k", y.clone()), Span::raw("  Select setting")]),
                Line::from(vec![Span::styled("Enter", y.clone()), Span::raw("  Toggle value")]),
                Line::from(vec![Span::styled("h / l / Left / Right", y.clone()), Span::raw("  Adjust value")]),
                Line::from(""),
                Line::from(vec![Span::styled("Host", y.clone()), Span::raw("  Bind address (127.0.0.1 or 0.0.0.0)")]),
                Line::from(vec![Span::styled("Backend", y.clone()), Span::raw("  Acceleration backend (cpu / vulkan)")]),
                Line::from(vec![Span::styled("Threads", y.clone()), Span::raw("  CPU threads for generation (1 to max)")]),
                Line::from(vec![Span::styled("Threads Batch", y.clone()), Span::raw("  CPU threads for batch processing (1 to 32)")]),
            ],
            ActivePanel::LlmSettings => vec![
                Line::from(Span::styled("LLM SETTINGS", y.clone().add_modifier(Modifier::BOLD))),
                Line::from(""),
                Line::from("Fine-tuned settings for loading and running a model."),
                Line::from(""),
                Line::from(vec![Span::styled("j / k", y.clone()), Span::raw("  Navigate settings")]),
                Line::from(vec![Span::styled("Enter", y.clone()), Span::raw("  Apply value")]),
                Line::from(vec![Span::styled("h / l / Left / Right", y.clone()), Span::raw("  Adjust value")]),
                Line::from(vec![Span::styled("0-9, -, .", y.clone()), Span::raw("  Type numeric value")]),
                Line::from(vec![Span::styled("Esc", y.clone()), Span::raw("  Cancel edit")]),
                Line::from(""),
                Line::from(vec![Span::styled("Ctrl+S", y.clone()), Span::raw("  Save settings for selected model")]),
                Line::from(vec![Span::styled("Ctrl+R", y.clone()), Span::raw("  Reset to defaults")]),
                Line::from(vec![Span::styled("Ctrl+E", y.clone()), Span::raw("  Toggle enabled/disabled")]),
                Line::from(""),
                Line::from(vec![Span::styled("--- Loading ---", y.clone())]),
                Line::from(vec![Span::styled("Context", y.clone()), Span::raw("  Context window size in tokens. Determines how much of the conversation history is kept in memory. A larger context allows longer conversations but uses more RAM. Typical: 8192-65536 depending on model and RAM.")]),
                Line::from(vec![Span::styled("Prompt", y.clone()), Span::raw("  System prompt preset. Pre-configured prompts that shape how the model behaves (e.g., 'coder', 'assistant', 'creative'). Affects the model's personality and output style.")]),
                Line::from(vec![Span::styled("Keep in memory", y.clone()), Span::raw("  Lock model weights in RAM (mlock). Prevents the OS from swapping model weights to disk. Slows model load time but ensures faster inference once loaded. Useful for repeated use.")]),
                Line::from(""),
                Line::from(vec![Span::styled("--- GPU Offload ---", y.clone())]),
                Line::from(vec![Span::styled("GPU Layers", y.clone()), Span::raw("  Number of model layers to run on GPU. Higher = faster inference, more VRAM usage. Set to -1 to offload all layers. Typical: fill VRAM but leave room for KV cache. Use 'all' to auto-detect.")]),
                Line::from(vec![Span::styled("Flash Attention", y.clone()), Span::raw("  Enable Flash Attention (flash-attn) for faster inference. Requires compatible GPU (Ampere+ / Ada). Significantly speeds up long-context inference. Only works with certain GGUF formats.")]),
                Line::from(vec![Span::styled("KV Cache Offload", y.clone()), Span::raw("  Offload KV cache to RAM when GPU memory is full. Allows larger batch sizes and contexts at the cost of some speed. Useful when VRAM is limited but you still want longer conversations.")]),
                Line::from(vec![Span::styled("Cache Type K / V", y.clone()), Span::raw("  Quantization precision for KV cache (K = keys, V = values). Lower precision (e.g., Q4, Q8) saves VRAM but may slightly reduce quality. Default is usually FP16. Use lower values if running out of VRAM.")]),
                Line::from(vec![Span::styled("Active Experts", y.clone()), Span::raw("  Number of MoE (Mixture of Experts) experts to activate per token. -1 = auto (all active). Reducing this speeds up inference for MoE models like Mixtral but may reduce quality. Typical: 2-8 for Mixtral.")]),
                Line::from(""),
                Line::from(vec![Span::styled("--- Evaluation ---", y.clone())]),
                Line::from(vec![Span::styled("Eval Batch", y.clone()), Span::raw("  Batch size for evaluation (inference). Larger batches use more VRAM but can improve throughput via parallelism. Small values (1-8) for low VRAM, larger (16-128) for high VRAM setups.")]),
                Line::from(vec![Span::styled("Unified KV", y.clone()), Span::raw("  Share KV cache across sequences. Reduces VRAM usage when running multiple requests by reusing allocated cache. May slightly reduce performance but enables more concurrent users.")]),
                Line::from(vec![Span::styled("Max Concurrent Pred", y.clone()), Span::raw("  Maximum number of models that can run simultaneously. Useful when managing multiple models. Each model needs its own VRAM/CPU resources. Set based on available hardware.")]),
                Line::from(""),
                Line::from(vec![Span::styled("--- Sampling ---", y.clone())]),
                Line::from(vec![Span::styled("Seed", y.clone()), Span::raw("  Random seed for reproducible outputs. -1 = random (default). Set to a fixed value for deterministic, repeatable responses — useful for debugging or testing prompts.")]),
                Line::from(vec![Span::styled("Temp", y.clone()), Span::raw("  Sampling temperature. Controls creativity: 0 = deterministic (most predictable), 0.7 = balanced, 1.0+ = creative. Lower values produce more focused, factual outputs. Typical: 0.7-0.9 for general use.")]),
                Line::from(vec![Span::styled("Top-k", y.clone()), Span::raw("  Only consider the top k most likely tokens at each step. Smaller top-k (e.g., 10-40) makes output more deterministic. Larger values allow more variety. Typical: 40-50. Set to 0 to disable.")]),
                Line::from(vec![Span::styled("Top-p", y.clone()), Span::raw("  Nucleus sampling: only consider tokens whose cumulative probability reaches p. Smaller top-p (e.g., 0.9) is more conservative, larger (e.g., 0.95-0.99) allows more variety. Often preferred over top-k. Typical: 0.9-0.95.")]),
                Line::from(vec![Span::styled("Min P", y.clone()), Span::raw("  Minimum probability threshold relative to the most likely token. Tokens below min_p * max_prob are excluded. A filter that's more principled than top-k/top-p for controlling diversity. Typical: 0.01-0.1.")]),
                Line::from(vec![Span::styled("Max Tokens", y.clone()), Span::raw("  Maximum number of tokens to generate in the response. Prevents runaway responses. Set to 0 or Disabled for no limit. Typical: 4096-8192 for chat, higher for code generation.")]),
            ],
            ActivePanel::Profiles => vec![
                Line::from(Span::styled("PROFILES PANEL", y.clone().add_modifier(Modifier::BOLD))),
                Line::from(""),
                Line::from("Saved presets of settings for quick switching."),
                Line::from(""),
                Line::from(vec![Span::styled("j / k", y.clone()), Span::raw("  Select profile")]),
                Line::from(vec![Span::styled("Enter", y.clone()), Span::raw("  Apply profile settings")]),
                Line::from(vec![Span::styled("s", y.clone()), Span::raw("  Save current settings as new profile")]),
                Line::from(vec![Span::styled("d", y.clone()), Span::raw("  Delete user profile")]),
                Line::from(vec![Span::styled("Esc", y.clone()), Span::raw("  Back to settings")]),
            ],
            ActivePanel::SystemPromptPresets => vec![
                Line::from(Span::styled("SYSTEM PROMPT PRESETS", y.clone().add_modifier(Modifier::BOLD))),
                Line::from(""),
                Line::from("Named system prompts for different use cases."),
                Line::from(""),
                Line::from(vec![Span::styled("j / k", y.clone()), Span::raw("  Select preset")]),
                Line::from(vec![Span::styled("Enter", y.clone()), Span::raw("  Apply preset")]),
                Line::from(vec![Span::styled("e", y.clone()), Span::raw("  Edit selected preset")]),
                Line::from(vec![Span::styled("n", y.clone()), Span::raw("  Create new preset")]),
                Line::from(vec![Span::styled("Esc", y.clone()), Span::raw("  Back to settings")]),
            ],
            ActivePanel::SearchReadme => vec![
                Line::from(Span::styled("README PANEL", y.clone().add_modifier(Modifier::BOLD))),
                Line::from(""),
                Line::from("README documentation for the selected model."),
                Line::from(""),
                Line::from(vec![Span::styled("j / k / Arrow keys", y.clone()), Span::raw("  Scroll")]),
                Line::from(vec![Span::styled("h / l", y.clone()), Span::raw("  Scroll horizontally")]),
                Line::from(vec![Span::styled("Enter", y.clone()), Span::raw("  Expand to fullscreen")]),
                Line::from(vec![Span::styled("Esc", y.clone()), Span::raw("  Collapse / Exit")]),
            ],
        }
    }

    pub fn reset_to_defaults(&mut self) {
        let defaults = crate::models::ModelSettings::default();
        self.settings = defaults;
        // Clear dirty flag by updating the cache snapshot to match new settings
        self.model_settings_cache = self.settings.clone();
        self.add_log("Reset LLM Settings to defaults", crate::config::LogLevel::Info);
    }
}
