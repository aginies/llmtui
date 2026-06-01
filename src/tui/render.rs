use ratatui::{
    Frame,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
};

use crate::tui::app::{ActivePanel, App, ModelsMode};
use crate::tui::panel;

mod hints;
mod overlays;
mod status;

fn render_scrollbar(f: &mut Frame, area: ratatui::layout::Rect, total_items: usize, scroll_offset: usize) {
    let scrollbar_area = ratatui::layout::Rect {
        x: area.right().saturating_sub(1),
        y: area.top(),
        width: 1,
        height: area.height,
    };
    let mut scrollbar_state = ScrollbarState::new(total_items).position(scroll_offset);
    f.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓")),
        scrollbar_area,
        &mut scrollbar_state,
    );
}

pub fn render(f: &mut Frame, app: &mut App) {
    if overlays::render_overlays(f, app) {
        return;
    }

    let is_search = matches!(app.models_mode, ModelsMode::Search { .. });
    let active_model_visible = app.is_panel_visible(4) && !is_search;
    let log_visible = app.is_panel_visible(5);

    let chunks = if app.log.log_expanded {
        ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .margin(0)
            .constraints([
                ratatui::layout::Constraint::Length(1),
                ratatui::layout::Constraint::Fill(1),
            ])
            .split(f.area())
    } else {
        let active_model_constraint = if active_model_visible {
            ratatui::layout::Constraint::Length(6)
        } else {
            ratatui::layout::Constraint::Length(0)
        };

        let bottom_constraint = if log_visible && !active_model_visible {
            ratatui::layout::Constraint::Fill(1)
        } else {
            let mut h = 0;
            if log_visible {
                h += 3;
            }
            if app.download.downloading {
                h += 7;
            }

            if h > 0 {
                ratatui::layout::Constraint::Min(h)
            } else {
                ratatui::layout::Constraint::Length(0)
            }
        };

        ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .margin(0)
            .constraints([
                ratatui::layout::Constraint::Length(1),
                ratatui::layout::Constraint::Fill(1),
                active_model_constraint,
                bottom_constraint,
            ])
            .split(f.area())
    };

    let status = status::render_status_bar(app, chunks[0]);
    f.render_widget(Paragraph::new(status), chunks[0]);

    if app.log.log_expanded {
        let log_area = chunks[1];
        panel::log::render(f, log_area, app);
        return;
    }

    let top_chunks = if !app.is_panel_visible(1)
        && !app.is_panel_visible(3)
        && !matches!(
            app.ui.active_panel,
            ActivePanel::Profiles | ActivePanel::SystemPromptPresets | ActivePanel::SearchReadme
        ) {
        ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([
                ratatui::layout::Constraint::Percentage(100),
                ratatui::layout::Constraint::Length(0),
            ])
            .split(chunks[1])
    } else {
        let left_pct = app.ui.left_pct.max(20).min(80);
        ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([
                ratatui::layout::Constraint::Fill(left_pct),
                ratatui::layout::Constraint::Fill(100 - left_pct),
            ])
            .split(chunks[1])
    };

    let info_visible = app.is_panel_visible(2);
    let (left_chunks, info_lines) = if info_visible {
        let lines = panel::tabbed::get_info_lines(app, top_chunks[0].width);
        let info_height = (lines.len() as u16 + 2).max(3);
        let chunks = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                ratatui::layout::Constraint::Min(5),
                ratatui::layout::Constraint::Length(info_height),
            ])
            .split(top_chunks[0]);
        (chunks, Some(lines))
    } else {
        let chunks = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([ratatui::layout::Constraint::Fill(1)])
            .split(top_chunks[0]);
        (chunks, None)
    };

    panel::models::render(f, left_chunks[0], app);

    if let Some(lines) = info_lines {
        panel::tabbed::render_info_with_lines(f, left_chunks[1], lines);
    }

    match app.ui.active_panel {
        ActivePanel::Profiles => {
            let all_profiles = app.config.merged_profiles();
            let (profile_lines, count) = panel::profiles::render_all(
                &all_profiles,
                app.settings_state.settings_selected_idx,
                &app.settings,
            );
            if app.settings_state.settings_selected_idx >= count {
                app.settings_state.settings_selected_idx = count.saturating_sub(1);
            }

            let area = top_chunks[1];
            let available_height = area.height.saturating_sub(2);

            let max_offset = profile_lines
                .len()
                .saturating_sub(available_height as usize) as u16;
            if app.picker.profiles_scroll_offset > max_offset.into() {
                app.picker.profiles_scroll_offset = max_offset.into();
            }

            let start_idx = app.picker.profiles_scroll_offset;
            let visible_lines: Vec<Line> = profile_lines
                .iter()
                .skip(start_idx)
                .take(available_height as usize)
                .cloned()
                .collect();

            let block = Block::default()
                .title(" Profiles ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow));
            let paragraph = Paragraph::new(visible_lines).block(block);
            f.render_widget(paragraph, area);

            if profile_lines.len() > available_height as usize {
                render_scrollbar(f, area, profile_lines.len(), app.picker.profiles_scroll_offset);
            }
        }
        ActivePanel::SystemPromptPresets => {
            let presets = app.config.merged_presets();
            let preset_lines = panel::system_prompt_presets::render_all(
                &presets,
                app.settings_state.settings_selected_idx,
                app.edit.editing_preset.is_some(),
                &app.settings_state.settings_edit_buffer,
                app.edit.edit_cursor_pos,
            );

            let area = top_chunks[1];
            let available_height = area.height.saturating_sub(2);

            let max_offset = preset_lines.len().saturating_sub(available_height as usize) as u16;
            if app.picker.system_prompt_presets_scroll_offset > max_offset.into() {
                app.picker.system_prompt_presets_scroll_offset = max_offset.into();
            }

            let start_idx = app.picker.system_prompt_presets_scroll_offset;
            let visible_lines: Vec<Line> = preset_lines
                .iter()
                .skip(start_idx)
                .take(available_height as usize)
                .cloned()
                .collect();

            let block = Block::default()
                .title(" System Prompt Presets ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow));
            let paragraph = Paragraph::new(visible_lines).block(block);
            f.render_widget(paragraph, area);

            if preset_lines.len() > available_height as usize {
                render_scrollbar(f, area, preset_lines.len(), app.picker.system_prompt_presets_scroll_offset);
            }
        }
        _ => {
            let show_readme = match &app.models_mode {
                ModelsMode::Search { show_readme, .. } => *show_readme,
                ModelsMode::Files { .. } => true,
                _ => false,
            };
            if show_readme {
                panel::readme::render(f, top_chunks[1], app);
            } else {
                let server_visible = app.is_panel_visible(1);
                let llm_visible = app.is_panel_visible(3);
                if server_visible && llm_visible {
                    panel::tabbed::render_settings_only(f, top_chunks[1], app);
                } else if server_visible {
                    panel::tabbed::render_server_only(f, top_chunks[1], app);
                } else if llm_visible {
                    panel::tabbed::render_llm_only(f, top_chunks[1], app);
                }
            }
        }
    }

    if active_model_visible {
        panel::active::render(f, chunks[2], app);
    }

    let bottom_area = chunks[3];
    if log_visible && app.download.downloading {
        let bottom_chunks = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                ratatui::layout::Constraint::Fill(1),
                ratatui::layout::Constraint::Length(7),
            ])
            .split(bottom_area);

        let downloads_focused = app.ui.active_panel == ActivePanel::Downloads;

        panel::log::render(f, bottom_chunks[0], app);

        let total_speed: f64 = app
            .download
            .download_progress
            .iter()
            .map(|d| d.bytes_per_second)
            .sum();
        panel::models::render_download_panel(
            f,
            bottom_chunks[1],
            &app.download.download_progress,
            total_speed,
            &mut app.download.download_scroll_state,
            downloads_focused,
        );
    } else if log_visible {
        panel::log::render(f, bottom_area, app);
    } else if app.download.downloading {
        let total_speed: f64 = app
            .download
            .download_progress
            .iter()
            .map(|d| d.bytes_per_second)
            .sum();
        let downloads_focused = app.ui.active_panel == ActivePanel::Downloads;
        panel::models::render_download_panel(
            f,
            bottom_area,
            &app.download.download_progress,
            total_speed,
            &mut app.download.download_scroll_state,
            downloads_focused,
        );
    }
}
