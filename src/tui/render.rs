use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
};

use crate::config::Profile;
use crate::tui::app::{App, ActivePanel, GlobalMode, ModelsMode};
use crate::tui::panel;

pub fn render(f: &mut Frame, app: &mut App) {
    // If help is showing, overlay it on top of everything
    if app.global_mode == GlobalMode::Help {
        let area = f.area();
        let help_area = Rect {
            x: area.width.saturating_sub(44) / 2,
            y: area.height.saturating_sub(20) / 2,
            width: 44,
            height: 20,
        };
        panel::help::render(f, help_area, app);
        return;
    }

    // Panel-specific help overlay
    if app.panel_help {
        let area = f.area();
        let help_area = Rect {
            x: area.width.saturating_sub(50) / 2,
            y: area.height.saturating_sub(25) / 2,
            width: 50,
            height: 25,
        };
        panel::help::render_panel(f, help_area, app);
        return;
    }

    // CmdLine full-screen overlay
    if app.global_mode == GlobalMode::CmdLine {
        let area = f.area();
        let cmd = app.cmd_line.as_deref().unwrap_or("(no command)");
        let lines: Vec<Line> = cmd
            .split('\n')
            .map(|line| Line::from(Span::raw(line)))
            .collect();
        let paragraph = Paragraph::new(lines);
        f.render_widget(paragraph, area);
        return;
    }

    if app.global_mode == GlobalMode::ExitConfirmation {
        let area = f.area();
        let popup_area = Rect {
            x: area.width.saturating_sub(50) / 2,
            y: area.height.saturating_sub(8) / 2,
            width: 50,
            height: 8,
        };

        // Exit confirmation
        let block = ratatui::widgets::Block::default()
            .title(" Exit Application? ")
            .borders(ratatui::widgets::Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow));

        let loaded_count = app.model_states.values().filter(|s| matches!(s, crate::models::ModelState::Loaded { .. })).count();
        let text = vec![
            Line::from(""),
            Line::from(vec![
                Span::raw("There are "),
                Span::styled(format!("{}", loaded_count), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw(" model(s) loaded."),
            ]),
            Line::from("Are you sure you want to exit?"),
            Line::from(""),
            Line::from(vec![
                Span::styled("  [y] Yes  ", Style::default().fg(Color::Black).bg(Color::Yellow)),
                Span::raw("    "),
                Span::styled("  [n] No   ", Style::default().fg(Color::Black).bg(Color::DarkGray)),
            ]),
        ];

        f.render_widget(ratatui::widgets::Clear, popup_area);
        f.render_widget(Paragraph::new(text).block(block).alignment(ratatui::layout::Alignment::Center), popup_area);
        return;
    }

    if app.global_mode == GlobalMode::ResetConfirmation {
        let area = f.area();
        let popup_area = Rect {
            x: area.width.saturating_sub(50) / 2,
            y: area.height.saturating_sub(8) / 2,
            width: 50,
            height: 8,
        };

        // Settings reset confirmation
        let block = ratatui::widgets::Block::default()
            .title(" Reset Settings? ")
            .borders(ratatui::widgets::Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow));

        let text = vec![
            Line::from(""),
            Line::from("Reset all LLM settings to defaults?"),
            Line::from(""),
            Line::from(vec![
                Span::styled("  [y] Yes  ", Style::default().fg(Color::Black).bg(Color::Yellow)),
                Span::raw("    "),
                Span::styled("  [n] No   ", Style::default().fg(Color::Black).bg(Color::DarkGray)),
            ]),
        ];

        f.render_widget(ratatui::widgets::Clear, popup_area);
        f.render_widget(Paragraph::new(text).block(block).alignment(ratatui::layout::Alignment::Center), popup_area);
        return;
    }

    if app.global_mode == GlobalMode::DeleteConfirmation {
        let area = f.area();
        let popup_area = Rect {
            x: area.width.saturating_sub(50) / 2,
            y: area.height.saturating_sub(8) / 2,
            width: 50,
            height: 8,
        };

        let model_name = app.selected_model().map(|m| m.name.as_str()).unwrap_or("Unknown");
        let block = ratatui::widgets::Block::default()
            .title(" Delete Model? ")
            .borders(ratatui::widgets::Borders::ALL)
            .border_style(Style::default().fg(Color::Red));

        let text = vec![
            Line::from(""),
            Line::from(vec![
                Span::raw("Are you sure you want to delete "),
                Span::styled(model_name, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw("?"),
            ]),
            Line::from(""),
            Line::from("This action cannot be undone."),
            Line::from(""),
            Line::from(vec![
                Span::styled("  [y] Yes  ", Style::default().fg(Color::Black).bg(Color::Red)),
                Span::raw("    "),
                Span::styled("  [n] No   ", Style::default().fg(Color::Black).bg(Color::DarkGray)),
            ]),
        ];

        f.render_widget(ratatui::widgets::Clear, popup_area);
        f.render_widget(Paragraph::new(text).block(block).alignment(ratatui::layout::Alignment::Center), popup_area);
        return;
    }

    // Main layout: status bar + top panels + active model + log

    let chunks = if app.log_expanded {
        // Expanded: just status bar and log panel
        ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .margin(0)
            .constraints([
                ratatui::layout::Constraint::Length(1),  // status bar
                ratatui::layout::Constraint::Fill(1),    // log (full remaining)
            ])
            .split(f.area())
    } else if app.readme_expanded {
        // Expanded: just status bar and README panel
        ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .margin(0)
            .constraints([
                ratatui::layout::Constraint::Length(1),  // status bar
                ratatui::layout::Constraint::Fill(1),    // README (full remaining)
            ])
            .split(f.area())
    } else {
    ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .margin(0)
        .constraints([
            ratatui::layout::Constraint::Length(1),   // status bar
            ratatui::layout::Constraint::Fill(1),     // top panels
            ratatui::layout::Constraint::Length(6),   // active model
            ratatui::layout::Constraint::Min(5),      // log (reduced height)
        ])
        .split(f.area())
    };

    // Status bar (model name and profile info)
    let status = render_status_bar(app);
    f.render_widget(Paragraph::new(status), chunks[0]);

    if app.log_expanded {
        let log_area = chunks[1];
        if !app.download_progress.is_empty() {
            let chunks = ratatui::layout::Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .constraints([
                    ratatui::layout::Constraint::Fill(1),    // log
                    ratatui::layout::Constraint::Length(7),  // downloads
                ])
                .split(log_area);
            panel::log::render(f, chunks[0], app);
            let total_speed: f64 = app.download_progress.iter().map(|d| d.bytes_per_second).sum();
            panel::models::render_download_panel(f, chunks[1], &app.download_progress, total_speed, &mut app.download_scroll_state, app.active_panel == ActivePanel::Downloads);
        } else {
            panel::log::render(f, log_area, app);
        }
        return;
    }

    if app.readme_expanded {
        let readme_area = chunks[1];
        if !app.download_progress.is_empty() {
            let chunks = ratatui::layout::Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .constraints([
                    ratatui::layout::Constraint::Fill(1),    // readme
                    ratatui::layout::Constraint::Length(7),  // downloads
                ])
                .split(readme_area);
            panel::readme::render(f, chunks[0], app);
            let total_speed: f64 = app.download_progress.iter().map(|d| d.bytes_per_second).sum();
            panel::models::render_download_panel(f, chunks[1], &app.download_progress, total_speed, &mut app.download_scroll_state, app.active_panel == ActivePanel::Downloads);
        } else {
            panel::readme::render(f, readme_area, app);
        }
        return;
    }

    let top_chunks = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([
            ratatui::layout::Constraint::Percentage(66), // Left side (Models + Info)
            ratatui::layout::Constraint::Fill(1),        // Right side (Settings)
        ])
        .split(chunks[1]);

    // Split left side vertically into Models list and Info
    let info_height = (panel::tabbed::get_info_lines(app, top_chunks[0].width).len() as u16 + 2).max(3);
    let left_chunks = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            ratatui::layout::Constraint::Min(5),            // Models list (at least 3 models + borders)
            ratatui::layout::Constraint::Length(info_height), // Model Info (dynamic)
        ])
        .split(top_chunks[0]);

    // Top-Left: models
    panel::models::render(f, left_chunks[0], app);

    // Bottom-Left: model info
    panel::tabbed::render_info_only(f, left_chunks[1], app);

    // Right: Settings or Profiles
    match app.active_panel {
        ActivePanel::Profiles => {
            let builtin = crate::config::builtin_profiles();
            let mut all_profiles: Vec<Profile> = builtin.iter().cloned().collect();
            for p in &app.config.profiles {
                if !builtin.iter().any(|b| b.name == p.name) {
                    all_profiles.push(p.clone());
                }
            }
            let (profile_lines, count) = panel::profiles::render_all(
                &all_profiles,
                app.settings_selected_idx,
                &app.settings,
            );
            if app.settings_selected_idx >= count {
                app.settings_selected_idx = count.saturating_sub(1);
            }
            
            let area = top_chunks[1];
            let available_height = area.height.saturating_sub(2);
            
            // Clamp scroll offset to max
            let max_offset = profile_lines.len().saturating_sub(available_height as usize) as u16;
            if app.profiles_scroll_offset > max_offset {
                app.profiles_scroll_offset = max_offset;
            }
            
            // Build visible profile lines with scroll offset applied
            let start_idx = app.profiles_scroll_offset as usize;
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
            
            // Render scrollbar if profiles overflow
            if profile_lines.len() > available_height as usize {
                let scrollbar_area = Rect {
                    x: area.right().saturating_sub(1),
                    y: area.top(),
                    width: 1,
                    height: area.height,
                };
                
                let mut scrollbar_state = ScrollbarState::new(profile_lines.len())
                    .position(app.profiles_scroll_offset as usize);
                
                f.render_stateful_widget(
                    Scrollbar::new(ScrollbarOrientation::VerticalRight)
                        .begin_symbol(Some("↑"))
                        .end_symbol(Some("↓")),
                    scrollbar_area,
                    &mut scrollbar_state,
                );
            }
        }
        ActivePanel::SystemPromptPresets => {
            let presets = &app.config.system_prompt_presets;
            let preset_lines = panel::system_prompt_presets::render_all(
                presets,
                app.settings_selected_idx,
                app.editing_preset.is_some(),
                &app.settings_edit_buffer,
                app.edit_cursor_pos,
            );
            
            let area = top_chunks[1];
            let available_height = area.height.saturating_sub(2);
            
            // Clamp scroll offset to max
            let max_offset = preset_lines.len().saturating_sub(available_height as usize) as u16;
            if app.system_prompt_presets_scroll_offset > max_offset {
                app.system_prompt_presets_scroll_offset = max_offset;
            }
            
            // Build visible preset lines with scroll offset applied
            let start_idx = app.system_prompt_presets_scroll_offset as usize;
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

            // Render scrollbar if presets overflow
            if preset_lines.len() > available_height as usize {
                let scrollbar_area = Rect {
                    x: area.right().saturating_sub(1),
                    y: area.top(),
                    width: 1,
                    height: area.height,
                };
                
                let mut scrollbar_state = ScrollbarState::new(preset_lines.len())
                    .position(app.system_prompt_presets_scroll_offset as usize);
                
                f.render_stateful_widget(
                    Scrollbar::new(ScrollbarOrientation::VerticalRight)
                        .begin_symbol(Some("↑"))
                        .end_symbol(Some("↓")),
                    scrollbar_area,
                    &mut scrollbar_state,
                );
            }
        }
        ActivePanel::SearchReadme => {
            panel::readme::render(f, top_chunks[1], app);
        }
        _ => {
            // In search/files mode with README shown, display it in the right panel by default
            let show_readme = match &app.models_mode {
                ModelsMode::Search { show_readme, .. } => *show_readme,
                ModelsMode::Files { .. } => {
                    // Files mode inherits show_readme from the previous search
                    // We track this via the fact that README is shown when we entered Files mode
                    true
                }
                _ => false,
            };
            if show_readme {
                panel::readme::render(f, top_chunks[1], app);
            } else {
                panel::tabbed::render_settings_only(f, top_chunks[1], app);
            }
        }
    }

    // Active model (full width)
    panel::active::render(f, chunks[2], app);

    // Log & Download (download panel below log, full width)
    if app.downloading {
        let bottom_chunks = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                ratatui::layout::Constraint::Fill(1),    // log
                ratatui::layout::Constraint::Length(7),  // downloads
            ])
            .split(chunks[3]);
        
        panel::log::render(f, bottom_chunks[0], app);
        
        let total_speed: f64 = app.download_progress.iter().map(|d| d.bytes_per_second).sum();
        panel::models::render_download_panel(
            f, bottom_chunks[1],
            &app.download_progress,
            total_speed,
            &mut app.download_scroll_state,
            app.active_panel == ActivePanel::Downloads,
        );
    } else {
        panel::log::render(f, chunks[3], app);
    }
}

fn render_status_bar<'a>(app: &'a App) -> Line<'a> {
    let mut parts = Vec::new();

    match &app.models_mode {
        crate::tui::app::ModelsMode::Search { query: _, sort_by, show_readme, .. } => {
            parts.push(Span::styled("SEARCH", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
            parts.push(Span::raw(" "));
            parts.push(Span::styled("Enter", Style::default().fg(Color::Cyan)));
            parts.push(Span::raw(" search  "));
            parts.push(Span::styled("Esc", Style::default().fg(Color::Cyan)));
            parts.push(Span::raw(" exit  "));
            parts.push(Span::styled("l", Style::default().fg(Color::Yellow)));
            parts.push(Span::raw(" files  "));
            parts.push(Span::styled("S", Style::default().fg(Color::Yellow)));
            parts.push(Span::raw(" sort  "));
            if *show_readme {
                if app.readme_expanded {
                    parts.push(Span::styled("R", Style::default().fg(Color::Yellow)));
                    parts.push(Span::raw(" fullscreen  "));
                    parts.push(Span::styled("Esc", Style::default().fg(Color::Cyan)));
                    parts.push(Span::raw(" collapse  "));
                } else {
                    parts.push(Span::styled("R", Style::default().fg(Color::Yellow)));
                    parts.push(Span::raw(" README  "));
                }
            } else {
                parts.push(Span::styled("R", Style::default().fg(Color::Yellow)));
                parts.push(Span::raw(" README  "));
            }
            parts.push(Span::styled("j/k", Style::default().fg(Color::Cyan)));
            parts.push(Span::raw(" navigate  "));
            parts.push(Span::styled(sort_by.label(), Style::default().fg(Color::Magenta)));
        }
        crate::tui::app::ModelsMode::Files { model_id, .. } => {
            parts.push(Span::styled("FILES", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
            parts.push(Span::raw(" "));
            parts.push(Span::styled(model_id, Style::default().fg(Color::Cyan)));
            parts.push(Span::raw("  "));
            parts.push(Span::styled("Enter", Style::default().fg(Color::Yellow)));
            parts.push(Span::raw(" download"));
            parts.push(Span::raw("  "));
            parts.push(Span::styled("Esc", Style::default().fg(Color::Cyan)));
            parts.push(Span::raw(" back"));
            parts.push(Span::raw("  "));
            parts.push(Span::styled("j/k", Style::default().fg(Color::Cyan)));
            parts.push(Span::raw(" navigate"));
            if app.readme_expanded {
                parts.push(Span::raw("  "));
                parts.push(Span::styled("Esc", Style::default().fg(Color::Cyan)));
                parts.push(Span::raw(" collapse"));
            } else {
                parts.push(Span::raw("  "));
                parts.push(Span::styled("R", Style::default().fg(Color::Yellow)));
                parts.push(Span::raw(" fullscreen"));
            }
        }
        crate::tui::app::ModelsMode::List => {
            if app.active_panel == crate::tui::app::ActivePanel::Profiles {
                parts.push(Span::styled("PROFILES", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
                parts.push(Span::raw("  "));
                parts.push(Span::styled("j/k", Style::default().fg(Color::Cyan)));
                parts.push(Span::raw(" nav  "));
                parts.push(Span::styled("Enter", Style::default().fg(Color::Cyan)));
                parts.push(Span::raw(" apply  "));
                parts.push(Span::styled("s", Style::default().fg(Color::Cyan)));
                parts.push(Span::raw(" save  "));
                parts.push(Span::styled("Esc", Style::default().fg(Color::Cyan)));
                parts.push(Span::raw(" done"));
            } else if app.active_panel == crate::tui::app::ActivePanel::SystemPromptPresets {
                parts.push(Span::styled("PROMPTS", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
                parts.push(Span::raw("  "));
                parts.push(Span::styled("j/k", Style::default().fg(Color::Cyan)));
                parts.push(Span::raw(" nav  "));
                parts.push(Span::styled("Enter", Style::default().fg(Color::Cyan)));
                parts.push(Span::raw(" apply  "));
                parts.push(Span::styled("e", Style::default().fg(Color::Cyan)));
                parts.push(Span::raw(" edit  "));
                parts.push(Span::styled("n", Style::default().fg(Color::Cyan)));
                parts.push(Span::raw(" new  "));
                parts.push(Span::styled("Esc", Style::default().fg(Color::Cyan)));
                parts.push(Span::raw(" done"));
            } else if app.active_panel == crate::tui::app::ActivePanel::ServerSettings {
                parts.push(Span::styled("SERVER SETTINGS", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
                parts.push(Span::raw("  "));
                parts.push(Span::styled("j/k", Style::default().fg(Color::Cyan)));
                parts.push(Span::raw(" nav  "));
                parts.push(Span::styled("Enter", Style::default().fg(Color::Yellow)));
                parts.push(Span::raw(" toggle  "));
                parts.push(Span::styled("h/l", Style::default().fg(Color::Cyan)));
                parts.push(Span::raw(" adjust  "));
                parts.push(Span::styled("Tab", Style::default().fg(Color::Cyan)));
                parts.push(Span::raw(" panels"));
            } else if app.active_panel == crate::tui::app::ActivePanel::LlmSettings {
                parts.push(Span::styled("LLM SETTINGS", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
                parts.push(Span::raw("  "));
                parts.push(Span::styled("j/k", Style::default().fg(Color::Cyan)));
                parts.push(Span::raw(" nav  "));
                parts.push(Span::styled("h/l", Style::default().fg(Color::Cyan)));
                parts.push(Span::raw(" adjust  "));
                parts.push(Span::styled("Ctrl+S", Style::default().fg(Color::Yellow)));
                parts.push(Span::raw(" save  "));
                parts.push(Span::styled("Ctrl+R", Style::default().fg(Color::Yellow)));
                parts.push(Span::raw(" reset  "));
                parts.push(Span::styled("Ctrl+E", Style::default().fg(Color::Yellow)));
                parts.push(Span::raw(" toggle  "));
                if app.is_settings_dirty() {
                    parts.push(Span::styled("*unsaved*", Style::default().fg(Color::Red)));
                    parts.push(Span::raw("  "));
                }
                parts.push(Span::styled("Tab", Style::default().fg(Color::Cyan)));
                parts.push(Span::raw(" panels"));
            } else {
                let panel_label = match app.active_panel {
                    crate::tui::app::ActivePanel::Models => "MODELS",
                    crate::tui::app::ActivePanel::Log => "LOG",
                    crate::tui::app::ActivePanel::Downloads => "DOWNLOADS",
                    crate::tui::app::ActivePanel::SearchReadme => "README",
                    _ => "APP",
                };
                parts.push(Span::styled(panel_label, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
                parts.push(Span::raw("  "));
                if app.active_panel == crate::tui::app::ActivePanel::Downloads {
                    parts.push(Span::styled("j/k", Style::default().fg(Color::Cyan)));
                    parts.push(Span::raw(" nav  "));
                    parts.push(Span::styled("c", Style::default().fg(Color::Cyan)));
                    parts.push(Span::raw(" cancel  "));
                }
                parts.push(Span::styled("Tab", Style::default().fg(Color::Cyan)));
                parts.push(Span::raw(" panels  "));

                if app.active_panel == crate::tui::app::ActivePanel::Log {
                    if app.log_expanded {
                        parts.push(Span::styled("Esc", Style::default().fg(Color::Cyan)));
                        parts.push(Span::raw(" collapse  "));
                    } else {
                        parts.push(Span::styled("Enter", Style::default().fg(Color::Cyan)));
                        parts.push(Span::raw(" expand  "));
                    }
                }
                if app.active_panel == crate::tui::app::ActivePanel::SearchReadme {
                    if app.readme_expanded {
                        parts.push(Span::styled("Esc", Style::default().fg(Color::Cyan)));
                        parts.push(Span::raw(" collapse  "));
                    } else {
                        parts.push(Span::styled("Enter", Style::default().fg(Color::Cyan)));
                        parts.push(Span::raw(" expand  "));
                    }
                }
                parts.push(Span::styled("/", Style::default().fg(Color::Yellow)));
                parts.push(Span::raw(" search  "));
                parts.push(Span::styled("j/k", Style::default().fg(Color::Cyan)));
                parts.push(Span::raw(" nav  "));
                parts.push(Span::styled("l", Style::default().fg(Color::Yellow)));
                parts.push(Span::raw(" load  "));
                parts.push(Span::styled("u", Style::default().fg(Color::Yellow)));
                parts.push(Span::raw(" unload  "));
                parts.push(Span::styled("g/G", Style::default().fg(Color::Cyan)));
                parts.push(Span::raw(" log  "));
                parts.push(Span::styled("Ctrl+H", Style::default().fg(Color::Cyan)));
                parts.push(Span::raw(" help  "));
                if app.panel_help {
                    parts.push(Span::styled("Esc", Style::default().fg(Color::Cyan)));
                    parts.push(Span::raw(" help  "));
                }
                parts.push(Span::styled("Ctrl+Shift+H", Style::default().fg(Color::Cyan)));
                parts.push(Span::raw(" global help  "));
                parts.push(Span::styled("p", Style::default().fg(Color::Yellow)));
                parts.push(Span::raw(" profiles"));
            }
        }
    }

    if parts.is_empty() {
        return Line::from("");
    }

    Line::from(parts)
}
