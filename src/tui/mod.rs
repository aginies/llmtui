use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState},
};

use crate::models::BenchTuneParamValue;

pub mod app;
pub mod render;
pub mod event;
pub mod panel;

/// Format a byte count into a human-readable size string.
pub fn format_size(bytes: u64) -> String {
    let kb = 1024.0;
    let mb = kb * 1024.0;
    let gb = mb * 1024.0;
    let tb = gb * 1024.0;
    let s = bytes as f64;
    if s < kb {
        format!("{} B", bytes)
    } else if s < mb {
        format!("{:.1} KB", s / kb)
    } else if s < gb {
        format!("{:.1} MB", s / mb)
    } else if s < tb {
        format!("{:.1} GB", s / gb)
    } else {
        format!("{:.1} TB", s / tb)
    }
}

/// Format a number into an abbreviated human-readable string (e.g., 1.5K, 2.3M, 1.2B).
pub fn format_number(n: u64) -> String {
    if n >= 1_000_000_000 {
        format!("{:.1}B", n as f64 / 1_000_000_000.0)
    } else if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        format!("{}", n)
    }
}

/// Render a vertical scrollbar on the right edge of an area.
///
/// `top_offset` and `height_offset` allow adjusting the scrollbar position
/// (e.g., offset 1 from top, 2 from height for panels with title borders).
pub fn render_vertical_scrollbar(
    f: &mut Frame,
    area: Rect,
    total_items: usize,
    position: usize,
    top_offset: u16,
    height_offset: u16,
) {
    let scrollbar_area = Rect {
        x: area.right().saturating_sub(1),
        y: area.top() + top_offset,
        width: 1,
        height: area.height.saturating_sub(height_offset),
    };
    let mut scrollbar_state = ScrollbarState::new(total_items).position(position);
    f.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓")),
        scrollbar_area,
        &mut scrollbar_state,
    );
}

/// Format benchmark parameters as display strings.
///
/// If `verbose` is true, returns full "key: value" lines with 2-decimal floats.
/// If `verbose` is false, returns compact "key=value" strings (1-decimal floats).
pub fn format_bench_params(params: &BenchTuneParamValue, verbose: bool) -> Vec<String> {
    let mut parts = Vec::new();
    if let Some(v) = params.temperature {
        parts.push(if verbose {
            format!("  temperature: {:.2}", v)
        } else {
            format!("temp={:.1}", v)
        });
    }
    if let Some(v) = params.top_p {
        parts.push(if verbose {
            format!("  top_p: {:.2}", v)
        } else {
            format!("top_p={:.1}", v)
        });
    }
    if let Some(v) = params.top_k {
        parts.push(if verbose {
            format!("  top_k: {}", v)
        } else {
            format!("top_k={}", v)
        });
    }
    if let Some(v) = params.repeat_penalty {
        parts.push(if verbose {
            format!("  repeat_penalty: {:.2}", v)
        } else {
            format!("repeat_penalty={:.1}", v)
        });
    }
    if let Some(v) = params.context_length {
        parts.push(if verbose {
            format!("  context_length: {}", v)
        } else {
            format!("context_length={}", v)
        });
    }
    if let Some(v) = params.batch_size {
        parts.push(if verbose {
            format!("  batch_size: {}", v)
        } else {
            format!("batch={}", v)
        });
    }
    if let Some(v) = params.threads {
        parts.push(if verbose {
            format!("  threads: {}", v)
        } else {
            format!("threads={}", v)
        });
    }
    if let Some(v) = params.flash_attn {
        parts.push(if verbose {
            format!("  flash_attn: {}", if v { "on" } else { "off" })
        } else {
            format!("fa={}", if v { "on" } else { "off" })
        });
    }
    if let Some(v) = params.expert_count {
        parts.push(if verbose {
            format!("  expert_count: {}", v)
        } else {
            format!("experts={}", v)
        });
    }
    parts
}

