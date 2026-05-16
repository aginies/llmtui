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
