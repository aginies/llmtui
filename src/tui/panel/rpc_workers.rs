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
    editing_ts: bool,
    ts_buffer: &str,
    ts_cursor_pos: usize,
) -> Vec<Line<'a>> {
    let mut lines = Vec::new();

    if editing_ts {
        // ── Tensor Split editor ──────────────────────────────
        lines.push(Line::from(vec![
            Span::styled(
                "Editing Tensor Split",
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" — Value (default: 1)", Style::default().fg(DIM_GRAY)),
        ]));
        lines.push(Line::from(""));

        let mut spans = Vec::new();
        if let Some(c) = ts_buffer.chars().nth(ts_cursor_pos) {
            let before: String = ts_buffer.chars().take(ts_cursor_pos).collect();
            let after: String = ts_buffer.chars().skip(ts_cursor_pos + 1).collect();

            spans.push(Span::raw(before));
            spans.push(Span::styled(
                c.to_string(),
                Style::default().fg(BLACK).bg(ACCENT),
            ));
            spans.push(Span::raw(after));
        } else {
            spans.push(Span::raw(ts_buffer.to_string()));
        }
        if ts_cursor_pos == ts_buffer.chars().count() {
            spans.push(Span::styled("_", Style::default().fg(BLACK).bg(ACCENT)));
        }
        lines.push(Line::from(spans));

        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            "[↵] Save  [⎋] Cancel",
            Style::default().fg(CYAN),
        )]));
    } else if editing {
        // ── Edit mode ──────────────────────────────────────────
        lines.push(Line::from(vec![
            Span::styled(
                "Editing RPC Worker",
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                " — Format: [Name], IP, Port, TensorSplit",
                Style::default().fg(DIM_GRAY),
            ),
        ]));
        lines.push(Line::from(""));

        let mut spans = Vec::new();
        if let Some(c) = edit_content.chars().nth(edit_cursor_pos) {
            let before: String = edit_content.chars().take(edit_cursor_pos).collect();
            let after: String = edit_content.chars().skip(edit_cursor_pos + 1).collect();

            spans.push(Span::raw(before));
            spans.push(Span::styled(
                c.to_string(),
                Style::default().fg(BLACK).bg(ACCENT),
            ));
            spans.push(Span::raw(after));
        } else {
            spans.push(Span::raw(edit_content.to_string()));
        }
        if edit_cursor_pos == edit_content.chars().count() {
            spans.push(Span::styled("_", Style::default().fg(BLACK).bg(ACCENT)));
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
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                " — Space: Toggle | n: New | e: Edit | d: Delete | ts: Tensor Split",
                Style::default().fg(DIM_GRAY),
            ),
        ]));
        lines.push(Line::from(""));

        for (i, worker) in workers.iter().enumerate() {
            let marker = if i == selected { "> " } else { "  " };
            let checkbox = if worker.selected { "[x] " } else { "[ ] " };

            let row_style = if i == selected {
                Style::default()
                    .fg(BLACK)
                    .bg(ACCENT)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(WHITE)
            };

            let name_display = if worker.name.is_empty() {
                "(no name)"
            } else {
                &worker.name
            };

            let ts_display = if worker.tensor_split.is_empty() || worker.tensor_split == "1" {
                "1"
            } else {
                &worker.tensor_split
            };
            lines.push(Line::from(vec![
                Span::styled(marker, Style::default().fg(ACCENT)),
                Span::styled(
                    checkbox,
                    Style::default().fg(if worker.selected { GREEN } else { DIM_GRAY }),
                ),
                Span::styled(
                    format!(
                        "{:<15} | {}:{} | ts:{}",
                        name_display, worker.ip, worker.port, ts_display
                    ),
                    row_style,
                ),
            ]));
        }

        if workers.is_empty() {
            lines.push(Line::from(Span::styled(
                "  No RPC workers configured.",
                Style::default().fg(DIM_GRAY),
            )));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            "[Space] Toggle  [e] Edit  [n] New  [d] Delete  [t] Tensor Split  [⎋] Back",
            Style::default().fg(CYAN),
        )]));
    }

    lines
}
