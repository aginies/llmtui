pub mod sub;
use crate::config::Config;
use crate::config::Profile;
use crate::models::Backend;
use crate::models::{
    BenchTuneConfig, BenchTuneField, DiscoveredModel, ListSort, ModelSettings, ModelState,
    SearchResult, SearchSort, ServerMetrics,
};
use ratatui::layout::Rect;
use ratatui::text::Line;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

// Re-export sub-structs
pub use sub::{
    BenchTuneState, DownloadState, EditState, LoadingState, LogState, PendingOperations,
    PickerState, SearchState, ServerState, SettingsState, UIState,
};

/// Static cell for caching the API port string in help text.
pub static API_PORT_CACHE: Mutex<(u16, String)> = Mutex::new((0, String::new()));

/// Identifies a concrete list picker so a single renderer and a single key handler
/// can route per-picker title, help, footer and commit — adding a new list picker never
/// needs a new render or key arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ListPickerKind {
    /// `--spec-type` values (Off, DeepSeek, etc.).
    #[default]
    SpecType,
    /// Tensor-split preset selector with a trailing manual-entry editor.
    TensorSplit,
}

/// One selectable row in a [`ListPicker`].
///
/// `primary`/`secondary` are what get rendered; `value` is what gets committed on
/// <kbd>Enter</kbd> (defaults to `primary`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListPickerEntry {
    pub primary: String,
    pub secondary: Option<String>,
    /// Value committed on Enter. Defaults to `primary`.
    pub value: String,
}

impl ListPickerEntry {
    /// A single-label entry with an explicit committed value (e.g. tensor-split).
    pub fn with_value<S: Into<String>>(primary: impl Into<String>, value: S) -> Self {
        let primary = primary.into();
        Self {
            primary: primary.clone(),
            secondary: None,
            value: value.into(),
        }
    }
}

/// Generic editable list picker — replaces the many near-identical
/// `GlobalMode::*Picker` variants (spec-type, chat-template, tensor-split, …).
///
/// State lives in [`App`][crate::tui::app::App] rather than in `GlobalMode` so a single
/// renderer / key-handler pair can serve every picker without a new render or key arm, and
/// so the commit step can borrow `&mut App` without fighting the picker's borrow.
#[derive(Debug, Clone, Default)]
pub struct ListPicker {
    pub kind: ListPickerKind,
    pub entries: Vec<ListPickerEntry>,
    pub selected: usize,
    pub scroll_offset: usize,
    pub editing: bool,
    pub edit_buffer: String,
    pub edit_cursor: usize,
}

impl ListPicker {
    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

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
    pub version: u64,
    pub selected: usize,
    pub edit_buf: String,
    pub lines: Vec<Line<'static>>,
    pub selected_content_line: usize,
}

/// Which panel has focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ActivePanel {
    #[default]
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
    List { sort_by: ListSort },
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
    /// Empty state: no local models, show discovery UI.
    Empty,
}

/// Global mode that overlays all panels.
#[derive(Debug, Clone, PartialEq)]
pub enum GlobalMode {
    Normal,
    CmdLine {
        cmd_line: String,
    },
    HostPicker {
        entries: Vec<(String, String)>, // (ip, interface_name)
        selected: usize,
    },
    BackendPicker {
        entries: Vec<(Backend, Option<String>)>,
        selected: usize,
    },
    Confirmation {
        selected: bool,
        kind: ConfirmationKind,
        display_name: String,
        detail: Option<String>,
    },
    RpcManager,
    About,
    MaxConcurrentPicker {
        value: String,
    },
    YarnRoPESettings {
        scale: String,
        freq_base: String,
        freq_scale: String,
        selected_field: i32, // -1=enabled, 0=scale, 1=freq_base, 2=freq_scale
        editing: bool,
        edit_buffer: String,
        edit_cursor_pos: usize,
    },
    BenchTuneSetup {
        config: BenchTuneConfig,
        selected_idx: usize,
        /// Single-field text editing (prompt/kwargs/tokens/iters): which field, if any.
        editing_field: Option<BenchTuneField>,
        field_edit_buffer: String,
        field_edit_cursor: usize,
        /// Range editing (Min/Max/Step) for the selected param.
        editing_param: bool,
        editing_param_field: i32,
        param_edit_buffer: String,
        param_edit_cursor_pos: usize,
        /// Full-screen matrix preview of every combination is open.
        show_matrix: bool,
        /// Scroll offset for the matrix preview.
        matrix_scroll: usize,
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
        profiles: Vec<Profile>,
    },
    ModelSettingsPicker {
        entries: Vec<(String, String)>, // (name, description)
        selected: usize,
    },
    ProfileCreate {
        name: String,
        description: String,
        editing: bool,
        edit_buffer: String,
        edit_cursor_pos: usize,
        field: i32, // 0=name, 1=description
    },
    DashboardPicker {
        enabled: bool,
        port: String,
        auth_key: String,
        tls_enabled: bool,
        tls_cert: String,
        tls_key: String,
        selected_field: i32, // -1=enabled, 0=port, 1=auth_key, 2=tls_enabled, 3=tls_cert, 4=tls_key
        editing: bool,
        edit_buffer: String,
        edit_cursor_pos: usize,
    },
    DirectoryPicker {
        title: String,
        selected: usize,
        current_path: std::path::PathBuf,
        confirm_action: bool,
    },
    ApiEndpointPicker {
        enabled: bool,
        port: String,
        api_key: String,
        tls_enabled: bool,
        tls_cert: String,
        tls_key: String,
        selected_field: i32, // -1=enabled, 0=port, 1=api_key, 2=tls_enabled, 3=tls_cert, 4=tls_key
        editing: bool,
        edit_buffer: String,
        edit_cursor_pos: usize,
    },
    DashboardUrl {
        host: String,
        ws_port: String,
        api_port: u16,
        llm_port: u16,
        auth_key: String,
        ws_enabled: bool,
        tls_enabled: bool,
    },
    SearchInput {
        buffer: String,
        cursor_pos: usize,
    },
    /// Fuzzy action finder (Ctrl+Space). `selected` indexes into the
    /// filtered results, not the full action registry.
    CommandPalette {
        buffer: String,
        cursor_pos: usize,
        selected: usize,
    },
    GgufNaming {
        explanation: crate::tui::gguf_naming::GgufExplanation,
        filename: String,
    },
    Onboarding {
        step: usize,
    },
    ChatTemplatePicker {
        entries: Vec<String>,
        selected: usize,
    },
    ChatTemplateFilePicker {
        entries: Vec<(String, String)>, // (display_name, full_path)
        selected: usize,
    },
    WebSearchPicker {
        enabled: bool,
        engine: String,
        engine_url: String,
        api_key: Option<String>,
        selected_field: i32, // -1=enabled, 0=engine, 1=url, 2=api_key
        engine_picker_selected: usize,
        editing: bool,
        edit_buffer: String,
        edit_cursor_pos: usize,
        check_status: Option<WebSearchCheckStatus>,
    },
    LlamaServerOptionsPicker {
        port: String,
        threads: u32,
        threads_batch: u32,
        log_level: String,
        webui: bool,
        selected_field: i32, // -1=port, 0=threads, 1=threads_batch, 2=mode, 3=log_level, 4=webui
        mode_picker_selected: usize,
        editing: bool,
        edit_buffer: String,
        edit_cursor_pos: usize,
    },
    GpuDevicePicker {
        // (name, info); index 0 is the special "(default) — all devices" entry.
        entries: Vec<(String, String)>,
        selected: usize,
        checked: Vec<bool>,
        loading: bool,
        error: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum WebSearchCheckStatus {
    Checking,
    Ok,
    Error(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmationKind {
    Exit,
    Reset,
    Delete,
    Unload,
    DeleteBackend,
    DeleteSettingsProfile,
}

/// Scroll state for text that exceeds display width.
#[derive(Debug, Clone)]
pub struct TextScrollState {
    pub offset: usize,
    pub last_tick: std::time::Instant,
    pub direction: i8,
    pub hold_count: u8,
    pub max_offset: usize,
    pub visible: bool,
    /// Cached scrolled text output.
    pub cached_output: Option<String>,
    /// Width used for the cached output.
    pub cached_width: u16,
    /// Offset used for the cached output.
    pub cached_offset: usize,
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
            LoadingPhase::ServerStarting => crate::t!("loading.server_starting"),
            LoadingPhase::LoadingModel => crate::t!("loading.loading_weights"),
            LoadingPhase::LoadingMeta => crate::t!("loading.loading_metadata"),
            LoadingPhase::LoadingTensors => crate::t!("loading.loading_tensors"),
            LoadingPhase::ServerListening => crate::t!("loading.server_listening"),
            LoadingPhase::Complete => crate::t!("loading.ready"),
        }
    }
}

/// The main application state.
pub struct App {
    // Core state
    pub running: bool,
    pub config: Config,
    pub models: Vec<DiscoveredModel>,
    pub selected_model_idx: Option<usize>,
    pub models_mode: ModelsMode,
    pub settings: ModelSettings,
    pub model_settings_cache: ModelSettings,
    pub model_states: HashMap<String, ModelState>,
    pub metrics: ServerMetrics,
    pub max_threads: u32,
    pub server_mode: crate::models::ServerMode,
    pub router_max_models: u32,
    pub ws_server_handle: Option<tokio::task::JoinHandle<()>>,
    pub ws_shutdown_tx: Option<tokio::sync::watch::Sender<bool>>,
    pub background_tasks: HashMap<String, tokio::task::JoinHandle<()>>,

    // Sub-structs
    pub settings_state: SettingsState,
    pub picker: PickerState,
    pub download: DownloadState,
    pub server: ServerState,
    pub bench_tune: BenchTuneState,
    pub log: LogState,
    pub loading: LoadingState,
    pub pending: PendingOperations,
    pub search: SearchState,
    pub ui: UIState,
    pub edit: EditState,

    // ── Generic list picker overlay ────────────────────────────
    /// The active generic editable list picker (spec-type, chat-template,
    /// tensor-split, …). A single renderer ([`crate::tui::render::overlays::render_list_picker`])
    /// and key handler ([`crate::tui::event::list_picker::handle_list_picker`]) serve
    /// every variant, so new pickers need no new render/key arms.
    pub list_picker: Option<ListPicker>,

    // ── Per-model settings profiles ────────────────────────────
    /// Active LLM settings profile per model (model display name → profile name).
    pub active_settings_profiles: HashMap<String, String>,

    // ── Performance hints ────────────────────────────────────
    /// Cached model to display in the Active Model card (see
    /// `tui::panel::active::compute_active_model`), to avoid scanning
    /// model_states every render. Invalidated via `active_model_hint_dirty`.
    pub active_model_hint: Option<(String, crate::models::ModelState)>,

    // ── Scheduler ────────────────────────────────────────────
    pub pending_tx: tokio::sync::mpsc::Sender<super::pending_events::PendingEvent>,
    pub pending_rx: tokio::sync::mpsc::Receiver<super::pending_events::PendingEvent>,
    pub server_ready: bool,
}
