# Introduction

**LLM Manager** is a terminal UI (TUI) for managing local LLM models. It lets you search HuggingFace, download GGUF models, load them via llama.cpp's `llama-server`, and chat with them — all from your terminal.

## Features

- **Model search** on HuggingFace (filters to GGUF models, paginated with infinite scroll)
- **Download** GGUF model files with progress tracking and cancellation
- **Load/unload** models via llama.cpp server with progress visualization
- **Chat** with loaded models in the terminal
- **Configure** loading and inference parameters per model
- **GGUF file browser** — list and select specific GGUF files for a model
- **Log panel** — expand/collapse with Enter/Esc
- **CmdLine overlay** — view the full llama-server command line (`Ctrl+K`)
- **API proxy** — expose an OpenAI-compatible API on a configurable port
- **Profiles** — save and apply named presets of settings
- **Multi-backend** — CPU, Vulkan, and ROCm (AMD) support with version picker

## Prerequisites

- Rust toolchain (edition 2024)
- A HuggingFace account (for downloading gated models)
- An NVIDIA GPU (Vulkan) or AMD GPU (ROCm) for GPU inference, or a CPU for CPU-only inference

## Quick Start

```bash
git clone https://github.com/aginies/llmtui.git
cd llmtui
cargo build --release
cargo run
```
