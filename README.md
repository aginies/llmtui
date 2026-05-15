# llm-manager

A terminal UI (TUI) for managing local LLM models with HuggingFace search, download, and inference control.

![Screenshot](docs/screenshot.png)

## Features

- **Search models** on HuggingFace by name (filters to GGUF models, 100 results per page)
- **Download** GGUF model files with progress tracking
- **Load/unload** models via llama.cpp server
- **Chat** with loaded models in the terminal
- **Configure** loading and inference parameters per model
- **GGUF file browser** — list and select specific GGUF files for a model
- **Log panel** — expand/collapse with Enter/Esc
- **HuggingFace URL links** — navigate to model pages from Model Info
- **Version picker** — select llama.cpp binary versions per backend (CPU, Vulkan, ROCm)

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
