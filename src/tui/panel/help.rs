use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::tui::app::App;

pub fn render_panel(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            Constraint::Length(2),   // title
            Constraint::Fill(1),     // scrollable content
            Constraint::Length(1),   // footer
        ])
        .split(area);

    // Title
    let title = Paragraph::new(Line::from(vec![
        Span::styled("Help", Style::default().add_modifier(Modifier::BOLD)),
        Span::styled(" — ", Style::default().fg(Color::DarkGray)),
        Span::styled("Esc to close", Style::default().fg(Color::DarkGray)),
    ]))
    .block(Block::default().borders(Borders::ALL).title(" "))
    .style(Style::default().fg(Color::White));
    f.render_widget(title, chunks[0]);

    // Scrollable content
    let lines = app.panel_help_lines();
    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(" "))
        .wrap(Wrap { trim: true })
        .scroll((app.panel_help_offset as u16, 0));
    f.render_widget(paragraph, chunks[1]);

    // Footer
    let footer = Paragraph::new("j/k scroll · ⎋ close")
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(footer, chunks[2]);
}
