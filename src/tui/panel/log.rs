use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
};

use crate::tui::app::App;

fn render_status_bar<'a>(app: &'a App) -> Line<'a> {
    let mut parts = Vec::new();

    match &app.models_mode {
        crate::tui::app::ModelsMode::Search { query: _, sort_by, show_readme, .. } => {
            parts.push(Span::styled("SEARCH", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
            parts.push(Span::raw(" "));
            parts.push(Span::styled("Enter", Style::default().fg(Color::Cyan)));
            parts.push(Span::raw(" search  "));
            parts.push(Span::styled("Esc", Style::default().fg(Color::Cyan)));
            parts.push(Span::raw(" exit  "));
            parts.push(Span::styled("l", Style::default().fg(Color::Yellow)));
            parts.push(Span::raw(" files  "));
            parts.push(Span::styled("S", Style::default().fg(Color::Yellow)));
            parts.push(Span::raw(" sort  "));
            if *show_readme {
                if app.readme_expanded {
                    parts.push(Span::styled("R", Style::default().fg(Color::Yellow)));
                    parts.push(Span::raw(" fullscreen  "));
                    parts.push(Span::styled("Esc", Style::default().fg(Color::Cyan)));
                    parts.push(Span::raw(" collapse  "));
                } else {
                    parts.push(Span::styled("R", Style::default().fg(Color::Yellow)));
                    parts.push(Span::raw(" README  "));
                }
            } else {
                parts.push(Span::styled("R", Style::default().fg(Color::Yellow)));
                parts.push(Span::raw(" README  "));
            }
            parts.push(Span::styled("j/k", Style::default().fg(Color::Cyan)));
            parts.push(Span::raw(" navigate  "));
            parts.push(Span::styled(sort_by.label(), Style::default().fg(Color::Magenta)));
        }
        crate::tui::app::ModelsMode::Files { model_id, .. } => {
            parts.push(Span::styled("FILES", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
            parts.push(Span::raw(" "));
            parts.push(Span::styled(model_id, Style::default().fg(Color::Cyan)));
            parts.push(Span::raw("  "));
            parts.push(Span::styled("Enter", Style::default().fg(Color::Yellow)));
            parts.push(Span::raw(" download"));
            parts.push(Span::raw("  "));
            parts.push(Span::styled("Esc", Style::default().fg(Color::Cyan)));
            parts.push(Span::raw(" back"));
            parts.push(Span::raw("  "));
            parts.push(Span::styled("j/k", Style::default().fg(Color::Cyan)));
            parts.push(Span::raw(" navigate"));
            if app.readme_expanded {
                parts.push(Span::raw("  "));
                parts.push(Span::styled("Esc", Style::default().fg(Color::Cyan)));
                parts.push(Span::raw(" collapse"));
            } else {
                parts.push(Span::raw("  "));
                parts.push(Span::styled("R", Style::default().fg(Color::Yellow)));
                parts.push(Span::raw(" fullscreen"));
            }
        }
        crate::tui::app::ModelsMode::List => {
            if app.active_panel == crate::tui::app::ActivePanel::Profiles {
                parts.push(Span::styled("PROFILES", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
                parts.push(Span::raw("  "));
                parts.push(Span::styled("j/k", Style::default().fg(Color::Cyan)));
                parts.push(Span::raw(" nav  "));
                parts.push(Span::styled("Enter", Style::default().fg(Color::Cyan)));
                parts.push(Span::raw(" apply  "));
                parts.push(Span::styled("s", Style::default().fg(Color::Cyan)));
                parts.push(Span::raw(" save  "));
                parts.push(Span::styled("Esc", Style::default().fg(Color::Cyan)));
                parts.push(Span::raw(" done"));
            } else if app.active_panel == crate::tui::app::ActivePanel::SystemPromptPresets {
                parts.push(Span::styled("PROMPTS", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
                parts.push(Span::raw("  "));
                parts.push(Span::styled("j/k", Style::default().fg(Color::Cyan)));
                parts.push(Span::raw(" nav  "));
                parts.push(Span::styled("Enter", Style::default().fg(Color::Cyan)));
                parts.push(Span::raw(" apply  "));
                parts.push(Span::styled("e", Style::default().fg(Color::Cyan)));
                parts.push(Span::raw(" edit  "));
                parts.push(Span::styled("n", Style::default().fg(Color::Cyan)));
                parts.push(Span::raw(" new  "));
                parts.push(Span::styled("Esc", Style::default().fg(Color::Cyan)));
                parts.push(Span::raw(" done"));
            } else {
                let panel_label = match app.active_panel {
                    crate::tui::app::ActivePanel::Models => "MODELS",
                    crate::tui::app::ActivePanel::Log => "LOG",
                    crate::tui::app::ActivePanel::Downloads => "DOWNLOADS",
                    crate::tui::app::ActivePanel::ServerSettings => "SERVER SETTINGS",
                    crate::tui::app::ActivePanel::LlmSettings => "LLM SETTINGS",
                    crate::tui::app::ActivePanel::SearchReadme => "README",
                    crate::tui::app::ActivePanel::Profiles => unreachable!(),
                    crate::tui::app::ActivePanel::SystemPromptPresets => unreachable!(),
                };
                parts.push(Span::styled(panel_label, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
                parts.push(Span::raw("  "));
                if app.active_panel == crate::tui::app::ActivePanel::Downloads {
                    parts.push(Span::styled("j/k", Style::default().fg(Color::Cyan)));
                    parts.push(Span::raw(" nav  "));
                    parts.push(Span::styled("c", Style::default().fg(Color::Cyan)));
                    parts.push(Span::raw(" cancel  "));
                }
                if app.active_panel == crate::tui::app::ActivePanel::ServerSettings {
                    parts.push(Span::styled("Enter", Style::default().fg(Color::Yellow)));
                    parts.push(Span::raw(" toggle  "));
                }
                if app.active_panel == crate::tui::app::ActivePanel::LlmSettings {
                    parts.push(Span::styled("Ctrl+S", Style::default().fg(Color::Yellow)));
                    parts.push(Span::raw(" save  "));
                    if app.is_settings_dirty() {
                        parts.push(Span::styled("*unsaved*", Style::default().fg(Color::Red)));
                    }
                }
                parts.push(Span::styled("Tab", Style::default().fg(Color::Cyan)));
                parts.push(Span::raw(" panels  "));
            }
            if app.active_panel == crate::tui::app::ActivePanel::Log {
                if app.log_expanded {
                    parts.push(Span::styled("Esc", Style::default().fg(Color::Cyan)));
                    parts.push(Span::raw(" collapse  "));
                } else {
                    parts.push(Span::styled("Enter", Style::default().fg(Color::Cyan)));
                    parts.push(Span::raw(" expand  "));
                }
            }
            if app.active_panel == crate::tui::app::ActivePanel::SearchReadme {
                if app.readme_expanded {
                    parts.push(Span::styled("Esc", Style::default().fg(Color::Cyan)));
                    parts.push(Span::raw(" collapse  "));
                } else {
                    parts.push(Span::styled("Enter", Style::default().fg(Color::Cyan)));
                    parts.push(Span::raw(" expand  "));
                }
            }
            parts.push(Span::styled("/", Style::default().fg(Color::Yellow)));
            parts.push(Span::raw(" search  "));
            parts.push(Span::styled("j/k", Style::default().fg(Color::Cyan)));
            parts.push(Span::raw(" nav  "));
            parts.push(Span::styled("l", Style::default().fg(Color::Yellow)));
            parts.push(Span::raw(" load  "));
            parts.push(Span::styled("u", Style::default().fg(Color::Yellow)));
            parts.push(Span::raw(" unload  "));
            parts.push(Span::styled("g/G", Style::default().fg(Color::Cyan)));
            parts.push(Span::raw(" log  "));
            parts.push(Span::styled("Ctrl+H", Style::default().fg(Color::Cyan)));
            parts.push(Span::raw(" help  "));
            parts.push(Span::styled("p", Style::default().fg(Color::Yellow)));
            parts.push(Span::raw(" profiles"));
        }
    }

    Line::from(parts)
}
pub fn render(f: &mut Frame, area: Rect, app: &mut App) {
    if area.height < 3 {
        return;
    }

    let chunks = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            ratatui::layout::Constraint::Fill(1),
            ratatui::layout::Constraint::Length(1),
        ])
        .split(area);

    let log_area = chunks[0];
    let status_area = chunks[1];

    let title = " Log ";
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
        .scroll((app.log_scroll_offset, 0));


    f.render_widget(paragraph, log_area);

    let shortcut = render_status_bar(app);
    f.render_widget(Paragraph::new(shortcut), status_area);

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
