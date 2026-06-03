# Introduction

**LLM Manager** is a terminal UI (TUI) for managing local LLM models. It lets you search HuggingFace, download GGUF models, and load them via llama.cpp's `llama-server` — all from your terminal.

## Features

### Model Discovery & Downloads
- **HuggingFace search** — GGUF-filtered, paginated results with multiple sort options (relevance, downloads, likes, trending, created)
- **GGUF file browser** — browse and select specific GGUF files for each model
- **Download manager** — progress tracking with speed, ETA, and cancellation support; automatic disk space checks before downloading
- **Multi-word search** — space-separated words use AND logic for precise filtering

### Inference
- **Model loading** — progress visualization through server start, weight loading, metadata, tensor loading, and server listening phases
- **Chat with models** — interact with loaded models via the API proxy
- **RPC Workers** — manage distributed inference nodes from a dedicated window
- **Router Mode** — load multiple models simultaneously

### Configuration
- **Per-model settings** — full control over context length, GPU layers, sampling parameters, and more
- **Profiles** — save and quickly switch between named presets of settings
- **System Prompt Presets** — named system prompts for different use cases (Coder, Thinker, Mathematician, etc.)
- **Multi-backend support** — CPU, Vulkan, ROCm, ROCm Lemonade, and CUDA with per-backend version picker (13 platform-specific variants)
- **Speculative decoding** — MTP and other speculative decoding types via SpecTypePicker
- **YaRN RoPE** — extend context beyond training length with YaRN RoPE parameter tuning
- **Benchmark Tuning** — auto-tune model parameters for optimal performance (RuntimeOnly or Full modes)

### Dashboard & Networking
- **WebSocket Dashboard** — real-time metrics visualization in a web browser (TPS, VRAM, RAM, CPU, latency)
- **TLS support** — secure WebSocket dashboard with auto-generated self-signed certificates
- **API proxy** — expose an OpenAI-compatible API with CORS and SSE streaming support
- **API key authentication** — Bearer token authentication for the API proxy

### Interface
- **Log panel** — expandable/collapsible with following and manual scroll modes
- **README rendering** — full markdown renderer for HuggingFace model documentation
- **Model info** — GGUF metadata display with HuggingFace URL navigation
- **CmdLine overlay** — view the full llama-server command line (`Ctrl+K`), export to script (`e`)
- **Panel resize** — drag the border between left and right panels, or use `Shift+←/→`
- **Mouse support** — click panels to focus, scroll in logs, README, and settings
- **Local Model Filter** — quickly find models with `f`
- **About box** — application info and GPLv3 license link (`A`)
- **Dashboard URL modal** — copy dashboard URL to clipboard with `Ctrl+U`

## Prerequisites

- **Rust toolchain** — edition 2024
- **HuggingFace account** — required for downloading gated models
- **GPU (optional)** — NVIDIA (CUDA), AMD (ROCm/ROCm Lemonade), or Intel (Vulkan) for accelerated inference; CPU-only inference is fully supported

## Screenshot

![LLM Manager](main.png)

## Quick Start

```bash
git clone https://github.com/aginies/llmtui.git
cd llmtui
cargo build --release
cargo run
```

See the [Getting Started](getting-started.md) guide for detailed installation and usage instructions.
