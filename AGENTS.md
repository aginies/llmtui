# AGENTS.md — llm-manager

## Project overview

**llm-manager** is a terminal UI (TUI) for managing local LLM models. It searches HuggingFace, downloads GGUF models, loads them via llama.cpp's `llama-server`, and lets you chat with them.

**Stack:** Rust 2024, ratatui 0.29, crossterm 0.28, tokio, reqwest.

## Directory structure

```
src/
├── main.rs          # Entry point, event loop, model discovery, metrics polling
├── config.rs        # Config loading/saving, YAML-based, profiles, presets
├── models.rs        # Domain types (SearchResult, DownloadState, ModelSettings, etc.)
├── serve.rs         # Standalone serve mode CLI (--model, --profile, --api-port, --api-key)
├── serve_api.rs     # Axum-based API proxy server for serve mode
├── backend/
│   ├── hub.rs       # HuggingFace API: search, list files, download
│   ├── server.rs    # llama.cpp server spawning (resolve_backend_binary, spawn_server)
│   └── benchmark.rs # Benchmark tuning system (RuntimeOnly and Full modes)
└── tui/
    ├── mod.rs       # Module declaration
    ├── app.rs       # App state (App struct, enums for modes/panels)
    ├── event.rs     # Keyboard event handler
    ├── render.rs    # Top-level render dispatcher
    ├── panel/
    │   ├── mod.rs
    │   ├── models.rs  # Left panel: model list / search results / download
    │   ├── info.rs    # GGUF metadata rendering for local models
    │   ├── active.rs  # Active Model panel: real-time metrics, benchmarking state
    │   ├── tabbed.rs  # Right panel: Model Info / Settings tabs
    │   ├── settings.rs
    │   ├── log.rs
    │   ├── help.rs
    │   ├── about.rs   # About dialog (version, license, website)
    │   ├── profiles.rs # Profiles management (create, apply, delete presets)
    │   ├── readme.rs  # README markdown renderer (pulldown_cmark)
    │   ├── rpc_workers.rs # RPC Workers manager for distributed inference
    │   └── system_prompt_presets.rs # System prompt presets management
```

## Key architectural patterns

### App state machine (`src/tui/app.rs`)

`App` holds all state. `models_mode` is the mode enum that controls rendering:

```rust
pub enum ModelsMode {
    List,       // Local model list
    Search { query, results, sort_by, show_readme, page, loading, has_more },
    Files { model_id, files, selected_idx, previous_query, previous_results, selected_result },
    BenchTune,  // Benchmark tuning mode showing results table
}
```

`ActivePanel` enum controls which panel has focus:

```rust
pub enum ActivePanel {
    Models, Log, ServerSettings, LlmSettings, Profiles,
    SystemPromptPresets, SearchReadme, ActiveModel, ModelInfo, Downloads,
}
```

`GlobalMode` enum controls overlay modes:

```rust
pub enum GlobalMode {
    Normal,
    CmdLine { cmd_line: String },
    HostPicker { entries: Vec<(String, String)>, selected: usize },
    BackendPicker { entries: Vec<(Backend, Option<String>)>, selected: usize },
    Confirmation { selected: bool, kind: ConfirmationKind },
    RpcManager,
    About,
    MaxConcurrentPicker { value: String },
    BenchTuneSetup { config, selected_idx, bench_mode_selection, editing_prompt, editing_kwargs },
}
```

`ConfirmationKind` variants: `Exit`, `Reset`, `Delete`, `Unload`, `DeleteBackend`.

`LoadingPhase` variants: `ServerStarting`, `LoadingModel`, `LoadingMeta`, `LoadingTensors`, `ServerListening`, `Complete`.

### Log panel expand/collapse (`src/tui/app.rs`, `src/tui/event.rs`, `src/tui/render.rs`)

The `App` struct has a `log_expanded: bool` field. When true:
- Layout switches to 2-chunk: status bar + log fills remaining space
- Models panel, Settings panel, and active model info are hidden
- Log panel shows `[Enter] expand` / `[Esc] collapse` hint in status bar

Enter in the log panel expands it; Esc collapses it. Mouse handling in `handle_mouse()` uses the same layout logic to determine panel hit regions.

### Event handling (`src/tui/event.rs`)

Key handling is hierarchical:
1. Global shortcuts (Ctrl+C, Tab, Ctrl+H, etc.)
2. CmdLine overlay
3. Exit/Reset confirmation
4. Version picker mode (takes priority when `ModelsMode::VersionPicker`)
5. Search mode (takes priority when `ModelsMode::Search`)
6. Files mode
7. Download mode
8. Normal mode → dispatch to panel-specific handlers

**Important:** Each branch calls `return` to prevent fallthrough. Adding a new mode requires early returns.

### Token count display (`src/main.rs`)

Token count (`ctx_used`) is driven by log-parsed `n_tokens` from llama.cpp stderr, NOT from the `/metrics` endpoint. When the metrics channel receives endpoint data, the log-parsed value takes priority if it exists. This ensures the display reflects actual inference state including compaction drops.

**Key code (main.rs):**
- Log parser (line ~566): parses `n_tokens = X` from stderr, updates `app.metrics.ctx_used`
- Metrics channel (line ~690): only overrides log-parsed value when log hasn't seen one (`app.metrics.ctx_used == 0`)

### Search filtering (`src/backend/hub.rs`)

Search uses `&filter=gguf` on the HuggingFace API URL so the API itself only returns GGUF models. A post-filter then checks that the model_id contains the search query (case-insensitive), since the HF API does full-text search across descriptions/tags and can return unrelated models. Default 70 results per page, max 200.

### Panel help (`src/tui/panel/help.rs`)

The help system has two modes:
- **Ctrl+H** — Panel-specific help via `render_panel()` (contextual for current panel)
- Panel-specific help content is generated by `App::panel_help_lines()` (app.rs), which returns different lines based on `ActivePanel`

The global help overlay (`GlobalMode::Help`, Ctrl+Shift+H) was removed in favor of the more useful panel context help.

### Help modal rendering (`src/tui/render.rs`)

The `render_panel` function in help.rs renders a structured help window with:
- Title bar ("Help — Esc to close")
- Scrollable content with `Wrap { trim: true }` to prevent cut-off lines
- Status bar with panel-specific hints

The help window is displayed at 70% of terminal size (min 60×20, max 80×35).

### Rendering (`src/tui/render.rs`)

Top-level layout: status bar → top panels → active model → log. The models panel renders differently based on `models_mode`.

### Download cancellation (shared state)

Download runs in a spawned tokio task. Cancellation uses `Arc<AtomicBool>` shared between the task and the UI. Pressing `c` sets the flag; the download loop checks it each iteration.

### Backend picker (`src/tui/event.rs`)

The backend picker allows selecting llama.cpp binary versions per-backend. Triggered from the "LLama.cpp Version" field in LLM Settings. Lists installed backends and allows downloading new ones.

- `Enter` selects backend+version for the active backend
- `d` deletes a backend version from disk (with `ConfirmationKind::DeleteBackend`)
- `Esc` exits back to settings

Auto-downloads if the selected backend version is not installed (triggers `resolve_backend_binary`).

**HostPicker** (`GlobalMode::HostPicker`): Shows network interfaces and their IPs. `Enter` selects host, `d` refreshes, `Ctrl+H` closes. Opens from ServerSettings field 0 (Host).

**MaxConcurrentPicker** (`GlobalMode::MaxConcurrentPicker`): Numeric entry modal for max concurrent predictions (1-10). Opens when pressing Enter on field index 11 in settings.

**BenchTuneSetup** (`GlobalMode::BenchTuneSetup`): Full benchmark configuration modal. `Alt+m` toggles benchmark mode (RuntimeOnly vs Full), `Alt+p` edits prompt, `Alt+n` edits n_predict, `Alt+i` edits iterations, `Alt+c` edits chat template kwargs. Space toggles parameter enablement, Enter starts benchmark.

**Dirty tracking** (`is_settings_dirty` in `app.rs`) compares each field index-by-index. When a field is dirty, its label is rendered in yellow.

**Index consistency** — all indices must be identical across:
- `settings.rs` dirty check match arms (line ~133)
- `event.rs` `apply_numeric_setting` / `adjust_setting` match arms
- `event.rs` `handle_settings_key` toggle shortcuts (`e` / `Ctrl+E`)
- `event.rs` comment block (line ~836)
- `app.rs` `is_settings_dirty` match arms

### LLM Settings panel (24 fields, `src/tui/panel/settings.rs`, `src/tui/event.rs`)

The settings panel has 24 fields organized into 6 groups:

```
Loading (0-2):   Context length, System prompt preset, Keep in memory (mlock)
GPU (3-8):       GPU Layers, Flash Attention, KV Cache Offload, Cache Type K, Cache Type V, Active Experts
Evaluation (9-11): Eval Batch, Unified KV, Max Concurrent Predictions
Sampling (12-17): Seed, Temperature, Top-k, Top-p, Min P, Max Tokens
Repetition (18-21): Repetition Penalty, Rep. Last N, Presence Penalty, Frequency Penalty
Backend (22):    Tags (semicolon-separated list)
Backend (23):    LLama.cpp Version (shows CPU / Vulkan / ROCm / CUDA versions)
```

Each group is rendered with a header line. Arrow keys adjust values; `+`/`-` for coarse, `Left`/`Right` for fine. Toggle fields (Flash Attention, Unified KV, Keep in memory) respond to `e`/`Ctrl+E`.

**Index consistency** — all field indices must be identical across:
- `settings.rs` dirty check match arms (line ~133)
- `event.rs` `apply_numeric_setting` / `adjust_setting` match arms
- `event.rs` `handle_settings_key` toggle shortcuts (`e` / `Ctrl+E`)
- `event.rs` comment block (line ~836)
- `app.rs` `is_settings_dirty` match arms

### GPU Layers cycling (`src/models.rs::GpuLayersMode`, `src/tui/event.rs`)

GPU Layers uses a `GpuLayersMode` enum with three variants:

```rust
pub enum GpuLayersMode {
    Auto,       // llama.cpp auto-detects based on VRAM (default)
    Specific(u32), // exact number of layers
    All,        // -ngl 999 (all layers)
}
```

Arrow keys cycle through modes: `Auto` → `1` → `2` → ... → `N` (total layers) → `All` → `Auto`.
- `Enter` from a specific number opens an edit buffer for direct input.
- `Enter` from `Auto` or `All` sets the max available layers.

The `-ngl` parameter is only added to the llama-server command for `Specific` and `All` modes; `Auto` omits it entirely.

**VRAM estimation** (`src/models.rs::estimate_vram_mib`):
- `Auto` uses a heuristic (~60% of total layers)
- `Specific(n)` uses exactly `n` layers
- `All` uses all layers

## Llama.cpp binary management

### Backend selection (`src/models.rs`)

Five backends are supported:

```rust
pub enum Backend {
    Cpu,          // CPU-only inference
    Vulkan,       // GPU via Vulkan (AMD/NVIDIA/Intel)
    Rocm,         // GPU via ROCm (AMD)
    RocmLemonade, // Optimized ROCm via Lemonade
    Cuda,         // GPU via CUDA (NVIDIA)
}
```

### Binary storage

Binaries are downloaded from various GitHub releases and stored in versioned directories:

```
~/.local/share/llm-manager/bin/
├── llama-server-cpu-{version}/llama-server
├── llama-server-vulkan-{version}/llama-server
├── llama-server-rocm-{version}/llama-server
├── llama-server-rocm-lemonade-{version}/llama-server
└── llama-server-cuda-{version}/llama-server
```

Switching versions is instant — no re-download. The version is stored per-backend in config.

### Per-backend version config

`DefaultParams` and `ModelSettings` have separate version fields:

```yaml
llama_cpp_version_cpu: null      # null = latest
llama_cpp_version_vulkan: null   # null = latest
llama_cpp_version_rocm: null     # null = latest
llama_cpp_version_rocm_lemonade: null  # null = latest
llama_cpp_version_cuda: null     # null = latest
```

### Asset names

- **CPU:** `llama-{tag}-bin-ubuntu-x64.tar.gz`
- **Vulkan:** `llama-{tag}-bin-ubuntu-vulkan-x64.tar.gz`
- **ROCm:** `llama-{tag}-bin-ubuntu-rocm-7.2-x64.tar.gz`
- **ROCm Lemonade:** `llama-{tag}-ubuntu-rocm-{gfx_suffix}-x64.zip` (ZIP, auto-detects GFX architecture)
- **CUDA:** `llama.cpp-{tag}-cuda-12.8-amd64.tar.gz`

### Binary resolution (`src/backend/hub.rs`)

`resolve_backend_binary(backend, version)` checks if the binary + `libllama.so` exist. If not, it downloads and extracts the tar.gz archive, pulling out `llama-server` and `.so` files.

### Server spawning (`src/backend/server.rs`)

`spawn_server()` resolves the binary using the per-backend version from `settings`, then spawns the server process. Log message: `Downloading {backend} (v{version}) binary...`

## New domain types (`src/models.rs`)

### Server mode (`ServerMode`, lines 573-598)

```rust
pub enum ServerMode {
    Normal,    // Single model via CLI
    Router,    // Multiple models via API
    Bench,     // GPU benchmarking
    BenchTune, // Parameter auto-tuning
}
```

### Reasoning mode (`ReasoningMode`, lines 600-619)

```rust
pub enum ReasoningMode {
    Default, // DeepSeek/OpenAI style: <think>...</think>
    Gemma,   // Gemma style: <|channel>thought<|channel|>
}
```

### Cache types (`CacheType`, `CacheQuantType`, lines 286-414)

- **CacheType** (main KV cache): `F16`, `BF16`, `Fq8_0`, `Fq4_1`
- **CacheQuantType** (KV quantization): `F32`, `F16`, `BF16`, `Q8_0`, `Q4_0`, `Q4_1`, `Iq4Nl`, `Q5_0`, `Q5_1`

### Split mode (`SplitMode`, lines 417-441)

`None`, `Layer` (default), `Row`, `Tensor`

### NUMA mode (`NumMode`, lines 444-468)

`None` (default), `Distribute`, `Isolate`, `Numactl`

### RoPE scaling (`RopeScaling`, lines 471-492)

`None` (default), `Linear`, `Yarn`

### Mirostat (`Mirostat`, lines 495-516)

`Off` (default), `Mirostat`, `Mirostat2`

### Samplers (`Samplers`, lines 518-533)

Semicolon-separated sampler order string. Default: `penalties;dry;top_n_sigma;top_k;typ_p;top_p;min_p;xtc;temperature`

### ModelState (`ModelState`, lines 7-19)

```rust
pub enum ModelState {
    Available,
    Loading,
    Benchmarking,
    Loaded { port: u16, pid: u32 },
    Failed { error: String },
}
```

### Search sort (`SearchSort`, lines 22-51)

`Relevance`, `Downloads`, `Likes`, `Trending`, `CreatedAt` — cycled with `S` key.

### SearchResult fields (lines 54-75)

All fields: `model_id`, `model_name`, `tags`, `downloads`, `likes`, `pipeline_tag`, `size`, `parameters`, `capabilities`, `context_length`, `readme`, `quantization`, `license`, `trending_score`, `created_at`.

### GGUF metadata (`GgufMetadata`, lines 909-926)

`layers`, `hidden_size`, `n_ctx_train`, `n_head`, `n_kv_head`, `arch`, `file_type`, `quantization`, `model_parameters`, `domain`, `capabilities`, `tokenizer`, `vocab_size`, `draft_tokens`.

### Load progress (`LoadProgress`, lines 956-967)

`layers_total`, `layers_loaded`, `tensors_loaded`, `buffers: Vec<GPUBuffer>`.

### Model settings additions

New fields in `ModelSettings`: `threads_batch`, `batch_size`, `ubatch_size`, `parallel`, `max_concurrent_predictions`, `keep`, `swa_full`, `mlock`, `mmap`, `numa`, `reasoning_mode`, `split_mode`, `tensor_split`, `main_gpu`, `fit`, `embedding`, `expert_count`, `jinja`, `chat_template`, `chat_template_kwargs`, `typical_p`, `mirostat`, `mirostat_lr`, `mirostat_ent`, `ignore_eos`, `samplers`, `repeat_penalty`, `repeat_last_n`, `presence_penalty`, `frequency_penalty`.

## Benchmark Tuning (`src/backend/benchmark.rs`)

A comprehensive benchmark system with two modes:

- **RuntimeOnly**: Single server, params sent in request body (no server restarts)
- **Full**: New server spawned for each parameter combination

### Tunable parameters

| Parameter | Range | Step |
|-----------|-------|------|
| temperature | 0.4 - 1.0 | 0.1 |
| top_p | 0.8 - 1.0 | 0.1 |
| top_k | 40 - 50 | 10 |
| repeat_penalty | 1.0 - 1.2 | 0.1 |
| flash_attn | 0 / 1 | - |
| threads | 4 - 16 | 4 |
| batch_size | 512 - 2048 | 512 |
| expert_count | 1 - 4 | 1 |

### Benchmark types

- `BenchTuneConfig`: Model path, iterations, prompt, params to test, duration, mode, n_predict, chat template kwargs
- `BenchTuneParam`: name, min, max, step, enabled
- `BenchTuneParamValue`: Actual values for each tunable parameter
- `BenchTuneResult`: params, metrics, outputs, per-iteration metrics, base settings
- `BenchTuneMetrics`: prompt_tps, generation_tps, combined_tps, latency_per_token, first_token_time
- `BenchTuneStatus`: Running (with progress), Completed, Error
- `BenchTuneProgress`: Running, Completed, Error (UI-facing)
- `BenchTuneMode`: RuntimeOnly, Full

### Output formats

Markdown table, JSON, YAML, and HTML report with summary cards, winner section, impact analysis, Chart.js charts, and filterable/sortable results table.

## Profiles, System Prompt Presets, and RPC Workers (`src/config.rs`)

### Profiles (`Profile` struct, lines 78-86)

Named profiles of settings presets. Built-in profiles: Qwen, Gemma, Llama, Mistral, Phi. User profiles can be created, applied, and deleted.

```rust
pub struct Profile {
    pub name: String,
    pub description: String,
    pub settings: ModelOverride,
}
```

### System Prompt Presets (`SystemPromptPreset` struct, lines 97-102)

Named system prompts for different use cases. Built-in presets: General, Coder, Thinker, Mathematician.

```rust
pub struct SystemPromptPreset {
    pub name: String,
    pub description: String,
    pub content: String,
}
```

### RPC Workers (`RpcWorker` struct, lines 37-46)

Remote workers for distributed inference.

```rust
pub struct RpcWorker {
    pub selected: bool,
    pub name: String,
    pub ip: String,
    pub port: u16,  // default: 50052
}
```

### Config struct (`Config` struct, lines 51-71)

```rust
pub struct Config {
    pub models_dir: PathBuf,
    pub llama_server: PathBuf,
    pub default: DefaultParams,
    pub model_overrides: HashMap<String, ModelOverride>,
    pub profiles: Vec<Profile>,
    pub system_prompt_presets: Vec<SystemPromptPreset>,
    pub rpc_workers: Vec<RpcWorker>,
    pub search_limit: u32,  // default: 50
}
```

## Serve mode and API proxy (`src/serve.rs`, `src/serve_api.rs`)

### Standalone serve CLI

Run a model directly with llama-server and expose an OpenAI-compatible API:

```bash
./build.sh serve --model /path/to/model.gguf --api-port 49222 --api-key secret
```

CLI flags: `--model` (required), `--profile`, `--config`, `--api-port`, `--api-key`.

Automatically resolves the llama-server binary from the backend-specific directory and sets `LD_LIBRARY_PATH` for shared libraries.

### API Proxy Server

An `axum`-based HTTP proxy that forwards requests to the running llama-server instance. Explicitly handled endpoints:

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/v1/chat/completions` | POST | Chat completions (OpenAI) |
| `/v1/completions` | POST | Completions (OpenAI) |
| `/v1/responses` | POST | Responses (Anthropic) |
| `/v1/messages` | POST | Messages (Anthropic) |
| `/v1/messages/count_tokens` | POST | Count tokens (Anthropic) |
| `/v1/embeddings` | POST | Embeddings |
| `/v1/models` | GET | List models |
| `/completion` | POST | Legacy completion |
| `/infill` | POST | Code completion (FIM) |
| `/reranking` | POST | Re-ranking |
| `/tokenize` | POST | Tokenize text |
| `/detokenize` | POST | Detokenize tokens |
| `/apply-template` | POST | Apply chat template |
| `/health` | GET | Health check |
| `/metrics` | GET | Prometheus metrics |
| `/props` | GET/POST | Get/set server properties |
| `/slots` | GET | Slot monitoring |
| `/lora-adapters` | GET/POST | List/load LoRA adapters |
| `/models/load` | POST | Load a model (router mode) |
| `/models/unload` | POST | Unload a model (router mode) |
| `/api/status` | GET | Server status (pid, uptime, loaded models) |

All paths not listed above are automatically proxied to the llama-server instance.

## Coding rules

### Planning

**Always plan before making changes.** For any non-trivial task:

1. Identify the root cause of the issue, not just the symptoms.
2. List affected files and functions.
3. Use a `todowrite` tool to track the work as a numbered TODO list.
4. Mark each item as `in_progress` before starting, `completed` when done.
5. Keep the TODO list visible so progress is clear.

For bug fixes: explain the bug, the root cause, and the fix before implementing.
For feature additions: describe the approach, then implement.

### Dependencies

- No new dependencies without asking. The project avoids external crates.
- If a crate is needed, prefer `ratatui` widgets over custom rendering.

### Error handling

- Use `anyhow::Result` for async/API functions.
- Use `thiserror` for application-specific error types.
- Log errors with `app.add_log()` in the TUI.

### Naming conventions

- `snake_case` for functions, variables, modules.
- `PascalCase` for types, enums, variants.
- Module names are lowercase (`backend`, `panel`).
- Public types get `pub` visibility; helpers stay private to their module.

### Async

- `handle_key` is async (for search queries).
- Download is spawned as a tokio task; progress flows through a `mpsc` channel.
- The main loop uses `crossterm::event::poll()` with a 100ms timeout.

### TUI specifics

- Use `ratatui` widgets when possible (Table, List, Paragraph, etc.).
- Style with `Style` / `Color` / `Modifier` — prefer semantic colors:
  - Yellow: headers, active elements
  - Cyan: navigation hints
  - Green: success/completed
  - Red: errors/failure
- Avoid hardcoding terminal dimensions; use `rect` and `area` from ratatui.

### Configuration

- Config is YAML-based, stored in `~/.config/llm-manager/`.
- New config fields go in `config.rs`; add defaults in `Default` impls.

### Testing

- No test framework yet. Add unit tests in `mod tests` blocks when writing new logic.
- Integration testing is manual (run the app).

## Common tasks

### Adding a new panel

1. Create `src/tui/panel/name.rs` with a `render(f, area, app)` function.
2. Add `mod name;` to `src/tui/panel/mod.rs`.
3. Add to `ActivePanel` enum in `app.rs`.
4. Dispatch in `render.rs` and `event.rs`.

### Adding a new keyboard shortcut

1. Add to `handle_key()` in `event.rs`.
2. Update the status bar in `render_status_bar()` in `render.rs`.
3. If it changes state, update `App` fields in `app.rs`.

### Adding a new API endpoint

1. Add the function in `src/backend/hub.rs`.
2. Call from `event.rs` (usually in the search/files branch).
3. Update `SearchResult` or other types in `models.rs` if needed.

### Adding a new backend

1. Add variant to `Backend` enum in `models.rs` with serde/Display impl.
2. Add `llama_cpp_version_{backend}` field to `DefaultParams` and `ModelSettings` in `config.rs` and `models.rs`.
3. Update `from_settings()` / `apply()` in `config.rs`.
4. Update `resolve_backend_binary()` in `hub.rs` for asset name.
5. Update `spawn_server()` in `server.rs` for version lookup.
6. Update `refresh_cached_versions()` in `app.rs` for directory detection.
7. Update version picker in `models.rs` and event handling in `event.rs`.
