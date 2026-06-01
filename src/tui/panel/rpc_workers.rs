use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use crate::config::RpcWorker;

pub fn render_all<'a>(
    workers: &'a [RpcWorker],
    selected: usize,
    editing: bool,
    edit_content: &str,
    edit_cursor_pos: usize,
) -> Vec<Line<'a>> {
    let mut lines = Vec::new();

    if editing {
        // ── Edit mode ──────────────────────────────────────────
        lines.push(Line::from(vec![
            Span::styled(
                "Editing RPC Worker",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                " — Format: [Name], IP, Port",
                Style::default().fg(Color::DarkGray),
            ),
        ]));
        lines.push(Line::from(""));

        let mut spans = Vec::new();
        for (j, ch) in edit_content.chars().enumerate() {
            if j == edit_cursor_pos {
                spans.push(Span::styled(
                    ch.to_string(),
                    Style::default().fg(Color::Black).bg(Color::Yellow),
                ));
            } else {
                spans.push(Span::raw(ch.to_string()));
            }
        }
        if edit_cursor_pos == edit_content.chars().count() {
            spans.push(Span::styled(
                "_",
                Style::default().fg(Color::Black).bg(Color::Yellow),
            ));
        }
        lines.push(Line::from(spans));

        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            "[Enter] Save  [Esc] Cancel",
            Style::default().fg(Color::Cyan),
        )]));
    } else {
        // ── List mode ──────────────────────────────────────────
        lines.push(Line::from(vec![
            Span::styled(
                "RPC Workers",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                " — Space: Toggle | n: New | e: Edit | d: Delete",
                Style::default().fg(Color::DarkGray),
            ),
        ]));
        lines.push(Line::from(""));

        for (i, worker) in workers.iter().enumerate() {
            let marker = if i == selected { "> " } else { "  " };
            let checkbox = if worker.selected { "[x] " } else { "[ ] " };

            let row_style = if i == selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let name_display = if worker.name.is_empty() {
                "(no name)"
            } else {
                &worker.name
            };

            lines.push(Line::from(vec![
                Span::styled(marker, Style::default().fg(Color::Yellow)),
                Span::styled(
                    checkbox,
                    Style::default().fg(if worker.selected {
                        Color::Green
                    } else {
                        Color::DarkGray
                    }),
                ),
                Span::styled(
                    format!("{:<15} | {}:{}", name_display, worker.ip, worker.port),
                    row_style,
                ),
            ]));
        }

        if workers.is_empty() {
            lines.push(Line::from(Span::styled(
                "  No RPC workers configured.",
                Style::default().fg(Color::DarkGray),
            )));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            "[Space] Toggle  [e] Edit  [n] New  [d] Delete  [Esc] Back",
            Style::default().fg(Color::Cyan),
        )]));
    }

    lines
}
