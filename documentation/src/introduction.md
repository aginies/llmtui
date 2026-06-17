# Introduction

**LLM Manager** is a terminal UI (TUI) for managing local LLM models. It lets you search HuggingFace, download GGUF models, load them via llama.cpp's `llama-server`, and chat with them — all from your terminal.

## Features

### Model Discovery & Downloads
- **HuggingFace search** — GGUF-filtered, paginated results with multiple sort options
- **GGUF file browser** — browse and select specific GGUF files for each model
- **Download manager** — progress tracking with speed, ETA, and cancellation support
- **Multi-word search** — space-separated words use AND logic for precise filtering

### Inference
- **Model loading** — progress visualization through server start, weight loading, metadata, tensor loading, and server listening phases
- **Chat with models** — interact with loaded models via the API proxy
- **RPC Workers** — manage distributed inference nodes from a dedicated window
- **Router Mode** — load multiple models simultaneously *(Work in Progress)*

### Configuration
- **Per-model settings** — full control over context length, GPU layers, sampling parameters, and more
- **Profiles** — save and quickly switch between named presets of settings
- **System Prompt Presets** — named system prompts for different use cases
- **Multi-backend support** — CPU, Vulkan, ROCm, ROCm Lemonade, and CUDA (13 platform-specific variants)
- **Speculative decoding** — MTP and other speculative decoding types
- **YaRN RoPE** — extend context beyond training length
- **Benchmark Tuning** — auto-tune model parameters for optimal performance

### Dashboard & Networking
- **WebSocket Dashboard** — real-time metrics visualization in a web browser
- **TLS support** — secure connections with auto-generated self-signed certificates
- **API proxy** — expose an OpenAI-compatible API with CORS and SSE streaming
- **Web Search** — automatically search the web via SearXNG when messages contain comparison/research keywords

### Interface
- **Log panel** — expandable/collapsible with following and manual scroll modes
- **README rendering** — full markdown renderer for HuggingFace model documentation
- **Model info** — GGUF metadata display with HuggingFace URL navigation
- **CmdLine overlay** — view the full llama-server command line (`Ctrl+K`)
- **Panel resize** — drag the border between left and right panels, or use `Shift+←/→`
- **Mouse support** — click panels to focus, scroll in logs, README, and settings
- **Multi-language UI** — switch between English, French, and Italian with `Ctrl+L`

## Prerequisites

- **Rust toolchain** — edition 2024
- **HuggingFace account** — required for downloading gated models
- **GPU (optional)** — NVIDIA (CUDA), AMD (ROCm/ROCm Lemonade), or Intel (Vulkan) for accelerated inference; CPU-only inference is fully supported

## Screenshot

![LLM Manager](images/main.png)

## Quick Start

```bash
git clone https://github.com/aginies/llmtui.git
cd llmtui
cargo build --release
cargo run
```

See the [Getting Started](getting-started.md) guide for a full walkthrough.
