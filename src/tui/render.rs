use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Cell, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Table, Wrap},
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

        // Profile picker overlay
        if let GlobalMode::ProfilePicker { entries, selected } = &app.global_mode {
            let area = f.area();
            let w = (area.width as f64 * 0.5).clamp(40.0, 60.0) as u16;
            let h = ((entries.len() + 8).min((area.height as usize - 4) as usize)) as u16;
            let picker_area = Rect {
                x: (area.width - w) / 2,
                y: (area.height - h) / 2,
                width: w,
                height: h,
            };

            let mut picker_lines: Vec<Line> = Vec::new();
            picker_lines.push(Line::from(Span::styled(
                " [↑/↓] Select  [Enter] Apply  [Esc] Cancel ",
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            )));
            picker_lines.push(Line::from(""));

            let builtin_names: std::collections::HashSet<&str> = ["Qwen", "Gemma", "Llama", "Mistral", "Phi"].into_iter().collect();
            for (i, (name, desc)) in entries.iter().enumerate() {
                let marker = if i == *selected { "> " } else { "  " };
                let is_builtin = builtin_names.contains(name.as_str());
                let style = if i == *selected {
                    Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                let display_name = if is_builtin {
                    format!("{} (built-in)", name)
                } else {
                    name.clone()
                };
                picker_lines.push(Line::from(vec![
                    Span::styled(marker, Style::default().fg(Color::Yellow)),
                    Span::styled(display_name, style),
                ]));
                if !desc.is_empty() {
                    picker_lines.push(Line::from(Span::styled(
                        format!("        {}", desc),
                        Style::default().fg(Color::DarkGray),
                    )));
                }
            }

            f.render_widget(ratatui::widgets::Clear, picker_area);
            f.render_widget(Paragraph::new(picker_lines).wrap(Wrap { trim: true }).block(
                Block::default()
                    .title(" Profiles ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow)),
            ), picker_area);
            return;
        }

  // Prompt picker overlay
        if let GlobalMode::PromptPicker { entries, selected, editing, edit_buffer, edit_cursor_pos, confirm_delete } = &app.global_mode {
            let area = f.area();
            let w = (area.width as f64 * 0.7).clamp(60.0, 80.0) as u16;
            let h = if *editing {
                (area.height as f64 * 0.8).clamp(25.0, 40.0) as u16
            } else {
                (area.height as f64 * 0.7).clamp(20.0, 35.0) as u16
            };
            let picker_area = Rect {
                x: (area.width - w) / 2,
                y: (area.height - h) / 2,
                width: w,
                height: h,
            };

            let mut picker_lines: Vec<Line> = Vec::new();

            // Delete confirmation
            if *confirm_delete && *selected < entries.len() {
                let name = &entries[*selected].0;
                let is_builtin = matches!(name.as_str(), "General" | "Coder" | "Thinker" | "Mathematician");
                let display_name = if is_builtin {
                    format!("{} (built-in)", name)
                } else {
                    name.clone()
                };
                picker_lines.push(Line::from(Span::styled(
                    format!(" Delete '{}'?", display_name),
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                )));
                picker_lines.push(Line::from(""));
                picker_lines.push(Line::from(Span::styled(
                    " [Y] Yes  [N] Cancel  [Esc] Cancel ",
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                )));
                picker_lines.push(Line::from(""));
            }
            // Edit mode
            else if *editing {
                picker_lines.push(Line::from(Span::styled(
                    format!(" Editing: {}", if *selected < entries.len() { &entries[*selected].0 } else { "New Preset" }),
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                )));
                picker_lines.push(Line::from(""));

                let content_lines: Vec<&str> = edit_buffer.split('\n').collect();
                let max_lines = (h as usize).saturating_sub(6);
                let cursor_pos = *edit_cursor_pos;
                let mut current_char_idx = 0usize;
                for line in content_lines.iter().take(max_lines) {
                    let line_chars: Vec<char> = line.chars().collect();
                    let line_len = line_chars.len();
                    let in_range = cursor_pos >= current_char_idx && cursor_pos <= current_char_idx + line_len;
                    
                    if in_range {
                        let pos_in_line = cursor_pos - current_char_idx;
                        let before: String = line_chars.iter().take(pos_in_line).collect();
                        let after: String = line_chars.iter().skip(pos_in_line).collect();
                        picker_lines.push(Line::from(vec![
                            Span::raw(before),
                            Span::styled("|", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                            Span::raw(after),
                        ]));
                    } else {
                        picker_lines.push(Line::from(Span::raw(line.to_string())));
                    }
                    current_char_idx += line_len + 1; // +1 for the newline
                }

                picker_lines.push(Line::from(""));
                picker_lines.push(Line::from(Span::styled(
                    "[Enter] new line  [Esc] cancel  [Ctrl+S] save",
                    Style::default().fg(Color::Cyan),
                )));
            }
            // List mode
            else {
                picker_lines.push(Line::from(Span::styled(
                    " [↑/↓] Select  [Enter] Confirm  [e] Edit  [n] New  [d] Delete  [Esc] Cancel ",
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                )));
                picker_lines.push(Line::from(""));

                let builtin_names: std::collections::HashSet<&str> = ["General", "Coder", "Thinker", "Mathematician"].into_iter().collect();
                for (i, (name, desc)) in entries.iter().enumerate() {
                    let marker = if i == *selected { "> " } else { "  " };
                    let is_builtin = builtin_names.contains(name.as_str());
                    let style = if i == *selected {
                        Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    };
                    let display_name = if is_builtin {
                        format!("{} (built-in)", name)
                    } else {
                        name.clone()
                    };
                    picker_lines.push(Line::from(vec![
                        Span::styled(marker, Style::default().fg(Color::Yellow)),
                        Span::styled(display_name, style),
                    ]));
                    if !desc.is_empty() {
                        picker_lines.push(Line::from(Span::styled(
                            format!("        {}", desc),
                            Style::default().fg(Color::DarkGray),
                        )));
                    }
                }
            }

            f.render_widget(ratatui::widgets::Clear, picker_area);
            f.render_widget(Paragraph::new(picker_lines).wrap(Wrap { trim: true }).block(
                Block::default()
                    .title(" Prompt Presets ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow)),
            ), picker_area);
            return;
        }

        // Tags modal
        if app.tags_editing {
            let area = f.area();
            let w = (area.width as f64 * 0.5).clamp(40.0, 60.0) as u16;
            let h = (app.settings.tags.len() + 8).min(area.height as usize - 4) as u16;
            let modal_area = Rect {
                x: (area.width - w) / 2,
                y: (area.height - h) / 2,
                width: w,
                height: h,
            };

            let mut modal_lines: Vec<Line> = Vec::new();
            modal_lines.push(Line::from(Span::styled(
                " Tags Editor ",
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            )));
            modal_lines.push(Line::from(""));

            // Show instructions
            if app.tags_insert_mode {
                modal_lines.push(Line::from(Span::styled(
                    " [Enter] Add tag  [Esc] Cancel  [Tab] Switch to edit mode ",
                    Style::default().fg(Color::DarkGray),
                )));
            } else {
                modal_lines.push(Line::from(Span::styled(
                    " [e/i] Edit  [d/Del] Delete  [a] Add  [Tab] Switch to add mode ",
                    Style::default().fg(Color::DarkGray),
                )));
            }
            modal_lines.push(Line::from(""));

            // Show tags
            for (i, tag) in app.settings.tags.iter().enumerate() {
                let marker = if Some(i) == app.tags_selected_idx { "> " } else { "  " };
                let style = if Some(i) == app.tags_selected_idx {
                    Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                modal_lines.push(Line::from(vec![
                    Span::styled(marker, Style::default().fg(Color::Yellow)),
                    Span::styled(tag.clone(), style),
                ]));
            }

            // Show insert/edit line
            if app.tags_insert_mode {
                modal_lines.push(Line::from(vec![
                    Span::styled(" New: ", Style::default().fg(Color::Yellow)),
                    Span::styled(&app.tags_edit_buffer, Style::default().fg(Color::Black).bg(Color::Yellow)),
                    Span::styled("_", Style::default().fg(Color::Black).bg(Color::Yellow)),
                ]));
            } else if app.tags_selected_idx.is_some() {
                modal_lines.push(Line::from(vec![
                    Span::styled(" Edit: ", Style::default().fg(Color::Yellow)),
                    Span::styled(&app.tags_edit_buffer, Style::default().fg(Color::Black).bg(Color::Yellow)),
                    Span::styled("_", Style::default().fg(Color::Black).bg(Color::Yellow)),
                ]));
            }

            f.render_widget(ratatui::widgets::Clear, modal_area);
            f.render_widget(Paragraph::new(modal_lines).block(
                Block::default()
                    .title(" Tags Editor ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow)),
            ), modal_area);
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
                    crate::models::Backend::CpuArm64 => "CPU ARM64",
                    crate::models::Backend::CpuWindows => "CPU (Windows)",
                    crate::models::Backend::VulkanWindows => "Vulkan (Windows)",
                    crate::models::Backend::CudaWindows12_4 => "CUDA 12.4 (Windows)",
                    crate::models::Backend::CudaWindows13_1 => "CUDA 13.1 (Windows)",
                    crate::models::Backend::HipWindows => "HIP Radeon (Windows)",
                    crate::models::Backend::CpuMacosArm64 => "CPU (macOS ARM64)",
                    crate::models::Backend::CpuMacosX64 => "CPU (macOS Intel)",
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

            // BenchTune setup overlay
            if let GlobalMode::BenchTuneSetup { config, selected_idx, bench_mode_selection, editing_prompt, editing_kwargs: _ } = &app.global_mode {
            let area = f.area();
            let w = 70u16;
            let h = 26u16; // Fixed height for predictability, scroll if needed (but 26 should fit all)
            let popup_area = Rect {
                x: (area.width.saturating_sub(w)) / 2,
                y: (area.height.saturating_sub(h)) / 2,
                width: w.min(area.width),
                height: h.min(area.height),
            };

            let mode_idx = *bench_mode_selection.min(&1);
            let mode_name = if mode_idx == 0 { "Runtime Only" } else { "Full (inc. load)" };

            // Main block for the popup
            let block = Block::default()
                .title(Span::styled(" Benchmark Configuration ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow));

            let inner_area = block.inner(popup_area);
            f.render_widget(ratatui::widgets::Clear, popup_area);
            f.render_widget(block, popup_area);

            let regions = Layout::vertical([
                Constraint::Length(1), // Top Spacer
                Constraint::Length(1), // Mode & Iterations
                Constraint::Length(1), // Spacer
                Constraint::Length(5), // Prompt (fixed height preview)
                Constraint::Length(1), // Spacer
                Constraint::Length(1), // Parameters Header
                Constraint::Min(0),    // Parameters Table
                Constraint::Length(3), // Footer / Shortcuts
            ]).split(inner_area);

            // Region 1: Mode & Basic Config
            let iters_display = if app.editing_iters {
                format!("{}|", app.iters_edit_buffer)
            } else {
                config.num_iterations.to_string()
            };
            
            let tokens_display = if app.editing_n_predict {
                format!("{}|", app.n_predict_edit_buffer)
            } else {
                config.n_predict.to_string()
            };

            let mode_line = Line::from(vec![
                Span::styled(" Mode: ", Style::default().fg(Color::Yellow)),
                Span::styled(mode_name, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                Span::raw(" | "),
                Span::styled("Iters: ", Style::default().fg(Color::Yellow)),
                Span::styled(iters_display, if app.editing_iters { Style::default().fg(Color::Black).bg(Color::Cyan) } else { Style::default().fg(Color::Cyan) }),
                Span::raw(" | "),
                Span::styled("Max Tokens: ", Style::default().fg(Color::Yellow)),
                Span::styled(tokens_display, if app.editing_n_predict { Style::default().fg(Color::Black).bg(Color::Cyan) } else { Style::default().fg(Color::Cyan) }),
            ]);
            f.render_widget(Paragraph::new(mode_line), regions[1]);

            // Region 3: Prompt
            let prompt_title = if *editing_prompt { " Editing Prompt... " } else { " Prompt (Alt+P to edit) " };
            let prompt_content = if config.prompt.is_empty() { "(Empty prompt)" } else { &config.prompt };
            
            let prompt_lines = if *editing_prompt {
                let mut display_text = config.prompt.clone();
                let cursor_pos = app.edit_cursor_pos.min(display_text.len());
                display_text.insert(cursor_pos, '|');
                vec![
                    Line::from(display_text),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled(format!(" [{} chars] ", config.prompt.len()), Style::default().fg(Color::Cyan)),
                        Span::styled(" [Press Esc to finish] ", Style::default().fg(Color::Yellow)),
                    ]),
                ]
            } else {
                vec![
                    Line::from(prompt_content),
                    Line::from(vec![
                        Span::styled(format!(" [{} chars] ", config.prompt.len()), Style::default().fg(Color::DarkGray)),
                    ]),
                ]
            };

            f.render_widget(
                Paragraph::new(prompt_lines)
                    .block(Block::default().title(prompt_title).borders(Borders::ALL).border_style(Style::default().fg(if *editing_prompt { Color::Green } else { Color::DarkGray })))
                    .wrap(Wrap { trim: true }),
                regions[3]
            );

            // Region 5: Parameters Header
            f.render_widget(Paragraph::new(Line::from(vec![
                Span::raw(" Select parameters to vary:"),
                Span::styled(" (Space to toggle)", Style::default().fg(Color::DarkGray)),
            ])), regions[5]);

            // Region 6: Parameters Table
            let data_rows: Vec<Row> = config.params_to_test.iter().enumerate().map(|(i, p)| {
                let marker = if i == *selected_idx { ">" } else { " " };
                let checkbox = if p.enabled { "[X]" } else { "[ ]" };
                let name = p.name.replace("_", " ");

                let desc_str = match p.name.as_str() {
                    "flash_attn" => "(On/Off)".to_string(),
                    "threads" => format!("{} to {}, step {}", p.min as u32, p.max as u32, p.step as u32),
                    "top_k" => format!("{} to {}, step {}", p.min as i32, p.max as i32, p.step as i32),
                    "expert_count" => format!("{} to {}, step {}", p.min as i32, p.max as i32, p.step as i32),
                    _ => format!("{:.1} to {:.1}, step {:.1}", p.min, p.max, p.step),
                };

                let row_style = if i == *selected_idx {
                    Style::default().fg(Color::Black).bg(Color::Yellow)
                } else {
                    Style::default().fg(Color::White)
                };

                Row::new(vec![
                    Cell::from(Span::styled(marker, Style::default().fg(Color::Yellow))),
                    Cell::from(Span::styled(checkbox, if p.enabled { Style::default().fg(Color::Green) } else { Style::default().fg(Color::DarkGray) })),
                    Cell::from(name),
                    Cell::from(Span::styled(desc_str, Style::default().fg(Color::DarkGray))),
                ]).style(row_style)
            }).collect();

            let table = Table::new(
                data_rows,
                [
                    Constraint::Length(2),
                    Constraint::Length(4),
                    Constraint::Length(16),
                    Constraint::Fill(1),
                ],
            );
            f.render_widget(table, regions[6]);

            // Region 7: Footer
            let total_tests = config.get_total_tests_count();
            let footer_lines = vec![
                Line::from(vec![
                    Span::raw(" Total tests: "),
                    Span::styled(total_tests.to_string(), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                    Span::raw("  |  "),
                    Span::styled(" [Alt+M]", Style::default().fg(Color::Yellow)), Span::raw(" Mode "),
                    Span::styled(" [Alt+N]", Style::default().fg(Color::Yellow)), Span::raw(" Tokens "),
                    Span::styled(" [Alt+I]", Style::default().fg(Color::Yellow)), Span::raw(" Iters "),
                ]),
                Line::from(vec![
                    Span::styled(" [Enter]", Style::default().fg(Color::Yellow)), Span::styled(" START ", Style::default().fg(Color::Black).bg(Color::Green)),
                    Span::raw("  "),
                    Span::styled(" [Esc]", Style::default().fg(Color::Yellow)), Span::raw(" Cancel "),
                ]),
            ];
            f.render_widget(Paragraph::new(footer_lines).alignment(ratatui::layout::Alignment::Center), regions[7]);
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
            if app.rpc_workers_scroll_offset > max_offset.into() {
                app.rpc_workers_scroll_offset = max_offset.into();
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

            // MaxConcurrentPicker overlay
            if let GlobalMode::MaxConcurrentPicker { value } = &app.global_mode {
            let area = f.area();
            let w = 55u16;
            let h = 10u16;
            let picker_area = Rect {
                x: (area.width - w) / 2,
                y: (area.height - h) / 2,
                width: w,
                height: h,
            };

            let ctx_len = app.settings.context_length;
            let entered = value.parse::<u32>().unwrap_or(0).clamp(1, 10);
            let per_model = if entered > 0 && ctx_len > 0 {
                ctx_len / entered
            } else {
                ctx_len
            };

            let mut picker_lines: Vec<Line> = Vec::new();
            picker_lines.push(Line::from(Span::styled(
                " Max Concurrent Predictions ",
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            )));
            picker_lines.push(Line::from(""));
            picker_lines.push(Line::from(vec![
                Span::raw("This divides the context length per loaded model: "),
            ]));
            picker_lines.push(Line::from(vec![
                Span::styled(ctx_len.to_string(), Style::default().fg(Color::Yellow)),
                Span::raw(" / "),
                Span::styled(format!("{}", if entered > 0 { entered } else { 1 }), Style::default().fg(Color::Cyan)),
                Span::raw(" = "),
                Span::styled(format!("{}", per_model), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::raw(" tokens per model"),
            ]));
            picker_lines.push(Line::from(""));
            picker_lines.push(Line::from(vec![
                Span::raw("Value: "),
                Span::styled(value.as_str(), Style::default().fg(Color::Black).bg(Color::Yellow)),
            ]));
            picker_lines.push(Line::from(""));
            picker_lines.push(Line::from(vec![
                Span::styled("  [Enter] confirm  ", Style::default().fg(Color::Black).bg(Color::Yellow)),
                Span::raw("  "),
                Span::styled("  [Esc] cancel  ", Style::default().fg(Color::Black).bg(Color::DarkGray)),
            ]));

            f.render_widget(ratatui::widgets::Clear, picker_area);
            f.render_widget(Paragraph::new(picker_lines).block(
                Block::default()
                    .title(" Max Concurrent Predictions ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow)),
            ), picker_area);
            return;
            }

            // BenchTune output view modal (fullscreen)
            if let Some(result_idx) = app.bench_tune_output_view {
                let results = app.bench_tune_results.clone();
                if let Some(result) = results.get(result_idx) {
                    let area = f.area();
                    let modal_area = Rect {
                        x: 0,
                        y: 0,
                        width: area.width,
                        height: area.height,
                    };

                    // Clear the entire area first
                    f.render_widget(ratatui::widgets::Clear, modal_area);

                    // Main Title - show what parameters were varied for this result
                    let mut p_parts = Vec::new();
                    if let Some(v) = result.params.temperature { p_parts.push(format!("temp={:.1}", v)); }
                    if let Some(v) = result.params.top_p { p_parts.push(format!("top_p={:.1}", v)); }
                    if let Some(v) = result.params.threads { p_parts.push(format!("th={}", v)); }
                    if let Some(v) = result.params.batch_size { p_parts.push(format!("bs={}", v)); }
                    if let Some(v) = result.params.expert_count { p_parts.push(format!("experts={}", v)); }
                    if let Some(v) = result.params.flash_attn { p_parts.push(format!("fa={}", if v { "on" } else { "off" })); }
                    
                    let p_str = if p_parts.is_empty() { "Baseline".to_string() } else { p_parts.join(", ") };

                    let main_title = Line::from(vec![
                        Span::styled(" BenchTune Result: ", Style::default().fg(Color::Yellow)),
                        Span::styled(p_str, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                    ]);

                    // Parameters table
                    let settings = result.base_settings.as_ref();
                    let param_rows: Vec<Row> = vec![
                        Row::new(vec![
                            Cell::from(Span::styled("temperature", Style::default().fg(Color::Yellow))),
                            Cell::from(Span::styled(settings.map(|s| format!("{:.2}", s.temperature)).unwrap_or_else(|| result.params.temperature.map(|v| format!("{:.2}", v)).unwrap_or_else(|| "-".to_string())), Style::default().fg(Color::Cyan))),
                        ]),
                        Row::new(vec![
                            Cell::from(Span::styled("top_p", Style::default().fg(Color::Yellow))),
                            Cell::from(Span::styled(settings.map(|s| format!("{:.2}", s.top_p)).unwrap_or_else(|| result.params.top_p.map(|v| format!("{:.2}", v)).unwrap_or_else(|| "-".to_string())), Style::default().fg(Color::Cyan))),
                        ]),
                        Row::new(vec![
                            Cell::from(Span::styled("top_k", Style::default().fg(Color::Yellow))),
                            Cell::from(Span::styled(settings.map(|s| s.top_k.to_string()).unwrap_or_else(|| result.params.top_k.map(|v| v.to_string()).unwrap_or_else(|| "-".to_string())), Style::default().fg(Color::Cyan))),
                        ]),
                        Row::new(vec![
                            Cell::from(Span::styled("repeat_penalty", Style::default().fg(Color::Yellow))),
                            Cell::from(Span::styled(settings.map(|s| format!("{:.2}", s.repeat_penalty)).unwrap_or_else(|| result.params.repeat_penalty.map(|v| format!("{:.2}", v)).unwrap_or_else(|| "-".to_string())), Style::default().fg(Color::Cyan))),
                        ]),
                        Row::new(vec![
                            Cell::from(Span::styled("context_length", Style::default().fg(Color::Yellow))),
                            Cell::from(Span::styled(settings.map(|s| s.context_length.to_string()).unwrap_or_else(|| result.params.context_length.map(|v| v.to_string()).unwrap_or_else(|| "-".to_string())), Style::default().fg(Color::Cyan))),
                        ]),
                        Row::new(vec![
                            Cell::from(Span::styled("batch_size", Style::default().fg(Color::Yellow))),
                            Cell::from(Span::styled(settings.map(|s| s.batch_size.to_string()).unwrap_or_else(|| result.params.batch_size.map(|v| v.to_string()).unwrap_or_else(|| "-".to_string())), Style::default().fg(Color::Cyan))),
                        ]),
                        Row::new(vec![
                            Cell::from(Span::styled("flash_attn", Style::default().fg(Color::Yellow))),
                            Cell::from(Span::styled(settings.map(|s| if s.flash_attn { "on".to_string() } else { "off".to_string() }).unwrap_or_else(|| result.params.flash_attn.map(|v| if v { "on".to_string() } else { "off".to_string() }).unwrap_or_else(|| "-".to_string())), Style::default().fg(Color::Cyan))),
                        ]),
                        Row::new(vec![
                            Cell::from(Span::styled("threads", Style::default().fg(Color::Yellow))),
                            Cell::from(Span::styled(settings.map(|s| s.threads.to_string()).unwrap_or_else(|| result.params.threads.map(|v| v.to_string()).unwrap_or_else(|| "-".to_string())), Style::default().fg(Color::Cyan))),
                        ]),
                        Row::new(vec![
                            Cell::from(Span::styled("expert_count", Style::default().fg(Color::Yellow))),
                            Cell::from(Span::styled(settings.map(|s| s.expert_count.to_string()).unwrap_or_else(|| result.params.expert_count.map(|v| v.to_string()).unwrap_or_else(|| "-".to_string())), Style::default().fg(Color::Cyan))),
                        ]),
                    ];

                    let params_table = Table::new(param_rows, [
                        Constraint::Fill(1),
                        Constraint::Fill(1),
                    ])
                    .header(Row::new(vec![
                        Cell::from(Span::styled("Parameter", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
                        Cell::from(Span::styled("Value", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
                    ]))
                    .block(Block::default()
                        .title(" Parameters ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Cyan)));

                    // Metrics table (for the selected output)
                    let output_idx = app.bench_tune_output_index.min(result.outputs.len().saturating_sub(1));
                    let metrics_for_output = result.per_iteration_metrics.get(output_idx).unwrap_or(&result.metrics);

                    let metric_rows: Vec<Row> = vec![
                        Row::new(vec![
                            Cell::from(Span::styled("prompt_tps", Style::default().fg(Color::Green))),
                            Cell::from(Span::styled(format!("{:.2}", metrics_for_output.prompt_tps), Style::default().fg(Color::Cyan))),
                        ]),
                        Row::new(vec![
                            Cell::from(Span::styled("gen_tps", Style::default().fg(Color::Green))),
                            Cell::from(Span::styled(format!("{:.2}", metrics_for_output.generation_tps), Style::default().fg(Color::Cyan))),
                        ]),
                        Row::new(vec![
                            Cell::from(Span::styled("combined_tps", Style::default().fg(Color::Green))),
                            Cell::from(Span::styled(format!("{:.2}", metrics_for_output.combined_tps), Style::default().fg(Color::Cyan))),
                        ]),
                        Row::new(vec![
                            Cell::from(Span::styled("latency/token", Style::default().fg(Color::Green))),
                            Cell::from(Span::styled(format!("{:.2} ms", metrics_for_output.latency_per_token), Style::default().fg(Color::Cyan))),
                        ]),
                        Row::new(vec![
                            Cell::from(Span::styled("first_token", Style::default().fg(Color::Green))),
                            Cell::from(Span::styled(format!("{:.2} ms", metrics_for_output.first_token_time), Style::default().fg(Color::Cyan))),
                        ]),
                    ];

                    let metrics_table = Table::new(metric_rows, [
                        Constraint::Fill(1),
                        Constraint::Fill(1),
                    ])
                    .header(Row::new(vec![
                        Cell::from(Span::styled("Metric", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))),
                        Cell::from(Span::styled("Value", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))),
                    ]))
                    .block(Block::default()
                        .title(format!(" Metrics (Iter {}/{}) ", output_idx + 1, result.outputs.len()))
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Cyan)));

                    // Output section
                    let output_lines: Vec<Line> = if !result.outputs.is_empty() {
                        let output = &result.outputs[output_idx];
                        output.lines().map(|l| Line::from(l.to_string())).collect()
                    } else {
                        vec![Line::from("No output captured.")]
                    };

                    // Layout Definition
                    // y=0: Title
                    // y=1..11: Params (left) and Metrics (right)
                    // y=12..area.height-1: Output (full width)
                    // y=area.height-1: Controls

                    let title_area = Rect { x: 0, y: 0, width: area.width, height: 1 };
                    f.render_widget(Paragraph::new(main_title).alignment(ratatui::layout::Alignment::Center), title_area);

                    let top_height = 11; // enough for 9 params + header + borders
                    let left_width = area.width / 2;
                    let right_width = area.width - left_width;

                    let left_area = Rect {
                        x: 0,
                        y: 1,
                        width: left_width,
                        height: top_height,
                    };
                    f.render_widget(params_table, left_area);

                    let metrics_area = Rect {
                        x: left_width,
                        y: 1,
                        width: right_width,
                        height: top_height,
                    };
                    f.render_widget(metrics_table, metrics_area);

                    // Output area (Full Width)
                    let output_y = 1 + top_height;
                    let controls_height = 1;
                    if output_y < area.height.saturating_sub(controls_height) {
                        let output_area = Rect {
                            x: 0,
                            y: output_y,
                            width: area.width,
                            height: area.height.saturating_sub(output_y + controls_height),
                        };

                        let output_block = Block::default()
                            .title(" Captured Output ")
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(Color::Cyan));

                        if !output_lines.is_empty() {
                            let scroll = app.bench_tune_output_scroll as u16;
                            f.render_widget(
                                Paragraph::new(output_lines)
                                    .block(output_block)
                                    .wrap(ratatui::widgets::Wrap { trim: false })
                                    .scroll((scroll, 0)),
                                output_area
                            );
                        } else {
                            f.render_widget(Paragraph::new(output_lines).block(output_block), output_area);
                        }
                    }

                    // Calculate absolute index and total across all results
                    let mut absolute_idx = 0;
                    let mut total_outputs = 0;
                    for (r_idx, r) in results.iter().enumerate() {
                        if r_idx < result_idx {
                            absolute_idx += r.outputs.len();
                        }
                        total_outputs += r.outputs.len();
                    }
                    absolute_idx += output_idx + 1;

                    // Controls line
                    let controls = Line::from(vec![
                        Span::styled("  [Esc] close  ", Style::default().fg(Color::Black).bg(Color::Yellow)),
                        Span::raw("  "),
                        Span::styled("[j/k] scroll  ", Style::default().fg(Color::Yellow)),
                        Span::raw("  "),
                        Span::styled(format!("[p] prev({}/{}) ", absolute_idx, total_outputs), Style::default().fg(Color::Yellow)),
                        Span::raw("  "),
                        Span::styled("[n] next  ", Style::default().fg(Color::Yellow)),
                    ]);
                    let controls_area = Rect {
                        x: 0,
                        y: area.height.saturating_sub(1),
                        width: area.width,
                        height: 1,
                    };
                    f.render_widget(Paragraph::new(controls).alignment(ratatui::layout::Alignment::Center), controls_area);
                    return;
                }
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
            if app.profiles_scroll_offset > max_offset.into() {
                app.profiles_scroll_offset = max_offset.into();
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
            if app.system_prompt_presets_scroll_offset > max_offset.into() {
                app.system_prompt_presets_scroll_offset = max_offset.into();
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
        
        let downloads_focused = app.active_panel == ActivePanel::Downloads;
        
        panel::log::render(f, bottom_chunks[0], app);
        
        let total_speed: f64 = app.download_progress.iter().map(|d| d.bytes_per_second).sum();
        panel::models::render_download_panel(
            f, bottom_chunks[1],
            &app.download_progress,
            total_speed,
            &mut app.download_scroll_state,
            downloads_focused,
        );
    } else if log_visible {
        panel::log::render(f, bottom_area, app);
    } else if app.downloading {
        let total_speed: f64 = app.download_progress.iter().map(|d| d.bytes_per_second).sum();
        let downloads_focused = app.active_panel == ActivePanel::Downloads;
        panel::models::render_download_panel(
            f, bottom_area,
            &app.download_progress,
            total_speed,
            &mut app.download_scroll_state,
            downloads_focused,
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
                if app.is_settings_dirty() {
                    parts.push(Span::raw("  "));
                    parts.push(Span::styled("*unsaved*", r));
                    parts.push(Span::raw("  "));
                }
                parts.push(Span::styled("Ctrl+P profiles", y));
                parts.push(Span::raw("  "));
                parts.push(Span::styled("⇥ panels", c));
                parts.push(Span::raw("  "));
                parts.push(Span::styled("A about", c));
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
                            Span::styled("⌃H help", c),
                            Span::raw("  "),
                            Span::styled("A about", c),
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
                            Span::styled("⇥ panels", c),
                            Span::raw("  "),
                            Span::styled("A about", c),
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
                            Span::styled("⎋ done", c),
                            Span::raw("  "),
                            Span::styled("⇥ panels", c),
                            Span::raw("  "),
                            Span::styled("A about", c),
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
                            Span::styled("⎋ done", c),
                            Span::raw("  "),
                            Span::styled("⇥ panels", c),
                            Span::raw("  "),
                            Span::styled("A about", c),
                        ]
                    }
                    crate::tui::app::ActivePanel::SearchReadme => {
                        vec![
                            Span::styled("j/k nav", c),
                            Span::raw("  "),
                            Span::styled("⎋ collapse", c),
                            Span::raw("  "),
                            Span::styled("⇥ panels", c),
                            Span::raw("  "),
                            Span::styled("A about", c),
                        ]
                    }
                    crate::tui::app::ActivePanel::Downloads => {
                        vec![
                            Span::styled("j/k nav", c),
                            Span::raw("  "),
                            Span::styled("p pause", y),
                            Span::raw("  "),
                            Span::styled("⌃C cancel", y),
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
                            Span::styled("⌃H help", c),
                            Span::raw("  "),
                            Span::styled("A about", c),
                        ]
                    }
                };
                parts
            }
        }
        crate::tui::app::ModelsMode::BenchTune => {
            if app.bench_tune_progress.is_some() && matches!(app.bench_tune_progress.as_ref().unwrap(), crate::models::BenchTuneProgress::Running { .. }) {
                vec![
                    Span::styled("⎋ stop", r),
                    Span::raw("  "),
                    Span::styled("⇥ panels", c),
                ]
            } else if !app.bench_tune_results.is_empty() {
                vec![
                    Span::styled("↵ view output", y),
                    Span::raw("  "),
                    Span::styled("⎋ stop", r),
                    Span::raw("  "),
                    Span::styled("⇥ panels", c),
                ]
            } else {
                vec![
                    Span::styled("⎋ stop", r),
                    Span::raw("  "),
                    Span::styled("⇥ panels", c),
                ]
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
        ModelsMode::BenchTune => "BenchTune".to_string(),
    };
    parts.push(Span::styled(format!("[Mode: {}] ", mode_name), Style::default().fg(Color::DarkGray)));

    if let Some(handle) = &app.server_handle {
        let label = if app.server_mode == crate::models::ServerMode::Bench {
            "BENCHMARKING".to_string()
        } else {
            format!("{} {}", handle.port, app.server_mode)
        };
        parts.push(Span::styled(format!("● {}", label), Style::default().fg(Color::Green)));
    } else if app.server_mode == crate::models::ServerMode::BenchTune {
        // Show benchmark tuning status
        if let Some(progress) = &app.bench_tune_progress {
            match progress {
                crate::models::BenchTuneProgress::Running { current, total, progress, current_params: _ } => {
                    let progress_str = format!("BENCH TUNE {}/{} ({:.0}%)", current, total, progress);
                    parts.push(Span::styled(format!("● {}", progress_str), Style::default().fg(Color::Yellow)));
                }
                crate::models::BenchTuneProgress::Completed { total_tests, successful_tests, elapsed } => {
                    let elapsed_str = format!("{}s", elapsed.as_secs());
                    let progress_str = format!("BENCH TUNE COMPLETED ({}/{}) in {}", total_tests, successful_tests, elapsed_str);
                    parts.push(Span::styled(format!("● {}", progress_str), Style::default().fg(Color::Green)));
                }
                crate::models::BenchTuneProgress::Error { error } => {
                    parts.push(Span::styled(format!("● BENCH TUNE ERROR: {}", error), Style::default().fg(Color::Red)));
                }
            }
        } else {
            parts.push(Span::styled("● BENCH TUNE READY", Style::default().fg(Color::Yellow)));
        }
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

    if let GlobalMode::BenchTuneSetup { editing_prompt, .. } = &app.global_mode {
        parts.push(Span::raw("  "));
        if *editing_prompt {
            parts.push(Span::styled("[EDITING PROMPT]", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));
        } else {
            parts.push(Span::styled("[BENCH SETUP]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
        }
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
                crate::tui::app::ActivePanel::Downloads => "DOWNLOADS",
                _ => "APP",
            };
            parts.push(Span::styled(panel_label, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
        }
        crate::tui::app::ModelsMode::BenchTune => {
            parts.push(Span::raw("  "));
            parts.push(Span::styled("BENCHTUNE", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
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
