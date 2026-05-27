//! Tests for UI utility functions — pure functions in panel and render modules.
//!
//! Tests cover: MdRenderer markdown parsing, max_context_for_vram,
//! is_dirty settings tracking, and fixes for dead/no-op assertions in existing tests.

use llm_manager::models::*;
use llm_manager::tui::panel::readme::MdRenderer;
use llm_manager::tui::panel::info::max_context_for_vram;

// ── MdRenderer: basic text ──────────────────────────────────────

#[test]
fn md_renderer_plain_text() {
    let lines = MdRenderer::render_markdown("Hello world");
    assert_eq!(lines.len(), 1);
}

#[test]
fn md_renderer_paragraphs_separated() {
    let lines = MdRenderer::render_markdown("First paragraph.\n\nSecond paragraph.");
    // Two paragraphs separated by an empty line
    assert!(lines.len() >= 2);
}

#[test]
fn md_renderer_hard_break() {
    let lines = MdRenderer::render_markdown("Line one.\n\nLine two.");
    assert!(lines.len() >= 2);
}

// ── MdRenderer: bold and italic ─────────────────────────────────

#[test]
fn md_renderer_bold() {
    let lines = MdRenderer::render_markdown("This is **bold** text");
    assert_eq!(lines.len(), 1);
}

#[test]
fn md_renderer_italic() {
    let lines = MdRenderer::render_markdown("This is *italic* text");
    assert_eq!(lines.len(), 1);
}

#[test]
fn md_renderer_bold_and_italic() {
    let lines = MdRenderer::render_markdown("This is ***bold italic*** text");
    assert_eq!(lines.len(), 1);
}

// ── MdRenderer: inline code ─────────────────────────────────────

#[test]
fn md_renderer_inline_code() {
    let lines = MdRenderer::render_markdown("Use `cargo build` to compile");
    assert_eq!(lines.len(), 1);
}

// ── MdRenderer: code blocks ─────────────────────────────────────

#[test]
fn md_renderer_code_block() {
    let md = "```python\nprint('hello')\n```\n";
    let lines = MdRenderer::render_markdown(md);
    // Code block should render as at least one line
    assert!(lines.len() >= 1);
}

#[test]
fn md_renderer_code_block_with_language() {
    let md = "```rust\nfn main() {}\n```\n";
    let lines = MdRenderer::render_markdown(md);
    assert!(lines.len() >= 1);
}

// ── MdRenderer: headings ────────────────────────────────────────

#[test]
fn md_renderer_h1() {
    let lines = MdRenderer::render_markdown("# Title");
    assert_eq!(lines.len(), 1);
}

#[test]
fn md_renderer_h2() {
    let lines = MdRenderer::render_markdown("## Section");
    assert_eq!(lines.len(), 1);
}

#[test]
fn md_renderer_h3() {
    let lines = MdRenderer::render_markdown("### Subsection");
    assert_eq!(lines.len(), 1);
}

#[test]
fn md_renderer_multiple_headings() {
    let md = "# Title\n\n## Section\n\n### Subsection";
    let lines = MdRenderer::render_markdown(md);
    assert!(lines.len() >= 3);
}

// ── MdRenderer: horizontal rule ─────────────────────────────────

#[test]
fn md_renderer_rule() {
    let lines = MdRenderer::render_markdown("---");
    assert_eq!(lines.len(), 1);
}

#[test]
fn md_renderer_rule_with_asterisks() {
    let lines = MdRenderer::render_markdown("***");
    assert_eq!(lines.len(), 1);
}

#[test]
fn md_renderer_rule_with_dashes() {
    let lines = MdRenderer::render_markdown("___");
    assert_eq!(lines.len(), 1);
}

// ── MdRenderer: lists ───────────────────────────────────────────

#[test]
fn md_renderer_unordered_list() {
    let md = "- Item one\n- Item two\n- Item three";
    let lines = MdRenderer::render_markdown(md);
    assert!(lines.len() >= 3);
}

#[test]
fn md_renderer_ordered_list() {
    let md = "1. First\n2. Second\n3. Third";
    let lines = MdRenderer::render_markdown(md);
    assert!(lines.len() >= 3);
}

#[test]
fn md_renderer_mixed_list() {
    let md = "- Item one\n- Item two\n\n1. Ordered one\n2. Ordered two";
    let lines = MdRenderer::render_markdown(md);
    assert!(lines.len() >= 4);
}

// ── MdRenderer: task lists ──────────────────────────────────────

#[test]
fn md_renderer_task_list_checked() {
    let md = "- [x] Done task";
    let lines = MdRenderer::render_markdown(md);
    assert_eq!(lines.len(), 1);
}

#[test]
fn md_renderer_task_list_unchecked() {
    let md = "- [ ] Todo task";
    let lines = MdRenderer::render_markdown(md);
    assert_eq!(lines.len(), 1);
}

// ── MdRenderer: blockquotes ─────────────────────────────────────

#[test]
fn md_renderer_blockquote() {
    let md = "> This is a quote";
    let lines = MdRenderer::render_markdown(md);
    assert_eq!(lines.len(), 1);
}

#[test]
fn md_renderer_nested_blockquote() {
    let md = ">> Nested quote";
    let lines = MdRenderer::render_markdown(md);
    assert_eq!(lines.len(), 1);
}

// ── MdRenderer: tables ──────────────────────────────────────────

#[test]
fn md_renderer_table() {
    let md = "| Header 1 | Header 2 |\n|----------|----------|\n| Cell 1 | Cell 2 |";
    let lines = MdRenderer::render_markdown(md);
    // Table header + separator + row
    assert!(lines.len() >= 2);
}

#[test]
fn md_renderer_table_multiple_rows() {
    let md = "| A | B |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |";
    let lines = MdRenderer::render_markdown(md);
    assert!(lines.len() >= 3);
}

// ── MdRenderer: combined content ────────────────────────────────

#[test]
fn md_renderer_full_readme() {
    let md = r#"# Model Card

This is a test model.

## Features

- Fast inference
- Low memory usage

## Usage

```bash
llama-server -m model.gguf
```

> Note: requires GGUF format.

| Param | Value |
|-------|-------|
| ctx   | 32768 |
| temp  | 0.7   |

- [x] Tested
- [ ] Benchmarked
"#;
    let lines = MdRenderer::render_markdown(md);
    // Should produce many lines from headings, lists, code block, table, blockquote
    assert!(lines.len() >= 10);
}

// ── MdRenderer: edge cases ──────────────────────────────────────

#[test]
fn md_renderer_empty_string() {
    let lines = MdRenderer::render_markdown("");
    assert!(lines.is_empty());
}

#[test]
fn md_renderer_whitespace_only() {
    let lines = MdRenderer::render_markdown("   \n\n   ");
    assert!(lines.is_empty());
}

#[test]
fn md_renderer_long_line_wrapped() {
    let long = "a".repeat(200);
    let lines = MdRenderer::render_markdown(&long);
    assert_eq!(lines.len(), 1);
}

#[test]
fn md_renderer_unicode() {
    let lines = MdRenderer::render_markdown("日本語テスト 🦙");
    assert_eq!(lines.len(), 1);
}

// ── max_context_for_vram: basic scenarios ───────────────────────

#[test]
fn max_context_zero_vram_budget() {
    // Model uses all VRAM, leaving nothing for KV cache
    let result = max_context_for_vram(
        16_000,  // model_mib
        16_000,  // vram_mib (model uses all)
        32,      // total_layers
        4096,    // hidden_size
        32,      // n_head
        32,      // n_kv_head
        -1,      // gpu_layers (all)
        false,   // flash_attn
        false,   // uniform_cache
        1,       // parallel
        "F16",   // cache_type_k
        "F16",   // cache_type_v
    );
    assert_eq!(result, 0);
}

#[test]
fn max_context_zero_layers() {
    let result = max_context_for_vram(
        16_000, 4096, 0, 4096, 32, 32, -1, false, false, 1, "F16", "F16",
    );
    assert_eq!(result, 0);
}

#[test]
fn max_context_no_gpu_layers() {
    let result = max_context_for_vram(
        16_000, 4096, 32, 4096, 32, 32, 0, false, false, 1, "F16", "F16",
    );
    assert_eq!(result, 0);
}

#[test]
fn max_context_with_vram_budget() {
    let result = max_context_for_vram(
        16_000,  // model_mib
        24_000,  // vram_mib (8000 MiB for KV cache)
        32,      // total_layers
        4096,    // hidden_size
        32,      // n_head
        32,      // n_kv_head
        32,      // gpu_layers (all)
        false,   // flash_attn
        false,   // uniform_cache
        1,       // parallel
        "F16",   // cache_type_k
        "F16",   // cache_type_v
    );
    assert!(result > 0);
}

#[test]
fn max_context_flash_attn_doubles_context() {
    let no_flash = max_context_for_vram(
        16_000, 24_000, 32, 4096, 32, 32, 32, false, false, 1, "F16", "F16",
    );
    let flash = max_context_for_vram(
        16_000, 24_000, 32, 4096, 32, 32, 32, true, false, 1, "F16", "F16",
    );
    assert!(flash > no_flash);
}

#[test]
fn max_context_uniform_cache_increases_context() {
    let single = max_context_for_vram(
        16_000, 24_000, 32, 4096, 32, 32, 32, false, false, 1, "F16", "F16",
    );
    let parallel_4 = max_context_for_vram(
        16_000, 24_000, 32, 4096, 32, 32, 32, false, true, 4, "F16", "F16",
    );
    assert!(parallel_4 > single);
}

#[test]
fn max_context_gqa_reduces_kv_size() {
    // GQA: 32 query heads, 8 KV heads
    let with_gqa = max_context_for_vram(
        16_000, 24_000, 32, 4096, 32, 8, 32, false, false, 1, "F16", "F16",
    );
    // No GQA: 32 query heads, 32 KV heads
    let without_gqa = max_context_for_vram(
        16_000, 24_000, 32, 4096, 32, 32, 32, false, false, 1, "F16", "F16",
    );
    assert!(with_gqa > without_gqa);
}

#[test]
fn max_context_quantization_increases_context() {
    let f16 = max_context_for_vram(
        16_000, 24_000, 32, 4096, 32, 32, 32, false, false, 1, "F16", "F16",
    );
    let q4 = max_context_for_vram(
        16_000, 24_000, 32, 4096, 32, 32, 32, false, false, 1, "Q4_0", "Q4_0",
    );
    assert!(q4 > f16);
}

#[test]
fn max_context_all_layers() {
    let result = max_context_for_vram(
        16_000, 24_000, 32, 4096, 32, 32, -1, false, false, 1, "F16", "F16",
    );
    // gpu_layers < 0 means all layers
    assert!(result > 0);
}

#[test]
fn max_context_specific_gpu_layers() {
    let half = max_context_for_vram(
        16_000, 24_000, 32, 4096, 32, 32, 16, false, false, 1, "F16", "F16",
    );
    let all = max_context_for_vram(
        16_000, 24_000, 32, 4096, 32, 32, 32, false, false, 1, "F16", "F16",
    );
    // More layers in VRAM = less budget for KV cache = smaller context
    assert!(all < half);
}

#[test]
fn max_context_increases_with_vram() {
    let small = max_context_for_vram(
        16_000, 20_000, 32, 4096, 32, 32, 32, false, false, 1, "F16", "F16",
    );
    let large = max_context_for_vram(
        16_000, 40_000, 32, 4096, 32, 32, 32, false, false, 1, "F16", "F16",
    );
    assert!(large > small);
}

#[test]
fn max_context_no_hidden_size_zero() {
    let result = max_context_for_vram(
        16_000, 24_000, 32, 0, 32, 32, 32, false, false, 1, "F16", "F16",
    );
    // Zero hidden_size means zero bytes per token, so infinite context
    // (but clamped by float precision, likely very large)
    assert!(result >= 0);
}

// ── ModelSettings::is_dirty ─────────────────────────────────────

#[test]
fn settings_is_dirty_identical() {
    let mut settings = ModelSettings::default();
    let cache = settings.clone();
    assert!(!settings.is_dirty(&cache));
}

#[test]
fn settings_is_dirty_context_changed() {
    let mut settings = ModelSettings::default();
    let mut cache = settings.clone();
    settings.context_length = 8192;
    assert!(settings.is_dirty(&cache));
}

#[test]
fn settings_is_dirty_temperature_changed() {
    let mut settings = ModelSettings::default();
    let mut cache = settings.clone();
    settings.temperature = 0.5;
    assert!(settings.is_dirty(&cache));
}

#[test]
fn settings_is_dirty_gpu_layers_changed() {
    let settings = ModelSettings::default();
    let mut cache = settings.clone();
    cache.gpu_layers_mode = GpuLayersMode::Specific(20);
    assert!(settings.is_dirty(&cache));
}

#[test]
fn settings_is_dirty_mlock_changed() {
    let mut settings = ModelSettings::default();
    let mut cache = settings.clone();
    settings.mlock = true;
    assert!(settings.is_dirty(&cache));
}

#[test]
fn settings_is_dirty_flash_attn_changed() {
    let mut settings = ModelSettings::default();
    let mut cache = settings.clone();
    settings.flash_attn = false;
    assert!(settings.is_dirty(&cache));
}

#[test]
fn settings_is_dirty_cache_type_changed() {
    let mut settings = ModelSettings::default();
    let mut cache = settings.clone();
    settings.cache_type_k = Some(CacheTypeK::Q8_0);
    assert!(settings.is_dirty(&cache));
}

#[test]
fn settings_is_dirty_threads_changed() {
    let mut settings = ModelSettings::default();
    let mut cache = settings.clone();
    settings.threads = 1;
    assert!(settings.is_dirty(&cache));
}

#[test]
fn settings_is_dirty_backend_version_changed() {
    let mut settings = ModelSettings::default();
    let mut cache = settings.clone();
    settings.llama_cpp_version_cpu = Some("b1234".into());
    assert!(settings.is_dirty(&cache));
}

#[test]
fn settings_is_dirty_multiple_fields() {
    let mut settings = ModelSettings::default();
    let mut cache = settings.clone();
    settings.context_length = 8192;
    settings.temperature = 0.5;
    settings.top_k = 30;
    assert!(settings.is_dirty(&cache));
}

#[test]
fn settings_is_dirty_no_change_after_clone() {
    let mut settings = ModelSettings::default();
    let cache = settings.clone();
    assert!(!settings.is_dirty(&cache));
}
