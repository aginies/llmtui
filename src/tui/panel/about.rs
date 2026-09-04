use crate::tui::colors::*;
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

pub fn render_about() -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let y = Style::default().fg(ACCENT).add_modifier(Modifier::BOLD);
    let c = Style::default().fg(CYAN);
    let gray = Style::default().fg(DIM_GRAY);

    lines.push(Line::from(vec![
        Span::styled(crate::t!("about.title"), y),
        Span::raw(" v"),
        Span::raw(env!("CARGO_PKG_VERSION")),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(crate::t!("about.description")));
    lines.push(Line::from(""));
    for author in env!("CARGO_PKG_AUTHORS").split(':') {
        let author = author.trim();
        if !author.is_empty() && author.contains('<') {
            if let Some(name) = author.split('<').next() {
                lines.push(Line::from(vec![
                    Span::styled(crate::t!("about.author"), gray),
                    Span::styled(name.trim(), c),
                ]));
            }
        } else if !author.is_empty() {
            lines.push(Line::from(vec![
                Span::styled(crate::t!("about.author"), gray),
                Span::styled(author.trim(), c),
            ]));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::raw(crate::t!("about.license")),
        Span::styled(
            crate::t!("about.license_name"),
            Style::default().fg(GREEN).add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(crate::t!("about.warranty1")));
    lines.push(Line::from(crate::t!("about.warranty2")));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(crate::t!("about.website"), gray),
        Span::styled("https://github.com/aginies/llmtui", c),
    ]));
    lines.push(Line::from(vec![
        Span::styled(crate::t!("about.license_link"), gray),
        Span::styled("https://www.gnu.org/licenses/gpl-3.0.html", c),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(crate::t!("about.tech")));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![Span::styled(
        crate::t!("about.close"),
        Style::default().fg(DIM_GRAY),
    )]));

    lines
}
