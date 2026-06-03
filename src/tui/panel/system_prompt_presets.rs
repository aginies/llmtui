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
            lines.push(Line::from(vec![Span::styled(
                format!("Editing: {}", presets[selected].name),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )]));
            lines.push(Line::from(""));
        } else {
            lines.push(Line::from(vec![Span::styled(
                "Creating new preset",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )]));
            lines.push(Line::from(""));
        }

        // Show the content as lines, with cursor
        let mut current_char_idx = 0;
        let content_lines: Vec<&str> = edit_content.split('\n').collect();

        for line in content_lines.iter() {
            let mut spans = Vec::new();
            let line_chars: Vec<char> = line.chars().collect();

            if line_chars.is_empty() && current_char_idx == edit_cursor_pos {
                spans.push(Span::styled(
                    "_",
                    Style::default().fg(Color::Black).bg(Color::Yellow),
                ));
            } else {
                for &ch in line_chars.iter() {
                    if current_char_idx == edit_cursor_pos {
                        spans.push(Span::styled(
                            ch.to_string(),
                            Style::default().fg(Color::Black).bg(Color::Yellow),
                        ));
                    } else {
                        spans.push(Span::raw(ch.to_string()));
                    }
                    current_char_idx += 1;
                }

                // If cursor is at the end of this line
                if current_char_idx == edit_cursor_pos {
                    spans.push(Span::styled(
                        "_",
                        Style::default().fg(Color::Black).bg(Color::Yellow),
                    ));
                }
            }

            lines.push(Line::from(spans));
            current_char_idx += 1; // for the newline char
        }

        // Special case: if content ends with a newline, we might need an extra line
        if edit_content.ends_with('\n') && current_char_idx - 1 == edit_cursor_pos {
            lines.push(Line::from(Span::styled(
                "_",
                Style::default().fg(Color::Black).bg(Color::Yellow),
            )));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            "[Enter] new line  [Esc] cancel  [^S] save",
            Style::default().fg(Color::Cyan),
        )]));
    } else {
        // ── List mode ──────────────────────────────────────────
        lines.push(Line::from(vec![
            Span::styled(
                "System Prompt Presets",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                " — Select a preset to apply",
                Style::default().fg(Color::DarkGray),
            ),
        ]));
        lines.push(Line::from(""));

        for (i, preset) in presets.iter().enumerate() {
            let marker = if i == selected { "> " } else { "  " };
            let name_style = if i == selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
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

        lines.push(Line::from(vec![Span::styled(
            "[Enter] apply  [e] edit  [n] new  [d] delete  [Esc] cancel",
            Style::default().fg(Color::Cyan),
        )]));
    }

    lines
}
