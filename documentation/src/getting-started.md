# Getting Started

## Installation

### From source

```bash
git clone https://github.com/aginies/llmtui.git
cd llmtui
cargo build --release
```

### Platform Support

llm-manager runs on Linux, macOS, and Windows. GPU backends available per platform:

| Platform | CPU | Vulkan | ROCm | ROCm Lemonade | CUDA |
|----------|-----|--------|------|---------------|------|
| **Linux x64** | Yes | Yes | Yes | Yes | Yes |
| **Linux ARM64** | Yes | — | — | — | — |
| **Windows x64** | Yes | Yes | Yes (HIP) | — | Yes (12.4 / 13.1) |
| **macOS ARM64** | Yes | — | — | — | — |
| **macOS x64** | Yes | — | — | — | — |

ROCm Lemonade (AMD optimized) is Linux-only and auto-detects your GPU architecture (e.g. `gfx1100`).

### Using the build script

A convenience script is included for common operations:

```bash
./build.sh build      # Build (debug)
./build.sh run        # Build and run (TUI mode)
./build.sh serve      # Serve a model
./build.sh servedoc   # Serve docs with watch mode
./build.sh release    # Release build
./build.sh clean      # Remove build artifacts
./build.sh format     # Format code
./build.sh clippy     # Run clippy
./build.sh doc        # Build documentation
./build.sh help       # Show help
```

## First Run

On first launch, llm-manager creates a default configuration in `~/.config/llm-manager/config.yaml` and sets up the models directory at `~/.local/share/llm-manager/models/`.

```bash
cargo run
```

The application will:

1. Load (or create) the config file
2. Discover any `.gguf` files in the models directory
3. Start the TUI

## Navigating the Interface

The TUI is divided into several panels:

- **Models panel** (left) — list of local GGUF models
- **Settings panel** (right) — server and LLM settings
- **Log panel** (bottom) — live output from llama.cpp
- **Download panel** — appears when downloading files

Use `Tab` to cycle between panels, and `Ctrl+H` for panel-specific help.

## Searching for Models

To search HuggingFace for models:

1. Press `/` to enter search mode
2. Type your query and press `Enter`
3. Results appear sorted by relevance by default
4. Press `Ctrl+S` to cycle sort order (Relevance / Downloads / Likes / Trending / Created)
5. Press `Ctrl+B` to go back one page, or scroll down at the bottom for more results
6. Press -> to fetch the model's README (auto-fetched when navigating results)

**Multi-word search:** Type space-separated words (e.g. `qwen opus`) to search with AND logic — all words must match the model name.

### Downloading Models

To download a model from HuggingFace:

1. Press `/` to enter search mode
2. Type your query and press `Enter`
3. Press `l` on a result to browse available GGUF files
4. Select a file and press `Enter` to download
5. Press `⌥C` (Alt+C) to cancel, or `p` to pause/resume the download at any time

The download progress is shown in the Download panel with speed (MiB/s), ETA, and status indicators. Before downloading, the app checks available disk space and warns if insufficient. Cancelled downloads automatically remove the temporary file. Once complete, the model appears in the Models panel (in your models directory).

### Loading Models

Once a model is downloaded (or has one locally in your models directory):

1. Select the model in the Models panel
2. Press `l` (or `Enter`) to load it

The loading process shows a progress bar with phases:

- Server starting
- Loading model weights
- Loading metadata
- Loading tensors (with GPU layer count)
- Server listening
- Ready (detected via `/health` API polling)

### Log Panel

The Log panel shows live output from the llama.cpp server. Press `Enter` to expand to fullscreen, `Esc` to collapse. Press `f` to toggle between Following (auto-scroll) and Manual (scroll history) modes.

### Other Features

- **Profiles** (`p`) — Quick-switch between saved settings presets
- **Profile Picker** (`Ctrl+P`) — Open a modal to select from built-in or user profiles
- **System Prompt Presets** — Named system prompts for different use cases (Coder, Thinker, Mathematician)
- **RPC Workers** — Manage distributed inference nodes from Server Settings
- **Benchmark Tuning** — Auto-tune model parameters for optimal performance (set Mode to BenchTune)
- **Router Mode** — Load multiple models simultaneously
- **Panel Resize** — Drag the border between left and right panels, or use `Shift+←/→` (20%-80%)
- **Mouse support** — Click panels to focus, scroll in logs, README, and settings

## Using Serve Mode

You can also start a model directly from the command line:

```bash
./build.sh serve --model /path/to/model.gguf
```

Or with a settings profile:

```bash
./build.sh serve --model model.gguf --profile qwen
```

With a custom backend binary:

```bash
./build.sh serve --model model.gguf --backend-binary /opt/rocm/bin/llama-server
```

Bound to a specific network interface:

```bash
./build.sh serve --model model.gguf --host 0.0.0.0
```

Logs redirected to a file:

```bash
./build.sh serve --model model.gguf --log-file /var/log/llm-manager/model.log
```

### API Proxy

Start with an OpenAI-compatible API proxy:

```bash
./build.sh serve --model model.gguf --api-port 49222
```

With authentication:

```bash
./build.sh serve --model model.gguf --api-port 49222 --api-key secret
```

The API proxy forwards requests to the llama-server instance and supports all llama.cpp endpoints including chat completions, embeddings, and more. It supports **SSE (Server-Sent Events) streaming** for chat completions and other streaming endpoints, and **CORS** is enabled for all origins.
