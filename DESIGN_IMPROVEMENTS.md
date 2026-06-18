# LLM Manager — Design & Graphical Improvements

> Generated from TUI codebase analysis. All file paths reference `llmtui` repo.

---

## Layout & Structure

### 1. Visual Resize Handle
- **Problem:** No indicator where left/right panel split is.
- **Location:** `src/tui/render.rs:96` — `left_pct.clamp(20, 80)` silently.
- **Fix:** Add `│` or `┃` character at panel boundary that changes color on drag.
- **Impact:** Medium effort, high usability gain.

### 2. Panel Minimization
- **Problem:** All panels fixed size. No way to collapse log or active model panel.
- **Fix:** Allow collapsing to 1-line status strip. Click to expand.
- **Impact:** Medium effort.

### 3. Tab Widget for Settings
- **Problem:** `src/tui/panel/tabbed.rs` manually renders server+llm tabs.
- **Fix:** Use ratatui `Tabs` widget for polished tab UI with underline indicators.
- **Impact:** Low effort, visual polish.

---

## Colors & Theming

### 4. Dark/Light Theme Toggle
- **Problem:** All colors hardcoded in `src/tui/colors.rs`. No theme support.
- **Fix:** Add `Theme` enum (Dark/Light) that remaps the `LazyLock` styles. Save preference to config.
- **Impact:** Medium effort, **biggest visual bang for least code**.

### 5. Accent Color Customization
- **Problem:** YELLOW primary accent everywhere in `colors.rs` (14 color constants, all fixed).
- **Fix:** Let users pick accent via config.
- **Impact:** Low effort if done right.

### 6. Status Bar Contrast
- **Problem:** `src/tui/render/status.rs` uses `SingleLine` with yellow text on default bg. Harder to read.
- **Fix:** Dark background strip for status bar.
- **Impact:** Low effort.

---

## Components & Polish

### 7. Ratatui Progress Widget
- **Problem:** `src/tui/panel/active.rs` uses manual `█░` progress bars.
- **Fix:** Replace with ratatui `ProgressState` for smoother rendering, proper sizing, configurable block chars.
- **Impact:** Low effort.

### 8. Error Toast
- **Problem:** `last_error_message` in UIState but no dedicated toast/badge. Errors only shown in log panel.
- **Fix:** Brief flash notification top-right on errors.
- **Impact:** Low effort, high UX value.

### 9. Keyboard Shortcut Badges
- **Problem:** Panel titles like `MODELS (F7)` are static text.
- **Fix:** Add visual shortcut key badges (rounded rect with key label) like modern IDEs. Some overlays already do this.
- **Impact:** Medium effort.

### 10. Animations & Transitions
- **Problem:** Only spinner `⠋⠙⠹⠸` exists. No transitions.
- **Fix:** Panel focus transitions (border color fade), dialog fade-in/out, status bar pulse on state change. Manual frame counter or ratatui `StateTransition`.
- **Impact:** Medium effort.

---

## Readability

### 11. Hint Bar Density
- **Problem:** `src/tui/render/hints.rs` packs 8+ key hints per line.
- **Fix:** Wrap to 2 lines when too many. Show only actionable hints (hide nav hints when not needed).
- **Impact:** Low effort.

### 12. Table Row Hover
- **Problem:** `src/tui/panel/models.rs` — selected row gets BLACK-on-YELLOW bold. No hover state.
- **Fix:** Add subtle bg highlight (not full invert) for hovered-but-not-selected rows via mouse move tracking.
- **Impact:** Medium effort.

### 13. Quality Dots Tooltip
- **Problem:** `🟢🟡🟠🔴⚫` emoji dots are font-dependent, no explanation.
- **Fix:** Tooltip on hover showing what quality score means.
- **Impact:** Medium effort.

---

## Code Organization

### 14. Split overlays.rs
- **Problem:** `src/tui/render/overlays.rs` ~1600 lines.
- **Fix:** Split into per-overlay modules like `event/` is organized. Each overlay function → separate file.
- **Impact:** Low effort, refactoring.

### 15. Duplicate title_style
- **Problem:** `src/tui/panel/models.rs:44-45` calls `.title_style()` twice.
- **Fix:** Remove duplicate. Minor but signals inconsistency.
- **Impact:** Trivial.

---

## Quick Wins (Least Effort, Most Impact)

| Rank | Improvement | Effort | Impact |
|------|-------------|--------|--------|
| 1 | Dark/light theme (#4) | Medium | **High** |
| 2 | Resize handle indicator (#1) | Low | **High** |
| 3 | Error toast (#8) | Low | **High** |
| 4 | Progress widget (#7) | Low | **Medium** |
| 5 | Hint bar wrapping (#11) | Low | **Medium** |

---

## Current Color Reference

| Constant | Value | Usage |
|----------|-------|-------|
| `YELLOW` | `Color::Yellow` | Primary accent, labels, titles |
| `GREEN` | `Color::Green` | Success, focus borders |
| `LIGHT_GREEN` | `Color::Rgb(29,168,29)` | Focused panel borders |
| `RED` | `Color::Red` | Errors, danger, dirty state |
| `WHITE` | `Color::White` | Default body text |
| `CYAN` | `Color::Cyan` | Links, URLs, metrics |
| `MAGENTA` | `Color::Magenta` | Sort labels, MTP, MoE |
| `DIM_GRAY` | `Color::Gray` | Secondary text, timestamps |
| `SELECTED_ROW` | WHITE on YELLOW, BOLD | Selected table row |
| `TITLE` | YELLOW, BOLD | Panel titles |
| `BORDER_FOCUSED` | LIGHT_GREEN | Focused panel borders |
| `BORDER_UNFOCUSED` | MID_GRAY | Unfocused panel borders |

## Current Layout

```
+---------------------------------------------------+
| Status Bar (1 line)                               |
+---------------------------------------------------+
|                                                   |
|   Top Area                                      |
|  +------------------+  +-----------------------+  |
|  | Left Panel       |  | Right Panel           |  |
|  | (models/info)    |  | Settings              |  |
|  +------------------+  +-----------------------+  |
+---------------------------------------------------+
| Active Model (6 lines, or 0 if hidden)            |
+---------------------------------------------------+
| Log/Downloads                                     |
+---------------------------------------------------+
```

Left panel split controlled by `left_pct` (default 55, range 20-80). Resizable via Shift+Left/Right or mouse drag.

## Files of Interest

| File | Lines | Notes |
|------|-------|-------|
| `src/tui/colors.rs` | 145 | All color definitions |
| `src/tui/render.rs` | ~300 | Main layout rendering |
| `src/tui/render/status.rs` | 225 | Status bar |
| `src/tui/render/hints.rs` | 249 | Key hint bar |
| `src/tui/render/overlays.rs` | ~1600 | All popup dialogs |
| `src/tui/render/onboarding.rs` | 172 | Onboarding wizard |
| `src/tui/panel/models.rs` | 1143 | Main models panel |
| `src/tui/panel/active.rs` | 534 | Active model metrics |
| `src/tui/panel/tabbed.rs` | 905 | Settings tabs |
| `src/tui/app/types/sub.rs` | 217 | UI state structs |
