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
    Search { query, results, sort_by, loading, has_more },
    Files { model_id, files, selected_idx, previous_query, previous_results, selected_result },
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
2. `pending_download` is set with `(model_id, filename, url)`
3. A tokio task calls `hub::download_file()` with an `Arc<AtomicBool>` cancel token
4. Progress updates flow through `download_tx` → `download_rx`
5. The main loop polls `download_rx` each iteration and updates the Download panel
6. Pressing `c` sets the cancel flag on the download

## Server Spawning

When a model is loaded, `spawn_server()` in `backend/server.rs`:

1. Resolves the llama-server binary using `resolve_backend_binary()`
2. If the binary doesn't exist, downloads and extracts it from GitHub releases
3. Spawns the process with the model path and all settings
4. Sets up a log channel (`server_log_rx`) for parsing output

The main loop polls `server_log_rx` and parses:
- TPS from "tokens per second" lines
- Context usage from "n_tokens = X" lines
- VRAM from "KV buffer size = X MiB" lines
- Loading phases (model, metadata, tensors) from log messages

## Metrics & Logging

Metrics are collected from two sources:

1. **Log parsing** — parses `n_tokens`, TPS, and VRAM from llama.cpp stderr
2. **Metrics endpoint** — polls `/metrics` every 2 seconds

When both are available, log-parsed values take priority. This ensures the display reflects actual inference state, including context compaction drops.

Each log entry is stored in `log_entries: VecDeque<LogEntry>` with a max of 500 entries. The log panel supports scrolling and expansion (Enter/Esc).

## Search

Search uses the HuggingFace API with `&filter=gguf` to only return GGUF models:

```rust
pub async fn search_models(query: &str, limit: u32, offset: u32) -> Result<(Vec<SearchResult>, bool)>
```

- Default: 70 results per page
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
| Complete | "LOADED SUCCESSFULLY" | — |

During tensor loading, the progress bar refines using layer counts parsed from "offloaded X/Y layers" log messages.

## Error Handling

Errors are detected from log patterns:

- **OOM**: "OUTOFDEVICEMEMORY" / "OUT OF MEMORY"
- **Crash**: "LLAMA-SERVER EXITED" / "TERMINATED"
- **General error**: "ERROR", "FAILED TO LOAD", "EXCEPTION"

On error, affected models are marked as `Failed` with the error message, and the server is killed if it crashed.
