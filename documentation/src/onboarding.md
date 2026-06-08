# Onboarding Tour

On first launch, llm-manager displays an 8-step interactive guided tour covering the essential features. The tour can be skipped at any step with `Esc` or `q`.

## Tour Steps

### Step 0: Welcome

Introduction to llm-manager and key navigation shortcuts:
- `Tab` / `Shift+Tab` — switch panels
- `j` / `k` — navigate
- `F1`–`F6` — toggle panels

### Step 1: Models Panel

The Models panel shows all local GGUF models. `l` loads, `u` unloads, `Ctrl+D` deletes.

### Step 2: Search

Search mode lets you browse and download models from HuggingFace. Press `/` to search, `Enter` to select GGUF files.

### Step 3: Settings

The Settings panel has two tabs:
- **Server Settings** — host, backend, threads, mode
- **LLM Settings** — 46 fields including context, temperature, GPU layers, sampling params

### Step 4: GGUF Metadata

The Model Info panel shows parsed GGUF metadata: architecture, layers, context length, quantization, and parameters.

### Step 5: Load Progress

The loading progress bar tracks server start, tensor loading, and model readiness. Shows offloaded layers during loading.

### Step 6: Log Panel

The Log panel displays live llama.cpp server output with level-based coloring. Press `f` to toggle follow mode.

### Step 7: Completion

Tour complete. You can reopen it later by pressing `?` in the Help panel.

## Navigation

Within each step:
- `Enter` / `Right` / `n` — next step
- `Left` / `p` / `p` — previous step
- `Esc` / `q` — skip and close

A progress bar at the top shows your position in the tour. Key shortcuts are highlighted in yellow within each step's text.
