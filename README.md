# llm-manager

[![CI](https://github.com/aginies/llmtui/actions/workflows/ci.yml/badge.svg)](https://github.com/aginies/llmtui/actions/workflows/ci.yml)
[![Docs](https://img.shields.io/badge/docs-GitHub%20Pages-blue)](https://aginies.github.io/llmtui/)
[![Version](https://img.shields.io/crates/v/llm-manager)](https://crates.io/crates/llm-manager)

A terminal UI (TUI) for managing local LLM models with HuggingFace search, download, and inference control.

> **NOTE:** This app is WIP (Work in Progress).

![Screenshot](documentation/src/images/main.png)

## Features

- **Search models** on HuggingFace by name (multi-word AND search, highlights matches, GGUF filter, 70 results per page)
- **Download** GGUF model files with progress tracking (pause/resume with `p`)
- **Load/unload** models via llama.cpp server
- **Local Model Filter** — quickly find models in your list with `f`
- **RPC Workers Manager** — dedicated window to manage distributed inference nodes
- **Chat** with loaded models in the terminal
- **Configure** loading and inference parameters per model
- **GGUF file browser** — list and select specific GGUF files for a model
- **Log panel** — expand/collapse with Enter/Esc, follow mode with `f`
- **About Box** — application info and GPLv3 license link (`A`)
- **Multi-language UI** — switch between English, French, and Italian with `Ctrl+L` (locale files in `locales/`)
- **HuggingFace URL links** — navigate to model pages from Model Info
- **CmdLine overlay** — full-screen view of the computed llama-server command line (`Ctrl+K`)
- **Export to script** — write the llama-server command to `/tmp/test_llamaserver.sh` from the CmdLine overlay (`e`)
- **Benchmark Tuning** — auto-tune model parameters for optimal performance
- **Profiles** — saved presets of settings for quick switching (`p`)
- **System Prompt Presets** — named system prompts for different use cases
- **Router Mode** — load multiple models simultaneously
- **WebSocket Dashboard** — live metrics and settings visualization via WebSocket server (configurable port, optional auth)
- **Multiple Model Directories** — scan multiple directories for models; downloads go to the first directory
- **Panel Resize** — drag the border between left and right panels, or use `Shift+←/→` to adjust (20%-80%)
- **Web Search** — automatically search the web via SearXNG when messages contain comparison/research keywords, inject results into prompt

## Prerequisites

- Rust toolchain (edition 2024)

## Installation

```bash
git clone https://github.com/aginies/llmtui.git
cd llmtui
cargo build --release
```

## Usage

Two subcommands are available: `tui` (terminal UI) and `serve` (headless server).

```bash
# Start the TUI (default mode)
llm-manager tui

# Start the TUI with custom options
llm-manager tui --models-dir /path/to/models --backend vulkan --config /path/to/config.yaml
```

### Build script

### Build script

A convenience script is included for common operations:

```bash
./build.sh build      # Build (debug)
./build.sh run        # Build and run (TUI mode)
./build.sh serve      # Build and serve a model (llama-server)
./build.sh servedoc   # Serve documentation with watch mode
./build.sh release    # Release build
./build.sh clean      # Remove build artifacts
./build.sh format     # Format code
./build.sh check      # cargo check
./build.sh test       # Run tests
./build.sh clippy     # Run clippy
./build.sh doc        # Build documentation
./build.sh help       # Show help
```

### Testing

The project includes a comprehensive test suite with 413 tests covering domain types, configuration, app state, benchmark tuning, event handling, rendering, backend API, and server management.

```bash
./build.sh test       # Run all tests (verbose, all features)
cargo test            # Alternative: run tests directly
```

Tests are organized in the `tests/` directory:
- `tests/models_tests.rs` — 78 tests (domain types, VRAM estimation, enums)
- `tests/config_tests.rs` — 60 tests (defaults, validation, overrides)
- `tests/app_tests.rs` — 40 tests (App state, filtering, panels)
- `tests/benchmark_tests.rs` — 50 tests (config, parameter generation)
- `tests/event_tests.rs` — 106 tests (keyboard event handling, key presses)
- `tests/hub_tests.rs` — 19 tests (HuggingFace API, binary management, archive extraction)
- `tests/render_tests.rs` — 38 tests (TUI rendering, layout, overlays)
- `tests/server_tests.rs` — 22 tests (command building, server management)

CI runs `cargo build` and `cargo test` on every PR and push to main.

### Multiple model directories

Store models across multiple directories — the app scans all of them and merges the results into a single list. Downloads go to the first directory.

Via CLI:
```bash
llm-manager --models-dir /path/to/models/1 --models-dir /path/to/models/2
```

Via config (`~/.config/llm-manager/config.yaml`):
```yaml
models_dirs:
  - /path/to/models/1
  - /path/to/models/2
```

### Configuration layout

llm-manager uses XDG directories:

```
~/.config/llm-manager/              # Config
```

UI translations are stored as JSON files in the `locales/` directory alongside the binary or in the project root. Supported languages: **en** (English), **fr** (French), **it** (Italian). Cycle languages with `Ctrl+L`. Missing translations fall back to English. To add a new language, copy `locales/en.json` to `locales/<code>.json` and translate all values.

#### Configuration layout

llm-manager uses XDG directories:

```
~/.config/llm-manager/              # Config
├── config.yaml                     # Global settings
├── models/                         # Per-model YAML configs
├── profiles/                       # Per-profile YAML configs
├── presets/                        # Per-preset YAML configs
├── unused/                         # Deleted model configs
├── unused_profiles/                # Deleted profiles
└── unused_presets/                 # Deleted presets

~/.local/share/llm-manager/         # Data
├── models/                         # GGUF files
└── bin/                            # llama-server binaries
```

Per-model configs are named `<model_name>.yaml` (without `.gguf`). Deleted configs are moved to `unused/` (recoverable).

### Serve mode

Run a model directly with llama-server and expose an OpenAI-compatible API:

```bash
# Serve a model with API proxy on port 49222
./build.sh serve --model /path/to/model.gguf --api-port 49222

# Serve with a settings profile
./build.sh serve --model model.gguf --profile qwen

# Serve with API key authentication (Bearer token)
./build.sh serve --model model.gguf --api-port 49222 --api-key secret

# Serve with API proxy and WebSocket dashboard
./build.sh serve --model model.gguf --api-port 49222 --ws-enable

# Serve with custom dashboard port and auth
./build.sh serve --model model.gguf --api-port 49222 --ws-enable --ws-port 8081 --ws-auth mykey

# Serve with a custom backend binary path
./build.sh serve --model model.gguf --backend-binary /path/to/custom/llama-server

# Serve bound to a specific network interface
./build.sh serve --model model.gguf --host 0.0.0.0

# Redirect logs to a file (useful for systemd)
./build.sh serve --model model.gguf --log-file /var/log/llm-manager/model.log

# Serve with TLS enabled (auto-generates self-signed certificates)
./build.sh serve --model model.gguf --api-port 49222 --tls-enable

# Serve with custom TLS certificates
./build.sh serve --model model.gguf --api-port 49222 --tls-enable --tls-cert /path/to/cert.pem --tls-key /path/to/key.pem

# Combine options
./build.sh serve --model model.gguf --api-port 49222 --host 0.0.0.0 --backend-binary /opt/rocm/bin/llama-server --log-file /var/log/llm-manager/model.log
```

The serve command automatically resolves the llama-server binary from the backend-specific directory (`~/.local/share/llm-manager/bin/llama-server-{cpu,vulkan,rocm}-{version}/`) and sets `LD_LIBRARY_PATH` for shared libraries. If the binary is not found, it downloads it from the llama.cpp GitHub releases. Use `--backend-binary` to specify a custom binary path, `--host` to override the network bind address for both the API proxy and WebSocket servers (default is from config), and `--log-file` to redirect logs to a file instead of stdout (useful for systemd).

The API proxy forwards requests to the running llama-server instance. Explicitly handled endpoints:

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
| `/v1/health` | GET | Health check (alias) |
| `/metrics` | GET | Prometheus metrics |
| `/props` | GET/POST | Get/set server properties |
| `/slots` | GET | Slot monitoring |
| `/lora-adapters` | GET/POST | List/load LoRA adapters |
| `/models/load` | POST | Load a model (router mode) |
| `/models/unload` | POST | Unload a model (router mode) |
| `/api/status` | GET | Server status (pid, uptime, loaded models) |

> **Note:** All paths not listed above are automatically proxied to the llama-server instance. New endpoints added to llama.cpp work without any code changes.

The API proxy supports **SSE (Server-Sent Events) streaming** for chat completions and other streaming endpoints, and **CORS** is enabled for all origins with GET/POST/PUT/DELETE/OPTIONS methods.

### Server Settings

The Server Settings panel (top-right) shows server configuration:

| Setting | Description |
|---------|-------------|
| Host | Bind address — press `↵` to open host picker (lists all network interfaces) |
| Backend | Acceleration backend (cpu / vulkan / rocm / rocm-lemonade / cuda) — shows selected version |
| Threads | CPU threads for generation |
| Threads Batch | CPU threads for batch processing |
| Mode | Server mode — `↵` toggles between Normal, Router, Bench, and BenchTune |
| API Endpoint | Enable API proxy — `↵` toggles (disabled while server is running) |
| RPC Workers | Open the distributed inference manager window — press `↵` |
| API Key | API key for SearXNG authentication (optional) |
| Web Search | Web search configuration — `↵` opens picker (enabled, engine, URL, API key) |
| API Port | Port for the API proxy server (default: 49222) |
| Dashboard | WebSocket dashboard server — `↵` opens configuration picker (enabled, port, auth key, TLS) |
| Language | UI language — `↵` cycles between en/fr/it |

When API Endpoint is enabled, a proxy server starts on port `49222` that forwards requests to the running llama-server instance, exposing the full llama.cpp API (see Serve mode above).

Router mode allows loading multiple models simultaneously. The server starts without a model, then loads via `/load` API. The `Max Concurrent Predictions` setting limits how many models can be loaded at once.

> **Note:** Router mode is still WIP. It is not reachable via the Mode cycling key; it must be enabled through config.yaml (`default.server_mode: router`).

> **Note:** The Server Settings panel is hidden when a server is already running. Press `F2` to toggle Server Settings only when no server is active.

### Profiles

The Profiles panel (`p`) shows saved presets of settings for quick switching. Profiles include both built-in presets shipped with the application and user-defined ones (stored as YAML files in `~/.config/llm-manager/profiles/`). User profiles can be created (`s`), applied (`↵`), and deleted (`d`).

### System Prompt Presets

The System Prompt Presets panel contains named system prompts for different use cases (stored as YAML files in `~/.config/llm-manager/presets/`). Presets support create (`n`), edit (`e`), apply (`↵`), and delete (`d`). During edit, `⌃S` saves and `⎋` cancels.

### Keyboard shortcuts

- `j` / `k` or `↓` / `↑` — Navigate up/down in lists and menus
- `h` / `l` or `←` / `→` — Navigate left/right (e.g., horizontal scroll in README)
- `↵` (Enter) — Load model / Download selected / Expand log / Apply profile / Edit setting / Select GGUF files (in search)
- `f` — Filter local models list / Toggle Follow mode (in Log panel)
- `⎋` (Esc) — Back / Exit search / Collapse log / Clear local filter / Close modals
- `⇥` (Tab) — Switch active panels (next)
- `⇧⇥` (Shift+Tab) — Switch active panels (previous)
- `t` — Switch settings tab / Open tags modal (in LLM Settings)
- `/` — Search models on HuggingFace (opens search input modal)
- `l` — Load selected model / `u` — Unload selected model
- `d` — Delete model (in Models panel) / Delete backend version (in backend picker)
- `D` — Delete selected backend version (in LLM Settings)
- `A` — About box (license and version info)
- `⌃X` (Ctrl+X) — Toggle expert mode (in LLM Settings)
- `I` — Open Info panel
- `Y` — Edit YaRN RoPE parameters (rope_scale, rope_freq_base, rope_freq_scale)
- `N` — New preset (in System Prompt Presets) / Next Benchmark result
- `B` — Go back one page in search results
- `⌃H` (Ctrl+H) — Show Help overlay
- `⌃K` (Ctrl+K) — Show CmdLine overlay
- `⌃D` (Ctrl+D) — Delete model (with confirmation)
- `p` — Open Profiles panel / Pause or resume download / Previous Benchmark result (context-sensitive)
- `⌃P` (Ctrl+P) — Open Profile Picker modal (select from built-in or user profiles)
- `⌃S` (Ctrl+S) — Cycle search sort (Relevance/Downloads/Likes/Trending/Created) / Cycle local models sort / Save settings
- `⌃B` (Ctrl+B) — Back one page in search results
- `⌃L` (Ctrl+L) — Cycle UI language (en → fr → it → en)
- `↓` at bottom — Load more search results (infinite scroll)
- `⌃⌥K` (Ctrl+Alt+K) — Kill llama-server process forcefully
- `g` / `G` — Jump to top/bottom of log panel
- `PageUp` / `PageDown` — Scroll fast in logs, README, and Benchmark Output
- `⌃R` (Ctrl+R) — Fetch README for selected model (in search) / Reset LLM settings (in LLM Settings)
- `⌃E` (Ctrl+E) — Toggle enabled/disabled for specific settings
- `⌃O` (Ctrl+O) — Re-trigger onboarding wizard
- `⌃⇟` / `⌃⇞` (Ctrl+PgDn/PgUp) — Jump 10 settings down/up
- `Shift+←` / `Shift+→` — Resize horizontal panel split (20%-80%)
- `0-9, -, .` — Type numeric values directly (in LLM Settings)
- `Right` — View README (in search/files mode)
- `Esc` (in BenchTune) — Cancel benchmark tuning
- `F1`–`F6` — Focus/toggle individual panels (Models, Server, Info, Settings, Active, Log)
- `F9` / `F10` / `Ctrl+F10` — Show all panels
- `Ctrl+F7` — Focus Models panel
- `Ctrl+F8` — Focus Server Settings panel
- `Ctrl+F9` — Focus LLM Settings panel
- `e` (in CmdLine) — Export command to script
- `⌥C` (Alt+C, in Downloads) — Cancel download and remove temp file
- `Space` — Toggle selection (RPC workers, Benchmark parameters)
- `Alt+M` (in BenchTuneSetup) — Toggle benchmark mode
- `Alt+P` (in BenchTuneSetup) — Edit benchmark prompt
- `Alt+N` (in BenchTuneSetup) — Edit max tokens (n_predict)
- `Alt+I` (in BenchTuneSetup) — Edit iterations per test
- `Alt+C` (in BenchTuneSetup) — Edit chat template kwargs

### GPU Layers cycling

In the LLM Settings panel, the GPU Layers field cycles through three modes with arrow keys:

| Mode | Behavior |
|------|----------|
| Auto | Lets llama.cpp auto-detect based on available VRAM (default) |
| Specific number | Offloads exactly that many layers to GPU |
| All | Offloads all layers (equivalent to `-ngl 999`) |

Arrow keys cycle: `Auto` → `1` → `2` → ... → `N` → `All` → `Auto`. Pressing `↵` from a specific number opens an edit buffer for direct input.

### MTP (Multi-Token Prediction)

MTP is an experimental feature that uses a draft model to predict multiple tokens in parallel, improving inference speed. When a model with MTP architecture is selected, the app automatically detects it and enables the `--draft-mtp` flag. The number of draft tokens is read from the GGUF metadata and displayed in the Model Info panel.

### Log panel modes

The Log panel supports two modes:

| Mode | Behavior |
|------|----------|
| **Following** (default) | Auto-scrolls to the bottom as new log entries arrive. Press `g` to exit. |
| **Manual** | Allows manual scrolling through log history. Press `G` to return to bottom and re-enable following. |

Press `f` in the Log panel to toggle between modes. The current mode is displayed in the panel title (e.g., "Log (F6) - Following" or "Log (F6) - Manual"). PageUp/PageDown keys scroll 15 lines at a time.

### Panels

The app has several panels that can be toggled visible or hidden:

| Panel | Description |
|-------|-------------|
| **Models** | Left panel: local model list, search results, download progress |
| **Server Settings** | Server configuration (host, backend, threads, mode, API) |
| **Model Info** | GGUF metadata: architecture, parameters, tokenizer, VRAM estimate |
| **LLM Settings** | Loading, GPU, evaluation, sampling, and repetition parameters |
| **Active Model** | Real-time metrics: TPS, context usage, CPU/RAM/VRAM, benchmarking state (no tokens generated) |
| **WebSocket Dashboard** | Live WebSocket metrics and settings visualization (port 49223 by default) |
| **Log** | Server log with expand/collapse and level coloring |
| **Profiles** | Saved presets of settings for quick switching |
| **System Prompt Presets** | Named system prompts for different use cases |
| **README** | Markdown-rendered documentation for HuggingFace models |
| **Downloads** | Download progress with pause/resume and cancel |

Panels can be individually toggled on/off via `F1`–`F6` (Models=1, ServerSettings=2, ModelInfo=3, LlmSettings=4, ActiveModel=5, Log=6). Focus panels with `Ctrl+F7` (Models), `Ctrl+F8` (Server Settings), `Ctrl+F9` (LLM Settings). Press `F9`, `F10`, or `Ctrl+F10` to show all panels. When a panel is hidden, other panels expand to fill the space.

### Search features

| Feature | Description |
|---------|-------------|
| **Search input** | `/` opens a text input modal; type query and press `Enter` to search |
| **Sort cycling** | `⌃S` (Ctrl+S) cycles through Relevance, Downloads, Likes, Trending, Created |
| **Pagination** | `⌃B` (Ctrl+B) goes back one page; `Down` at bottom loads more results (infinite scroll) |
| **File selection** | `Enter` selects the highlighted model's GGUF files |
| **README viewing** | `->` fetches and displays the model's README from HuggingFace; `Enter` in README expands to fullscreen |
| **README horizontal scroll** | `h`/`l` keys scroll horizontally |
| **Multi-word search** | Type space-separated words (e.g. "qwen opus"); all must match the model name. Matching words are highlighted in cyan. |
| **README rendering** | Full markdown renderer with headings, code blocks, lists, blockquotes, tables, and task lists |

### Web Search

llm-manager can automatically search the web when your chat messages contain research-oriented keywords (compare, vs, review, which is better, pros and cons, latest, best, recommend, etc.). Results are fetched via [SearXNG](https://github.com/searxng/searxng) and injected into the prompt before your message.

**Server-side flow:** Web search runs entirely on the llm-manager server. External clients connect to the API proxy (default port `49222`) — no special headers or endpoints needed. The server intercepts chat completions, performs the SearXNG search, injects results into the prompt, and forwards the enriched request to llama-server. Clients never need direct access to SearXNG.

**Configuration:** Open the Server Settings panel (F2), navigate to the **Web Search** field, and press `↵`. The Web Search Picker dialog shows:

| Field | Type | Description |
|-------|------|-------------|
| **Enabled** | Toggle | Turn web search on/off |
| **Engine** | Picker | Search engine (searxng, duckduckgo, brave, google, startpage) — only SearXNG with a configured URL returns results |
| **Engine URL** | Text input | URL of your self-hosted SearXNG instance (e.g., `https://search.example.com`) |
| **API Key** | Text input | Bearer token for SearXNG authentication (optional, masked as `****` when set) |

**How it works:** When a message matches a search keyword, the app:
1. Uses the message as the search query
2. Fetches up to 10 results via SearXNG (`{engine_url}/search?q={query}&format=json`)
3. Fetches Wikipedia pages and up to 5 other URLs in parallel
4. Prepends a `[WEB CONTEXT]...[END WEB CONTEXT]` block to the message with search results and page content

**Timeout:** Web search has a 15-second timeout per request.

**Config YAML:**
```yaml
default:
  web_search_enabled: true
  web_search_engine: searxng
  web_search_engine_url: "https://search.example.com"
  web_search_api_key: null  # optional
```

### WebSocket Dashboard

The WebSocket Dashboard provides a real-time visualization of model metrics and settings via a web browser. Access it by navigating to `http://localhost:49223` (configurable) in any browser. The dashboard displays:

| Metric | Description |
|--------|-------------|
| Generation Speed | Tokens per second (TPS) |
| Prompt Speed | Prompt processing TPS |
| Latency | Milliseconds per token |
| Tokens | Tokens generated with progress bar (decoded_tokens / max_tokens, or '∞' if not configured) |
| VRAM | GPU memory used/total (progress bar) |
| RAM | System memory usage |
| CPU | CPU usage percentage |

The dashboard also shows current inference settings (backend, threads, temperature, sampling parameters, etc.) and the full server command line.

To configure the dashboard:
1. Open **Server Settings** panel (F2)
2. Navigate to **Dashboard** field and press `↵`
3. Toggle enabled/disabled, set port (default: 49223), and optionally set an auth key
4. Press `↵` to save, `⎋` to close

When an auth key is set, clients must include `?auth=<key>` in the URL (e.g., `http://localhost:49223/dashboard?auth=mypassword`).

The `--host` option in serve mode binds **both** the API proxy server and the WebSocket dashboard server to the same address, ensuring they use the same network interface.

### Backend selection

Multiple backends are supported via the llama.cpp server:

| Backend | Source | Description |
|---------|--------|-------------|
| **CPU** | [ggml-org/llama.cpp](https://github.com/ggml-org/llama.cpp) | CPU-only inference (standard) |
| **Vulkan** | [ggml-org/llama.cpp](https://github.com/ggml-org/llama.cpp) | GPU via Vulkan (Universal: AMD/NVIDIA/Intel) |
| **ROCm** | [ggml-org/llama.cpp](https://github.com/ggml-org/llama.cpp) | GPU via ROCm (AMD Native) |
| **ROCm Lemonade** | [lemonade-sdk/llamacpp-rocm](https://github.com/lemonade-sdk/llamacpp-rocm) | GPU via ROCm (AMD Optimized, auto-detects GFX architecture) |
| **CUDA** | [ai-dock/llama.cpp-cuda](https://github.com/ai-dock/llama.cpp-cuda) | GPU via CUDA (NVIDIA Native, CUDA 12.8) |

### Server modes

| Mode | Description |
|------|-------------|
| **Normal** | Single model via CLI (default) |
| **Router (XP!)** | Multiple models via API, loads via `/load` endpoint |
| **Bench GPU** | GPU benchmarking mode |
| **BenchTune** | Parameter auto-tuning mode |

### Benchmark Tuning

The Benchmark Tuning system auto-tunes model parameters for optimal performance. Two modes are available:

- **RuntimeOnly**: Single server, params sent in request body (no server restarts)
- **Full**: New server spawned for each parameter combination

Tunable parameters: temperature (0.4-1.0), top_p (0.8-1.0), top_k (10-40), repeat_penalty (1.0-1.2), flash_attn (0/1), threads (4-16), batch_size (512-2048), expert_count (1-4).

Results can be exported as Markdown table, JSON, YAML, or HTML report with summary cards, winner section, impact analysis, and Chart.js charts.

### RPC Workers

> **NOTE:** RPC Workers is WIP and not yet tested.

Remote workers for distributed inference. Each worker has a name, IP address, and port (default: 50052). Managed via the dedicated RPC Workers window — add (`n`), edit (`e`), delete (`d`), or toggle selection (`Space`).

### CmdLine overlay

Press `Ctrl+K` to view the full command line that would be executed to start the llama.cpp server. The overlay shows the binary path, model path, and all parameters (threads, context size, GPU layers, temperatures, samplers, etc.) so you can copy or inspect the exact invocation. Note that `-ngl` is only included when GPU Layers is set to a specific number or "All"; in "Auto" mode the flag is omitted so llama.cpp can decide dynamically.

From the CmdLine overlay, press `e` to export the command to `/tmp/test_llamaserver.sh` as a bash script (overwrites if it exists). Press `⎋` to close.

### LLM Settings

The LLM Settings panel (28 standard fields in 6 groups, 55 total in expert mode):

**Loading (0-2):** Context length, System prompt preset, Keep in memory (mlock)

**GPU (3-8):** GPU Layers, Flash Attention, KV Cache Offload, Cache Type K, Cache Type V, Active Experts

**Evaluation (9-11):** Eval Batch, Unified KV, Max Concurrent Predictions

**Sampling (12-17):** Seed, Temperature, Top-k, Top-p, Min P, Max Tokens

**Repetition (18-21):** Repetition Penalty, Rep. Last N, Presence Penalty, Frequency Penalty

**Backend (22-23):** Tags (semicolon-separated), LLama.cpp Version (per-backend: CPU / Vulkan / ROCm / ROCm Lemonade / CUDA)

**Additional settings:** threads_batch, batch_size, ubatch_size, parallel, keep, swa_full, mmap, numa (None/Distribute/Isolate/Numactl), split_mode (None/Layer/Row/Tensor), tensor_split, main_gpu, fit, embedding, expert_count, jinja, auto_chat_template, chat_template, chat_template_kwargs, typical_p, mirostat (Off/1/2), mirostat_lr, mirostat_ent, ignore_eos, samplers (semicolon-separated order), repeat_penalty, repeat_last_n, presence_penalty, frequency_penalty, lora, lora_scaled, webui

**Speculative decoding:** Spec Type (Off, draft-mtp, draft-simple, draft-eagle3, ngram variants), Spec Draft N Max (0-16)

**Cache Type K/V options:** F32, F16, BF16, Q8_0, Q5_0, Q5_1, Q4_0, Q4_1, Iq4Nl

**Dirty tracking:** Modified fields are shown with the name in red and a trailing `*`. The status bar shows `*unsaved*` when settings are dirty. Press `⌃S` to save or `⌃R` to reset to defaults.

**VRAM estimate:** The app computes a detailed VRAM estimate based on model size, GPU layers, KV cache, activation overhead, and fixed overhead. The formula accounts for GQA ratio, FlashAttention (0.5x KV cache reduction), unified KV cache, KV cache quantization bytes, activation overhead (8x multiplier), and fixed overhead (3.8% of max VRAM or 500 MiB fallback). The estimate is shown in the LLM Settings title (e.g., "VRAM ~= 8.2 GB").

### GGUF Metadata

The Model Info panel shows parsed GGUF metadata including: architecture, layers, hidden size, context length, attention heads, KV heads, domain, capabilities, quantization, parameters (e.g., "7B", "405B"), tokenizer type, vocabulary size, and max context for VRAM. Metadata is parsed once and cached (debounced by file mtime).

### Active Model Metrics

The Active Model panel shows real-time metrics:

| Metric | Description |
|--------|-------------|
| TPS | Tokens per second (generation speed) |
| Prompt TPS | Prompt processing speed |
| Context usage | Progress bar showing ctx_used/ctx_max |
| CPU% | CPU usage percentage |
| RAM | RAM usage |
| VRAM | GPU memory used/total |
| Total VRAM | Sum of VRAM across all loaded models (in title bar) |

The panel also shows benchmarking state with progress bar and current parameter display when running BenchTune.

### Model Loading

Models load through several phases detected from llama.cpp log output: ServerStarting → LoadingModel → LoadingMeta → LoadingTensors → ServerListening. Completion is detected via the `/health` API endpoint. During loading, a progress bar shows the phase and details (layers loaded/total, tensor count, VRAM used).

Models have status states: Available, Loading, Loaded, Failed (with error message shown in red, e.g., "OOM", "Router Crash"), and Benchmarking.

### Download Management

Downloads can be paused and resumed by pressing `p` while a download is selected. Press `⌥C` (Alt+C) to cancel a download entirely, which also removes the temporary file. Before downloading, the app checks available disk space and warns if insufficient. Download progress shows bytes per second and percentage complete. The Downloads panel shows all active downloads with individual controls.

### Confirmation dialogs

The app uses confirmation dialogs for:
- **Exit** — warns about loaded models
- **Delete** — confirms irreversible deletion
- **Reset** — confirms resetting all LLM settings
- **Unload** — confirms unloading a model via API
- **DeleteBackend** — confirms deleting a backend binary version from disk

### Mouse support

Mouse interactions are supported: clicking on panels to focus them, and scrolling in the log panel, README panel, settings, profiles, and presets panels.

### Panel Resize

The horizontal split between left panels (Models + Info) and right panels (Settings/README) can be resized:

| Method | Description |
|--------|-------------|
| **Drag border** | Click and drag the vertical border between left and right panels |
| **Scroll on border** | Scroll mouse wheel while hovering over the border (1% steps) |
| **Keyboard** | `Shift+←` / `Shift+→` to adjust by 1% (range: 20%-80%) |

The current split percentage is shown in the status bar (e.g., `│ 55%`). While actively resizing, the indicator shows `│ 55% ← resize →`.

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines on how to report bugs, suggest features, and submit pull requests.

## License

GPLv3
