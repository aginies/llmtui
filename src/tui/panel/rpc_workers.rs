use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::config::RpcWorker;
use crate::tui::colors::*;

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
                    .fg(YELLOW)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                " — Format: [Name], IP, Port",
                Style::default().fg(DARK_GRAY),
            ),
        ]));
        lines.push(Line::from(""));

        let mut spans = Vec::new();
        if let Some(c) = edit_content.chars().nth(edit_cursor_pos) {
            let before: String = edit_content.chars().take(edit_cursor_pos).collect();
            let after: String = edit_content.chars().skip(edit_cursor_pos + 1).collect();

            spans.push(Span::raw(before));
            spans.push(           Span::styled(
                c.to_string(),
                Style::default().fg(BLACK).bg(YELLOW),
            ));
            spans.push(Span::raw(after));
        } else {
            spans.push(Span::raw(edit_content.to_string()));
        }
        if edit_cursor_pos == edit_content.chars().count() {
            spans.push(Span::styled(
                "_",
                Style::default().fg(BLACK).bg(YELLOW),
            ));
        }
        lines.push(Line::from(spans));

        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            "[↵] Save  [⎋] Cancel",
            Style::default().fg(CYAN),
        )]));
    } else {
        // ── List mode ──────────────────────────────────────────
        lines.push(Line::from(vec![
            Span::styled(
                "RPC Workers",
                Style::default()
                    .fg(YELLOW)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                " — Space: Toggle | n: New | e: Edit | d: Delete",
                Style::default().fg(DARK_GRAY),
            ),
        ]));
        lines.push(Line::from(""));

        for (i, worker) in workers.iter().enumerate() {
            let marker = if i == selected { "> " } else { "  " };
            let checkbox = if worker.selected { "[x] " } else { "[ ] " };

            let row_style = if i == selected {
                Style::default()
                    .fg(BLACK)
                    .bg(GREEN)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(WHITE)
            };

            let name_display = if worker.name.is_empty() {
                "(no name)"
            } else {
                &worker.name
            };

            lines.push(Line::from(vec![
                Span::styled(marker, Style::default().fg(YELLOW)),
                Span::styled(
                    checkbox,
                    Style::default().fg(if worker.selected {
                        GREEN
                    } else {
                        DARK_GRAY
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
                Style::default().fg(DARK_GRAY),
            )));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            "[Space] Toggle  [e] Edit  [n] New  [d] Delete  [⎋] Back",
            Style::default().fg(CYAN),
        )]));
    }

    lines
}
