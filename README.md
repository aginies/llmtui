# llm-manager

A terminal UI (TUI) for managing local LLM models with HuggingFace search, download, and inference control.

![Screenshot](docs/screenshot.png)

## Features

- **Search models** on HuggingFace by name
- **Download** GGUF model files with progress tracking
- **Load/unload** models via llama.cpp server
- **Chat** with loaded models in the terminal
- **Configure** loading and inference parameters per model
- **GGUF file browser** — list and select specific GGUF files for a model
- **Log panel** — expand/collapse with Enter/Esc
- **HuggingFace URL links** — navigate to model pages from Model Info

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

## Configuration

Configuration is stored in the application's config directory (typically `~/.config/llm-manager/`).

## License

MIT
