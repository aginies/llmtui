use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::config::Profile;
use crate::tui::colors::*;
use crate::tui::settings::profile_settings_parts;

pub fn render_all<'a>(
    profiles: &'a [Profile],
    selected: usize,
    current_settings: &crate::models::ModelSettings,
) -> (Vec<Line<'a>>, usize) {
    let mut lines = Vec::new();

    lines.push(Line::from(vec![
        Span::styled(
            "Profiles",
            Style::default()
                .fg(YELLOW)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            " — Select a profile to apply its settings",
            Style::default().fg(DARK_GRAY),
        ),
    ]));
    lines.push(Line::from(""));

    for (i, profile) in profiles.iter().enumerate() {
        let marker = if i == selected { "> " } else { "  " };
        let name_style = if i == selected {
            Style::default()
                .fg(BLACK)
                .bg(GREEN)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(WHITE)
        };
        lines.push(Line::from(vec![
            Span::styled(marker, Style::default().fg(YELLOW)),
            Span::styled(&profile.name, name_style),
        ]));

        // Show key settings that differ from defaults
        let parts = profile_settings_parts(profile, current_settings);
        if !parts.is_empty() {
            for part in parts {
                lines.push(Line::from(vec![
                    Span::raw("    "),
                    Span::styled(part, Style::default().fg(DARK_GRAY)),
                ]));
            }
        }

        lines.push(Line::from(""));
    }

    lines.push(Line::from(vec![
        Span::styled("[↵] Apply  ", Style::default().fg(CYAN)),
        Span::styled("[d] Delete  ", Style::default().fg(CYAN)),
        Span::styled("[⎋] Cancel", Style::default().fg(CYAN)),
    ]));

    (lines, profiles.len())
}
