---
name: debug-loading-progress
description: Debugging LLM model loading progress bar stalls and inaccuracies in the llmtui TUI
source: auto-skill
extracted_at: '2026-06-08T10:13:54.266Z'
---

## Debugging Loading Progress Bar Issues

When the loading progress bar stalls, goes backward, or never reaches 100%, follow this systematic approach:

### 1. Understand the phase-weight model

The progress bar uses weighted phases defined in `src/tui/app/types.rs:205`:

| Phase | Weight | Cumulative |
|---|---|---|
| ServerStarting | 8% | 8% |
| LoadingModel | 7% | 15% |
| LoadingMeta | 7% | 22% |
| LoadingTensors | 70% | 92% |
| ServerListening | 8% | 100% |

**Key insight:** A stall at ~22% means LoadingTensors is entered but within-phase fraction is 0. A stall at ~89% means tensor fraction is near 1.0 but ServerListening never triggers.

### 2. Check phase detection regexes

Phases are detected via regex in `src/tui/app/state/parsing.rs`. Each regex must match actual llama.cpp log output:

- **ServerStarting:** `llama.*server` or `ggml version`
- **LoadingModel:** `llama_model_loader` (old format) OR `load_model:\s*loading\s*model` (new format)
- **LoadingMeta:** `loaded N meta` or `meta data` (old format) OR `fitting params to device memory` (new format)
- **LoadingTensors:** `load tensors:` (old format only; newer versions have no dedicated start line)
- **ServerListening:** `server listening` / `http server listening` / `load_model:\s*initializing\s+slots`

**If a phase never appears:** compare the regex against actual llama.cpp log lines. Different llama.cpp versions may produce different log formats.

### 3. Detect llama.cpp log format version

Newer llama.cpp versions (post-~2025) changed their log format:

| Old format (pre-change) | New format (current) |
|---|---|
| `llama_model_loader: - arch: llama` | `srv load_model: loading model '/path/to/model.gguf'` |
| `llama_model_loader: loaded 423 meta data` | `common_init_result: fitting params to device memory ...` |
| `llama_model_loader: load tensors:` | *(no dedicated line — loading happens silently)* |
| `loading tensor 1 of 640, n_loaded 1` | *(no per-tensor output at all)* |
| `offloading 32 repeating layers to GPU` | `offloading 32 repeating layers to GPU` (still present) |
| `offloaded 16/32 layers to GPU` | `offloaded 16/32 layers to GPU` (still present) |

**Detection strategy:** If `llama_model_loader:` prefix is absent from logs, the new format is in use. Check for `srv load_model:` lines instead.

### 4. Handle missing tensor progress (new format)

When per-tensor output is absent, the progress bar needs a fallback:

- **Priority 1:** Use layer offloading data (`offloading N repeating layers` + `offloaded X/Y layers`) — this regex still works in new format.
- **Priority 2 (time-based fallback):** When LoadingTensors starts, record the timestamp. When ServerListening fires, interpolate 0→1 over the elapsed duration.

#### Inferred LoadingTensors phase (new format)

In newer llama.cpp versions, there is no `load tensors:` line — tensor loading starts immediately after meta loading. To handle this:

1. **Add a fallback in `detect_loading_phases()`** (`src/tui/app/state/state_impl.rs`): After checking all regexes, if `LoadingMeta` is in the phase set but `LoadingTensors` is not, auto-enter `LoadingTensors` and record `tensor_start_time`.

```rust
if self.loading.loading_phases.contains(&LoadingMeta)
    && !self.loading.loading_phases.contains(&LoadingTensors)
{
    self.loading.loading_phases.insert(LoadingTensors);
    self.loading.last_active_phase = Some(LoadingTensors);
    self.loading.tensor_start_time = Some(tokio::time::Instant::now());
}
```

2. **Add `tensor_start_time` field** to `LoadingState` in `src/tui/app/types/sub.rs`:
```rust
pub tensor_start_time: Option<tokio::time::Instant>,
```

3. **Add time-based interpolation in `compute_progress()`** (`src/tui/app/state/state_impl.rs`): When in LoadingTensors with no layer/tensor data, use elapsed time:
```rust
if tensor_fraction == 0.0 {
    if let Some(start_time) = self.loading.tensor_start_time {
        let elapsed = start_time.elapsed().as_secs_f32();
        tensor_fraction = (elapsed / 120.0).min(0.95);
    }
}
```

4. **Clear the field** in `reset_loading_state()` and `tick_server_exit()`.

### 5. Check detail parsing for the active phase

Once `LoadingTensors` is active, `parse_loading_details()` in `src/tui/app/state/state_impl.rs:69` tries to extract:

- **Tensor count:** regex `loading tensor\s+(\d+)\s+(?:of|out of)\s+(\d+)` → sets `tensors_loaded` (overwrite) and `tensors_total`
- **Layer count:** regex `offloading\s+(\d+)\s+repeating layers` → sets `layers_total`
- **Layer progress:** regex `offloaded\s+(\d+)\s*(?:out\s+of|/)\s*(\d+)\s*layers` → sets `layers_loaded` and `layers_total`
- **Buffer sizes:** regex for `MiB` lines → populates GPU buffer list

**Bug pattern:** If the tensor regex doesn't match, `tensors_loaded` stays at 0, and the within-phase fraction stays at 0, stalling progress at 22%.

### 6. Check within-phase fraction calculation

In `compute_progress()` at `src/tui/app/state/state_impl.rs:223`, the tensor phase fraction is computed as:

```
layer_fraction = layers_loaded / layers_total  (if both known)
tensor_fraction = tensors_loaded / estimated_total  (capped at 0.95)
tensor_fraction = max(layer_fraction, tensor_fraction)
```

**Bug pattern:** `tensors_loaded` is set via assignment (`=`), not accumulation. If the regex matches once but then stops matching, `tensors_loaded` freezes at that value.

### 7. Verify the fix

After any change:

1. Run `cargo test loading_parser_tests` — covers all regex patterns
2. Run `cargo test app_tests` — covers `compute_progress` calculations
3. Test with actual llama.cpp output (check `src/tui/app/state/parsing.rs` regexes against real logs)
4. Check edge cases: llama.cpp version differences, missing log lines, different GPU backends (Vulkan vs CUDA vs CPU)

### Common fixes

| Symptom | Likely cause | Fix location |
|---|---|---|
| Stalls at 22% | Tensor regex doesn't match (new format) | `parsing.rs` — add new format regexes |
| Stalls at 22% | Phase never entered (new format) | `parsing.rs` — update `LOADING_MODEL`/`LOADING_META`/`LOAD_TENSORS` |
| Stalls at 22% | No `load tensors:` line in new format | `state_impl.rs` — infer LoadingTensors after LoadingMeta |
| Progress goes backward | `tensors_loaded` overwritten instead of accumulated | `state_impl.rs:77` — use `max()` or accumulate |
| Stalls at ~89% | `ServerListening` regex doesn't match | `parsing.rs` — update `SERVER_LISTENING` regex |
| Tensor detail never shows | `tensors_total` unknown, estimated total wrong | `state_impl.rs:237` — improve fallback estimation |
| Dot-fallback fires after explicit count set | `|| Some(100)` in `is_dot_fallback` condition — should be `is_none()` only | `state_impl.rs` — remove `|| self.loading.load_progress.tensors_total == Some(100)` branch |
