# Code Duplication Report — llm-manager

Generated: 2026-06-01 | Updated: 2026-06-01

---

## CRITICAL

### 1. ModelOverride / ModelSettings / DefaultParams tripling

**Severity:** CRITICAL | **Status:** 🔍 Analyzed — partially addressed (#14 done, macro generation still pending)

**Files:**
- `src/config.rs` — `ModelOverride` (70 `Option<T>` fields)
- `src/config.rs` — `DefaultParams` (60 concrete fields + serde defaults)
- `src/models.rs` — `ModelSettings` (65 concrete fields)

**Root cause:** Each struct serves a different purpose:
- `DefaultParams`: YAML-serialized config with serde `#[default]` attrs
- `ModelSettings`: runtime in-memory settings (concrete types, no Options except truly optional)
- `ModelOverride`: partial override (all `Option<T>`, merged with `unwrap_or`)

Adding a new field requires touching 6 locations (3 structs + 3 mapping functions). A single omission silently drops the field from config serialization, override application, or default loading.

**What's been done:** #14 (`ModelOverride::apply()` macros) was completed in commit `56b05fa`, reducing the `apply()` method from 126 lines to ~45 using `apply_scalar!`, `apply_clone!`, and `apply_option!` macros.

**Remaining — Proposed fix (phased approach):**

Phase 1 — Low-risk: Create a compile-time audit macro that verifies field count parity:

```rust
// Add to end of each struct's impl block — fails to compile if counts differ
const _: () = {
    assert!(
        std::mem::size_of::<ModelSettings>() == std::mem::size_of::<DefaultParams>(),
        "ModelSettings and DefaultParams must have identical fields"
    );
};
```

Phase 2 — High-risk (future): Full macro-generated structs with a single source of truth:

```rust
// src/models/fields.rs
declare_model_fields! {
    context_length: u32,
    threads: u32,
    threads_batch: u32,
    // ... all 65+ fields — generates struct bodies + conversion impls
}
```

---

## HIGH

### 2. cache_type_k / cache_type_v handler duplication

**Severity:** HIGH | **Status:** ✅ Done

**File:** `src/tui/event/panel/settings.rs`

**Duplication:** Two blocks of ~41 lines each, identical logic differing only in field name and type alias.

**Fix applied:** Both handlers reduced to ~5 lines each. Buffer entry, arrow cycling, and digit input now handled by the generic bottom match via `f.apply_edit()` and `f.adjust()`. Field-specific handlers only intercept Enter with empty buffer to cycle.

**Lines saved:** ~82 → ~18 (64 lines removed)

---

### 5. `settings_edit_buffer.is_empty()` guard repeated

**Severity:** HIGH | **Status:** ✅ Done

**File:** `src/tui/event/panel/settings.rs`

**Occurrences eliminated:** 9 field handlers (5 toggles + 4 modals)

**Fix applied:** Removed redundant guard pattern (`if !buffer.is_empty() { buffer.clear(); } else if key == Enter`). Combined into single condition: `if field_id == Some("X") && key.code == KeyCode::Enter && buffer.is_empty()`. Buffer clearing on non-Enter keys delegated to generic handler.

**Also removed:** Entire `expert_count` handler (redundant — generic handler handles it via `f.adjust()` + `f.apply_edit()`) and simplified `max_concurrent_predictions` handler (only Enter-with-empty-buffer to open picker remains; Left/Right + `sync_global_settings()` already handled by generic match).

**Lines saved:** ~108 net lines removed from settings.rs (518 → 410 lines)

---

## MEDIUM

### 10. F-key panel visibility toggling

**Severity:** MEDIUM | **Status:** 🔍 Analyzed — still open

**File:** `src/tui/event/key.rs`

**Seven F-key handlers (same pattern × 7) plus Ctrl+F variants**

**Proposed fix — parameterized helper:**

```rust
fn handle_fkey_toggle(
    app: &mut App,
    panel_idx: u32,
    target_panel: Option<ActivePanel>,
    require_no_server: bool,
) {
    if require_no_server && app.server.server_handle.is_some() {
        return;
    }
    app.toggle_panel_visibility(panel_idx);
    if app.is_panel_visible(panel_idx) {
        if let Some(panel) = target_panel {
            app.ui.active_panel = panel;
        }
    }
}
```

Usage:
```rust
KeyCode::F(1) => { app.ui.active_panel = ActivePanel::Models; }
KeyCode::F(2) => handle_fkey_toggle(app, 1, Some(ActivePanel::ServerSettings), true);
KeyCode::F(3) => handle_fkey_toggle(app, 2, Some(ActivePanel::ModelInfo), false);
KeyCode::F(4) => handle_fkey_toggle(app, 3, Some(ActivePanel::LlmSettings), false);
KeyCode::F(5) => handle_fkey_toggle(app, 4, None, false);
KeyCode::F(6) => handle_fkey_toggle(app, 5, Some(ActivePanel::Log), false);
```

**Lines saved:** ~80 → ~25 (55 lines removed)

---

### 11. Scrollbar rendering copy-pasted

**Severity:** MEDIUM | **Status:** ✅ Done (`d91f180`)

**File:** `src/tui/render.rs`

**Profiles panel and SystemPromptPresets panel** — identical ~18-line blocks, only variable names differ.

**Fix applied:** `render_scrollbar()` helper extracted to `src/tui/render.rs`.

**Lines saved:** ~36 → ~18 (22 lines removed)

---

### 12. Text editing logic duplicated

**Severity:** MEDIUM | **Status:** ✅ Done (`d91f180`)

**File:** `src/tui/event/key.rs`

**Three overlays with nearly identical text editing logic:**
- DashboardPicker (~42 lines of edit logic)
- YarnRoPESettings (~18 lines)
- PromptPicker (~27 lines)

**Fix applied:** `TextEditor` struct extracted to `src/tui/event/helpers.rs` with `insert_char()`, `backspace()`, `move_left()`, `move_right()`, `home()`, `end()` methods.

**Note (fixed):** DashboardPicker had a bug — `*edit_cursor_pos += c.len_utf8()` should be `*edit_cursor_pos += 1` since cursor tracks character positions, not bytes. The TextEditor fixes this. Additionally fixed the same bug in YarnRoPESettings and BenchTuneSetup param editing.

**Lines saved:** ~86 → ~40 (46 lines removed, plus bug fix)

---

### 13. profile_settings_parts() manual field list

**Severity:** MEDIUM | **Status:** 🔍 Analyzed — still open

**File:** `src/tui/settings.rs`

**Pattern (22 fields manually compared, 70+ total):**
```rust
if let Some(v) = s.context_length {
    if Some(v) != Some(current.context_length) {
        parts.push(format!("ctx={}", v));
    }
}
// ... repeated for each field
```

**Missing fields (not compared):**
`threads_batch`, `batch_size`, `ubatch_size`, `parallel`, `max_concurrent_predictions`, `cache_type_k`, `cache_type_v`, `keep`, `swa_full`, `numa`, `split_mode`, `tensor_split`, `main_gpu`, `fit`, `expert_count`, `seed`, `mirostat`, `mirostat_lr`, `mirostat_ent`, `ignore_eos`, `samplers`, `dry_*`, `rope_*`, `cache_reuse`, `spec_type`, `draft_tokens`, and more.

**Proposed fix — macro for field comparisons:**

```rust
macro_rules! diff_field {
    ($s:expr, $current:expr, $field:ident, @int $label:expr) => { /* ... */ };
    ($s:expr, $current:expr, $field:ident, @float $label:expr) => { /* ... */ };
    ($s:expr, $current:expr, $field:ident, @bool $label:expr) => { /* ... */ };
    ($s:expr, $current:expr, $field:ident, @string $label:expr) => { /* ... */ };
}
```

**Lines saved:** ~127 → ~35 (92 lines removed)

---

## DONE

The following items have been resolved:

| # | Issue | Commit | Lines Saved |
|---|-------|--------|-------------|
| 3+4 | `settings_render_cache = None` (48x) + `update_vram_estimate()` (23x) | `cc4c8b1` | -69 (3 files, net) |
| 6 | 6 field constructor functions | `130cd6f` | -141 (settings.rs) |
| 7 | Benchmark iteration accumulation | `130cd6f` | -85 (benchmark.rs) |
| 8 | Picker navigation patterns (4×) | `81473e1` | -21 (key.rs) |
| 9 | `build_server_cmd` / `build_bench_cmd` shared logic + bugfix | `81473e1` | -23 (server.rs) |
| 11 | Scrollbar rendering copy-paste (2×) | `d91f180` | -22 (render.rs) |
| 12 | Text editing logic duplicated (4 overlays) + cursor bugfix | `d91f180` | -46 (key.rs) |
| 14 | `ModelOverride::apply()` repeated pattern | `56b05fa` | -28 (config.rs) |
| 2 | cache_type_k/v handler duplication | TBD | -64 (settings.rs) |
| 5 | settings_edit_buffer guard (9 handlers) + expert_count/redundant max_concurrent | TBD | -108 (settings.rs) |

**Details:**
- **#3+4** → `mark_settings_dirty(app, recalc_vram)` helper in `src/tui/event/helpers.rs`
- **#6** → `make_field_fn!` macro generating 6 field constructors from one definition
- **#7** → `run_iteration_loop()` shared function in `src/backend/benchmark.rs`
- **#8** → `picker_nav_up()` / `picker_nav_down()` helpers in `src/tui/event/key.rs`
- **#9** → `push_gpu_layers()` / `push_spec_decoding()` helpers in `src/backend/server.rs`; also fixed bug where `Specific`/`All` were separate `if` blocks
- **#11** → `render_scrollbar()` helper in `src/tui/render.rs`
- **#12** → `TextEditor` struct in `src/tui/event/helpers.rs`; fixed cursor position bugs (`c.len_utf8()` → `1`) in DashboardPicker, YarnRoPESettings, and BenchTuneSetup param editing
- **#14** → `apply_scalar!`, `apply_clone!`, `apply_option!` macros in `src/config.rs`
- **#2** → cache_type_k/v handlers reduced to Enter-with-empty-buffer cycle; buffer/arrow/number entry delegated to generic `f.apply_edit()` / `f.adjust()` handlers
- **#5** → guard pattern removed from 9 field handlers (5 toggles + 4 modals); expert_count handler fully removed (redundant); max_concurrent_predictions simplified to picker-only

---

## NO ACTION NEEDED

| # | Issue | Reason |
|---|-------|--------|
| 15 | Small enum Display implementations | LOW severity, well-contained, not worth macro overhead |
| 16 | SearchSort::next() / SearchSort::label() mirror matches | Different operations (cycle vs. label), not genuine duplication |

---

## Summary

### Remaining open items

| # | Severity | Issue | Est. Lines Saved |
|---|----------|-------|-----------------|
| 1 | CRITICAL | ModelOverride/ModelSettings/DefaultParams tripling | N/A (preventive, phased) |
| 2 | HIGH | cache_type_k/v handler duplication | ~42 |
| 5 | HIGH | settings_edit_buffer guard (21×) | ~63 |
| 10 | MEDIUM | F-key panel visibility toggling | ~55 |
| 13 | MEDIUM | profile_settings_parts() manual field list | ~92 |

**Remaining estimated lines saved: ~152 lines**

### Priority order for remaining fixes:

1. **#5**: Edit buffer guard — simple refactor, eliminates 63 lines
2. **#2**: Cache type handler — 42 lines saved, straightforward generic
3. **#13**: diff_field macro — 92 lines saved, prevents missing fields
4. **#10**: F-key helper — 55 lines saved
5. **#1** (Phase 1): Compile-time audit — zero lines but prevents silent bugs
