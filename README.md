# llm-manager

A terminal UI (TUI) for managing local LLM models with HuggingFace search, download, and inference control.

![Screenshot](docs/screenshot.png)

## Features

- **Search models** on HuggingFace by name (filters to GGUF models, 70 results per page)
- **Download** GGUF model files with progress tracking
- **Load/unload** models via llama.cpp server
- **Chat** with loaded models in the terminal
- **Configure** loading and inference parameters per model
- **GGUF file browser** — list and select specific GGUF files for a model
- **Log panel** — expand/collapse with Enter/Esc
- **HuggingFace URL links** — navigate to model pages from Model Info
- **CmdLine overlay** — full-screen view of the computed llama-server command line (`Ctrl+K`)
- **Export to script** — write the llama-server command to `/tmp/test_llamaserver.sh` from the CmdLine overlay (`e`)

## Prerequisites

- Rust toolchain (edition 2024)
- A HuggingFace account (for downloading gated models)

## Installation

```bash
git clone https://github.com/aginies/llmtui.git
cd llmtui
cargo build --release
```

## Usage

```bash
cargo run
```

### Build script

A convenience script is included for common operations:

```bash
./build.sh build      # Build (debug)
./build.sh run        # Build and run
./build.sh release    # Release build
./build.sh clean      # Remove build artifacts
./build.sh format     # Format code
./build.sh check      # cargo check
./build.sh test       # Run tests
./build.sh clippy     # Run clippy
```

### Serve mode

Run a model directly with llama-server and expose an OpenAI-compatible API:

```bash
# Serve a model with API proxy on port 49222
./build.sh serve --model /path/to/model.gguf --api-port 49222

# Serve with a settings profile
./build.sh serve --model model.gguf --profile qwen

# Serve with API key authentication (Bearer token)
./build.sh serve --model model.gguf --api-port 49222 --api-key secret
```

The serve command automatically resolves the llama-server binary from the backend-specific directory (`~/.local/share/llm-manager/bin/llama-server-{cpu,vulkan,rocm}-{version}/`) and sets `LD_LIBRARY_PATH` for shared libraries. If the binary is not found, it downloads it from the llama.cpp GitHub releases.

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

### Server Settings

The Server Settings panel (top-right) shows server configuration:

| Setting | Description |
|---------|-------------|
| Host | Bind address (127.0.0.1 or 0.0.0.0) |
| Backend | Acceleration backend (cpu / vulkan / rocm) |
| Threads | CPU threads for generation |
| Threads Batch | CPU threads for batch processing |
| Mode | Server mode (Normal / Router) |
| API Endpoint | Enable API proxy (True / False) |
| API Port | Port for the API proxy server (default: 49222) |

When API Endpoint is enabled, a proxy server starts on port `49222` that forwards requests to the running llama-server instance, exposing the full llama.cpp API (see Serve mode above).

### Keyboard shortcuts

- `j` / `k` — Navigate up/down
- `Enter` — Load model / Download selected / Expand log panel
- `Esc` — Back / Exit search / Collapse log panel
- `Tab` — Switch panels
- `t` — Switch settings tab
- `/` — Search models
- `l` — Load / `u` — Unload
- `Ctrl+H` — Help
- `Ctrl+K` — CmdLine overlay

### GPU Layers cycling

In the LLM Settings panel, the GPU Layers field cycles through three modes with arrow keys:

| Mode | Behavior |
|------|----------|
| Auto | Lets llama.cpp auto-detect based on available VRAM (default) |
| Specific number | Offloads exactly that many layers to GPU |
| All | Offloads all layers (equivalent to `-ngl 999`) |

Arrow keys cycle: `Auto` → `1` → `2` → ... → `N` → `All` → `Auto`. Pressing `Enter` from a specific number opens an edit buffer for direct input.

### Backend selection

Three backends supported via the llama.cpp server:

| Backend | Description |
|---------|-------------|
| CPU | CPU-only inference |
| Vulkan | GPU via Vulkan (AMD/NVIDIA/Intel) |
| ROCm | GPU via ROCm 7.2 (AMD) |

### CmdLine overlay

Press `Ctrl+K` to view the full command line that would be executed to start the llama.cpp server. The overlay shows the binary path, model path, and all parameters (threads, context size, GPU layers, temperatures, samplers, etc.) so you can copy or inspect the exact invocation. Note that `-ngl` is only included when GPU Layers is set to a specific number or "All"; in "Auto" mode the flag is omitted so llama.cpp can decide dynamically.

From the CmdLine overlay, press `e` to export the command to `/tmp/test_llamaserver.sh` as a bash script (overwrites if it exists).

## Configuration

Configuration is stored in the application's config directory (typically `~/.config/llm-manager/`).

### Backend binary resolution

llama-server binaries are stored in `~/.local/share/llm-manager/bin/` with versioned directories:

```
~/.local/share/llm-manager/bin/
├── llama-server-cpu-{version}/llama-server
├── llama-server-vulkan-{version}/llama-server
└── llama-server-rocm-{version}/llama-server
```

Switching versions is instant — no re-download. The binary is downloaded from `ggml-org/llama.cpp` GitHub releases on first use.

Per-backend version config:

```yaml
llama_cpp_version_cpu: null
llama_cpp_version_vulkan: null
llama_cpp_version_rocm: null
```

## License

MIT
