use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

/// Render the Server Settings panel (Connectivity + Backend).
/// Returns (lines, total_count, height).
pub fn render_all(settings: &crate::models::ModelSettings, selected: usize, edit_buf: &str, editing: bool) -> (Vec<Line<'static>>, usize, usize) {
    let mut lines = Vec::new();
    let mut total_count = 0;

    // ── Connectivity ──────────────────────────────────────────
    lines.push(Line::from(vec![
        Span::styled("--- Connectivity (Global) ---", Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
    ]));

    let host_val = if settings.host == "127.0.0.1" { "localhost (127.0.0.1)" } else { &settings.host };
    add_setting(&mut lines, &mut total_count, "Host", host_val, selected, edit_buf, editing);
    lines.push(Line::from(vec![
        Span::styled("  (Enter to toggle: localhost <-> 0.0.0.0)", Style::default().fg(Color::DarkGray)),
    ]));

    // ── Backend ───────────────────────────────────────────────
    lines.push(Line::from(vec![
        Span::styled("--- Backend (Global) ---", Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
    ]));

    let backend_name = format!("{}", settings.backend);
    add_setting(&mut lines, &mut total_count, "Backend", &backend_name, selected, edit_buf, editing);
    lines.push(Line::from(vec![
        Span::styled("  (auto-download on load)", Style::default().fg(Color::DarkGray)),
    ]));

    (lines.clone(), total_count, lines.len())
}

fn add_setting(lines: &mut Vec<Line<'static>>, total_count: &mut usize, name: &str, val: &str, selected: usize, edit_buf: &str, editing: bool) {
    let current_idx = *total_count;
    let marker = if current_idx == selected { "> " } else { "  " };
    let name_style = Style::default().fg(Color::Yellow);
    let (display_val, val_style) = if current_idx == selected && editing {
        (edit_buf.to_string(), Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD))
    } else if current_idx == selected {
        (val.to_string(), Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD))
    } else {
        (val.to_string(), Style::default().fg(Color::White))
    };
    lines.push(Line::from(vec![
        Span::styled(marker, Style::default().fg(Color::Yellow)),
        Span::styled(format!("{name}: "), name_style),
        Span::styled(display_val, val_style),
    ]));
    *total_count += 1;
}
