# Usage

## Serve Mode

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

# Combine options
# Serve with API proxy and WebSocket dashboard on a specific host
./build.sh serve --model model.gguf --api-port 49222 --ws-enable --host 192.168.1.100

# Combine all options
./build.sh serve --model model.gguf --api-port 49222 --ws-enable --host 0.0.0.0 --backend-binary /opt/rocm/bin/llama-server --log-file /var/log/llm-manager/model.log
```

The serve command automatically resolves the llama-server binary from the backend-specific directory (`~/.local/share/llm-manager/bin/llama-server-{cpu,vulkan,rocm}-{version}/`) and sets `LD_LIBRARY_PATH` for shared libraries. If the binary is not found, it downloads it from the llama.cpp GitHub releases. Use `--backend-binary` to specify a custom binary path, `--host` to override the network bind address for both the API proxy and WebSocket servers (default is from config), and `--log-file` to redirect logs to a file instead of stdout.

## Model Management

### Listing Models

The Models panel shows all `.gguf` files found in your models directories (recursively). The display name is the relative path from the models directory.

- `f` — Filter local models by name (case-insensitive substring match)
- `Esc` — Clear active filter and return to full list

### Loading and Unloading

- `l` or `Enter` — Load selected model
- `u` — Unload model from server
- `Ctrl+D` — Delete model (with confirmation)

When a model is loaded, its state changes to **Loaded** showing the port and PID. You can load multiple models when using Router mode.

### Deleting Models

Pressing `Ctrl+D` prompts for confirmation before moving the model file and its YAML config to `~/.config/llm-manager/unused/`. Both can be restored later.

## Search

Search mode lets you browse and download GGUF models from HuggingFace:

| Key | Action |
|-----|--------|
| `/` | Open search input modal — type query and press `Enter` to search |
| `Enter` | Select GGUF files for the highlighted model |
| `Esc` | Exit search |
| `Ctrl+S` | Cycle sort order |
| `Ctrl+B` | Go back one page |
| `Down` (at bottom) | Load more results |
| `Ctrl+R` | Fetch and view README for the selected model |

### Multi-word Search

Type space-separated words (e.g. `qwen opus`) to search with AND logic — all words must match the model name. Matching words are highlighted in cyan in the results list.

![Search Results](images/search_result.png)

### GGUF File Browser

When viewing GGUF files for a model:

| Key | Action |
|-----|--------|
| `j` / `k` | Navigate files |
| `Enter` | Download selected file |
| `Esc` | Go back to search results |
| `⌥C` | Cancel download and remove temp file |

### Download Panel

When one or more files are downloading, the Download panel appears at the bottom of the screen, showing progress, speed (MiB/s), ETA, and status for each download. Before downloading, the app checks available disk space and warns if insufficient. Cancelled downloads automatically remove the temporary file.

| Key | Action |
|-----|--------|
| `j` / `k` | Navigate downloads |
| `p` | Pause / Resume selected download |
| `⌥C` | Cancel selected download and remove temp file |

Status indicators: **Downloading** (yellow), **Paused** (white), **Complete** (green), **Cancelled** (red), **Error** (red).

## Loading Models

When you load a model, the application:

1. Resolves the llama-server binary for the selected backend (CPU/Vulkan/ROCm)
2. Spawns the server with the current settings
3. Loads the model via the server's `/models/load` API
4. Polls the server's `/metrics` and `/health` endpoints for status
5. Displays a progress bar showing loading phases

### Loading Phases

The progress bar tracks:

- **Server starting** (8%) — llama.cpp binary is launched
- **Loading model** (7%) — weights file is being read
- **Loading metadata** (7%) — GGUF metadata is parsed
- **Loading tensors** (70%) — tensors are loaded and offloaded to GPU
- **Server listening** (8%) — HTTP server is ready
- **Complete** — model is ready for inference

During tensor loading, the progress bar shows offloaded layers (e.g., `16/32`) parsed from llama.cpp's log output.

## Settings

### Server Settings

| Setting | Default | Description |
|---------|---------|-------------|
| **Host** | 127.0.0.1 | Bind address for the llama.cpp server. Use `0.0.0.0` to accept connections from other machines. |
| **Backend** | auto-detected | Acceleration backend: auto-detected based on GPU (Cuda for NVIDIA, Rocm for AMD, Vulkan for Intel). Options: `cpu` (CPU-only), `vulkan` (NVIDIA/AMD/Intel GPU), `rocm` (AMD GPU), `rocm-lemonade` (AMD optimized), `cuda` (NVIDIA CUDA 12.8). Shows the currently selected version. |
| **Threads** | (physical cores) | CPU threads for generation. Set to your physical core count for best performance. |
| **Threads Batch** | 8 | CPU threads for batch processing (prompt evaluation). |
| **Mode** | Normal | Server mode: `Normal` (single model), `Router` (multiple models), `Bench` (run llama-bench), or `BenchTune` (parameter auto-tuning). |
| **API Endpoint** | false | Enable the API proxy server (see Serve Mode). |
| **Dashboard** | false | WebSocket dashboard server (port 49223). Press `Enter` to configure (enabled, port, auth key, TLS). |
| **RPC Workers** | None | Open a dedicated window to manage distributed inference nodes (IP:Port). |

> **Note:** The Server Settings panel is hidden when a server is already running. Press `F2` to toggle Server Settings only when no server is active.

### LLM Settings

The LLM Settings panel has 19 standard fields, 12 expert fields (revealed with `Ctrl+X`), and 15 ultra fields (hidden even in expert mode), for a total of 46 fields. Arrow keys adjust values; `+`/`-` for coarse changes, `Left`/`Right` for fine. Toggle fields respond to `e` or `Ctrl+E`.

#### Loading

| Field | Default | Description |
|-------|---------|-------------|
| **Prompt** | General | System prompt preset that defines the model's initial behavior. Presets include General, Coder, Thinker, Mathematician, and any user-defined prompts. |
| **Context** | 131072 | Context window size in tokens. Larger values consume more VRAM and RAM. Models often have a maximum context length (e.g., 32K, 128K). |
| **Keep in memory** | false | Locks model weights in RAM (`-mlock`) to prevent the OS from swapping them out. Useful when repeatedly loading/unloading models. Increases RAM usage. |

#### GPU Offload

| Field | Default | Description |
|-------|---------|-------------|
| **GPU Layers** | Auto | Number of model layers offloaded to GPU memory. `Auto` lets llama.cpp decide based on available VRAM. `Specific` sets an exact number. `All` offloads every layer (`-ngl 999`). |
| **Flash Attention** | true | Enables Flash Attention 2 for faster inference with lower memory usage. Requires GPU support. Can improve throughput by 20-40%. |
| **KV Cache Offload** | true | Offloads the KV cache to RAM when GPU memory is full. Trade-off: more VRAM available for model weights at the cost of slower cache access. |
| **Cache Type K** | F16 | Data type for the key cache. Options: F32 (most accurate, most memory), F16 (default), BF16 (better than F16 for some models), Q8_0, Q5_0, Q5_1, Q4_0, Q4_1, Iq4Nl. |
| **Cache Type V** | F16 | Data type for the value cache. Same options as Cache Type K. Using lower precision reduces VRAM but may affect quality. |
| **Active Experts** | 1 | For Mixture-of-Experts (MoE) models, the number of experts activated per token. Higher values improve quality but increase compute. |

#### Evaluation

| Field | Default | Description |
|-------|---------|-------------|
| **Eval Batch** | 512 | Logical maximum batch size for evaluation. Larger batches improve throughput but increase memory usage. Set to the model's native context length for single-sequence inference. |
| **Unified KV** | true | Shares KV cache across sequences, reducing memory usage when running multiple prompts. Can cause cache eviction conflicts. |

#### Sampling

| Field | Default | Description |
|-------|---------|-------------|
| **Seed** | -1 | Random seed for reproducible outputs. `-1` means random each time. Set to a fixed value for debugging or reproducibility. |
| **Temperature** | 0.8 | Controls randomness in sampling. Higher values (1.0-2.0) produce more creative/divergent outputs. Lower values (0.0-0.5) produce more deterministic/crisp outputs. |
| **Top-k** | 40 | Limits sampling to the k most likely next tokens. `0` disables. Smaller values make outputs more focused. Typical: 20-50. |
| **Top-p** | 0.95 | Nucleus sampling: limits to tokens whose cumulative probability reaches p. `1.0` disables. Lower values (0.8-0.95) reduce randomness. |
| **Min P** | 0.0 | Minimum probability threshold for sampling. Tokens with probability below this fraction of the highest-probability token are excluded. Useful for controlling extreme outputs. |
| **Max Tokens** | None (unlimited) | Maximum tokens to generate per response. None means no limit (until EOS token). |

#### Repetition Control

| Field | Default | Description |
|-------|---------|-------------|
| **Repetition Penalty** | 1.1 | Penalizes tokens that have already appeared. Values > 1.0 reduce repetition. Typical: 1.1-1.2. |
| **Rep. Last N** | 64 | Number of recent tokens to consider for repetition penalty. `-1` uses the full context. |

#### Yarn RoPE

| Field | Default | Description |
|-------|---------|-------------|
| **Yarn RoPE** | false | Enables YaRN (Yet another RoPE extensioN) for extending context beyond the model's training length. |
| **Yarn Params** | — | Opens a modal to configure three floating-point values: `rope_scale` (default 1.0, multiplies context), `rope_freq_base` (default 0.0, overrides the model's base frequency), `rope_freq_scale` (default 1.0, scales the frequency). Only digits, `.`, `-`, `e`, and `E` are accepted. |

#### Tags

| Field | Default | Description |
|-------|---------|-------------|
| **Tags** | None | Per-model tags stored in the YAML config. Press `Enter` to open the tag editor modal. Press `t` in the LLM Settings panel to open the tag editor. |

#### Backend

| Field | Default | Description |
|-------|---------|-------------|
| **LLama.cpp Version** | Latest | Shows the currently selected backend version. Press `Enter` to open the backend version picker. |

#### Expert Mode

Press `Ctrl+X` to toggle expert mode, which reveals 16 additional parameters:

**Loading (expert):** NUMA (None/Distribute/Isolate/Numactl)

**GPU (expert):** Cache Type K (toggle), Cache Type V (toggle), Main GPU, Fit, Active Experts (toggle)

**Sampling (expert):** Mirostat (Off/1/2), Mirostat LR, Mirostat Ent, Ignore EOS (toggle)

**Repetition (expert):** Presence Penalty (toggle, -2.0 to 2.0), Frequency Penalty (toggle, -2.0 to 2.0)

**Speculative (expert):** MTP (toggle), Spec Type, Spec Draft N Max

These fields follow the same navigation and editing rules as standard fields. Arrow keys adjust values, `Enter` enters direct edit mode, and dirty fields are highlighted in yellow.

#### Ultra Fields

19 ultra fields are hidden even in expert mode. They include: Typical P, Mirostat, Mirostat LR, Mirostat Ent, Ignore EOS, Samplers, DRY Multiplier, DRY Base, DRY Allowed Length, DRY Penalty Last N, Threads Batch, UBatch Size, Keep, Split Mode, Tensor Split, Main GPU, Fit, Embedding, RPC. These require direct config file editing or profile application.

**Cache Type K/V options:** F32, F16, BF16, Q8_0, Q5_0, Q5_1, Q4_0, Q4_1, Iq4Nl

#### Changing Values

Use `Left`/`Right` to adjust numeric fields by 1, or `Up`/`Down` for larger steps. Toggle fields respond to `e` or `Ctrl+E`. Dirty (changed) fields have the name in red and a trailing `*`. The status bar shows `*unsaved*` when settings are dirty.

### Saving Settings

- `Ctrl+S` — Save settings for the selected model
- `Ctrl+R` — Reset to defaults
- `e` / `Ctrl+E` — Toggle enabled/disabled (for Keep in memory, Flash Attention, KV Cache Offload, Cache Type K/V, Fit, Unified KV, Max Tokens, Presence/Frequency Penalty, Max Concurrent Pred, MTP, Ignore EOS, Yarn RoPE, Active Experts)
- `Ctrl+X` — Toggle expert mode (reveals 16 additional parameters)

Dirty (changed) fields are highlighted with red names and a trailing `*`.

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `j` / `k` | Navigate up/down |
| `Enter` | Load model / Select GGUF files (in search) / Expand log |
| `f` | Filter local models list / Toggle Follow mode (in Log panel) |
| `Esc` | Back / Exit search / Collapse log / Clear local filter |
| `Tab` | Switch panels (next) |
| `Shift+Tab` | Switch panels (previous) |
| `/` | Open search input modal |
| `l` | Load model |
| `u` | Unload model |
| `t` | Open tags editor (in LLM Settings) |
| `A` | About box (license and version info) |
| `Ctrl+D` | Delete model (with confirmation) |
| `Ctrl+H` | Panel-specific help |
| `Ctrl+K` | CmdLine overlay |
| `Ctrl+Alt+K` | Kill llama-server |
| `Ctrl+L` | Focus Log panel |
| `Ctrl+S` | Save settings / Cycle search sort (in search) |
| `Ctrl+R` | Reset settings (in LLM Settings) / Fetch README (in search) |
| `Ctrl+E` | Toggle field enabled/disabled (in LLM Settings: Cache Type K/V, Max Tokens, Presence/Frequency Penalty, Max Concurrent Pred, Flash Attention, Unified KV, Keep in memory, Fit, MTP, Ignore EOS, Yarn RoPE, Active Experts) |
| `Ctrl+X` | Toggle expert mode (in LLM Settings) |
| `Ctrl+P` | Open Profile Picker modal (in LLM Settings) |
| `Ctrl+U` | Open Dashboard URL modal (copy URL to clipboard) |
| `Ctrl+B` | Back one page in search |
| `g` / `G` | Jump to top/bottom of log |
| `PageUp` / `PageDown` | Fast scroll (logs, profiles, presets, benchmarks) |
| `F1`–`F6` | Toggle panels (Models, Server, Info, Settings, Active, Log) |
| `F9` / `F10` / `Ctrl+F10` | Show all panels |
| `Ctrl+F7` | Focus Models panel |
| `Ctrl+F8` | Focus Server Settings panel |
| `Ctrl+F9` | Focus LLM Settings panel |
| `Shift+←` / `Shift+→` | Resize horizontal panel split (20%-80%) |
| `p` | Pause/resume download / Previous benchmark result |
| `n` | New preset (System Prompt Presets) / Next benchmark result / Add new worker (RPC) |
| `Space` | Toggle selection (RPC workers, benchmark parameters) |
| `Alt+M` | Toggle benchmark mode (RuntimeOnly / Full) |
| `Alt+P` | Edit benchmark prompt |
| `Alt+N` | Edit n_predict (max tokens) |
| `Alt+I` | Edit iterations |
| `Alt+C` | Edit chat template kwargs / Cancel confirmation |
| `y` | Confirm destructive action |
| `Ctrl+h` | Cancel confirmation dialog |

## Log Panel

The Log panel displays live output from the llama.cpp server with level-based coloring.

### Log Modes

| Mode | Behavior |
|------|----------|
| **Following** (default) | Auto-scrolls to the bottom as new entries arrive. Press `g` to exit. |
| **Manual** | Allows manual scrolling through log history. Press `G` to return to bottom. |

Press `f` in the Log panel to toggle between modes. The current mode is shown in the panel title. Expand the log to fullscreen with `Enter`; collapse with `Esc`.

## RPC Workers

RPC Workers enable distributed inference across multiple machines. Each worker has a name, IP address, and port (default: 50052).

Open the RPC Workers manager from the Server Settings panel. Within the manager:

| Key | Action |
|-----|--------|
| `n` | Add new worker |
| `e` | Edit selected worker |
| `d` | Delete selected worker |
| `Space` | Toggle worker selection |
| `Esc` | Close manager |

## WebSocket Dashboard

The WebSocket Dashboard provides a real-time visualization of model metrics in any web browser. Access it at `http://localhost:49223` (default port).

### Configuration

Open the Server Settings panel, navigate to **Dashboard**, and press `Enter` to configure:

| Field | Description |
|-------|-------------|
| **Enabled** | Toggle the dashboard on/off |
| **Port** | Server port (default: 49223) |
| **Auth Key** | Optional authentication key |
| **TLS Enabled** | Enable TLS for secure dashboard access |
| **TLS Cert** | Path to TLS certificate file |
| **TLS Key** | Path to TLS private key file |

When an auth key is set, clients must include it as a URL parameter: `http://localhost:49223?auth=<key>`. With TLS enabled, the URL uses `https://`.

### Dashboard Display

The dashboard shows real-time metrics (TPS, prompt TPS, latency, context, VRAM, RAM, CPU) and current inference settings (backend, threads, temperature, sampling parameters, etc.) alongside the full server command line.

## Benchmark Tuning

Benchmark Tuning auto-tunes model parameters for optimal performance. Access it by setting the Server Mode to **BenchTune**.

Two modes are available:

- **RuntimeOnly** — Single server, params sent in request body (no server restarts)
- **Full** — New server spawned for each parameter combination

Tunable parameters: temperature (0.4–1.0), top_p (0.8–1.0), top_k (40–50), repeat_penalty (1.0–1.2), flash_attn (0/1), threads (4–16), batch_size (512–2048), expert_count (1–4), context_length, spec_type (speculative decoding type), draft_tokens.

![Benchmark Configuration](images/benchmark_configuration.png)

Results can be exported as Markdown table, JSON, YAML, or HTML report with summary cards, winner section, impact analysis, and Chart.js charts.

![Benchmark Results](images/bench_result_html.png)

Navigate between results with `p` (previous) and `n` (next).

## System Prompt Presets

Named system prompts for different use cases. Built-in presets: General, Coder, Thinker, Mathematician. User presets are stored as YAML files in `~/.config/llm-manager/presets/<name>.yaml`.

Open the System Prompt Presets panel and manage presets:

| Key | Action |
|-----|--------|
| `n` | Create new preset |
| `e` | Edit selected preset |
| `↵` | Apply preset |
| `d` | Delete selected preset (moved to `unused_presets/`) |
| `⌃S` | Save preset during edit |
| `Esc` | Close / Cancel edit |

## GPU Layers Cycling

In the LLM Settings panel, the GPU Layers field cycles through three modes with arrow keys:

| Mode | Behavior |
|------|----------|
| **Auto** | Lets llama.cpp auto-detect based on available VRAM (default) |
| **Specific number** | Offloads exactly that many layers to GPU |
| **All** | Offloads all layers (equivalent to `-ngl 999`) |

Arrow keys cycle: `Auto` → `1` → `2` → ... → `N` → `All` → `Auto`. Pressing `Enter` from a specific number opens an edit buffer for direct input. The `-ngl` flag is only added for Specific and All modes.

## Tags

Per-model tags can be edited in the LLM Settings panel. The Tags field opens an edit modal where you can add, remove, or modify tags associated with the model. Tags are stored in the per-model YAML config.

## MTP (Multi-Token Prediction)

MTP is an experimental feature that uses a draft model to predict multiple tokens in parallel, improving inference speed. When a model with MTP architecture is selected, the app automatically detects it and enables the `--draft-mtp` flag. The number of draft tokens is read from the GGUF metadata and displayed in the Model Info panel.

## GGUF Metadata

The Model Info panel shows parsed GGUF metadata including: architecture, layers, hidden size, context length, attention heads, KV heads, domain, capabilities, quantization, parameters (e.g., "7B", "405B"), tokenizer type, vocabulary size, and max context for VRAM. Metadata is parsed once and cached (debounced by file mtime).

## Active Model Metrics

The Active Model panel shows real-time metrics:

| Metric | Description |
|--------|-------------|
| TPS | Tokens per second (generation speed) |
| Prompt TPS | Prompt processing speed |
| Gen TPS | Generation tokens per second (separate from prompt TPS) |
| Context usage | Progress bar showing ctx_used/ctx_max |
| CPU% | CPU usage percentage |
| RAM | RAM usage |
| VRAM | GPU memory used/total |
| Total VRAM | Total GPU memory used (including non-model allocations) |
| Latency | Milliseconds per token (generation and prompt) |
| Tokens | Total decoded tokens generated |

The panel also shows benchmarking state with progress bar and current parameter display when running BenchTune.

## Backend Selection

Multiple backends are supported via the llama.cpp server:

| Backend | Source | Description |
|---------|--------|-------------|
| **CPU** | [ggml-org/llama.cpp](https://github.com/ggml-org/llama.cpp) | CPU-only inference (standard) |
| **Vulkan** | [ggml-org/llama.cpp](https://github.com/ggml-org/llama.cpp) | GPU via Vulkan (Universal: AMD/NVIDIA/Intel) |
| **ROCm** | [ggml-org/llama.cpp](https://github.com/ggml-org/llama.cpp) | GPU via ROCm (AMD Native) |
| **ROCm Lemonade** | [lemonade-sdk/llamacpp-rocm](https://github.com/lemonade-sdk/llamacpp-rocm) | GPU via ROCm (AMD Optimized, auto-detects GFX architecture) |
| **CUDA** | [ai-dock/llama.cpp-cuda](https://github.com/ai-dock/llama.cpp-cuda) | GPU via CUDA (NVIDIA Native, CUDA 12.8) |
| **CPU ARM64** | [ggml-org/llama.cpp](https://github.com/ggml-org/llama.cpp) | CPU-only for ARM64 Linux |
| **CPU Windows** | [ggml-org/llama.cpp](https://github.com/ggml-org/llama.cpp) | CPU-only for Windows |
| **Vulkan Windows** | [ggml-org/llama.cpp](https://github.com/ggml-org/llama.cpp) | Vulkan for Windows |
| **CUDA Windows 12.4** | [ai-dock/llama.cpp-cuda](https://github.com/ai-dock/llama.cpp-cuda) | CUDA 12.4 for Windows |
| **CUDA Windows 13.1** | [ai-dock/llama.cpp-cuda](https://github.com/ai-dock/llama.cpp-cuda) | CUDA 13.1 for Windows |
| **HIP Windows** | [ggml-org/llama.cpp](https://github.com/ggml-org/llama.cpp) | HIP (ROCm) for Windows |
| **CPU macOS ARM64** | [ggml-org/llama.cpp](https://github.com/ggml-org/llama.cpp) | CPU-only for macOS Apple Silicon |
| **CPU macOS x64** | [ggml-org/llama.cpp](https://github.com/ggml-org/llama.cpp) | CPU-only for macOS Intel |

Each backend has its own independently configurable llama.cpp version. Switching versions is instant — no re-download.

## Server Modes

| Mode | Description |
|------|-------------|
| **Normal** | Single model via CLI (default) |
| **Router** | Multiple models via API, loads via `/load` endpoint |
| **Bench** | GPU benchmarking mode (runs llama-bench) |
| **BenchTune** | Parameter auto-tuning mode |

## VRAM Estimate

The app computes a detailed VRAM estimate based on model size, GPU layers, KV cache, activation overhead, and fixed overhead. The formula accounts for GQA ratio, FlashAttention (0.5× KV cache reduction), unified KV cache, KV cache quantization bytes, activation overhead (8× multiplier), and fixed overhead (3.8% of max VRAM or 500 MiB fallback). The estimate is shown in the LLM Settings title (e.g., "VRAM ~= 8.2 GB").

## Confirmation Dialogs

The app uses confirmation dialogs for destructive actions:

- **Exit** — warns about loaded models
- **Delete** — confirms irreversible deletion
- **Reset** — confirms resetting all LLM settings
- **Unload** — confirms unloading a model via API
- **DeleteBackend** — confirms deleting a backend binary version from disk

## Mouse Support

Mouse interactions are supported: clicking on panels to focus them, and scrolling in the log panel, README panel, settings, profiles, and presets panels.

## Panel Resize

The horizontal split between left panels (Models + Info) and right panels (Settings/README) can be resized:

| Method | Description |
|--------|-------------|
| **Drag border** | Click and drag the vertical border between left and right panels |
| **Scroll on border** | Scroll mouse wheel while hovering over the border (1% steps) |
| **Keyboard** | `Shift+←` / `Shift+→` to adjust by 1% (range: 20%-80%) |

The current split percentage is shown in the status bar (e.g., `│ 55%`). While actively resizing, the indicator shows `│ 55% ← resize →`.

## CmdLine Overlay

Press `Ctrl+K` to view the full command line that would be executed to start the llama.cpp server. This shows the binary path, model path, and all parameters.

Press `e` in the overlay to export the command to `/tmp/test_llamaserver.sh`.

## Server Status

The status bar shows the current server status at the top:

- **Running:** `● 9090 Normal` (green dot with port and mode)
- **Stopped:** `○ Server` (gray)

Press `Ctrl+Alt+K` to kill the running llama-server. When stopped, all loaded models are reset to **Available** state.

## Profiles

Profiles are named presets of LLM settings. Built-in profiles include Qwen, Gemma, Llama, Mistral, and Phi. User profiles are stored as YAML files in `~/.config/llm-manager/profiles/<name>.yaml`.

- `p` — Apply a profile to current settings
- `Ctrl+S` — Save current settings as a new profile (in the Profiles panel)
- `Ctrl+D` — Delete a user-defined profile (moved to `unused_profiles/`)

![LLM Profiles](images/LLM_profiles.png)
