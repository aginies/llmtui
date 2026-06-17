# llm-manager

[![CI](https://github.com/aginies/llmtui/actions/workflows/ci.yml/badge.svg)](https://github.com/aginies/llmtui/actions/workflows/ci.yml)
[![Docs](https://img.shields.io/badge/docs-GitHub%20Pages-blue)](https://aginies.github.io/llmtui/)
[![Version](https://img.shields.io/crates/v/llm-manager)](https://crates.io/crates/llm-manager)

A terminal UI (TUI) for managing local LLM models with HuggingFace search, download, and inference control.

> **NOTE:** This app is WIP (Work in Progress).

![Screenshot](documentation/src/images/main.png)

## Features

- **Search & download** GGUF models from HuggingFace
- **Load/unload** models via llama.cpp server
- **Chat** with loaded models via OpenAI-compatible API
- **Configure** loading and inference parameters per model
- **WebSocket Dashboard** — real-time metrics in a web browser
- **API Proxy** — OpenAI-compatible endpoints with CORS and SSE streaming
- **Benchmark Tuning** — auto-tune model parameters for optimal performance
- **Profiles & Presets** — save and switch between named settings
- **Multi-backend** — CPU, Vulkan, ROCm, ROCm Lemonade, CUDA
- **Web Search** — automatic SearXNG integration for research queries
- **GNOME Extension** — real-time metrics in the top panel
- **Multi-language UI** — English, French, Italian

## Prerequisites

- Rust toolchain (edition 2024)
- GPU (optional): NVIDIA (CUDA), AMD (ROCm/ROCm Lemonade), or Intel (Vulkan)

## Quick Start

```bash
git clone https://github.com/aginies/llmtui.git
cd llmtui
cargo build --release
cargo run
```

## Documentation

- [Getting Started](documentation/src/getting-started.md) — full walkthrough
- [Usage](documentation/src/usage.md) — TUI, serve mode, keyboard shortcuts
- [Configuration](documentation/src/config.md) — config file, profiles, backends
- [API Endpoint](documentation/src/api-endpoint.md) — API proxy, TLS, authentication
- [opencode](documentation/src/opencode.md) — connect opencode to llm-manager

## Build Script

```bash
./build.sh build      # Build (debug)
./build.sh run        # Build and run (TUI mode)
./build.sh serve      # Serve a model
./build.sh release    # Release build
./build.sh test       # Run tests
./build.sh doc        # Build documentation
./build.sh help       # Show all options
```

## License

GPLv3
