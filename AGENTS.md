# AGENTS.md — llm-manager

## Project overview

**llm-manager** is a terminal UI (TUI) for managing local LLM models. It searches HuggingFace, downloads GGUF models, loads them via llama.cpp's `llama-server`, and lets you chat with them.

**Stack:** Rust 2024, ratatui 0.29, crossterm 0.28, tokio, reqwest, axum.

## Directory structure

```
src/
├── main.rs          # Entry point, event loop, model discovery, metrics polling
├── config.rs        # Config loading/saving, YAML-based, profiles, presets
├── models.rs        # Domain types (SearchResult, DownloadState, ModelSettings, etc.)
├── serve.rs         # Standalone serve mode CLI
├── serve_api.rs     # Axum-based API proxy server
├── backend/         # HuggingFace API, server spawning, benchmark, hardware, TLS, WS
├── tui/             # App state, event handling, rendering, panels
└── config/          # Per-model config, profiles, presets stores
```

## Key patterns

- `App` holds all state; `ActivePanel` controls focus, `ModelsMode` controls rendering
- Key handling is hierarchical — each branch calls `return` to prevent fallthrough
- Config is YAML-based in `~/.config/llm-manager/`, indexed by field
- Download runs in a tokio task; cancellation via `Arc<AtomicBool>`

## Rules

### Planning
1. Identify root cause, not just symptoms
2. List affected files and functions
3. **Use a `todowrite` tool to track work as a numbered TODO list**
4. Mark each item as `in_progress` before starting, `completed` when done
5. Keep the TODO list visible

### Always ask before deciding
- **Ask the user questions before making any design or implementation decision**
- Do not assume — clarify tradeoffs and options
- Present a plan and get approval before making changes

### Coding
- No new dependencies without asking
- Prefer `ratatui` widgets over custom rendering
- Use `anyhow::Result` for async/API functions, `thiserror` for app-specific errors
- `snake_case` for functions/variables, `PascalCase` for types/enums
- Log errors with `app.add_log()` in the TUI

### i18n / Translation
- **All user-facing strings MUST go through the i18n system.** No hardcoded English text in source code.
- Use `t!("key")` for simple strings, `t_fmt!("key", args...)` for strings with placeholders.
- UI strings live in `locales/<lang>.json` (en, fr, it). Add keys to ALL locale files, not just `en.json`.
- If a key does not exist in the current language, it falls back to English, then to the key itself.
- String key naming: dot-separated hierarchical keys matching the UI context (e.g. `dialog.exit.title`, `field.help.context`, `hints.nav`).
- Technical/internal strings (error messages for logs, debug output) may remain in code. User-facing strings (panel titles, button labels, help text, tooltips, dialog messages, hints) MUST use `t!()`.
- When adding a new UI string, add the key to `en.json`, `fr.json`, and `it.json` simultaneously.

### Git
- **Never commit changes yourself.** Always ask the user before committing.
- If the user explicitly asks you to commit, then do it.
