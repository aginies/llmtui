use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::tui::colors::*;
use crate::tui::settings::{self, SettingField};

/// Render the LLM Settings panel.
#[allow(clippy::too_many_arguments)]
pub fn render_all(
    app: &mut crate::tui::app::App,
    area: Rect,
    disabled: bool,
) -> (Vec<Line<'static>>, usize, usize, usize, Option<String>) {
    let settings = &app.settings;
    let cached = &app.model_settings_cache;
    let selected = app.settings_state.settings_selected_idx;

    let edit_buf = &app.settings_state.settings_edit_buffer;
    let editing = !edit_buf.is_empty();
    let hash = app.settings_fingerprint();

    let (lines_to_return, final_total_count, selected_content_line) = if let Some(c) =
        &app.settings_state.settings_render_cache
        && c.hash == hash
        && c.selected == selected
    {
        (c.lines.clone(), c.lines.len(), c.selected_content_line)
    } else {
        let mut lines = Vec::new();
        let mut total_count = 0;
        let mut selected_line_idx = 0;
        let mut selected_content_line = 0;
        let fields = settings::filtered_fields(app.settings_state.expert_mode);
        render_settings(
            &mut lines,
            &mut total_count,
            &mut selected_line_idx,
            &mut selected_content_line,
            &fields,
            settings,
            cached,
            selected,
            edit_buf,
            editing,
            disabled,
        );
        app.settings_state.settings_render_cache = Some(crate::tui::app::SettingsRenderCache {
            hash,
            selected,
            lines: lines.clone(),
            selected_content_line,
        });
        (lines, total_count, selected_content_line)
    };

    let settings_height = lines_to_return.len();

    // Scroll clamp (always executes)
    let available_height = area.height.saturating_sub(2);
    if selected_content_line < app.settings_state.settings_scroll_offset {
        app.settings_state.settings_scroll_offset = selected_content_line;
    } else if available_height > 0
        && (selected_content_line - app.settings_state.settings_scroll_offset)
            >= (available_height as usize)
    {
        app.settings_state.settings_scroll_offset = (selected_content_line)
            .saturating_sub(available_height as usize)
            .saturating_add(1);
    }
    let max_offset = settings_height.saturating_sub(available_height as usize);
    if app.settings_state.settings_scroll_offset > max_offset {
        app.settings_state.settings_scroll_offset = max_offset;
    }

    // Build help text line if visible
    let help_line = if app.settings_state.help_visible && !editing {
        let fields = settings::filtered_fields(app.settings_state.expert_mode);
        if let Some(field) = fields.get(selected) {
            let help = crate::tui::i18n::field_help(field.id);
            if !help.is_empty() { Some(help) } else { None }
        } else {
            None
        }
    } else {
        None
    };

    (
        lines_to_return,
        final_total_count,
        settings_height,
        selected_content_line,
        help_line,
    )
}

#[allow(clippy::too_many_arguments)]
fn render_settings(
    lines: &mut Vec<Line<'static>>,
    total_count: &mut usize,
    selected_line_idx: &mut usize,
    selected_content_line: &mut usize,
    fields: &[SettingField],
    settings: &crate::models::ModelSettings,
    cached: &crate::models::ModelSettings,
    selected: usize,
    edit_buf: &str,
    editing: bool,
    disabled: bool,
) {
    let mut prev_section: Option<&str> = None;

    for field in fields {
        // Section header
        if field.is_new_section(prev_section) {
            let section_style = if disabled {
                Style::default()
                    .fg(YELLOW)
                    .add_modifier(Modifier::DIM)
            } else {
                Style::default()
                    .fg(YELLOW)
                    .add_modifier(Modifier::BOLD)
            };
            lines.push(Line::from(vec![Span::styled(
                format!("--- {} ---", field.section),
                section_style,
            )]));
            prev_section = Some(field.section);
        }

        if *total_count == selected {
            *selected_line_idx = lines.len();
            *selected_content_line = lines.len();
        }

        let dirty = field.is_dirty(settings, cached);
        let field_enabled = field.is_enabled.is_none_or(|f| f(settings));
        let visually_disabled = disabled || !field_enabled;
        let display: String = if editing && *total_count == selected {
            edit_buf.to_string()
        } else if dirty {
            format!("{}*", field.display(settings))
        } else {
            field.display(settings)
        };

        let name_style = if visually_disabled {
            Style::default().fg(GRAY)
        } else if dirty {
            Style::default().fg(RED)
        } else if field.is_expert {
            Style::default()
                .fg(YELLOW)
                .add_modifier(Modifier::DIM)
        } else {
            Style::default().fg(YELLOW)
        };
        let indicator_style = if visually_disabled {
            Style::default().fg(GRAY)
        } else {
            Style::default().fg(YELLOW)
        };
        let final_val_style = if *total_count == selected {
            Style::default()
                .fg(BLACK)
                .bg(YELLOW)
                .add_modifier(Modifier::BOLD)
        } else if visually_disabled {
            Style::default().fg(GRAY)
        } else if dirty {
            Style::default().fg(RED)
        } else {
            Style::default().fg(WHITE)
        };

        lines.push(Line::from(vec![
            Span::styled(
                if *total_count == selected { "> " } else { "  " },
                indicator_style,
            ),
            Span::styled(format!("{}: ", field.name()), name_style),
            Span::styled(display, final_val_style),
        ]));
        *total_count += 1;
    }
}
