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
- **Version picker** — select llama.cpp binary versions per backend (CPU, Vulkan, ROCm)
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

### Backend selection

Three backends supported via the llama.cpp server:

| Backend | Description |
|---------|-------------|
| CPU | CPU-only inference |
| Vulkan | GPU via Vulkan (AMD/NVIDIA/Intel) |
| ROCm | GPU via ROCm 7.2 (AMD) |

### Version picker

Select llama.cpp binary versions per backend (CPU, Vulkan, ROCm) from the "LLama.cpp Version" field in LLM Settings.

- `TAB` — Switch backend
- `Enter` — Select version for the active backend
- `R` — Refresh releases from GitHub
- `C` — Toggle cached versions display
- `Esc` — Exit

Binaries are stored in `~/.local/share/llm-manager/bin/llama-server-{backend}-{version}/`. Switching versions is instant — no re-download.

### CmdLine overlay

Press `Ctrl+K` to view the full command line that would be executed to start the llama.cpp server. The overlay shows the binary path, model path, and all parameters (threads, context size, GPU layers, temperatures, samplers, etc.) so you can copy or inspect the exact invocation.

From the CmdLine overlay, press `e` to export the command to `/tmp/test_llamaserver.sh` as a bash script (overwrites if it exists).

## Configuration

Configuration is stored in the application's config directory (typically `~/.config/llm-manager/`).

Per-backend version config:

```yaml
llama_cpp_version_cpu: null
llama_cpp_version_vulkan: null
llama_cpp_version_rocm: null
```

## License

MIT
