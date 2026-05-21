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

    let title = if app.is_panel_visible(5) { " Log (F6) " } else { " Log " };
    let border_color = if app.active_panel == crate::tui::app::ActivePanel::Log {
        Color::Green
    } else {
        Color::Rgb(255, 165, 0)
    };
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let lines: Vec<Line> = app
        .log_entries
        .iter()
        .map(|e| {
            let level_color = match e.level {
                crate::config::LogLevel::Info => Color::Cyan,
                crate::config::LogLevel::Warning => Color::Yellow,
                crate::config::LogLevel::Error => Color::Red,
            };
            Line::from(vec![
                Span::styled(format!("[{}] ", e.timestamp), Style::default().fg(Color::DarkGray)),
                Span::styled(format!("[{}] ", e.level.label()), Style::default().fg(level_color).add_modifier(Modifier::BOLD)),
                Span::styled(&e.message, Style::default().fg(Color::White)),
            ])
        })
        .collect();

    // Height inside borders
    let inner_height = log_area.height.saturating_sub(2);
    let total_lines = lines.len();

    // If not focused on log, auto-scroll to bottom
    if app.active_panel != crate::tui::app::ActivePanel::Log {
        app.log_scroll_offset = total_lines.saturating_sub(inner_height as usize) as u16;
    }

    let paragraph = Paragraph::new(lines)
        .block(block)
        .scroll((app.log_scroll_offset, 0))
        .wrap(Wrap { trim: false });


    f.render_widget(paragraph, log_area);

    // Render scrollbar inside borders (below content)
    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(Some("↑"))
        .end_symbol(Some("↓"));
    let mut scrollbar_state = ScrollbarState::new(total_lines)
        .position(app.log_scroll_offset as usize);

    f.render_stateful_widget(
        scrollbar,
        log_area.inner(ratatui::layout::Margin { vertical: 1, horizontal: 0 }),
        &mut scrollbar_state,
    );
}
