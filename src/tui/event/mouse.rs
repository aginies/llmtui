use ratatui::layout::{Position, Rect};
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

use crate::tui::app::{App, ActivePanel};

pub fn handle_mouse(app: &mut App, mouse: MouseEvent, area: Rect) {
    let pos = Position::new(mouse.column, mouse.row);

    if app.log_expanded {
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
                    app.active_panel = ActivePanel::Log;
                    app.set_redraw();
                }
                MouseEventKind::ScrollUp => {
                    app.log_scroll_offset = app.log_scroll_offset.saturating_sub(1);
                    app.log_follow = false;
                    app.set_redraw();
                }
                MouseEventKind::ScrollDown => {
                    app.log_scroll_offset = app.log_scroll_offset + 1;
                    if app.log_scroll_offset >= app.log_total_lines.saturating_sub(5) {
                        app.log_follow = true;
                    } else {
                        app.log_follow = false;
                    }
                    app.set_redraw();
                }
                _ => {}
            }
        }
        return;
    }

    // If actively resizing, continue updating even if mouse moved outside the border area
    if let Some(ref rs) = app.resize_state {
        match mouse.kind {
            MouseEventKind::Drag(_) => {
                let dx = pos.x as i16 - rs.start_x as i16;
                let delta = (dx * 100 / rs.container.width as i16).max(-5).min(5);
                app.left_pct = (rs.start_pct as i16 + delta).clamp(20, 80) as u16;
                app.set_redraw();
            }
            MouseEventKind::Up(MouseButton::Left) => {
                app.resize_state = None;
                app.set_redraw();
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
        if app.downloading {
            let bottom_chunks = ratatui::layout::Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .constraints([
                    ratatui::layout::Constraint::Fill(1),    // log
                    ratatui::layout::Constraint::Length(7),  // downloads
                ])
                .split(chunks[3]);

            if bottom_chunks[1].contains(pos) {
                match mouse.kind {
                    MouseEventKind::Down(MouseButton::Left) => {
                        app.active_panel = ActivePanel::Downloads;
                        app.set_redraw();
                    }
                    MouseEventKind::ScrollUp => {
                        app.download_scroll_state.select_previous();
                        app.set_redraw();
                    }
                    MouseEventKind::ScrollDown => {
                        app.download_scroll_state.select_next();
                        app.set_redraw();
                    }
                    _ => {}
                }
                return;
            }
        }

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                app.active_panel = ActivePanel::Log;
                app.set_redraw();
            }
            MouseEventKind::ScrollUp => {
                app.log_scroll_offset = app.log_scroll_offset.saturating_sub(1);
                app.log_follow = false;
                app.active_panel = ActivePanel::Log;
                app.set_redraw();
            }
            MouseEventKind::ScrollDown => {
                app.log_scroll_offset = app.log_scroll_offset + 1;
                if app.log_scroll_offset >= app.log_total_lines.saturating_sub(5) {
                    app.log_follow = true;
                } else {
                    app.log_follow = false;
                }
                app.active_panel = ActivePanel::Log;
                app.set_redraw();
            }
            _ => {}
        }
        return;
    }

    // 2. Check Top panels
    if chunks[1].contains(pos) {
        let left_pct = app.left_pct.max(20).min(80);
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
                    app.resize_state = Some(crate::tui::app::ResizeState {
                        start_x: pos.x,
                        start_pct: app.left_pct,
                        container: chunks[1],
                    });
                    app.set_redraw();
                }
                MouseEventKind::Drag(_) => {
                    if let Some(ref rs) = app.resize_state {
                        let dx = pos.x as i16 - rs.start_x as i16;
                        let delta = (dx * 100 / rs.container.width as i16).max(-5).min(5);
                        app.left_pct = (rs.start_pct as i16 + delta).clamp(20, 80) as u16;
                        app.set_redraw();
                    }
                }
                MouseEventKind::Up(MouseButton::Left) => {
                    app.resize_state = None;
                    app.set_redraw();
                }
                MouseEventKind::ScrollUp => {
                    app.left_pct = app.left_pct.saturating_sub(1).max(20);
                    app.set_redraw();
                }
                MouseEventKind::ScrollDown => {
                    app.left_pct = app.left_pct.saturating_add(1).min(80);
                    app.set_redraw();
                }
                _ => {}
            }
            return;
        }

        // Right side: Settings/Profiles
        if top_chunks[1].contains(pos) {
            match mouse.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    let server_running = app.server_handle.is_some();
                    app.active_panel = match app.active_panel {
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
                    app.set_redraw();
                }
                MouseEventKind::ScrollUp => {
                    match app.active_panel {
                        ActivePanel::LlmSettings => {
                            app.settings_scroll_offset = app.settings_scroll_offset.saturating_sub(1);
                            app.set_redraw();
                        }
                        ActivePanel::Profiles => {
                            app.profiles_scroll_offset = app.profiles_scroll_offset.saturating_sub(1);
                            app.set_redraw();
                        }
                        ActivePanel::SystemPromptPresets => {
                            app.system_prompt_presets_scroll_offset = app.system_prompt_presets_scroll_offset.saturating_sub(1);
                            app.set_redraw();
                        }
                        _ => {}
                    }
                }
                MouseEventKind::ScrollDown => {
                    match app.active_panel {
                        ActivePanel::LlmSettings => {
                            app.settings_scroll_offset = app.settings_scroll_offset.saturating_add(1);
                            app.set_redraw();
                        }
                        ActivePanel::Profiles => {
                            app.profiles_scroll_offset = app.profiles_scroll_offset.saturating_add(1);
                            app.set_redraw();
                        }
                        ActivePanel::SystemPromptPresets => {
                            app.system_prompt_presets_scroll_offset = app.system_prompt_presets_scroll_offset.saturating_add(1);
                            app.set_redraw();
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
            return;
        }

        // Left side: Models + Info
        if top_chunks[0].contains(pos) {
            let info_height = (crate::tui::panel::tabbed::get_info_lines(app, top_chunks[0].width).len() as u16 + 2).max(3);
            let left_chunks = ratatui::layout::Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .constraints([
                    ratatui::layout::Constraint::Min(5),
                    ratatui::layout::Constraint::Length(info_height),
                ])
                .split(top_chunks[0]);

            if left_chunks[0].contains(pos)
                && let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
                    app.active_panel = ActivePanel::Models;
                    app.set_redraw();
                }
        }
    }

    // Handle downloads-only layout (when log is not visible but downloading)
    if app.downloading && !app.log_expanded {
        let bottom_area = chunks[3];
        let bottom_chunks = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                ratatui::layout::Constraint::Length(7),  // downloads
            ])
            .split(bottom_area);

        if bottom_chunks[0].contains(pos) {
            match mouse.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    app.active_panel = ActivePanel::Downloads;
                    app.set_redraw();
                }
                MouseEventKind::ScrollUp => {
                    app.download_scroll_state.select_previous();
                    app.set_redraw();
                }
                MouseEventKind::ScrollDown => {
                    app.download_scroll_state.select_next();
                    app.set_redraw();
                }
                _ => {}
            }
        }
    }
}
