use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use crate::config::Profile;

pub fn render_all<'a>(
    profiles: &'a [Profile],
    selected: usize,
    current_settings: &crate::models::ModelSettings,
) -> (Vec<Line<'a>>, usize) {
    let mut lines = Vec::new();

    lines.push(Line::from(vec![
        Span::styled("Profiles", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::styled(" — Select a profile to apply its settings", Style::default().fg(Color::DarkGray)),
    ]));
    lines.push(Line::from(""));

    for (i, profile) in profiles.iter().enumerate() {
        let marker = if i == selected { "> " } else { "  " };
        let name_style = if i == selected {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        lines.push(Line::from(vec![
            Span::styled(marker, Style::default().fg(Color::Yellow)),
            Span::styled(&profile.name, name_style),
        ]));

        // Show key settings that differ from defaults
        let parts = profile_settings_parts(profile, current_settings);
        if !parts.is_empty() {
            for part in parts {
                lines.push(Line::from(vec![
                    Span::raw("    "),
                    Span::styled(part, Style::default().fg(Color::DarkGray)),
                ]));
            }
        }

        lines.push(Line::from(""));
    }

    lines.push(Line::from(vec![
        Span::styled("[Enter] Apply  ", Style::default().fg(Color::Cyan)),
        Span::styled("[s] Save  ", Style::default().fg(Color::Cyan)),
        Span::styled("[d] Delete  ", Style::default().fg(Color::Cyan)),
        Span::styled("[Esc] Cancel", Style::default().fg(Color::Cyan)),
    ]));

    (lines, profiles.len())
}

/// Build a short description of the key settings that differ from the current settings.
fn profile_settings_parts(profile: &Profile, current: &crate::models::ModelSettings) -> Vec<String> {
    let mut parts = Vec::new();

    let temp = profile.settings.temperature.unwrap_or(current.temperature);
    let top_p = profile.settings.top_p.unwrap_or(current.top_p);
    let top_k = profile.settings.top_k.unwrap_or(current.top_k);
    let context = profile.settings.context_length.unwrap_or(current.context_length);
    let repeat = profile.settings.repeat_penalty.unwrap_or(current.repeat_penalty);
    let min_p = profile.settings.min_p.unwrap_or(current.min_p);
    let typical_p = profile.settings.typical_p.unwrap_or(current.typical_p);

    if temp != current.temperature {
        parts.push(format!("temp={:.2}", temp));
    }
    if top_p != current.top_p {
        parts.push(format!("top_p={:.2}", top_p));
    }
    if top_k != current.top_k {
        parts.push(format!("top_k={}", top_k));
    }
    if context != current.context_length {
        parts.push(format!("ctx={}", context));
    }
    if repeat != current.repeat_penalty {
        parts.push(format!("rep={:.2}", repeat));
    }
    if min_p != current.min_p {
        parts.push(format!("min_p={:.2}", min_p));
    }
    if typical_p != current.typical_p {
        parts.push(format!("typical_p={:.2}", typical_p));
    }

    parts
}
