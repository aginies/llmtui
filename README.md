# llm-manager

[![CI](https://github.com/aginies/llmtui/actions/workflows/ci.yml/badge.svg)](https://github.com/aginies/llmtui/actions/workflows/ci.yml)
[![Docs](https://img.shields.io/badge/docs-GitHub%20Pages-blue)](https://aginies.github.io/llmtui/)
[![Version](https://img.shields.io/crates/v/llm-manager)](https://crates.io/crates/llm-manager)

A terminal UI (TUI) for managing local LLM models with HuggingFace search, download, and inference control.

> **NOTE:** This app is WIP (Work in Progress).

![Screenshot](documentation/src/images/main.png)

## Features

- Search & download GGUF models from HuggingFace
- Load/unload models via llama.cpp server with real-time metrics
- Chat with loaded models via OpenAI-compatible API proxy
- WebSocket Dashboard — real-time metrics in a web browser
- Benchmark Tuning — auto-tune model parameters for optimal performance
- Profiles & Presets — save and switch between named settings
- Multi-backend — CPU, Vulkan, ROCm, CUDA
- Web Search — SearXNG integration
- Multi-language UI — English, French, Italian, German

## Quick Start

```bash
git clone https://github.com/aginies/llmtui.git
cd llmtui
cargo build --release
cargo run
```

## Documentation

[Full documentation](https://aginies.github.io/llmtui/)

## License

GPLv3
