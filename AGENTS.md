# AGENTS.md — llm-manager

## Project overview

**llm-manager** is a terminal UI (TUI) for managing local LLM models. It searches HuggingFace, downloads GGUF models, loads them via llama.cpp's `llama-server`, and lets you chat with them.

**Stack:** Rust 2024, ratatui 0.29, crossterm 0.28, tokio, reqwest.

## Directory structure

```
src/
├── main.rs          # Entry point, event loop, model discovery
├── config.rs        # Config loading/saving, YAML-based
├── models.rs        # Domain types (SearchResult, DownloadState, ModelSettings, etc.)
├── backend/
│   └── hub.rs       # HuggingFace API: search, list files, download
└── tui/
    ├── mod.rs       # Module declaration
    ├── app.rs       # App state (App struct, enums for modes/panels)
    ├── event.rs     # Keyboard event handler
    ├── render.rs    # Top-level render dispatcher
    ├── panel/
    │   ├── mod.rs
    │   ├── models.rs  # Left panel: model list / search results / download
    │   ├── info.rs    # GGUF metadata rendering for local models
    │   ├── tabbed.rs  # Right panel: Model Info / Settings tabs
    │   ├── settings.rs
    │   ├── log.rs
    │   └── help.rs
```

## Key architectural patterns

### App state machine (`src/tui/app.rs`)

`App` holds all state. `models_mode` is the mode enum that controls rendering:

```rust
pub enum ModelsMode {
    List,       // Local model list
    Search { query, results },
    Files { model_id, files, selected_idx, previous_query, previous_results, selected_result },
    Download { state },
}
```

### Log panel expand/collapse (`src/tui/app.rs`, `src/tui/event.rs`, `src/tui/render.rs`)

The `App` struct has a `log_expanded: bool` field. When true:
- Layout switches to 2-chunk: status bar + log fills remaining space
- Models panel, Settings panel, and active model info are hidden
- Log panel shows `[Enter] expand` / `[Esc] collapse` hint in status bar

Enter in the log panel expands it; Esc collapses it. Mouse handling in `handle_mouse()` uses the same layout logic to determine panel hit regions.

### Event handling (`src/tui/event.rs`)

Key handling is hierarchical:
1. Global shortcuts (Ctrl+C, Tab, Ctrl+H, etc.)
2. Search mode (takes priority when `ModelsMode::Search`)
3. Files mode
4. Download mode
5. Normal mode → dispatch to panel-specific handlers

**Important:** Each branch calls `return` to prevent fallthrough. Adding a new mode requires early returns.

### Rendering (`src/tui/render.rs`)

Top-level layout: status bar → top panels → active model → log. The models panel renders differently based on `models_mode`.

### Download cancellation (shared state)

Download runs in a spawned tokio task. Cancellation uses `Arc<AtomicBool>` shared between the task and the UI. Pressing `c` sets the flag; the download loop checks it each iteration.

### LLM Settings panel (22 fields, `src/tui/panel/settings.rs`, `src/tui/event.rs`)

The settings panel has 22 fields organized into 4 groups:

```
Loading (0-2):   Context length, System prompt preset, Keep in memory (mlock)
GPU (3-8):       GPU Layers, Flash Attention, KV Cache Offload, Cache Type K, Cache Type V, Active Experts
Evaluation (9-11): Eval Batch, Unified KV, Max Concurrent Predictions
Sampling (12-17): Seed, Temperature, Top-k, Top-p, Min P, Max Tokens
Repetition (18-21): Repetition Penalty, Rep. Last N, Presence Penalty, Frequency Penalty
```

Each group is rendered with a header line. Arrow keys adjust values; `+`/`-` for coarse, `Left`/`Right` for fine. Toggle fields (Flash Attention, Unified KV, Keep in memory) respond to `e`/`Ctrl+E`.

**Dirty tracking** (`is_settings_dirty` in `app.rs`) compares each field index-by-index. When a field is dirty, its label is rendered in yellow.

**Index consistency** — all indices must be identical across:
- `settings.rs` dirty check match arms (line ~133)
- `event.rs` `apply_numeric_setting` / `adjust_setting` match arms
- `event.rs` `handle_settings_key` toggle shortcuts (`e` / `Ctrl+E`)
- `event.rs` comment block (line ~836)
- `app.rs` `is_settings_dirty` match arms

## Coding rules

### Dependencies

- No new dependencies without asking. The project avoids external crates.
- If a crate is needed, prefer `ratatui` widgets over custom rendering.

### Error handling

- Use `anyhow::Result` for async/API functions.
- Use `thiserror` for application-specific error types.
- Log errors with `app.add_log()` in the TUI.

### Naming conventions

- `snake_case` for functions, variables, modules.
- `PascalCase` for types, enums, variants.
- Module names are lowercase (`backend`, `panel`).
- Public types get `pub` visibility; helpers stay private to their module.

### Async

- `handle_key` is async (for search queries).
- Download is spawned as a tokio task; progress flows through a `mpsc` channel.
- The main loop uses `crossterm::event::poll()` with a 100ms timeout.

### TUI specifics

- Use `ratatui` widgets when possible (Table, List, Paragraph, etc.).
- Style with `Style` / `Color` / `Modifier` — prefer semantic colors:
  - Yellow: headers, active elements
  - Cyan: navigation hints
  - Green: success/completed
  - Red: errors/failure
- Avoid hardcoding terminal dimensions; use `rect` and `area` from ratatui.

### Configuration

- Config is YAML-based, stored in `~/.config/llm-manager/`.
- New config fields go in `config.rs`; add defaults in `Default` impls.

### Testing

- No test framework yet. Add unit tests in `mod tests` blocks when writing new logic.
- Integration testing is manual (run the app).

## Common tasks

### Adding a new panel

1. Create `src/tui/panel/name.rs` with a `render(f, area, app)` function.
2. Add `mod name;` to `src/tui/panel/mod.rs`.
3. Add to `ActivePanel` enum in `app.rs`.
4. Dispatch in `render.rs` and `event.rs`.

### Adding a new keyboard shortcut

1. Add to `handle_key()` in `event.rs`.
2. Update the status bar in `render_status_bar()` in `render.rs`.
3. If it changes state, update `App` fields in `app.rs`.

### Adding a new API endpoint

1. Add the function in `src/backend/hub.rs`.
2. Call from `event.rs` (usually in the search/files branch).
3. Update `SearchResult` or other types in `models.rs` if needed.
