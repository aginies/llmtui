use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

pub fn render_about() -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let y = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let c = Style::default().fg(Color::Cyan);
    let gray = Style::default().fg(Color::DarkGray);

    lines.push(Line::from(vec![
        Span::styled("llm-manager", y),
        Span::raw(" v"),
        Span::raw(env!("CARGO_PKG_VERSION")),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(
        "A high-performance TUI for managing llama.cpp servers.",
    ));
    lines.push(Line::from(""));
    for author in env!("CARGO_PKG_AUTHORS").split(':') {
        let author = author.trim();
        if !author.is_empty() && author.contains('<') {
            if let Some(name) = author.split('<').next() {
                lines.push(Line::from(vec![
                    Span::styled("Author: ", gray),
                    Span::styled(name.trim(), c),
                ]));
            }
        } else if !author.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("Author: ", gray),
                Span::styled(author.trim(), c),
            ]));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::raw("License: "),
        Span::styled(
            "GNU GPLv3",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(
        "This is free software: you are free to change and redistribute it.",
    ));
    lines.push(Line::from(
        "There is NO WARRANTY, to the extent permitted by law.",
    ));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("Website & Source: ", gray),
        Span::styled("https://github.com/aginies/llmtui", c),
    ]));
    lines.push(Line::from(vec![
        Span::styled("License Link: ", gray),
        Span::styled("https://www.gnu.org/licenses/gpl-3.0.html", c),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from("Built with Rust, Ratatui, and Tokio."));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![Span::styled(
        "[⎋/Any] Close",
        Style::default().fg(Color::DarkGray),
    )]));

    lines
}
