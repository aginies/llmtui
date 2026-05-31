# Plan: SettingField Refactoring for LLM Settings

## Problem

The LLM Settings panel uses hardcoded integer indices (0-54) scattered across 4+ files:
- `render_settings()` — passes indices to `add_setting()`
- `add_setting()` — 55-arm match for dirty checking
- `adjust_setting()` — 55-arm match for arrow key adjustment
- `apply_numeric_setting()` — 20+ arm match for Enter-edit
- `handle_settings_key` Ctrl+E — 7-arm match for field toggles

Adding a new field requires updating all of these, and indices must stay consistent. This has already caused bugs (e.g., the Yarn RoPE backspace fix, the expert mode fields).

## Solution

Replace the index-based approach with a data-driven `SettingField` struct. Each field carries its own logic as closures. Adding a field is just adding one entry to a `Vec<SettingField>`.

## New File: `src/tui/settings.rs`

### Type Definition

```rust
use crate::models::ModelSettings;

pub enum SettingField {
    Section(String),
    Field {
        id: &'static str,
        name: &'static str,
        display: Box<dyn Fn(&ModelSettings) -> String>,
        dirty: Box<dyn Fn(&ModelSettings, &ModelSettings) -> bool>,
        adjust: Box<dyn Fn(&mut ModelSettings, i32, u32, u32)>,
        apply_edit: Box<dyn Fn(&mut ModelSettings, &str)>,
        ctrl_e_toggle: Option<Box<dyn Fn(&mut ModelSettings)>>,
        editable: bool,
    },
}
```

### Methods

```rust
impl SettingField {
    pub fn is_section(&self) -> bool {
        matches!(self, Self::Section(_))
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Section(name) | Self::Field { name, .. } => name,
        }
    }

    pub fn display(&self, settings: &ModelSettings) -> String {
        match self {
            Self::Field { display, .. } => display(settings),
            _ => String::new(),
        }
    }

    pub fn is_dirty(&self, settings: &ModelSettings, cached: &ModelSettings) -> bool {
        match self {
            Self::Field { dirty, .. } => dirty(settings, cached),
            _ => false,
        }
    }

    pub fn adjust(&self, settings: &mut ModelSettings, delta: i32, max_threads: u32, max_context: u32) {
        if let Self::Field { adjust, .. } = self {
            adjust(settings, delta, max_threads, max_context);
        }
    }

    pub fn apply_edit(&self, settings: &mut ModelSettings, buf: &str) {
        if let Self::Field { apply_edit, .. } = self {
            apply_edit(settings, buf);
        }
    }

    pub fn ctrl_e_toggle(&self, settings: &mut ModelSettings) {
        if let Self::Field { ctrl_e_toggle: Some(toggle), .. } = self {
            toggle(settings);
        }
    }
}
```

### Field Definition Functions

```rust
pub fn standard_fields() -> Vec<SettingField> {
    vec![
        SettingField::Section("Loading".to_string()),
        SettingField::Field {
            id: "system_prompt_preset_name",
            name: "Prompt",
            display: Box::new(|s| s.system_prompt_preset_name.clone()),
            dirty: Box::new(|s, c| s.system_prompt_preset_name != c.system_prompt_preset_name),
            adjust: Box::new(|s, _, _, _| {}), // not adjustable via arrow keys
            apply_edit: Box::new(|s, buf| { s.system_prompt_preset_name = buf.to_string(); }),
            ctrl_e_toggle: None,
            editable: true,
        },
        SettingField::Field {
            id: "context_length",
            name: "Context",
            display: Box::new(|s| s.context_length.to_string()),
            dirty: Box::new(|s, c| s.context_length != c.context_length),
            adjust: Box::new(|s, delta, _, max_ctx| {
                let mut val = (s.context_length as i32 + delta * 128).max(128) as u32;
                if max_ctx > 0 { val = val.min(max_ctx); }
                s.context_length = val;
            }),
            apply_edit: Box::new(|s, buf| {
                if let Ok(v) = buf.parse::<u32>() {
                    let mut val = v.max(128);
                    s.context_length = val;
                }
            }),
            ctrl_e_toggle: None,
            editable: true,
        },
        // ... (all 28 fields)
        SettingField::Section("Yarn RoPE".to_string()),
        SettingField::Field {
            id: "rope_yarn_enabled",
            name: "Yarn RoPE",
            display: Box::new(|s| s.rope_yarn_enabled.to_string()),
            dirty: Box::new(|s, c| s.rope_yarn_enabled != c.rope_yarn_enabled),
            adjust: Box::new(|s, _, _, _| { s.rope_yarn_enabled = !s.rope_yarn_enabled; }),
            apply_edit: Box::new(|_, _| {}),
            ctrl_e_toggle: None,
            editable: true,
        },
        SettingField::Field {
            id: "yarn_params",
            name: "Yarn Params",
            display: Box::new(|s| format!("scale={:.2} base={:.2} scale_f={:.2}", s.rope_scale, s.rope_freq_base, s.rope_freq_scale)),
            dirty: Box::new(|s, c| s.rope_scale != c.rope_scale || s.rope_freq_base != c.rope_freq_base || s.rope_freq_scale != c.rope_freq_scale),
            adjust: Box::new(|_, _, _, _| {}),
            apply_edit: Box::new(|_, _| {}),
            ctrl_e_toggle: None,
            editable: false, // opens modal instead
        },
        // ... more fields
    ]
}

pub fn expert_fields() -> Vec<SettingField> {
    vec![
        SettingField::Section("Loading (expert)".to_string()),
        SettingField::Field {
            id: "threads_batch",
            name: "Threads Batch",
            display: Box::new(|s| s.threads_batch.to_string()),
            dirty: Box::new(|s, c| s.threads_batch != c.threads_batch),
            adjust: Box::new(|s, delta, _, _| { s.threads_batch = (s.threads_batch as i32 + delta).max(1) as u32; }),
            apply_edit: Box::new(|s, buf| { if let Ok(v) = buf.parse::<u32>() { s.threads_batch = v.max(1); } }),
            ctrl_e_toggle: None,
            editable: true,
        },
        // ... (all 27 expert fields)
    ]
}
```

## Changes Across Files

### 1. `src/tui/settings.rs` (new)
- New file containing `SettingField` enum, methods, and `standard_fields()` / `expert_fields()`

### 2. `src/tui/panel/settings.rs`
**Before:** `render_settings()` with hardcoded sections and `add_setting()` with 55-arm dirty match
**After:** `render_settings()` iterates over `SettingField` vec

```rust
// render_settings() simplified:
fn render_settings(lines: &mut Vec<Line<'static>>, total_count: &mut usize,
    selected_line_idx: &mut usize, selected_content_line: &mut usize,
    settings: &ModelSettings, cached: &ModelSettings,
    selected: usize, edit_buf: &str, editing: bool, expert_mode: bool) {

    let fields = if expert_mode {
        standard_fields().into_iter().chain(expert_fields()).collect::<Vec<_>>()
    } else {
        standard_fields()
    };

    for field in &fields {
        if field.is_section() {
            lines.push(Line::from(vec![
                Span::styled(format!("--- {} ---", field.name()),
                    Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
            ]));
            continue;
        }

        if *total_count == selected {
            *selected_line_idx = lines.len();
            *selected_content_line = lines.len();
        }

        let dirty = field.is_dirty(settings, cached);
        let display = if editing && *total_count == selected {
            edit_buf.clone()
        } else if dirty {
            format!("{}*", field.display(settings))
        } else {
            field.display(settings)
        };

        lines.push(Line::from(vec![
            Span::styled(if *total_count == selected { "> " } else { "  " },
                Style::default().fg(Color::Yellow)),
            Span::styled(format!("{}: ", field.name()),
                Style::default().fg(Color::Yellow)),
            Span::styled(display, if dirty {
                Style::default().fg(Color::Red)
            } else if *total_count == selected {
                Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            }),
        ]));
        *total_count += 1;
    }
}
```

**Remove:** `add_setting()` function entirely (its logic is now in `render_settings()`)

### 3. `src/tui/event/panel/settings.rs`
**Before:** `adjust_setting()` and `apply_numeric_setting()` with 55-arm matches
**After:** Dispatch through SettingField

```rust
// Replace adjust_setting():
pub fn adjust_setting(settings: &mut ModelSettings, field: &SettingField, delta: i32,
    max_threads: u32, max_context: u32, total_layers: u32) {
    field.adjust(settings, delta, max_threads, max_context);
}

// Replace apply_numeric_setting():
pub fn apply_numeric_setting(settings: &mut ModelSettings, field: &SettingField, buf: &str) {
    field.apply_edit(settings, buf);
}

// In handle_settings_key():
// Before: match idx { 6 => ..., 7 => ..., ... }
// After:
if key.code == KeyCode::Char('e') && key.modifiers.contains(KeyModifiers::CONTROL) {
    let fields = if app.settings_state.expert_mode {
        standard_fields().into_iter().chain(expert_fields()).collect::<Vec<_>>()
    } else {
        standard_fields()
    };
    if let Some(field) = fields.get(idx) {
        field.ctrl_e_toggle(&mut app.settings);
    }
}
```

### 4. `src/tui/event/key.rs`
**Before:** Ctrl+E match on index
**After:** Look up field by index and call toggle

### 5. `documentation/src/usage.md`
Add note about SettingField architecture (optional, mainly for future maintainers).

### 6. `AGENTS.md`
No changes needed — the field list is already documented.

## Migration Checklist

1. [ ] Create `src/tui/settings.rs` with SettingField enum + methods
2. [ ] Define `standard_fields()` with all 28 fields
3. [ ] Define `expert_fields()` with all 27 expert fields
4. [ ] Update `render_settings()` in `panel/settings.rs`
5. [ ] Remove `add_setting()` from `panel/settings.rs`
6. [ ] Update `adjust_setting()` in `event/panel/settings.rs`
7. [ ] Update `apply_numeric_setting()` in `event/panel/settings.rs`
8. [ ] Update Ctrl+E toggle in `event/panel/settings.rs`
9. [ ] Update Ctrl+E toggle in `event/key.rs`
10. [ ] Update `handle_settings_key` Ctrl+X to work with SettingField
11. [ ] Run `cargo check`
12. [ ] Update documentation

## Benefits

- **Adding a field**: One `SettingField::Field { ... }` entry
- **Removing a field**: One entry removed
- **No index drift**: Each field's logic is self-contained
- **Type safety**: The compiler ensures all closures have correct signatures
- **Testability**: Field logic can be unit tested independently
- **Readability**: Field definitions are grouped by section, no magic numbers
