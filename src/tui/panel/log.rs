use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
};

use crate::tui::app::App;

pub fn render(f: &mut Frame, area: Rect, app: &mut App) {
    if area.height < 2 {
        return;
    }

    let log_area = area;

    let title = if app.is_panel_visible(5) {
        if app.log.log_follow { " Log (F6) - Following " } else { " Log (F6) - Manual " }
    } else {
        if app.log.log_follow { " Log - Following " } else { " Log - Manual " }
    };
    let border_color = if app.ui.active_panel == crate::tui::app::ActivePanel::Log {
        Color::Green
    } else {
        Color::Rgb(255, 165, 0)
    };
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let mut lines: Vec<Line> = Vec::new();
    for e in &app.log.log_entries {
        let level_color = match e.level {
            crate::config::LogLevel::Info => Color::Cyan,
            crate::config::LogLevel::Warning => Color::Yellow,
            crate::config::LogLevel::Error => Color::Red,
        };

        let ts_prefix = format!("[{}] ", e.timestamp);
        let lv_prefix = format!("[{}] ", e.level.label());
        let prefix_width = ts_prefix.len() + lv_prefix.len();

        let msg_lines: Vec<&str> = e.message.lines().collect();
        if msg_lines.is_empty() {
             lines.push(Line::from(vec![
                Span::styled(ts_prefix, Style::default().fg(Color::DarkGray)),
                Span::styled(lv_prefix, Style::default().fg(level_color).add_modifier(Modifier::BOLD)),
                Span::styled(&e.message, Style::default().fg(Color::White)),
            ]));
        } else {
            for (i, line) in msg_lines.into_iter().enumerate() {
                if i == 0 {
                    lines.push(Line::from(vec![
                        Span::styled(ts_prefix.clone(), Style::default().fg(Color::DarkGray)),
                        Span::styled(lv_prefix.clone(), Style::default().fg(level_color).add_modifier(Modifier::BOLD)),
                        Span::styled(line, Style::default().fg(Color::White)),
                    ]));
                } else {
                    lines.push(Line::from(vec![
                        Span::raw(" ".repeat(prefix_width)),
                        Span::styled(line, Style::default().fg(Color::White)),
                    ]));
                }
            }
        }
    }

    let inner_area = block.inner(log_area);
    let width = inner_area.width.max(1) as usize;

    // Calculate total lines after wrapping (estimation since line_count is unstable/private)
    let total_screen_lines = lines.iter()
        .map(|l| (l.width().max(1) + width - 1) / width)
        .sum::<usize>();
    
    app.log.log_total_lines = total_screen_lines;

    // Auto-scroll to bottom if follow is enabled
    if app.log.log_follow {
        app.log.log_scroll_offset = total_screen_lines.saturating_sub(inner_area.height as usize);
    }

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph.scroll((app.log.log_scroll_offset as u16, 0)), log_area);

    // Render scrollbar inside borders (below content)
    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(Some("↑"))
        .end_symbol(Some("↓"));
    let mut scrollbar_state = ScrollbarState::new(total_screen_lines)
        .position(app.log.log_scroll_offset as usize);

    f.render_stateful_widget(
        scrollbar,
        log_area.inner(ratatui::layout::Margin { vertical: 1, horizontal: 0 }),
        &mut scrollbar_state,
    );
}
