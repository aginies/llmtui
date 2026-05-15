use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::tui::app::{App, ConfigField};

pub fn render(f: &mut Frame, area: Rect, app: &mut App) {
    let chunks = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(area);

    // Title bar
    let title = Paragraph::new(Line::from(vec![
        Span::styled("Config", Style::default().add_modifier(Modifier::BOLD)),
        Span::styled(" — ", Style::default()),
        Span::styled("Enter save  Esc cancel", Style::default().fg(Color::DarkGray)),
    ]))
    .block(Block::default().borders(Borders::ALL).title(" CONFIG "))
    .style(Style::default().fg(Color::White));
    f.render_widget(title, chunks[0]);

    // Content
    let models_dir_style = if app.config_editing_field == ConfigField::ModelsDir {
        Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };

    let backend_name = if app.config_backend_idx == 0 { "cpu" } else { "vulkan" };
    let switch_key = if app.config_backend_idx == 0 { 'v' } else { 'a' };

    let backend_style = if app.config_editing_field == ConfigField::Backend {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Cyan)
    };

    let content = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("Models dir: ", Style::default().fg(Color::Yellow)),
            Span::styled(format!("{}>", app.config_models_dir_edit), models_dir_style),
        ]),
        Line::from(vec![
            Span::styled("Backend:   ", Style::default().fg(Color::Yellow)),
            Span::styled(format!("{}  [{}]", backend_name, switch_key), backend_style),
        ]),
        Line::from(""),
    ])
    .block(Block::default().borders(Borders::ALL).title(" "));
    f.render_widget(content, chunks[1]);
}
