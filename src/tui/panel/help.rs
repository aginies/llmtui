use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::tui::app::App;

pub fn render(f: &mut Frame, area: Rect, _app: &App) {
    let chunks = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(16),
            Constraint::Length(1),
        ])
        .split(area);

    // Title
    let title = Paragraph::new(Line::from(vec![
        Span::styled("Global Help", Style::default().add_modifier(Modifier::BOLD)),
        Span::styled(" — ", Style::default()),
        Span::styled("Press Ctrl+Shift+H or Esc to close", Style::default().fg(Color::DarkGray)),
    ]))
    .block(Block::default().borders(Borders::ALL).title(" HELP "))
    .style(Style::default().fg(Color::White));
    f.render_widget(title, chunks[0]);

    // Shortcuts list
    let shortcuts = vec![
        Line::from(vec![
            Span::styled("T", Style::default().fg(Color::Yellow)),
            Span::raw("  Cycle right panel tabs (Model Info / Settings)"),
        ]),
        Line::from(vec![
            Span::styled("Tab", Style::default().fg(Color::Yellow)),
            Span::raw("  Move focus to next panel"),
        ]),
        Line::from(vec![
            Span::styled("Shift+Tab", Style::default().fg(Color::Yellow)),
            Span::raw("  Move focus to prev panel"),
        ]),
        Line::from(vec![
            Span::styled("j / k / Arrow keys", Style::default().fg(Color::Yellow)),
            Span::raw("  Navigate"),
        ]),
        Line::from(vec![
            Span::styled("Enter", Style::default().fg(Color::Yellow)),
            Span::raw("  Load / Apply value"),
        ]),
        Line::from(vec![
            Span::styled("l", Style::default().fg(Color::Yellow)),
            Span::raw("  Load model"),
        ]),
        Line::from(vec![
            Span::styled("u", Style::default().fg(Color::Yellow)),
            Span::raw("  Unload model"),
        ]),
        Line::from(vec![
            Span::styled("0-9, -, .", Style::default().fg(Color::Yellow)),
            Span::raw("  Type numeric value in settings"),
        ]),
        Line::from(vec![
            Span::styled("Left / Right", Style::default().fg(Color::Yellow)),
            Span::raw("  Adjust value / Cycle cache"),
        ]),
        Line::from(vec![
            Span::styled("t", Style::default().fg(Color::Yellow)),
            Span::raw("  Toggle settings tab (Loading/Inference)"),
        ]),
        Line::from(vec![
            Span::styled("g / G", Style::default().fg(Color::Yellow)),
            Span::raw("  Scroll log to bottom / top"),
        ]),
        Line::from(vec![
            Span::styled("PageUp / PageDown", Style::default().fg(Color::Yellow)),
            Span::raw("  Scroll log 10 lines"),
        ]),
        Line::from(vec![
            Span::styled("Ctrl+C", Style::default().fg(Color::Yellow)),
            Span::raw("  Quit / Cancel download"),
        ]),
        Line::from(vec![
            Span::styled("/", Style::default().fg(Color::Yellow)),
            Span::raw("  Search HuggingFace"),
        ]),
        Line::from(vec![
            Span::styled("l", Style::default().fg(Color::Yellow)),
            Span::raw("  Download selected (in search)"),
        ]),
        Line::from(vec![
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::raw("  Cancel edit / search"),
        ]),
        Line::from(vec![
            Span::styled("Ctrl+H", Style::default().fg(Color::Yellow)),
            Span::raw("  Panel help (contextual)"),
        ]),
        Line::from(vec![
            Span::styled("Ctrl+Shift+H", Style::default().fg(Color::Yellow)),
            Span::raw("  Global help (this screen)"),
        ]),
        Line::from(vec![
            Span::styled("p", Style::default().fg(Color::Yellow)),
            Span::raw("  Open profiles panel"),
        ]),
        Line::from(vec![
            Span::styled("s", Style::default().fg(Color::Yellow)),
            Span::raw("  Save current settings as profile (in profiles panel)"),
        ]),
        Line::from(vec![
            Span::styled("d", Style::default().fg(Color::Yellow)),
            Span::raw("  Delete user profile (in profiles panel, excludes built-in)"),
        ]),
        Line::from(vec![
            Span::styled("Tab", Style::default().fg(Color::Yellow)),
            Span::raw("  Cycle panels (Models -> Log -> Settings -> Profiles -> Prompts)"),
        ]),
        Line::from(vec![
            Span::styled("j/k", Style::default().fg(Color::Yellow)),
            Span::raw("  Navigate in Settings, Profiles, Prompts panels"),
        ]),
        Line::from(vec![
            Span::styled("Enter", Style::default().fg(Color::Yellow)),
            Span::raw("  Apply preset (in Profiles/Prompts panels)"),
        ]),
        Line::from(vec![
            Span::styled("e", Style::default().fg(Color::Yellow)),
            Span::raw("  Edit preset (in Prompts panel)"),
        ]),
        Line::from(vec![
            Span::styled("n", Style::default().fg(Color::Yellow)),
            Span::raw("  New preset (in Prompts panel)"),
        ]),
        Line::from(vec![
            Span::styled("Ctrl+S", Style::default().fg(Color::Yellow)),
            Span::raw("  Save preset (in Prompts edit mode)"),
        ]),
        Line::from(vec![
            Span::styled("d", Style::default().fg(Color::Yellow)),
            Span::raw("  Delete custom preset (in Prompts panel)"),
        ]),
    ];

    let list = Paragraph::new(shortcuts)
        .block(Block::default().borders(Borders::ALL).title(" "));
    f.render_widget(list, chunks[1]);

    // Footer
    let footer = Paragraph::new("Press Ctrl+Shift+H or Esc to close")
        .style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD));
    f.render_widget(footer, chunks[2]);
}

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
        Span::styled(" — ", Style::default()),
        Span::styled("Esc to close", Style::default().fg(Color::DarkGray)),
    ]))
    .block(Block::default().borders(Borders::ALL).title(" "))
    .style(Style::default().fg(Color::White));
    f.render_widget(title, chunks[0]);

    // Scrollable content
    let lines = app.panel_help_lines();
    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(" "))
        .scroll((app.panel_help_offset, 0));
    f.render_widget(paragraph, chunks[1]);

    // Footer
    let footer = Paragraph::new("j/k scroll · Esc close")
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(footer, chunks[2]);
}
