use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Position, Rect};

use crate::tui::app::{ActivePanel, App};

pub fn handle_mouse(app: &mut App, mouse: MouseEvent, area: Rect) {
    let pos = Position::new(mouse.column, mouse.row);

    if app.log.log_expanded {
        let chunks = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                ratatui::layout::Constraint::Length(1), // status bar
                ratatui::layout::Constraint::Fill(1),   // log
            ])
            .split(area);

        if chunks[1].contains(pos) {
            match mouse.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    app.ui.active_panel = ActivePanel::Log;
                }
                MouseEventKind::ScrollUp => {
                    handle_log_scroll(app, true);
                }
                MouseEventKind::ScrollDown => {
                    handle_log_scroll(app, false);
                }
                _ => {}
            }
        }
        return;
    }

    // If actively resizing, continue updating even if mouse moved outside the border area
    if let Some(ref rs) = app.ui.resize_state {
        match mouse.kind {
            MouseEventKind::Drag(_) => {
                let dx = pos.x as i16 - rs.start_x as i16;
                let delta = (dx * 100 / rs.container.width as i16).clamp(-5, 5);
                app.ui.left_pct = (rs.start_pct as i16 + delta).clamp(20, 80) as u16;
            }
            MouseEventKind::Up(MouseButton::Left) => {
                app.ui.resize_state = None;
            }
            _ => {}
        }
        return;
    }

    // Default layout
    let chunks = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            ratatui::layout::Constraint::Length(1), // status bar
            ratatui::layout::Constraint::Fill(1),   // top panels
            ratatui::layout::Constraint::Length(5), // active model
            ratatui::layout::Constraint::Min(5),    // log
        ])
        .split(area);

    // 1. Check Log panel (and Downloads if downloading)
    if chunks[3].contains(pos) {
        // When downloading, check if we're in the downloads area (bottom 7 lines)
        if app.download.downloading {
            let bottom_chunks = ratatui::layout::Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .constraints([
                    ratatui::layout::Constraint::Fill(1),   // log
                    ratatui::layout::Constraint::Length(7), // downloads
                ])
                .split(chunks[3]);

            if bottom_chunks[1].contains(pos) {
                match mouse.kind {
                    MouseEventKind::Down(MouseButton::Left) => {
                        app.ui.active_panel = ActivePanel::Downloads;
                    }
                    MouseEventKind::ScrollUp => {
                        app.download.download_scroll_state.select_previous();
                    }
                    MouseEventKind::ScrollDown => {
                        app.download.download_scroll_state.select_next();
                    }
                    _ => {}
                }
                return;
            }
        }

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                app.ui.active_panel = ActivePanel::Log;
            }
            MouseEventKind::ScrollUp => {
                handle_log_scroll(app, true);
                app.ui.active_panel = ActivePanel::Log;
            }
            MouseEventKind::ScrollDown => {
                handle_log_scroll(app, false);
                app.ui.active_panel = ActivePanel::Log;
            }
            _ => {}
        }
        return;
    }

    // 2. Check Top panels
    if chunks[1].contains(pos) {
        let left_pct = app.ui.left_pct.clamp(20, 80);
        let top_chunks = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([
                ratatui::layout::Constraint::Fill(left_pct),
                ratatui::layout::Constraint::Fill(100 - left_pct),
            ])
            .split(chunks[1]);

        // Check for resize drag on the vertical border between left and right panels
        let border_x = top_chunks[0].right().saturating_sub(1);
        let border_y_start = chunks[1].top();
        let border_y_end = chunks[1].bottom().saturating_sub(1);
        let on_border = (pos.x as i16 - border_x as i16).abs() <= 2
            && pos.y as i16 >= border_y_start as i16
            && pos.y as i16 <= border_y_end as i16;

        if on_border {
            match mouse.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    app.ui.resize_state = Some(crate::tui::app::ResizeState {
                        start_x: pos.x,
                        start_pct: app.ui.left_pct,
                        container: chunks[1],
                    });
                }
                MouseEventKind::Drag(_) => {
                    if let Some(ref rs) = app.ui.resize_state {
                        let dx = pos.x as i16 - rs.start_x as i16;
                        let delta = (dx * 100 / rs.container.width as i16).clamp(-5, 5);
                        app.ui.left_pct = (rs.start_pct as i16 + delta).clamp(20, 80) as u16;
                    }
                }
                MouseEventKind::Up(MouseButton::Left) => {
                    app.ui.resize_state = None;
                }
                MouseEventKind::ScrollUp => {
                    app.ui.left_pct = app.ui.left_pct.saturating_sub(1).max(20);
                }
                MouseEventKind::ScrollDown => {
                    app.ui.left_pct = app.ui.left_pct.saturating_add(1).min(80);
                }
                _ => {}
            }
            return;
        }

        // Right side: Settings/Profiles
        if top_chunks[1].contains(pos) {
            match mouse.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    let server_running = app.server.server_handle.is_some();
                    app.ui.active_panel = match app.ui.active_panel {
                        ActivePanel::LlmSettings if !server_running => ActivePanel::ServerSettings,
                        ActivePanel::ServerSettings if !server_running => ActivePanel::LlmSettings,
                        ActivePanel::LlmSettings => ActivePanel::LlmSettings,
                        ActivePanel::ServerSettings => ActivePanel::LlmSettings,
                        ActivePanel::Profiles => ActivePanel::Profiles,
                        ActivePanel::SystemPromptPresets => ActivePanel::SystemPromptPresets,
                        ActivePanel::SearchReadme => ActivePanel::SearchReadme,
                        _ if !server_running => ActivePanel::ServerSettings,
                        _ => ActivePanel::LlmSettings,
                    };
                }
                MouseEventKind::ScrollUp => match app.ui.active_panel {
                    ActivePanel::LlmSettings => {
                        app.settings_state.settings_scroll_offset =
                            app.settings_state.settings_scroll_offset.saturating_sub(1);
                    }
                    ActivePanel::Profiles => {
                        app.picker.profiles_scroll_offset =
                            app.picker.profiles_scroll_offset.saturating_sub(1);
                    }
                    ActivePanel::SystemPromptPresets => {
                        app.picker.system_prompt_presets_scroll_offset = app
                            .picker
                            .system_prompt_presets_scroll_offset
                            .saturating_sub(1);
                    }
                    _ => {}
                },
                MouseEventKind::ScrollDown => match app.ui.active_panel {
                    ActivePanel::LlmSettings => {
                        app.settings_state.settings_scroll_offset =
                            app.settings_state.settings_scroll_offset.saturating_add(1);
                    }
                    ActivePanel::Profiles => {
                        app.picker.profiles_scroll_offset =
                            app.picker.profiles_scroll_offset.saturating_add(1);
                    }
                    ActivePanel::SystemPromptPresets => {
                        app.picker.system_prompt_presets_scroll_offset = app
                            .picker
                            .system_prompt_presets_scroll_offset
                            .saturating_add(1);
                    }
                    _ => {}
                },
                _ => {}
            }
            return;
        }

        // Left side: Models + Info
        if top_chunks[0].contains(pos) {
            let info_height = (crate::tui::panel::tabbed::get_info_lines(app, top_chunks[0].width)
                .len() as u16
                + 2)
            .max(3);
            let left_chunks = ratatui::layout::Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .constraints([
                    ratatui::layout::Constraint::Min(5),
                    ratatui::layout::Constraint::Length(info_height),
                ])
                .split(top_chunks[0]);

            if left_chunks[0].contains(pos)
                && let MouseEventKind::Down(MouseButton::Left) = mouse.kind
            {
                app.ui.active_panel = ActivePanel::Models;
            }
        }
    }

    // Handle downloads-only layout (when log is not visible but downloading)
    if app.download.downloading && !app.log.log_expanded {
        let bottom_area = chunks[3];
        let bottom_chunks = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                ratatui::layout::Constraint::Length(7), // downloads
            ])
            .split(bottom_area);

        if bottom_chunks[0].contains(pos) {
            match mouse.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    app.ui.active_panel = ActivePanel::Downloads;
                }
                MouseEventKind::ScrollUp => {
                    app.download.download_scroll_state.select_previous();
                }
                MouseEventKind::ScrollDown => {
                    app.download.download_scroll_state.select_next();
                }
                _ => {}
            }
        }
    }
}

fn handle_log_scroll(app: &mut App, scroll_up: bool) {
    if scroll_up {
        app.log.log_scroll_offset = app.log.log_scroll_offset.saturating_sub(1);
    } else {
        app.log.log_scroll_offset += 1;
    }
    app.log.log_follow = app.log.log_scroll_offset >= app.log.log_total_lines.saturating_sub(5);
}
