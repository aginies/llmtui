use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
};
use unicode_width::UnicodeWidthStr;

use crate::tui::app::{App, ActivePanel, ConfirmationKind, GlobalMode, ModelsMode};
use crate::tui::panel;

pub fn render(f: &mut Frame, app: &mut App) {
    // Panel-specific help overlay
    if app.panel_help {
        let area = f.area();
        let w = (area.width as f64 * 0.7).clamp(60.0, 80.0) as u16;
        let h = (area.height as f64 * 0.7).clamp(20.0, 35.0) as u16;
        let help_area = Rect {
            x: (area.width - w) / 2,
            y: (area.height - h) / 2,
            width: w,
            height: h,
        };
        panel::help::render_panel(f, help_area, app);
        return;
    }

    // CmdLine full-screen overlay
    if let GlobalMode::CmdLine { cmd_line } = &app.global_mode {
        let area = f.area();
        let max_width = (area.width - 2).max(10) as usize;
        let wrapped = wrap_text(cmd_line, max_width);
        let text = Text::from(wrapped);
        let block = ratatui::widgets::Block::default()
            .title(" CmdLine — Esc to close  e to export ")
            .borders(ratatui::widgets::Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow));
        let paragraph = Paragraph::new(text).block(block);
        f.render_widget(paragraph, area);
        return;
    }

    if let GlobalMode::Confirmation { selected, kind } = &app.global_mode {
        let area = f.area();
        let popup_area = Rect {
            x: area.width.saturating_sub(50) / 2,
            y: area.height.saturating_sub(8) / 2,
            width: 50,
            height: 8,
        };

        let (title, text_lines) = match kind {
            ConfirmationKind::Exit => {
                let loaded_count = app.model_states.values().filter(|s| matches!(s, crate::models::ModelState::Loaded { .. })).count();
                (" Exit Application? ", vec![
                    Line::from(""),
                    Line::from(vec![
                        Span::raw("There are "),
                        Span::styled(format!("{}", loaded_count), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                        Span::raw(" model(s) loaded."),
                    ]),
                    Line::from("Are you sure you want to exit?"),
                ])
            }
            ConfirmationKind::Reset => {
                (" Reset Settings? ", vec![
                    Line::from(""),
                    Line::from("Reset all LLM settings to defaults?"),
                ])
            }
            ConfirmationKind::Delete => {
                let model_name = app.selected_model().map(|m| m.name.as_str()).unwrap_or("Unknown");
                (" Delete Model? ", vec![
                    Line::from(""),
                    Line::from(vec![
                        Span::raw("Are you sure you want to delete "),
                        Span::styled(model_name, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                        Span::raw("?"),
                    ]),
                    Line::from(""),
                    Line::from("This action cannot be undone."),
                ])
            }
            ConfirmationKind::Unload => {
                let model_name = match &app.pending_api_unload {
                    Some((name, _)) => name.as_str(),
                    None => "Unknown",
                };
                (" Unload Model? ", vec![
                    Line::from(""),
                    Line::from(vec![
                        Span::raw("Unload "),
                        Span::styled(model_name, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                        Span::raw("?"),
                    ]),
                ])
            }
            ConfirmationKind::DeleteBackend => {
                let (backend, tag) = match &app.pending_backend_deletion {
                    Some((b, t)) => (b.to_string(), t.as_str()),
                    None => ("Unknown".to_string(), "latest"),
                };
                (" Delete Backend? ", vec![
                    Line::from(""),
                    Line::from(vec![
                        Span::raw("Delete backend "),
                        Span::styled(format!("{} ({})", backend, tag), Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                        Span::raw("?"),
                    ]),
                    Line::from(""),
                    Line::from("This will remove the binary and shared libraries."),
                ])
            }
        };

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(if *kind == ConfirmationKind::Delete || *kind == ConfirmationKind::DeleteBackend { Color::Red } else { Color::Yellow }));

        let mut lines = text_lines;
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("  [y] Yes  ", Style::default().fg(Color::Black).bg(if *selected { Color::Yellow } else { Color::DarkGray })),
            Span::raw("    "),
            Span::styled("  [n] No   ", Style::default().fg(Color::Black).bg(if *selected { Color::DarkGray } else { Color::Yellow })),
        ]));

        f.render_widget(ratatui::widgets::Clear, popup_area);
        f.render_widget(Paragraph::new(lines).block(block).alignment(ratatui::layout::Alignment::Center), popup_area);
        return;
    }

    // Host picker overlay
    if let GlobalMode::HostPicker { entries, selected } = &app.global_mode {
        let area = f.area();
        let w = (area.width as f64 * 0.7).clamp(60.0, 80.0) as u16;
        let h = (area.height as f64 * 0.7).clamp(20.0, 35.0) as u16;
        let picker_area = Rect {
            x: (area.width - w) / 2,
            y: (area.height - h) / 2,
            width: w,
            height: h,
        };

        let mut picker_lines: Vec<Line> = Vec::new();
        picker_lines.push(Line::from(Span::styled(
            " [↑] Select Host Address  [d] Refresh  [j/k] nav  [Esc] cancel ",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )));
        picker_lines.push(Line::from(""));

        for (i, (ip, iface)) in entries.iter().enumerate() {
            let marker = if i == *selected { "> " } else { "  " };
            let style = if i == *selected {
                Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            picker_lines.push(Line::from(vec![
                Span::styled(marker, Style::default().fg(Color::Yellow)),
                Span::styled(format!("{ip}"), style),
                Span::raw("  "),
                Span::styled(format!("({iface})"), Style::default().fg(Color::DarkGray)),
            ]));
        }

        f.render_widget(ratatui::widgets::Clear, picker_area);
        f.render_widget(Paragraph::new(picker_lines).block(
            Block::default()
                .title(" Host Picker ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        ), picker_area);
        return;
        }

        // Backend picker overlay
        if let GlobalMode::BackendPicker { entries, selected } = &app.global_mode {
            let area = f.area();
            let w = (area.width as f64 * 0.5).clamp(50.0, 70.0) as u16;
            let gpu_info_lines = if crate::backend::hardware::detect_gpu_model().is_some() { 1 } else { 0 };
            // Increase max height for version list
            let h = (entries.len() + 4 + gpu_info_lines).min(area.height as usize - 4) as u16;
            let picker_area = Rect {
                x: (area.width - w) / 2,
                y: (area.height - h) / 2,
                width: w,
                height: h,
            };

            use crate::backend::hardware::{detect_gpu_vendor, detect_gpu_model, GpuVendor};
            let vendor = detect_gpu_vendor();
            let gpu_model = detect_gpu_model();

            let mut picker_lines: Vec<Line> = Vec::new();
            picker_lines.push(Line::from(Span::styled(
                " Select Backend Acceleration ",
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            )));

            if let Some(model) = gpu_model {
                picker_lines.push(Line::from(vec![
                    Span::raw("Detected Hardware: "),
                    Span::styled(model, Style::default().fg(Color::Cyan)),
                ]));
            }
            picker_lines.push(Line::from(""));

            for (i, (backend, tag)) in entries.iter().enumerate() {
                let marker = if i == *selected { "> " } else { "  " };

                // For versioned entries, check if that specific version is installed
                // Actually they come from list_installed_backends so they ARE installed.
                // "None" tag means "latest" which might not be installed.
                let is_installed = if tag.is_some() {
                    true 
                } else {
                    crate::backend::hub::is_backend_any_version_installed(*backend)
                };

                let is_recommended = match (vendor, backend, tag) {
                    (GpuVendor::Amd, crate::models::Backend::Rocm, None) => true,
                    (GpuVendor::Amd, crate::models::Backend::RocmLemonade, None) => true,
                    (GpuVendor::Nvidia, crate::models::Backend::Cuda, None) => true,
                    (GpuVendor::Nvidia, crate::models::Backend::Vulkan, None) => true,
                    (GpuVendor::Intel, crate::models::Backend::Vulkan, None) => true,
                    (GpuVendor::Unknown, crate::models::Backend::Cpu, None) => true,
                    _ => false,
                };

                let style = if i == *selected {
                    Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };

                let label = match backend {
                    crate::models::Backend::Cpu => "CPU-only",
                    crate::models::Backend::Vulkan => "Vulkan (GPU Universal)",
                    crate::models::Backend::Rocm => "ROCm (AMD Native)",
                    crate::models::Backend::RocmLemonade => "ROCm Lemonade (AMD Optimized)",
                    crate::models::Backend::Cuda => "CUDA (NVIDIA Native)",
                };
                let display_label = if let Some(t) = tag {
                    format!("{} ({})", label, t)
                } else {
                    format!("{} (latest/auto)", label)
                };

                let mut line_spans = vec![
                    Span::styled(marker, Style::default().fg(Color::Yellow)),
                    Span::styled(display_label, style),
                ];

                if tag.is_none() && is_installed {
                    line_spans.push(Span::raw("  "));
                    line_spans.push(Span::styled("(Cached)", Style::default().fg(Color::Blue)));
                }

                if is_recommended {
                    line_spans.push(Span::raw("  "));
                    line_spans.push(Span::styled("(Recommended)", Style::default().fg(Color::Green)));
                }

                picker_lines.push(Line::from(line_spans));
            }

            f.render_widget(ratatui::widgets::Clear, picker_area);
            f.render_widget(Paragraph::new(picker_lines).block(
                Block::default()
                    .title(" Backend Picker ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow)),
            ), picker_area);
            return;
            }

            // RPC Manager overlay
            if matches!(app.global_mode, GlobalMode::RpcManager) {
            let area = f.area();
            let w = (area.width as f64 * 0.8).clamp(70.0, 90.0) as u16;
            let h = (area.height as f64 * 0.8).clamp(20.0, 35.0) as u16;
            let rpc_area = Rect {
                x: (area.width.saturating_sub(w)) / 2,
                y: (area.height.saturating_sub(h)) / 2,
                width: w,
                height: h,
            };

            let workers = &app.config.rpc_workers;
            let worker_lines = panel::rpc_workers::render_all(
                workers,
                app.rpc_workers_selected_idx,
                app.editing_rpc_worker.is_some(),
                &app.settings_edit_buffer,
                app.edit_cursor_pos,
            );

            let available_height = rpc_area.height.saturating_sub(2);
            let max_offset = worker_lines.len().saturating_sub(available_height as usize) as u16;
            if app.rpc_workers_scroll_offset > max_offset {
                app.rpc_workers_scroll_offset = max_offset;
            }

            let start_idx = app.rpc_workers_scroll_offset as usize;
            let visible_lines: Vec<Line> = worker_lines
                .iter()
                .skip(start_idx)
                .take(available_height as usize)
                .cloned()
                .collect();

            let block = Block::default()
                .title(" RPC Workers Manager ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));

            f.render_widget(ratatui::widgets::Clear, rpc_area);
            f.render_widget(Paragraph::new(visible_lines).block(block), rpc_area);

            if worker_lines.len() > available_height as usize {
                let scrollbar_area = Rect {
                    x: rpc_area.right().saturating_sub(1),
                    y: rpc_area.top() + 1,
                    width: 1,
                    height: rpc_area.height.saturating_sub(2),
                };
                let mut scrollbar_state = ScrollbarState::new(worker_lines.len())
                    .position(app.rpc_workers_scroll_offset as usize);
                f.render_stateful_widget(
                    Scrollbar::new(ScrollbarOrientation::VerticalRight)
                        .begin_symbol(Some("↑"))
                        .end_symbol(Some("↓")),
                    scrollbar_area,
                    &mut scrollbar_state,
                );
            }
            return;
            }

            // About overlay
            if matches!(app.global_mode, GlobalMode::About) {
            let area = f.area();
            let w = (area.width as f64 * 0.6).clamp(50.0, 70.0) as u16;
            let h = 16;
            let about_area = Rect {
                x: (area.width.saturating_sub(w)) / 2,
                y: (area.height.saturating_sub(h)) / 2,
                width: w,
                height: h,
            };

            let about_lines = panel::about::render_about();
            let block = Block::default()
                .title(" About ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));

            f.render_widget(ratatui::widgets::Clear, about_area);
            f.render_widget(Paragraph::new(about_lines).block(block).alignment(ratatui::layout::Alignment::Center), about_area);
            return;
            }

            // Main layout: status bar + top panels + active model + log
    let is_search = matches!(app.models_mode, ModelsMode::Search { .. });
    let active_model_visible = app.is_panel_visible(4) && !is_search;
    let log_visible = app.is_panel_visible(5);

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
    } else {
        let active_model_constraint = if active_model_visible {
            ratatui::layout::Constraint::Length(6)
        } else {
            ratatui::layout::Constraint::Length(0)
        };

        let bottom_constraint = if log_visible && !active_model_visible {
            // Log is visible and active model is hidden - let it expand
            ratatui::layout::Constraint::Fill(1)
        } else {
            // Calculate base height for bottom area
            let mut h = 0;
            if log_visible { h += 3; }
            if app.downloading { h += 7; }
            
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
                ratatui::layout::Constraint::Length(1),   // status bar
                ratatui::layout::Constraint::Fill(1),     // top panels
                active_model_constraint,
                bottom_constraint,
            ])
            .split(f.area())
    };

    // Status bar (model name and profile info)
    let status = render_status_bar(app, chunks[0]);
    f.render_widget(Paragraph::new(status), chunks[0]);

    if app.log_expanded {
        let log_area = chunks[1];
        panel::log::render(f, log_area, app);
        return;
    }



    let top_chunks = if !app.is_panel_visible(1) && !app.is_panel_visible(3) && !matches!(app.active_panel, ActivePanel::Profiles | ActivePanel::SystemPromptPresets | ActivePanel::SearchReadme) {
        // Both settings panels hidden — expand left side to full width
        ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([
                ratatui::layout::Constraint::Percentage(100),
                ratatui::layout::Constraint::Length(0),
            ])
            .split(chunks[1])
    } else {
        ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([
                ratatui::layout::Constraint::Percentage(55), // Left side (Models + Info)
                ratatui::layout::Constraint::Percentage(45), // Right side (Settings / README)
            ])
            .split(chunks[1])
    };

    let info_visible = app.is_panel_visible(2);
    let left_chunks = if info_visible {
        let info_height = (panel::tabbed::get_info_lines(app, top_chunks[0].width).len() as u16 + 2).max(3);
        ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                ratatui::layout::Constraint::Min(5),
                ratatui::layout::Constraint::Length(info_height),
            ])
            .split(top_chunks[0])
    } else {
        ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                ratatui::layout::Constraint::Fill(1),
            ])
            .split(top_chunks[0])
    };

    // Top-Left: models
    panel::models::render(f, left_chunks[0], app);

    // Bottom-Left: model info (only if visible)
    if info_visible {
        panel::tabbed::render_info_only(f, left_chunks[1], app);
    }

    // Right: Settings or Profiles
    match app.active_panel {
        ActivePanel::Profiles => {
            let all_profiles = app.config.merged_profiles();
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
                let server_visible = app.is_panel_visible(1);
                let llm_visible = app.is_panel_visible(3);
                if server_visible && llm_visible {
                    panel::tabbed::render_settings_only(f, top_chunks[1], app);
                } else if server_visible {
                    panel::tabbed::render_server_only(f, top_chunks[1], app);
                } else if llm_visible {
                    panel::tabbed::render_llm_only(f, top_chunks[1], app);
                } else {
                    // Both hidden — nothing to render on the right
                }
            }
        }
    }

    // Active model (full width)
    if active_model_visible {
        panel::active::render(f, chunks[2], app);
    }

    // Bottom area: Log and/or Download
    let bottom_area = chunks[3];
    if log_visible && app.downloading {
        let bottom_chunks = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                ratatui::layout::Constraint::Fill(1),    // log
                ratatui::layout::Constraint::Length(7),  // downloads
            ])
            .split(bottom_area);
        
        panel::log::render(f, bottom_chunks[0], app);
        
        let total_speed: f64 = app.download_progress.iter().map(|d| d.bytes_per_second).sum();
        panel::models::render_download_panel(
            f, bottom_chunks[1],
            &app.download_progress,
            total_speed,
            &mut app.download_scroll_state,
            false, // Never focused now since F3 panel is gone
        );
    } else if log_visible {
        panel::log::render(f, bottom_area, app);
    } else if app.downloading {
        let total_speed: f64 = app.download_progress.iter().map(|d| d.bytes_per_second).sum();
        panel::models::render_download_panel(
            f, bottom_area,
            &app.download_progress,
            total_speed,
            &mut app.download_scroll_state,
            false,
        );
    }
}

fn render_hints(app: &App) -> Vec<Span<'static>> {
    let y = Style::default().fg(Color::Yellow);
    let c = Style::default().fg(Color::Cyan);
    let r = Style::default().fg(Color::Red);

    match &app.models_mode {
        crate::tui::app::ModelsMode::Search { sort_by, show_readme, loading, .. } => {
            let mut parts = Vec::new();
            parts.push(Span::styled("⎋ exit", c));
            parts.push(Span::raw("  "));
            parts.push(Span::styled("↵ search", y));
            parts.push(Span::raw("  "));
            parts.push(Span::styled("L files", y));
            parts.push(Span::raw("  "));
            parts.push(Span::styled("S sort", y));
            parts.push(Span::raw("  "));
            parts.push(Span::styled("B back", y));
            if *show_readme {
                parts.push(Span::raw("  "));
                parts.push(Span::styled("R README", y));
            }
            parts.push(Span::raw("  "));
            parts.push(Span::styled("sort:", c));
            parts.push(Span::styled(sort_by.label(), Style::default().fg(Color::Magenta)));
            if *loading {
                parts.push(Span::raw("  "));
                parts.push(Span::styled("[loading]", Style::default().fg(Color::Yellow)));
            }
            parts
        }
        crate::tui::app::ModelsMode::Files { .. } => {
            let mut parts = Vec::new();
            parts.push(Span::styled("↵ download", y));
            parts.push(Span::raw("  "));
            parts.push(Span::styled("⎋ back", c));
            parts
        }
        crate::tui::app::ModelsMode::List => {
            if app.active_panel == crate::tui::app::ActivePanel::LlmSettings {
                let mut parts = Vec::new();
                parts.push(Span::styled("j/k nav", c));
                parts.push(Span::raw("  "));
                parts.push(Span::styled("⌃S save", y));
                parts.push(Span::raw("  "));
                parts.push(Span::styled("⌃R reset", y));
                parts.push(Span::raw("  "));
                parts.push(Span::styled("⌃E toggle", y));
                parts.push(Span::raw("  "));
                parts.push(Span::styled("A about", c));
                if app.is_settings_dirty() {
                    parts.push(Span::raw("  "));
                    parts.push(Span::styled("*unsaved*", r));
                }
                parts.push(Span::raw("  "));
                parts.push(Span::styled("p profiles", y));
                parts.push(Span::raw("  "));
                parts.push(Span::styled("⇥ panels", c));
                parts
            } else {
                let parts = match app.active_panel {
                    crate::tui::app::ActivePanel::Models => {
                        vec![
                            Span::styled("⇥ panels", c),
                            Span::raw("  "),
                            Span::styled("/ search", y),
                            Span::raw("  "),
                            Span::styled("f filter", y),
                            Span::raw("  "),
                            Span::styled("l/u un/load", y),
                            Span::raw("  "),
                            Span::styled("A about", c),
                            Span::raw("  "),
                            Span::styled("⌃H help", c),
                        ]
                    }
                    crate::tui::app::ActivePanel::Log => {
                        if app.log_expanded {
                            vec![
                                Span::styled("j/k scroll", c),
                                Span::raw("  "),
                                Span::styled("⎋ collapse", c),
                                Span::raw("  "),
                                Span::styled("g/G top/bottom", c),
                            ]
                        } else {
                            vec![
                                Span::styled("⎋ collapse", c),
                                Span::raw("  "),
                                Span::styled("g/G top/bottom", c),
                                Span::raw("  "),
                                Span::styled("⇥ panels", c),
                            ]
                        }
                    }
                    crate::tui::app::ActivePanel::ServerSettings => {
                        vec![
                            Span::styled("j/k nav", c),
                            Span::raw("  "),
                            Span::styled("↵ toggle", y),
                            Span::raw("  "),
                            Span::styled("A about", c),
                            Span::raw("  "),
                            Span::styled("⇥ panels", c),
                        ]
                    }
                    crate::tui::app::ActivePanel::Profiles => {
                        vec![
                            Span::styled("j/k nav", c),
                            Span::raw("  "),
                            Span::styled("↵ apply", y),
                            Span::raw("  "),
                            Span::styled("s save", c),
                            Span::raw("  "),
                            Span::styled("A about", c),
                            Span::raw("  "),
                            Span::styled("⎋ done", c),
                            Span::raw("  "),
                            Span::styled("⇥ panels", c),
                        ]
                    }
                    crate::tui::app::ActivePanel::SystemPromptPresets => {
                        vec![
                            Span::styled("j/k nav", c),
                            Span::raw("  "),
                            Span::styled("↵ apply", y),
                            Span::raw("  "),
                            Span::styled("e edit", c),
                            Span::raw("  "),
                            Span::styled("n new", c),
                            Span::raw("  "),
                            Span::styled("A about", c),
                            Span::raw("  "),
                            Span::styled("⎋ done", c),
                            Span::raw("  "),
                            Span::styled("⇥ panels", c),
                        ]
                    }
                    crate::tui::app::ActivePanel::SearchReadme => {
                        vec![
                            Span::styled("j/k nav", c),
                            Span::raw("  "),
                            Span::styled("A about", c),
                            Span::raw("  "),
                            Span::styled("⎋ collapse", c),
                            Span::raw("  "),
                            Span::styled("⇥ panels", c),
                        ]
                    }
                    _ => {
                        vec![
                            Span::styled("⇥ panels", c),
                            Span::raw("  "),
                            Span::styled("/ search", y),
                            Span::raw("  "),
                            Span::styled("f filter", y),
                            Span::raw("  "),
                            Span::styled("A about", c),
                            Span::raw("  "),
                            Span::styled("⌃H help", c),
                        ]
                    }
                };
                parts
            }
        }
    }
}

fn render_status_bar<'a>(app: &'a App, panel_area: Rect) -> Line<'a> {
    let mut parts = Vec::new();

    let mode_name = match &app.models_mode {
        ModelsMode::List => "List".to_string(),
        ModelsMode::Search { results, .. } => format!("Search({} results)", results.len()),
        ModelsMode::Files { files, .. } => format!("Files({} files)", files.len()),
    };
    parts.push(Span::styled(format!("[Mode: {}] ", mode_name), Style::default().fg(Color::DarkGray)));

    if let Some(handle) = &app.server_handle {
        parts.push(Span::styled(format!("● {} {}", handle.port, app.server_mode), Style::default().fg(Color::Green)));
    } else {
        parts.push(Span::styled("○ Server", Style::default().fg(Color::DarkGray)));
    }

    if matches!(app.global_mode, GlobalMode::HostPicker { .. }) {
        parts.push(Span::raw("  "));
        parts.push(Span::styled("[HOST PICKER]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
    }

    if matches!(app.global_mode, GlobalMode::RpcManager) {
        parts.push(Span::raw("  "));
        parts.push(Span::styled("[RPC MANAGER]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
    }

    if matches!(app.global_mode, GlobalMode::About) {
        parts.push(Span::raw("  "));
        parts.push(Span::styled("[ABOUT]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
    }

    match &app.models_mode {
        crate::tui::app::ModelsMode::Search { query: _, sort_by, .. } => {
            parts.push(Span::raw("  "));
            if app.active_panel == crate::tui::app::ActivePanel::Models {
                parts.push(Span::styled("SEARCH", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
            } else {
                let panel_label = match app.active_panel {
                    crate::tui::app::ActivePanel::Log => "LOG",
                    crate::tui::app::ActivePanel::ServerSettings => "SERVER",
                    crate::tui::app::ActivePanel::LlmSettings => "LLM",
                    crate::tui::app::ActivePanel::Profiles => "PROFILES",
                    crate::tui::app::ActivePanel::SystemPromptPresets => "PROMPTS",
                    crate::tui::app::ActivePanel::SearchReadme => "README",
                    _ => "SEARCH",
                };
                parts.push(Span::styled(panel_label, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
            }
            parts.push(Span::raw(" "));
            parts.push(Span::styled(sort_by.label(), Style::default().fg(Color::Magenta)));
        }
        crate::tui::app::ModelsMode::Files { model_id, .. } => {
            parts.push(Span::raw("  "));
            if app.active_panel == crate::tui::app::ActivePanel::Models {
                parts.push(Span::styled("FILES", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
            } else {
                let panel_label = match app.active_panel {
                    crate::tui::app::ActivePanel::Log => "LOG",
                    crate::tui::app::ActivePanel::ServerSettings => "SERVER",
                    crate::tui::app::ActivePanel::LlmSettings => "LLM",
                    crate::tui::app::ActivePanel::Profiles => "PROFILES",
                    crate::tui::app::ActivePanel::SystemPromptPresets => "PROMPTS",
                    crate::tui::app::ActivePanel::SearchReadme => "README",
                    _ => "FILES",
                };
                parts.push(Span::styled(panel_label, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
            }
            parts.push(Span::raw(" "));
            parts.push(Span::styled(model_id, Style::default().fg(Color::Cyan)));
        }
        crate::tui::app::ModelsMode::List => {
            parts.push(Span::raw("  "));
            let panel_label = match app.active_panel {
                crate::tui::app::ActivePanel::Models => "MODELS",
                crate::tui::app::ActivePanel::Log => "LOG",
                crate::tui::app::ActivePanel::ServerSettings => "SERVER",
                crate::tui::app::ActivePanel::LlmSettings => "LLM",
                crate::tui::app::ActivePanel::Profiles => "PROFILES",
                crate::tui::app::ActivePanel::SystemPromptPresets => "PROMPTS",
                crate::tui::app::ActivePanel::SearchReadme => "README",
                _ => "APP",
            };
            parts.push(Span::styled(panel_label, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
        }
    }

    if parts.is_empty() {
        return Line::from("");
    }

    let left_width: usize = parts.iter().map(|s| s.width()).sum();
    let remaining = panel_area.width.saturating_sub(left_width as u16) as usize;

    let hints = render_hints(app);
    let hints_width: usize = hints.iter().map(|s| s.width()).sum();
    let padding = remaining.saturating_sub(hints_width).max(0);

    if padding > 0 {
        parts.push(Span::raw(" ".repeat(padding)));
    }
    parts.extend(hints);

    Line::from(parts)
}

fn wrap_text(text: &str, max_width: usize) -> String {
    let mut lines = Vec::new();
    let mut current = String::new();

    for word in text.split_whitespace() {
        let word_width = word.width();
        let current_width = current.width();

        if current.is_empty() {
            current.push_str(word);
        } else if current_width + 1 + word_width > max_width {
            lines.push(std::mem::take(&mut current));
            current = word.to_string();
        } else {
            current.push(' ');
            current.push_str(word);
        }
    }

    if !current.is_empty() {
        lines.push(current);
    }

    lines.join("\n")
}
