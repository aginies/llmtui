# Architecture

LLM Manager is a Rust application built on ratatui and crossterm, using tokio for async operations. The codebase is organized into several modules:

```
src/
├── main.rs          # Entry point, event loop, model discovery
├── config.rs        # Config loading/saving, YAML-based
├── models.rs        # Domain types (SearchResult, DownloadState, etc.)
├── backend/
│   ├── hub.rs       # HuggingFace API: search, list files, download
│   └── server.rs    # llama.cpp server spawning
├── serve.rs         # Standalone serve mode
├── serve_api.rs     # API proxy server
└── tui/
    ├── mod.rs       # Module declaration
    ├── app.rs       # App state (App struct, enums for modes/panels)
    ├── event.rs     # Keyboard/mouse event handler
    ├── render.rs    # Top-level render dispatcher
    └── panel/
        ├── mod.rs
        ├── models.rs  # Left panel: model list / search / download
        ├── info.rs    # GGUF metadata rendering
        ├── tabbed.rs  # Right panel: Model Info / Settings tabs
        ├── settings.rs
        ├── log.rs
        └── help.rs
```

## App State Machine

The `App` struct in `src/tui/app.rs` holds all application state. The main state machine is controlled by `models_mode`:

```rust
pub enum ModelsMode {
    List,       // Local model list
    Search { query, results, sort_by, show_readme, loading, has_more, page },
    Files { model_id, files, selected_idx, previous_query, previous_results, selected_result },
    BenchTune,  // Benchmark tuning mode showing results table
}
```

Each mode controls rendering in `render.rs` and key handling in `event.rs`. The `GlobalMode` enum handles overlays that appear above all panels:

```rust
pub enum GlobalMode {
    Normal,
    Confirmation { selected: bool, kind: ConfirmationKind },
    CmdLine { cmd_line: String },
    HostPicker { entries: Vec<(String, String)>, selected: usize },
    BackendPicker { entries: Vec<(Backend, Option<String>)>, selected: usize },
    RpcManager,
    About,
    ProfilePicker { entries: Vec<(String, String)>, selected: usize },
    PromptPicker { entries, selected, editing, edit_buffer, edit_cursor_pos, confirm_delete },
    MaxConcurrentPicker { value: String },
    BenchTuneSetup { config, selected_idx, bench_mode_selection, editing_prompt, editing_kwargs },
    ApiEndpoints,
    Tags { editing, insert_mode, edit_buffer, selected_idx },
}
```

## Local Model Filter

The application supports real-time filtering of the local models list. Triggered by the `f` key when the Models panel is focused, it allows users to quickly narrow down large collections using case-insensitive substring matching.

## Model Discovery

The `discover_models()` function in `main.rs` recursively scans the models directory for `.gguf` files:

```rust
fn discover_models(dir: &Path) -> Vec<DiscoveredModel>
```

Each `DiscoveredModel` contains the file path, name, size, and display name (relative path from models directory). Discovery runs in a blocking task on startup.

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
pub async fn search_models(query: &str, limit: u32, offset: u32) -> Result<(Vec<SearchResult>, bool)>
```

A post-filter checks that the model_id contains the search query (case-insensitive), since the HF API does full-text search across descriptions/tags and can return unrelated models.

**Multi-word search:** Space-separated words are split and each word must match the model name (AND logic). Matching words are highlighted in cyan in the results list.

- Default: 70 results per page (max 200)
- Pagination: `B` goes back, `Down` at bottom loads more
- Sort order cycles: Relevance → Downloads → Likes → Trending → Created
- README fetching: `R` downloads and renders the model's README

## VRAM Estimation

The `estimate_vram_mib()` function in `src/models.rs` estimates VRAM usage:

```
total = model_vram + kv_cache + activation + fixed_overhead + 550
```

Where:
- **model_vram** — proportional to GPU layers loaded
- **kv_cache** — `2 * n_layer * n_ctx * n_embd_kv * sizeof(type)` with GQA ratio and FlashAttention factor
- **activation** — proportional to batch size and hidden size
- **fixed_overhead** — 3.8% of max VRAM (or 500 MiB if unknown)

## Loading Progress

Model loading phases are detected from llama.cpp log output:

| Phase | Log pattern | Weight |
|-------|-------------|--------|
| ServerStarting | (implicit) | 8% |
| LoadingModel | "LLAMA_MODEL_LOADER" / "LOADING MODEL" | 7% |
| LoadingMeta | "LOADED META" / "META DATA" | 7% |
| LoadingTensors | "LOAD_TENSORS:" | 70% |
| ServerListening | "SERVER LISTENING" | 8% |
| Complete | Detected via `/health` API polling | — |

During tensor loading, the progress bar refines using layer counts parsed from "offloaded X/Y layers" log messages.

## RPC Workers

Remote workers for distributed inference are stored in the config as `Vec<RpcWorker>`. Each worker has a name, IP address, and port (default: 50052). The `RpcManager` global mode provides a dedicated window for managing workers: add (`n`), edit (`e`), delete (`d`), toggle selection (`Space`).

## Benchmark Tuning

The benchmark system (`src/backend/benchmark.rs`) supports two modes:

- **RuntimeOnly**: Single server, params sent in request body (no server restarts)
- **Full**: New server spawned for each parameter combination

Key types:
- `BenchTuneConfig`: Model path, iterations, prompt, params to test, duration, mode
- `BenchTuneParam`: name, min, max, step, enabled
- `BenchTuneResult`: params, metrics (prompt_tps, generation_tps, combined_tps, latency_per_token, first_token_time), outputs, per-iteration metrics
- `BenchTuneStatus`: Running (with progress), Completed, Error

## Error Handling

Errors are detected from log patterns:

- **OOM**: "OUTOFDEVICEMEMORY" / "OUT OF MEMORY"
- **General error**: "ERROR", "FAILED TO LOAD", "EXCEPTION"

Server exit is detected via a dedicated channel (not log parsing). On error, affected models are marked as `Failed` with the error message.

## Confirmation Dialogs

Destructive actions trigger a `GlobalMode::Confirmation` overlay with `ConfirmationKind` variants: `Exit`, `Reset`, `Delete`, `Unload`, `DeleteBackend`. The user confirms with `Enter` or cancels with `Esc`.
