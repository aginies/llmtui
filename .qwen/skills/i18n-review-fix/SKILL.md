---
name: i18n-review-fix
description: Review Rust TUI code for hardcoded user-facing strings and fix them using the project's i18n system
source: auto-skill
extracted_at: '2026-06-08T12:16:09.800Z'
---

## i18n Review and Fix for llmtui TUI Code

When reviewing or modifying Rust TUI code in this project, **all user-facing strings must go through the i18n system**. Hardcoded English text in source code is a violation of AGENTS.md rules.

### 1. Identify hardcoded UI strings

Scan source for these patterns:

- **String literals used as labels, button text, dialog messages, tooltips, help text, panel titles** — these are user-facing.
- **Format strings with placeholders** like `"{} drafts"` or `"{} tokens"` used in UI rendering.
- **Struct fields that store display text** (e.g., `ModelInfoPair.label: String` storing `"Path"`, `"Arch"`, `"Capabilities"`).

**Do NOT translate:**
- Error messages for logs/debug output
- Technical/internal strings
- File paths, identifiers, CLI commands
- Git messages, commit text

### 2. Determine the correct i18n macro

| Pattern | Macro | Example |
|---------|-------|---------|
| Simple static string | `t!("key")` | `t!("dialog.exit.title")` |
| String with `{}` placeholders | `t_fmt!("key", args...)` | `t_fmt!("async.downloading", name)` |
| Key stored in struct field, resolved later | `crate::t!(field_key)` | `crate::t!(pair.label_key)` |

### 3. Adding new keys — all 3 locales simultaneously

When adding a new UI string, **add the key to ALL locale files at once** (`en.json`, `fr.json`, `it.json`). Never add only to `en.json` and leave others empty.

Key naming convention: dot-separated hierarchical keys matching UI context:
- `panel.title.*` — panel titles
- `dialog.*.title` — dialog titles
- `dialog.*.message` — dialog messages
- `model_info.*` — model info labels
- `field.help.*` — field help text
- `hints.*` — keyboard hints
- `async.*` — async operation messages
- `status.*` — status bar text

### 4. Fixing `ModelInfoPair`-style patterns

When a struct stores display text in a field (like `label: String`):

1. **Change the field type** from `String` to `&'static str` — stores the i18n key identifier.
2. **Update construction sites** to use the key literal instead of the display string.
3. **Update the consumer** (render function) to resolve the key via `crate::t!(pair.label_key)` before formatting.

Example transformation:

```rust
// Before
pub struct ModelInfoPair {
    pub label: String,
    pub value: String,
}
pairs.push(ModelInfoPair {
    label: "Path".to_string(),
    value: path,
});

// After
pub struct ModelInfoPair {
    pub label_key: &'static str,
    pub value: String,
}
pairs.push(ModelInfoPair {
    label_key: "model_info.path",
    value: path,
});

// In consumer:
let label = format!("{}: ", crate::t!(pair.label_key));
```

### 5. Verify the fix

After making i18n changes:

1. Confirm every locale file (`en.json`, `fr.json`, `it.json`) has the new key.
2. Check that `cargo check` / `cargo build` passes (i18n keys are string literals, no compilation changes expected).
3. Visually verify the TUI renders correctly in each language.
4. For `t_fmt!()` calls, verify the number of `{}` placeholders matches the number of arguments.

### 6. Common pitfalls

- **Fixed-width column padding** (`{:<12}`) in rendering may cause alignment issues with longer translations. This is a pre-existing constraint — be aware when translating labels.
- **`t_fmt!()` placeholder order** — if the locale translation reorders placeholders, the `{}` replacement in `t_fmt()` won't handle this. For complex translations, consider separate keys.
- **Dynamic values kept hardcoded** — per project practice, some dynamic format strings (like `"{} drafts"`, `"{} tokens"`) may remain as hardcoded Rust strings if translations don't require reordering. Document this decision.
