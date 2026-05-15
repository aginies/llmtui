use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use crate::config::SystemPromptPreset;

pub fn render_all<'a>(
    presets: &'a [SystemPromptPreset],
    selected: usize,
    editing: bool,
    edit_content: &str,
    edit_cursor_pos: usize,
) -> Vec<Line<'a>> {
    let mut lines = Vec::new();

    if editing {
        // ── Edit mode ──────────────────────────────────────────
        if selected < presets.len() {
            lines.push(Line::from(vec![
                Span::styled(format!("Editing: {}", presets[selected].name), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            ]));
            lines.push(Line::from(""));
        } else {
            lines.push(Line::from(vec![
                Span::styled("Creating new preset", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            ]));
            lines.push(Line::from(""));
        }

        // Show the content as lines, with cursor
        let content_lines: Vec<&str> = edit_content.split('\n').collect();
        for (i, line) in content_lines.iter().enumerate() {
            let mut spans = Vec::new();
            for (j, ch) in line.chars().enumerate() {
                if i == 0 && j == edit_cursor_pos {
                    spans.push(Span::styled(ch.to_string(), Style::default().fg(Color::Black).bg(Color::Yellow)));
                } else if i == 0 && j == edit_cursor_pos - 1 {
                    // cursor is between chars
                    spans.push(Span::styled(ch.to_string(), Style::default().fg(Color::Black).bg(Color::Yellow)));
                } else {
                    spans.push(Span::raw(ch.to_string()));
                }
            }
            if i == 0 && edit_cursor_pos == line.chars().count() {
                // cursor at end of line
                spans.push(Span::styled("_", Style::default().fg(Color::Black).bg(Color::Yellow)));
            }
            if !spans.is_empty() {
                lines.push(Line::from(spans));
            } else {
                if i == 0 && edit_cursor_pos == 0 {
                    lines.push(Line::from(Span::styled("_", Style::default().fg(Color::Black).bg(Color::Yellow))));
                } else {
                    lines.push(Line::from(""));
                }
            }
        }

        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("[Enter] new line  [Esc] cancel  [Ctrl+S] save", Style::default().fg(Color::Cyan)),
        ]));
    } else {
        // ── List mode ──────────────────────────────────────────
        lines.push(Line::from(vec![
            Span::styled("System Prompt Presets", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(" — Select a preset to apply", Style::default().fg(Color::DarkGray)),
        ]));
        lines.push(Line::from(""));

        for (i, preset) in presets.iter().enumerate() {
            let marker = if i == selected { "> " } else { "  " };
            let name_style = if i == selected {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            lines.push(Line::from(vec![
                Span::styled(marker, Style::default().fg(Color::Yellow)),
                Span::styled(&preset.name, name_style),
            ]));
            lines.push(Line::from(vec![
                Span::raw("    "),
                Span::styled(&preset.description, Style::default().fg(Color::DarkGray)),
            ]));
            lines.push(Line::from(""));
        }

        lines.push(Line::from(vec![
            Span::styled("[Enter] apply  [e] edit  [n] new  [d] delete  [Esc] cancel", Style::default().fg(Color::Cyan)),
        ]));
    }

    lines
}
