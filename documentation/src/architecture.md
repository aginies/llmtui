# Architecture

LLM Manager is a Rust application built on ratatui and crossterm, using tokio for async operations. The codebase is organized into several modules:

```
src/
├── main.rs              # Entry point, CLI parsing, event loop, model discovery
├── lib.rs               # Library root
├── config.rs            # Config loading/saving, YAML-based, profiles, presets, RPC workers
├── models.rs            # Domain types (SearchResult, DownloadState, ModelSettings, ServerMetrics, etc.)
├── serve.rs             # Standalone serve mode CLI (--model, --profile, --api-port, --api-key, --ws-enable)
├── serve_api.rs         # Axum-based API proxy server for serve mode
├── config/
│   ├── store.rs         # Generic named-item store
│   ├── profiles.rs      # ProfileStore
│   ├── presets.rs       # PresetStore
│   └── model_config.rs  # ModelConfigStore
├── backend/
│   ├── mod.rs           # Module root, USER_AGENT constant
│   ├── benchmark.rs     # Benchmark tuning engine (RuntimeOnly and Full modes)
│   ├── benchmark_report.html  # HTML report template for benchmarks
│   ├── hardware.rs      # GPU detection (AMD/NVIDIA/Intel), CPU core counting
│   ├── hub.rs           # HuggingFace API: search, list files, download
│   ├── server.rs        # llama.cpp server spawning, command building, metrics parsing
│   ├── tls.rs           # TLS certificate generation (self-signed CA), load_tls_config, ensure_tls_certs
│   ├── web_context.rs   # Web context helpers
│   ├── web_search.rs    # Web search (SearXNG) integration
│   └── ws_server.rs     # WebSocket metrics dashboard server
├── tui/
│   ├── mod.rs           # Module root
│   ├── app.rs           # App struct, main entry
│   ├── colors.rs        # Color constants (YELLOW, GREEN, RED, WHITE, DARK_GRAY, CYAN, etc.)
│   ├── settings.rs      # SettingField definitions, filtered_fields for expert mode
│   ├── i18n.rs          # Translation system (t! macro, language switching, locale loading)
│   ├── gguf_naming.rs   # GGUF filename explanation parser
│   ├── app/
│   │   ├── types.rs     # GlobalMode, ModelsMode, ActivePanel enum definitions
│   │   ├── types/sub.rs # Sub-structs: ServerState, DownloadState, SettingsState, etc.
│   │   ├── state/       # State module (parsing patterns, state impls)
│   │   ├── async_ops.rs # Async operations (server spawning, metrics polling, downloads)
│   │   ├── sync_ops.rs  # Sync operations (model discovery, settings sync)
│   │   ├── panels.rs    # Panel layout calculations
│   │   ├── pickers.rs   # Picker helpers
│   │   ├── profiles.rs  # Profile management
│   │   ├── help.rs      # Help text definitions
│   │   ├── metadata.rs  # Metadata handling
│   │   └── pending_events.rs  # PendingEvent enum + scheduler
│   ├── event/
│   │   ├── mod.rs       # Event module root
│   │   ├── key.rs       # Keyboard event handling (global shortcuts, panel handlers)
│   │   ├── mouse.rs     # Mouse event handling
│   │   ├── helpers.rs   # Shared helpers: TextEditor, picker_nav_*
│   │   ├── readme.rs    # README fetching
│   │   ├── rpc_workers.rs  # RPC worker key handling
│   │   ├── panel/       # Per-panel key handlers
│   │   │   ├── models.rs      # Models panel
│   │   │   ├── downloads.rs   # Downloads panel
│   │   │   ├── log.rs         # Log panel
│   │   │   ├── settings.rs    # Settings panel
│   │   │   ├── profiles.rs    # Profiles panel
│   │   │   ├── system_prompts.rs
│   │   │   ├── tags.rs        # Tags modal
│   │   │   └── mod.rs
│   │   └── overlay/       # Overlay handlers (21 handlers)
│   │       ├── mod.rs          # OverlayRegistry, OverlayHandler trait
│   │       ├── about.rs
│   │       ├── api_endpoint_picker.rs
│   │       ├── backend_picker.rs
│   │       ├── bench_tune_setup.rs
│   │       ├── chat_template_file_picker.rs
│   │       ├── chat_template_picker.rs
│   │       ├── cmd_line.rs
│   │       ├── confirmation.rs
│   │       ├── dashboard_picker.rs
│   │       ├── dashboard_url.rs
│   │       ├── directory_picker.rs
│   │       ├── gguf_naming.rs
│   │       ├── host_picker.rs
│   │       ├── max_concurrent_picker.rs
│   │       ├── onboarding.rs
│   │       ├── profile_picker.rs
│   │       ├── prompt_picker.rs
│   │       ├── rpc_manager.rs
│   │       ├── search_input.rs
│   │       ├── spec_type_picker.rs
│   │       ├── web_search_picker.rs
│   │       └── yarn_rope_settings.rs
│   ├── panel/           # Panel rendering
│   │   ├── mod.rs
│   │   ├── about.rs        # About panel
│   │   ├── active.rs       # Active model metrics panel
│   │   ├── help.rs         # Help panel
│   │   ├── info.rs         # Info line rendering
│   │   ├── log.rs          # Log panel
│   │   ├── models.rs       # Models panel (search, list, files)
│   │   ├── profiles.rs     # Profiles panel
│   │   ├── readme.rs       # README panel
│   │   ├── rpc_workers.rs  # RPC Workers panel
│   │   ├── settings.rs     # LLM Settings panel
│   │   ├── system_prompt_presets.rs
│   │   └── tabbed.rs       # Tabbed settings rendering
│   ├── render/          # Rendering
│   │   ├── mod.rs
│   │   ├── render.rs    # Main render function (layout, panel visibility)
│   │   ├── overlays.rs  # Overlay rendering (20+ overlay renderers)
│   │   ├── status.rs    # Status bar rendering
│   │   ├── hints.rs     # Bottom hints rendering
│   │   └── onboarding.rs  # Onboarding wizard rendering
│   └── render.rs
```

## App State Machine

The `App` struct in `src/tui/app.rs` holds all application state. The main state machine is controlled by `models_mode`:

```rust
pub enum ModelsMode {
    List { sort_by: ListSort },
    Search { query: String, results: Vec<SearchResult>, sort_by: SearchSort, show_readme: bool, page: usize, loading: bool, has_more: bool },
    Files { model_id: String, files: Vec<(String, u64, String)>, selected_idx: Option<usize>, previous_query: String, previous_results: Vec<SearchResult>, selected_result: Option<SearchResult> },
    BenchTune,
}
```

Each mode controls rendering in `render.rs` and key handling in `event/key.rs`. The `ActivePanel` enum controls focus:

```rust
pub enum ActivePanel {
    #[default] Models,
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
```

The `GlobalMode` enum handles overlays that appear above all panels (21 variants):

```rust
pub enum GlobalMode {
    Normal,
    CmdLine { cmd_line: String },
    HostPicker { entries: Vec<(String, String)>, selected: usize },
    BackendPicker { entries: Vec<(Backend, Option<String>)>, selected: usize },
    Confirmation { selected: bool, kind: ConfirmationKind, display_name: String, detail: Option<String> },
    RpcManager,
    About,
    MaxConcurrentPicker { value: String },
    SpecTypePicker { entries: Vec<String>, selected: usize },
    YarnRoPESettings { scale: String, freq_base: String, freq_scale: String, selected_field: i32, editing: bool, edit_buffer: String, edit_cursor_pos: usize },
    BenchTuneSetup { config: BenchTuneConfig, selected_idx: usize, editing_param: bool, editing_param_field: i32, param_edit_buffer: String, param_edit_cursor_pos: usize, bench_mode_selection: usize, editing_prompt: bool, editing_kwargs: bool },
    PromptPicker { entries: Vec<(String, String)>, selected: usize, editing: bool, edit_buffer: String, edit_cursor_pos: usize, confirm_delete: bool },
    ProfilePicker { entries: Vec<(String, String)>, selected: usize, profiles: Vec<Profile> },
    DashboardPicker { enabled: bool, port: String, auth_key: String, tls_enabled: bool, tls_cert: String, tls_key: String, selected_field: i32, editing: bool, edit_buffer: String, edit_cursor_pos: usize },
    ApiEndpointPicker { enabled: bool, port: String, api_key: String, tls_enabled: bool, tls_cert: String, tls_key: String, selected_field: i32, editing: bool, edit_buffer: String, edit_cursor_pos: usize },
    DashboardUrl { host: String, ws_port: String, api_port: u16, llm_port: u16, auth_key: String, ws_enabled: bool, tls_enabled: bool },
    SearchInput { buffer: String, cursor_pos: usize },
    GgufNaming { explanation: GgufExplanation, filename: String },
    Onboarding { step: usize },
    ChatTemplatePicker { entries: Vec<String>, selected: usize },
    ChatTemplateFilePicker { entries: Vec<(String, String)>, selected: usize },
    WebSearchPicker { enabled: bool, engine: String, engine_url: String, api_key: Option<String>, selected_field: i32, engine_picker_selected: usize, editing: bool, edit_buffer: String, edit_cursor_pos: usize, check_status: Option<WebSearchCheckStatus> },
}
```

## Server State

The `ServerState` struct in `src/tui/app/types/sub.rs` tracks server runtime state:

```rust
pub struct ServerState {
    pub server_handle: Option<ServerHandle>,
    pub metrics_task_handle: Option<JoinHandle<()>>,
    pub sync_task_handle: Option<JoinHandle<()>>,
    pub spawn_task_handle: Option<SpawnTaskHandle>,
    pub bench_tune_task_handle: Option<BenchTuneTaskHandle>,
    pub server_log_rx: Option<mpsc::Receiver<String>>,
    pub metrics_rx: Option<mpsc::Receiver<ServerMetrics>>,
    pub sync_rx: Option<SyncRx>,
    pub spawn_log_tx: Option<mpsc::Sender<String>>,
    pub metrics_model_name: Arc<Mutex<Option<String>>>,
    pub loaded_model_names: Arc<Mutex<Vec<String>>>,
    pub api_proxy_handle: Option<JoinHandle<()>>,
    pub metrics_tx: Option<mpsc::broadcast::Sender<WsMetrics>>,
    pub running_ws_port: Option<u16>,
    pub running_ws_auth: Option<String>,
    pub running_server_tls: Option<bool>,
    pub running_api_port: Option<u16>,
    pub running_api_server_port: Option<u16>,
    pub running_api_model: Option<String>,
    pub running_server_tls_cfg: Option<RustlsConfig>,
    pub running_server_tls_cert_path: Option<String>,
    pub running_server_tls_key_path: Option<String>,
    pub cmd_display: Option<String>,
    pub spawned_settings: Option<ModelSettings>,
    pub spawned_model_name: Option<String>,
    pub spawned_model_state: Option<String>,
    pub spawned_context_length: u32,
    pub server_exit_rx: Option<mpsc::Receiver<()>>,
    pub server_exit_tx: Option<mpsc::Sender<()>>,
    pub api_shutdown_tx: Option<watch::Sender<bool>>,
    pub last_server_logs_tick: Option<Instant>,
    pub last_sync_tick: Option<Instant>,
}
```

## Dashboard URL Modal (Ctrl+U)

Press `Ctrl+U` in any panel to open the Dashboard URL modal, which displays all server URLs and copies them to the clipboard on Enter.

The modal shows:
- Host address
- Server configuration (backend, threads, mode)
- API Endpoint status with port
- RPC Workers count
- Dashboard status with port
- **API URL**: `http(s)://host:api_port`
- **Metrics URL**: `http://host:llm_port/metrics`
- **Dashboard URL**: `http(s)://host:ws_port/dashboard?auth=key`
- **opencode baseURL**: `http(s)://host:api_port/v1`
- TLS status indicator (GREEN for On, GRAY for Off)

The modal is 72 columns wide and 20 rows tall, rendered as a centered overlay with yellow-bordered block.

## TLS / HTTPS

TLS is managed in `src/backend/tls.rs` (232 lines):

- `load_tls_config(cert_path, key_path)` — loads Rustls config from PEM files
- `generate_ca()` — generates self-signed CA (cert + key)
- `generate_server_cert(ca_cert, ca_key)` — signs server cert with CA
- `ensure_tls_certs()` — auto-generates certs if missing, stores in `~/.config/llm-manager/tls/`
- `validate_tls_path(path)` — validates cert/key file paths

Auto-generated certificates are stored in `~/.config/llm-manager/tls/`:
```
~/.config/llm-manager/tls/
├── ca.pem              # CA certificate
├── ca-key.pem          # CA private key
├── server.pem          # Server certificate
└── server-key.pem      # Server private key
```

Version tracking (`TLS_VERSION = "1"`) triggers regeneration on bump. CA expiry warnings show if certificate expires within 6 months.

TLS is used by:
- WebSocket dashboard server (`ws_server.rs`)
- API proxy server (`serve_api.rs`)
- Dashboard picker (`GlobalMode::DashboardPicker`)
- API endpoint picker (`GlobalMode::ApiEndpointPicker`)

## RPC Workers

Remote workers for distributed inference are stored in config as `Vec<RpcWorker>`. Each worker has:
- `name`: Human-readable identifier
- `ip`: Network address
- `port`: RPC port (default: 50052)
- `selected`: Whether to use this worker

The `RpcManager` global mode provides a dedicated window for managing workers:
- `n` — add new worker
- `e` — edit selected worker
- `d` — delete selected worker
- `Space` — toggle worker selection

Workers are combined into the `--rpc` flag when starting the server. Configuration is stored in `~/.config/llm-manager/config.yaml` under `rpc_workers`.

## Benchmark Tuning

The benchmark system (`src/backend/benchmark.rs`) supports two modes:

- **RuntimeOnly**: Single server, params sent in request body (no server restarts). Best for sampling parameters.
- **Full**: New server spawned for each parameter combination. Tests all parameters including server-level settings.

Key types:
- `BenchTuneConfig`: Model path, iterations, prompt, params to test, duration, mode
- `BenchTuneParam`: name, min, max, step, enabled
- `BenchTuneResult`: params, metrics (prompt_tps, generation_tps, combined_tps, latency_per_token, first_token_time), outputs, per-iteration metrics
- `BenchTuneStatus`: Running (with progress), Completed (with stats), PartiallyCompleted (with stats), Cancelled (with stats)

Results can be exported as Markdown table, JSON, YAML, or HTML report (with Chart.js charts).

## WebSocket Dashboard

The WebSocket Dashboard (`src/backend/ws_server.rs`) provides real-time metrics visualization:

- Built with `axum` and `tokio`
- Creates `broadcast::channel(64)` for metrics distribution
- Routes: `/dashboard` (serves embedded HTML), `/ws` (WebSocket for metrics), `/health`
- Auth: query param `?auth=KEY` or `window.__WS_AUTH` in dashboard
- TLS: supports both plain TCP and rustls TLS
- Connection indicator: green pulsing dot (connected), red dot (disconnected, auto-reconnects every 2s)

The HTML dashboard is embedded in the binary via `include_str!` and injected with the auth key.

## Web Search

Web search (`src/backend/web_search.rs`) integrates with SearXNG for research queries:

- Trigger: `$web` prefix in chat message
- Server-side flow: intercepts `/v1/chat/completions`, checks for search keywords, performs SearXNG search, injects results into prompt
- Configuration: `web_search_enabled`, `web_search_engine`, `web_search_engine_url`, `web_search_api_key`
- Supports custom SearXNG instances with Docker/Podman deployment
- Injected context format: `[WEB CONTEXT]...[END WEB CONTEXT]` block prepended to user message

## Configuration

Config is YAML-based in `~/.config/llm-manager/`:

### Config struct fields:
```rust
pub rpc_workers: Vec<RpcWorker>          // RPC workers for distributed inference
pub search_limit: u32                    // HuggingFace search results per query (default 50)
pub active_panel: ActivePanel            // Last focused panel
pub left_pct: u16                        // Left panel width % (default 55)
pub language: String                     // UI language (en/fr/it/de, default "en")
pub onboarding_complete: bool            // Onboarding wizard done flag
```

### DefaultParams fields:
```rust
pub ws_server_enabled: bool              // WebSocket dashboard enabled (default false)
pub ws_server_port: u16                  // WebSocket dashboard port (default 49223)
pub server_tls_enabled: bool             // TLS for server (default true)
pub server_tls_cert: Option<String>      // TLS certificate path
pub server_tls_key: Option<String>       // TLS key path
pub api_endpoint_enabled: bool           // API endpoint enabled (default false)
pub api_endpoint_port: u16               // API endpoint port (default 49222)
pub api_endpoint_key: Option<String>     // API bearer token
pub web_search_engine: String            // Web search engine (default "searxng")
pub web_search_engine_url: String        // Web search engine URL
pub web_search_enabled: bool             // Web search enabled (default false)
pub web_search_api_key: Option<String>   // Web search API key
```

### ModelOverride new fields:
```rust
pub chat_template: Option<String>
pub chat_template_kwargs: Option<String>
pub auto_chat_template: bool
pub expert_count: i32
pub gpu_layers_mode: GpuLayersMode
pub tags: Option<Vec<String>>
```

## Local Model Filter

The application supports real-time filtering of the local models list. Triggered by the `f` key when the Models panel is focused, it allows users to quickly narrow down large collections using case-insensitive substring matching.

## Model Discovery

The `discover_models()` function in `src/tui/app/sync_ops.rs` recursively scans the models directory for `.gguf` files:

```rust
fn discover_models(dir: &Path) -> Vec<DiscoveredModel>
```

Each `DiscoveredModel` contains the file path, name, file_size, and display name (relative path from models directory). Discovery runs in a blocking task on startup.

## Download System

Downloads run in a spawned tokio task with progress flowing through a broadcast channel:

1. User selects a file and presses `Enter`
2. `pending_download` is set with `(model_id, filename, url, file_size)`
3. Before starting, the app checks available disk space via `hub::get_free_space_bytes()` and warns if insufficient
4. A tokio task calls `hub::download_file()` with an `Arc<AtomicBool>` cancel token and `Arc<AtomicU8>` state
5. Progress updates flow through `download_tx` → `download_rx`
6. The main loop polls `download_rx` each iteration and updates the Download panel
7. Pressing `⌥C` (Alt+C) cancels the download and removes the temporary file; `p` pauses/resumes it

The download loop checks the state atomically each iteration: `1` = downloading, `2` = paused (sleeps 100ms and retries), `3` = cancelled (removes temp file, returns error). Each `DownloadState` tracks bytes downloaded, speed, ETA, destination path, and status (Downloading/Paused/Complete/Cancelled/Error).

## Server Spawning

When a model is loaded, `spawn_server()` in `backend/server.rs`:

1. Resolves the llama-server binary using `resolve_backend_binary()`
2. If the binary doesn't exist, downloads and extracts it from GitHub releases
3. Spawns the process with the model path and all settings
4. Sets up a log channel (`server_log_rx`) for parsing output

The main loop polls `server_log_rx` and parses log messages for:
- Loading phases (model, metadata, tensors) from log messages
- Error detection (OOM, crash) from log messages

Metrics (TPS, VRAM, context) are now collected exclusively from the `/metrics` and `/health` API endpoints rather than log parsing.

## Metrics & Logging

Metrics are collected from the `/metrics` and `/health` endpoints, which provide accurate real-time data. Loading completion is detected via the `/health` endpoint (polling for `"status": "ok"` and non-empty slots).

Each log entry is stored in `log_entries: VecDeque<LogEntry>` with a max of 500 entries. The log panel supports scrolling, expansion (Enter/Esc), and two modes: **Following** (auto-scroll to bottom) and **Manual** (free scroll). Press `f` to toggle modes.

## Search

Search uses the HuggingFace API with `&filter=gguf` to only return GGUF models:

```rust
pub async fn search_models(query: &str, limit: u32, offset: u32) -> Result<(Vec<SearchResult>, usize, Vec<String>)> // third element: raw model IDs for post-filtering
```

A post-filter checks that the model_id contains the search query (case-insensitive), since the HF API does full-text search across descriptions/tags and can return unrelated models.

**Multi-word search:** Space-separated words are split and each word must match the model name (AND logic). Matching words are highlighted in cyan in the results list.

- Default: 50 results per page (max 200)
- Pagination: `Ctrl+B` goes back, `Down` at bottom loads more
- Sort order cycles: Relevance → Downloads → Likes → Trending → Created
- README fetching: `->` downloads and renders the model's README

## VRAM Estimation

The `estimate_vram_mib()` function in `src/models.rs` estimates VRAM usage:

```
total = model_vram + kv_cache + activation + fixed_overhead + 550
```

Where:
- **model_vram** — proportional to GPU layers loaded, with MoE expert ratio applied to FFN portion (~60%) for mixture-of-experts models
- **kv_cache** — `2 * n_layer * n_ctx * n_embd_kv * sizeof(type)` with GQA ratio, FlashAttention factor, and effective context (context_length × rope_scale)
- **activation** — proportional to batch size and hidden size
- **fixed_overhead** — 3.8% of max VRAM (or 500 MiB if unknown)

## Loading Progress

Model loading phases are detected from llama.cpp log output and `/health` API polling:

| Phase | Detection | Weight |
|-------|-----------|--------|
| ServerStarting | (implicit) | 8% |
| LoadingModel | "LLAMA_MODEL_LOADER" / "LOADING MODEL" | 7% |
| LoadingMeta | "LOADED META" / "META DATA" | 7% |
| LoadingTensors | "LOAD_TENSORS:" | 70% |
| ServerListening | "SERVER LISTENING" | 8% |
| Complete | Detected via `/health` API polling | — |

During tensor loading, the progress bar refines using layer counts parsed from "offloaded X/Y layers" log messages.

## Error Handling

Errors are detected from log patterns:

- **OOM**: "OUTOFDEVICEMEMORY" / "OUT OF MEMORY"
- **General error**: "ERROR", "FAILED TO LOAD", "EXCEPTION"

Server exit is detected via a dedicated channel (not log parsing). On error, affected models are marked as `Failed` with the error message.

## Confirmation Dialogs

Destructive actions trigger a `GlobalMode::Confirmation` overlay with `ConfirmationKind` variants: `Exit`, `Reset`, `Delete`, `Unload`, `DeleteBackend`. The user confirms with `Enter` or cancels with `Esc`.

Dialog height is calculated as `lines.len() + 6` (content lines plus vertical padding), clamped to `area.height - 4` to ensure it fits within the terminal. The dialog requires a minimum terminal height of 12 lines to render, preventing display on very small terminals where buttons would be cut off.

## Internationalization (i18n)

All user-facing strings go through the i18n system defined in `src/tui/i18n.rs`. Translations are stored as JSON files in `locales/<lang>.json` (currently `en.json`, `fr.json`, `it.json`, `de.json`). The system loads all locale files at startup into a static `LazyLock<HashMap>` and switches language at runtime via `Ctrl+L` (cycles en → fr → it → en).

Key components:
- `TRANSLATIONS` — static HashMap keyed by language code, each containing a map of `key → string`
- `CURRENT_LANG` — thread-safe mutex holding the active language (persisted to config)
- `t!("key")` — macro for simple string lookup with fallback (current lang → English → key itself)
- `t_fmt!("key", args...)` — macro for strings with `{}` placeholders
- `field_help(field_id)` — helper that constructs `field.help.<id>` keys for LLM Settings tooltips

Naming convention: dot-separated hierarchical keys matching UI context (e.g. `dialog.exit.title`, `field.help.context`, `hints.nav`). Technical/internal strings (error messages for logs, debug output) may remain in code. User-facing strings (panel titles, button labels, help text, tooltips, dialog messages, hints) MUST use `t!()`. When adding a new key, it must be added to ALL locale files simultaneously.

Language switching persists the chosen language to `~/.config/llm-manager/config.yaml` under the `language` field. The locale directory is resolved at runtime by checking: (1) `locales/` alongside the binary, (2) `LLM_MANAGER_LOCALES` env var, (3) project root `locales/` directory.

## Key Bindings

### Global Shortcuts

| Key | Action |
|-----|--------|
| `/` (search mode) | Opens `GlobalMode::SearchInput` |
| `Ctrl+U` | Opens `DashboardUrl` modal (copies all URLs to clipboard) |
| `Ctrl+X` | Toggle expert mode |
| `Ctrl+G` | Opens `GgufNaming` overlay (GGUF filename explanation) |
| `Ctrl+P` | Opens `ProfilePicker` overlay |
| `Ctrl+L` | Cycles language: en → fr → it → en |
| `Ctrl+O` | Opens `Onboarding` wizard (resets onboarding_complete) |
| `Ctrl+C` | Exit confirmation if models loaded, else `app.running = false` |
| `Shift+Tab` | Focus prev |
| `Tab` | Focus next |
| `Ctrl+H` | Toggle panel help |
| `F1` | Focus Models panel |
| `F2` | Focus ServerSettings panel |
| `F3` | Focus LlmSettings panel |
| `F6` | Focus Log panel |
| `Ctrl+F2` | Toggle ServerSettings panel visibility |
| `Ctrl+F3` | Toggle LlmSettings panel visibility |
| `Alt+F3` | Toggle LlmSettings panel visibility |
| `Ctrl+F4` | Toggle ModelInfo panel visibility |
| `Ctrl+F5` | Toggle ActiveModel panel visibility |
| `Ctrl+F6` | Toggle Log panel visibility |
| `Ctrl+F10` | Show all panels |
| `F10` | Hide all panels except Models |

### Server Settings Field Navigation (Enter key)

| Index | Setting | Action |
|-------|---------|--------|
| 0 | Host | Opens HostPicker |
| 1 | Backend | Opens BackendPicker |
| 2 | Threads | Cycles 1..max_threads |
| 3 | Threads Batch | Cycles 1..32 |
| 4 | Mode | Cycles Normal → Router → Bench → BenchTune → Normal |
| 5 | API Endpoint | Opens ApiEndpointPicker |
| 6 | Dashboard | Opens DashboardPicker |
| 7 | RPC Workers | Opens RpcManager |
| 8 | Web Search | Opens WebSearchPicker |
| 9 | Language | Cycles language |

## Overlay Registry

The overlay system in `src/tui/event/overlay/mod.rs` uses a registry pattern with 21 handler types. Each handler implements the `OverlayHandler` trait with `can_handle()` and `handle()` methods. The registry dispatches key events to the appropriate handler based on the current `GlobalMode`.

## Render Pipeline

The render pipeline in `src/tui/render/render.rs` orchestrates all panel layout:

1. Status bar (1 line) — mode indicator, server status, bench progress
2. Main area (fill) — split by `left_pct` (20-80%) for models vs settings
3. Active model panel (6 lines) — metrics display
4. Bottom area — log panel (expandable) and downloads

Log expansion doubles the log height at the expense of other panels. Panel visibility is controlled by bitflags (`0b111111` = all panels visible).

## Settings Panel

The tabbed settings panel (`src/tui/panel/tabbed.rs`) combines Server Settings and LLM Settings into a unified interface with tabs:

- **UNSAVED** watermark in red dimmed text when settings are dirty
- Help text auto-display after 1.5s focus
- Settings rendered as key-value pairs with edit modes (toggle, cycle, text input, picker)
- Expert mode (Ctrl+X) reveals additional fields
