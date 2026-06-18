# UI Design Review — llm-manager

## Current Design Identity

Dark terminal theme. 13 named colors. Heavy yellow as primary accent. Green for focus. Classic TUI aesthetic with double/plain borders. Table-based layouts. Emoji quality dots. Marquee scrolling for long text.

---

## Strengths

- Clean color hierarchy (yellow labels, white values, gray secondary)
- Quality indicator dots (colored circles) survive row highlights
- Marquee text scrolling for overflow
- UNSAVED watermark is clever
- Toast notifications with level-based borders
- Hint system adapts to context
- Markdown renderer handles README well

---

## Weaknesses

### 1. Yellow overload
Yellow is used for panel titles, labels, hints, status, sort labels, section headers, badges, prompt brackets, TPS values, and more. Too many roles = low signal-to-noise ratio.

### 2. Border inconsistency
Some panels use `BorderType::Double`, others `BorderType::Plain`, others `BorderType::Rounded`. No unified border strategy. Focused/unfocused border switching adds cognitive load.

### 3. Status bar is text-dense
Single line packs mode name, server port, UNSAVED, benchmark progress, global mode indicators, sort labels, and key hints. At wide terminals, hints get pushed hard right. At narrow terminals, critical info gets truncated.

### 4. Selection style is harsh
Black text on bright yellow background. Works but visually aggressive. No subtle focus indicator for unfocused state.

### 5. Panel spacing is zero
Panels touch each other with no gap. Hard to visually separate sections.

### 6. Active model panel is cramped
6-line height for model name, TPS, latency, prompt TPS, context, CPU, RAM, VRAM, and progress bars. Information density is high but cramped.

### 7. Download panel has no visual progress
Progress shown as percentage text only. No visual bar in the table cell.

### 8. Log panel has no syntax highlighting
Log levels get colors, but the message content is plain white. No keyword highlighting, no stack trace formatting.

### 9. Settings panel lacks visual grouping
Section headers use `--- Section ---` format. No visual separator between sections beyond the text line. Hard to scan.

### 10. Dialog/overlay styling is inconsistent
Some use double borders, some single. No unified dialog aesthetic.

### 11. Color palette is limited for data visualization
Only 13 colors. Hard to create distinct visual encodings for metrics, charts, or multi-state data.

### 12. No theme support
All colors hardcoded. No light theme, no high-contrast, no custom themes.

---

## Proposed Improvements

### Priority 1: Visual Hierarchy

#### 1A. Reduce yellow usage by 60%
- Reserve yellow for primary actions and selected items only
- Use cyan for labels (already used for metrics values)
- Use gray for secondary text
- Use white for primary text/values
- Use green for success states only
- Creates clearer visual distinction between label, value, and action

#### 1B. Add panel gaps
- Add 1-cell gap between panels
- Use `Layout::margin(1)` or manual inset
- Creates visual breathing room

#### 1C. Standardize borders
- All focused panels: `BorderType::Rounded` with green border
- All unfocused panels: `BorderType::Plain` with dim gray border
- Remove the Double/Plain/Rounded mixing

### Priority 2: Data Visualization

#### 2A. Visual progress bars in download panel
- Replace percentage text with ASCII progress bar in progress column
- Format: `[████░░░░░░] 40%`
- Use green for progress, gray for empty

#### 2B. Sparkline for metrics
- Add mini sparkline in active model panel for TPS history
- Shows trend not just current value
- Uses block characters: `▁▂▃▅▆█`

#### 2C. VRAM usage bar
- Visual bar in active model panel title showing VRAM usage ratio
- Green → yellow → red gradient based on utilization

### Priority 3: Settings Panel

#### 3A. Visual section dividers
- Replace `--- Section ---` with actual line drawing: `┌──────── Section ────────┐`
- Or use colored left border on section header
- Adds visual structure

#### 3B. Settings field grouping
- Add subtle background shading for related field groups
- Use light gray background for expert fields

### Priority 4: Status Bar

#### 4A. Split into two tiers
- Top line: mode + server status (always visible)
- Bottom line: dynamic hints (context-sensitive)
- Or: use status bar more efficiently with collapsible sections

#### 4B. Smart truncation
- Truncate server port display when terminal is narrow
- Show "..." with hover to see full info

### Priority 5: Theme Support

#### 5A. Configurable color scheme
- Add `~/.config/llm-manager/theme.yaml`
- Allow overriding each color constant
- Ship with 2-3 built-in themes (dark, light, high-contrast)

#### 5B. Background color config
- Allow customizing panel background (not just black)
- Some users prefer dark blue, dark gray, etc.

### Priority 6: Polish

#### 6A. Log keyword highlighting
- Highlight keywords like "error", "warning", "failed", "loaded" in log messages
- Color matched to log level

#### 6B. Table hover state
- Subtle highlight on row hover (mouse) even when not selected
- Helps with large model lists

#### 6C. Smooth transitions
- Animate panel appearance/disappearance
- Fade toast in/out
- Smooth scrollbar movement

#### 6D. Empty state illustrations
- ASCII art for empty states (no models, no downloads)
- Makes empty screens feel intentional not broken

---

## Recommended Implementation Order

1. **1A** — Reduce yellow usage (highest impact, low effort)
2. **2A** — Download progress bars (visual clarity improvement)
3. **3A** — Section dividers in settings (scanability)
4. **5A** — Theme support (configurability)
5. **1B** — Panel gaps (polish)
6. **1C** — Border standardization (consistency)
7. **2B** — TPS sparkline (data viz)
8. **4A** — Status bar restructure (UX)
9. **6A-6D** — Polish items
