use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, Wrap},
};
use unicode_width::UnicodeWidthStr;

use super::App;
use crate::backend::hardware::{GpuVendor, detect_gpu_models, detect_gpu_vendors};
use crate::tui::app::{ConfirmationKind, GlobalMode};
use crate::tui::format_bench_params;
use crate::tui::panel;
use crate::tui::render_vertical_scrollbar;
use crate::tui::settings::profile_settings_parts;

pub fn render_overlays(f: &mut Frame, app: &mut App) -> bool {
    if app.ui.panel_help {
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
        return true;
    }

    if let GlobalMode::CmdLine { cmd_line } = &app.ui.global_mode {
        let area = f.area();
        let max_width = (area.width - 2).max(10) as usize;
        let wrapped = wrap_text(cmd_line, max_width);
        let text = Text::from(wrapped);
        let block = Block::default()
            .title(" CmdLine — ⎋ to close  e to export ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow));
        f.render_widget(Paragraph::new(text).block(block), area);
        return true;
    }

    if let GlobalMode::Confirmation { selected, kind } = &app.ui.global_mode {
        render_confirmation(f, f.area(), app, *selected, *kind);
        return true;
    }

    if let GlobalMode::HostPicker { entries, selected } = &app.ui.global_mode {
        render_host_picker(f, f.area(), entries, *selected);
        return true;
    }

    if let GlobalMode::ProfilePicker {
        entries,
        selected,
        profiles,
    } = &app.ui.global_mode
    {
        let selected_profile = profiles.get(*selected);
        render_profile_picker(
            f,
            f.area(),
            entries,
            *selected,
            &app.settings,
            selected_profile,
        );
        return true;
    }

    if let GlobalMode::PromptPicker {
        entries,
        selected,
        editing,
        edit_buffer,
        edit_cursor_pos,
        confirm_delete,
    } = &app.ui.global_mode
    {
        render_prompt_picker(
            f,
            f.area(),
            entries,
            *selected,
            *editing,
            edit_buffer,
            *edit_cursor_pos,
            *confirm_delete,
        );
        return true;
    }

    if app.edit.tags_editing {
        render_tags(f, f.area(), app);
        return true;
    }

    if let GlobalMode::BackendPicker { entries, selected } = &app.ui.global_mode {
        render_backend_picker(f, f.area(), entries, *selected);
        return true;
    }

    if let GlobalMode::BenchTuneSetup {
        config,
        selected_idx,
        editing_param,
        editing_param_field,
        param_edit_buffer,
        param_edit_cursor_pos,
        bench_mode_selection,
        editing_prompt,
        editing_kwargs: _,
    } = &app.ui.global_mode
    {
        render_bench_tune_setup(
            f,
            f.area(),
            app,
            config,
            *selected_idx,
            *editing_param,
            *editing_param_field,
            param_edit_buffer,
            *param_edit_cursor_pos,
            *bench_mode_selection,
            *editing_prompt,
        );
        return true;
    }

    if matches!(app.ui.global_mode, GlobalMode::RpcManager) {
        render_rpc_manager(f, f.area(), app);
        return true;
    }

    if matches!(app.ui.global_mode, GlobalMode::About) {
        render_about_overlay(f, f.area());
        return true;
    }

    if let GlobalMode::MaxConcurrentPicker { value } = &app.ui.global_mode {
        render_max_concurrent_picker(f, f.area(), app, value);
        return true;
    }

    if let GlobalMode::SpecTypePicker { entries, selected } = &app.ui.global_mode {
        render_spec_type_picker(f, f.area(), app, entries, *selected);
        return true;
    }

    if let GlobalMode::DashboardPicker {
        enabled,
        port,
        auth_key,
        tls_enabled,
        tls_cert,
        tls_key,
        selected_field,
        editing,
        edit_buffer,
        edit_cursor_pos: _,
    } = &app.ui.global_mode
    {
        render_dashboard_picker(
            f,
            f.area(),
            app,
            *enabled,
            port,
            auth_key,
            *tls_enabled,
            tls_cert,
            tls_key,
            *selected_field,
            *editing,
            edit_buffer,
        );
        return true;
    }

    if let GlobalMode::YarnRoPESettings {
        scale,
        freq_base,
        freq_scale,
        selected_field,
        editing,
        edit_buffer,
        edit_cursor_pos,
    } = &app.ui.global_mode
    {
        render_yarn_rope_picker(
            f,
            f.area(),
            app,
            scale,
            freq_base,
            freq_scale,
            *selected_field,
            *editing,
            edit_buffer,
            *edit_cursor_pos,
        );
        return true;
    }

    if let GlobalMode::DashboardUrl {
        host,
        port,
        auth_key,
        ws_enabled,
        tls_enabled,
    } = &app.ui.global_mode
    {
        render_dashboard_url(f, f.area(), app, host, port, auth_key, *ws_enabled, *tls_enabled);
        return true;
    }

    if let GlobalMode::SearchInput { buffer, cursor_pos } = &app.ui.global_mode {
        render_search_input(f, f.area(), buffer, *cursor_pos);
        return true;
    }

    if let Some(result_idx) = app.bench_tune.bench_tune_output_view {
        render_bench_tune_output(f, f.area(), app, result_idx);
        return true;
    }

    false
}

fn render_confirmation(
    f: &mut Frame,
    area: Rect,
    app: &App,
    selected: bool,
    kind: ConfirmationKind,
) {
    let popup_area = Rect {
        x: area.width.saturating_sub(50) / 2,
        y: area.height.saturating_sub(8) / 2,
        width: 50,
        height: 8,
    };
    let (title, text_lines) = match kind {
        ConfirmationKind::Exit => {
            let loaded_count = app
                .model_states
                .values()
                .filter(|s| matches!(s, crate::models::ModelState::Loaded { .. }))
                .count();
            (
                " Exit Application? ",
                vec![
                    Line::from(""),
                    Line::from(vec![
                        Span::raw("There are "),
                        Span::styled(
                            format!("{}", loaded_count),
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(" model(s) loaded."),
                    ]),
                    Line::from("Are you sure you want to exit?"),
                ],
            )
        }
        ConfirmationKind::Reset => (
            " Reset Settings? ",
            vec![
                Line::from(""),
                Line::from("Reset all LLM settings to defaults?"),
            ],
        ),
        ConfirmationKind::Delete => {
            let model_name = app
                .selected_model()
                .map(|m| m.name.as_str())
                .unwrap_or("Unknown");
            (
                " Delete Model? ",
                vec![
                    Line::from(""),
                    Line::from(vec![
                        Span::raw("Are you sure you want to delete "),
                        Span::styled(
                            model_name,
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::raw("?"),
                    ]),
                    Line::from(""),
                    Line::from("This action cannot be undone."),
                ],
            )
        }
        ConfirmationKind::Unload => {
            let model_name = match &app.pending.pending_api_unload {
                Some((name, _)) => name.as_str(),
                None => "Unknown",
            };
            (
                " Unload Model? ",
                vec![
                    Line::from(""),
                    Line::from(vec![
                        Span::raw("Unload "),
                        Span::styled(
                            model_name,
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::raw("?"),
                    ]),
                ],
            )
        }
        ConfirmationKind::DeleteBackend => {
            let (backend, tag) = match &app.pending.pending_backend_deletion {
                Some((b, t)) => (b.to_string(), t.as_str()),
                None => ("Unknown".to_string(), "latest"),
            };
            (
                " Delete Backend? ",
                vec![
                    Line::from(""),
                    Line::from(vec![
                        Span::raw("Delete backend "),
                        Span::styled(
                            format!("{} ({})", backend, tag),
                            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                        ),
                        Span::raw("?"),
                    ]),
                    Line::from(""),
                    Line::from("This will remove the binary and shared libraries."),
                ],
            )
        }
    };
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(
            if kind == ConfirmationKind::Delete || kind == ConfirmationKind::DeleteBackend {
                Color::Red
            } else {
                Color::Yellow
            },
        ));
    let mut lines = text_lines;
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(
            "  [y] Yes  ",
            Style::default().fg(Color::Black).bg(if selected {
                Color::Yellow
            } else {
                Color::DarkGray
            }),
        ),
        Span::raw("    "),
        Span::styled(
            "  [n] No   ",
            Style::default().fg(Color::Black).bg(if selected {
                Color::DarkGray
            } else {
                Color::Yellow
            }),
        ),
    ]));
    f.render_widget(Clear, popup_area);
    f.render_widget(
        Paragraph::new(lines)
            .block(block)
            .alignment(Alignment::Center),
        popup_area,
    );
}

fn render_host_picker(f: &mut Frame, area: Rect, entries: &[(String, String)], selected: usize) {
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
        " [↑] Select Host Address  [d] Refresh  [j/k] nav  [⎋] cancel ",
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )));
    picker_lines.push(Line::from(""));
    for (i, (ip, iface)) in entries.iter().enumerate() {
        let marker = if i == selected { "> " } else { "  " };
        let style = if i == selected {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        picker_lines.push(Line::from(vec![
            Span::styled(marker, Style::default().fg(Color::Yellow)),
            Span::styled(ip.to_string(), style),
            Span::raw("  "),
            Span::styled(format!("({iface})"), Style::default().fg(Color::DarkGray)),
        ]));
    }
    f.render_widget(Clear, picker_area);
    f.render_widget(
        Paragraph::new(picker_lines).block(
            Block::default()
                .title(" Host Picker ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        ),
        picker_area,
    );
}

fn render_profile_picker(
    f: &mut Frame,
    area: Rect,
    entries: &[(String, String)],
    selected: usize,
    current_settings: &crate::models::ModelSettings,
    selected_profile: Option<&crate::config::Profile>,
) {
    let w = (area.width as f64 * 0.5).clamp(40.0, 60.0) as u16;
    let mut picker_lines: Vec<Line> = Vec::new();
    picker_lines.push(Line::from(Span::styled(
            " [↑/↓] Select  [↵] Apply  [⎋] Cancel ",
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )));
    picker_lines.push(Line::from(""));
    let builtin_names: std::collections::HashSet<&str> = [
        "Qwen",
        "Qwen-MoE",
        "Qwen-Coding",
        "Gemma",
        "Llama",
        "Mistral",
        "Phi",
    ]
    .into_iter()
    .collect();
    for (i, (name, desc)) in entries.iter().enumerate() {
        let marker = if i == selected { "> " } else { "  " };
        let is_builtin = builtin_names.contains(name.as_str());
        let style = if i == selected {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
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
    if let Some(profile) = selected_profile {
        let preview_parts = profile_settings_parts(profile, current_settings);
        picker_lines.push(Line::from(Span::styled(
            "────────────────────────────────────────────────────────",
            Style::default().fg(Color::DarkGray),
        )));
        picker_lines.push(Line::from(Span::styled(
            " Changed settings:",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )));
        if preview_parts.is_empty() {
            picker_lines.push(Line::from(Span::styled(
                " (no changes)",
                Style::default().fg(Color::DarkGray),
            )));
        } else {
            for part in preview_parts {
                picker_lines.push(Line::from(Span::styled(
                    format!("    {}", part),
                    Style::default().fg(Color::Cyan),
                )));
            }
        }
    }
    let content_height = picker_lines.len();
    let max_h = (area.height as usize - 4).max(10);
    let h = (content_height.min(max_h)) as u16;
    let picker_area = Rect {
        x: (area.width - w) / 2,
        y: (area.height - h) / 2,
        width: w,
        height: h,
    };
    f.render_widget(Clear, picker_area);
    f.render_widget(
        Paragraph::new(picker_lines)
            .wrap(Wrap { trim: true })
            .block(
                Block::default()
                    .title(" Profiles ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow)),
            ),
        picker_area,
    );
}

fn render_prompt_picker(
    f: &mut Frame,
    area: Rect,
    entries: &[(String, String)],
    selected: usize,
    editing: bool,
    edit_buffer: &str,
    _edit_cursor_pos: usize,
    confirm_delete: bool,
) {
    let w = (area.width as f64 * 0.7).clamp(60.0, 80.0) as u16;
    let h = if editing {
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
    if confirm_delete && selected < entries.len() {
        let name = &entries[selected].0;
        let is_builtin = matches!(
            name.as_str(),
            "General" | "Coder" | "Thinker" | "Mathematician"
        );
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
            " [Y] Yes  [N] Cancel  [⎋] Cancel ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )));
        picker_lines.push(Line::from(""));
    } else if editing {
        picker_lines.push(Line::from(Span::styled(
            format!(
                " Editing: {}",
                if selected < entries.len() {
                    &entries[selected].0
                } else {
                    "New Preset"
                }
            ),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )));
        picker_lines.push(Line::from(""));
        let content_lines: Vec<&str> = edit_buffer.split('\n').collect();
        let max_lines = (h as usize).saturating_sub(6);
        let cursor_pos = _edit_cursor_pos;
        let mut current_char_idx = 0usize;
        for line in content_lines.iter().take(max_lines) {
            let line_chars: Vec<char> = line.chars().collect();
            let line_len = line_chars.len();
            let in_range =
                cursor_pos >= current_char_idx && cursor_pos <= current_char_idx + line_len;
            if in_range {
                let pos_in_line = cursor_pos - current_char_idx;
                let before: String = line_chars.iter().take(pos_in_line).collect();
                let after: String = line_chars.iter().skip(pos_in_line).collect();
                picker_lines.push(Line::from(vec![
                    Span::raw(before),
                    Span::styled(
                        "|",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(after),
                ]));
            } else {
                picker_lines.push(Line::from(Span::raw(line.to_string())));
            }
            current_char_idx += line_len + 1;
        }
        picker_lines.push(Line::from(""));
        picker_lines.push(Line::from(Span::styled(
            "[↵] new line  [⎋] cancel  [^S] save",
            Style::default().fg(Color::Cyan),
        )));
    } else {
        picker_lines.push(Line::from(Span::styled(
            " [↑/↓] Select  [↵] Confirm  [e] Edit  [n] New  [d] Delete  [⎋] Cancel ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )));
        picker_lines.push(Line::from(""));
        let builtin_names: std::collections::HashSet<&str> =
            ["General", "Coder", "Thinker", "Mathematician"]
                .into_iter()
                .collect();
        for (i, (name, desc)) in entries.iter().enumerate() {
            let marker = if i == selected { "> " } else { "  " };
            let is_builtin = builtin_names.contains(name.as_str());
            let style = if i == selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
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
    f.render_widget(Clear, picker_area);
    f.render_widget(
        Paragraph::new(picker_lines)
            .wrap(Wrap { trim: true })
            .block(
                Block::default()
                    .title(" Prompt Presets ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow)),
            ),
        picker_area,
    );
}

fn render_tags(f: &mut Frame, area: Rect, app: &App) {
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
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )));
    modal_lines.push(Line::from(""));
    if app.edit.tags_insert_mode {
        modal_lines.push(Line::from(Span::styled(
            " [↵] Add tag  [⎋] Cancel  [⇥] Switch to edit mode ",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        modal_lines.push(Line::from(Span::styled(
            " [e/i] Edit  [d/Del] Delete  [a] Add  [⇥] Switch to add mode ",
            Style::default().fg(Color::DarkGray),
        )));
    }
    modal_lines.push(Line::from(""));
    for (i, tag) in app.settings.tags.iter().enumerate() {
        let marker = if Some(i) == app.edit.tags_selected_idx {
            "> "
        } else {
            "  "
        };
        let style = if Some(i) == app.edit.tags_selected_idx {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        modal_lines.push(Line::from(vec![
            Span::styled(marker, Style::default().fg(Color::Yellow)),
            Span::styled(tag.clone(), style),
        ]));
    }
    if app.edit.tags_insert_mode {
        modal_lines.push(Line::from(vec![
            Span::styled(" New: ", Style::default().fg(Color::Yellow)),
            Span::styled(
                &app.edit.tags_edit_buffer,
                Style::default().fg(Color::Black).bg(Color::Yellow),
            ),
            Span::styled("_", Style::default().fg(Color::Black).bg(Color::Yellow)),
        ]));
    } else if app.edit.tags_selected_idx.is_some() {
        modal_lines.push(Line::from(vec![
            Span::styled(" Edit: ", Style::default().fg(Color::Yellow)),
            Span::styled(
                &app.edit.tags_edit_buffer,
                Style::default().fg(Color::Black).bg(Color::Yellow),
            ),
            Span::styled("_", Style::default().fg(Color::Black).bg(Color::Yellow)),
        ]));
    }
    f.render_widget(Clear, modal_area);
    f.render_widget(
        Paragraph::new(modal_lines).block(
            Block::default()
                .title(" Tags Editor ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        ),
        modal_area,
    );
}

fn render_backend_picker(
    f: &mut Frame,
    area: Rect,
    entries: &[(crate::models::Backend, Option<String>)],
    selected: usize,
) {
    let w = (area.width as f64 * 0.5).clamp(50.0, 70.0) as u16;
    let all_models = detect_gpu_models();
    let gpu_info_lines = if all_models.iter().any(|m| m.is_some()) {
        1
    } else {
        0
    };
    let h = (entries.len() + 4 + gpu_info_lines).min(area.height as usize - 4) as u16;
    let picker_area = Rect {
        x: (area.width - w) / 2,
        y: (area.height - h) / 2,
        width: w,
        height: h,
    };
    let vendors = detect_gpu_vendors();
    let mut picker_lines: Vec<Line> = Vec::new();
    picker_lines.push(Line::from(Span::styled(
        " Select Backend Acceleration ",
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )));
    let gpu_models: Vec<String> = all_models.iter().filter_map(|m| m.clone()).collect();
    if !gpu_models.is_empty() {
        picker_lines.push(Line::from(vec![
            Span::raw("Detected Hardware: "),
            Span::styled(gpu_models.join(", "), Style::default().fg(Color::Cyan)),
        ]));
    }
    picker_lines.push(Line::from(""));
    for (i, (backend, tag)) in entries.iter().enumerate() {
        let marker = if i == selected { "> " } else { "  " };
        let is_installed = if tag.is_some() {
            true
        } else {
            crate::backend::hub::is_backend_any_version_installed(*backend)
        };
        let is_recommended = vendors.iter().any(|v| matches!(
            (v, backend, tag),
            (GpuVendor::Amd, crate::models::Backend::Rocm, None)
                | (GpuVendor::Amd, crate::models::Backend::RocmLemonade, None)
                | (GpuVendor::Nvidia, crate::models::Backend::Cuda, None)
                | (GpuVendor::Nvidia, crate::models::Backend::Vulkan, None)
                | (GpuVendor::Intel, crate::models::Backend::Vulkan, None)
                | (GpuVendor::Unknown, crate::models::Backend::Cpu, None)
        ));
        let style = if i == selected {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
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
            line_spans.push(Span::styled(
                "(Recommended)",
                Style::default().fg(Color::Green),
            ));
        }
        picker_lines.push(Line::from(line_spans));
    }
    f.render_widget(Clear, picker_area);
    f.render_widget(
        Paragraph::new(picker_lines).block(
            Block::default()
                .title(" Backend Picker ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        ),
        picker_area,
    );
}

fn render_bench_tune_setup(
    f: &mut Frame,
    area: Rect,
    app: &App,
    config: &crate::models::BenchTuneConfig,
    selected_idx: usize,
    editing_param: bool,
    editing_param_field: i32,
    param_edit_buffer: &str,
    param_edit_cursor_pos: usize,
    bench_mode_selection: usize,
    editing_prompt: bool,
) {
    let w = 90u16;
    let h = 30u16;
    let popup_area = Rect {
        x: (area.width.saturating_sub(w)) / 2,
        y: (area.height.saturating_sub(h)) / 2,
        width: w.min(area.width),
        height: h.min(area.height),
    };
    let mode_idx = bench_mode_selection.min(1);
    let mode_name = if mode_idx == 0 {
        "Runtime Only"
    } else {
        "Full (inc. load)"
    };
    let block = Block::default()
        .title(Span::styled(
            " Benchmark Configuration ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    let inner_area = block.inner(popup_area);
    f.render_widget(Clear, popup_area);
    f.render_widget(block, popup_area);
    let regions = ratatui::layout::Layout::vertical([
        ratatui::layout::Constraint::Length(1),
        ratatui::layout::Constraint::Length(1),
        ratatui::layout::Constraint::Length(1),
        ratatui::layout::Constraint::Length(5),
        ratatui::layout::Constraint::Length(1),
        ratatui::layout::Constraint::Length(1),
        ratatui::layout::Constraint::Min(0),
        ratatui::layout::Constraint::Length(7),
    ])
    .split(inner_area);
    let iters_display = if app.edit.editing_iters {
        format!("{}|", app.edit.iters_edit_buffer)
    } else {
        config.num_iterations.to_string()
    };
    let tokens_display = if app.edit.editing_n_predict {
        format!("{}|", app.edit.n_predict_edit_buffer)
    } else {
        config.n_predict.to_string()
    };
    let mode_line = Line::from(vec![
        Span::styled(" Mode: ", Style::default().fg(Color::Yellow)),
        Span::styled(
            mode_name,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" | "),
        Span::styled("Iters: ", Style::default().fg(Color::Yellow)),
        Span::styled(
            iters_display,
            if app.edit.editing_iters {
                Style::default().fg(Color::Black).bg(Color::Cyan)
            } else {
                Style::default().fg(Color::Cyan)
            },
        ),
        Span::raw(" | "),
        Span::styled("Max Tokens: ", Style::default().fg(Color::Yellow)),
        Span::styled(
            tokens_display,
            if app.edit.editing_n_predict {
                Style::default().fg(Color::Black).bg(Color::Cyan)
            } else {
                Style::default().fg(Color::Cyan)
            },
        ),
    ]);
    f.render_widget(Paragraph::new(mode_line), regions[1]);
    let prompt_title = if editing_prompt {
        " Editing Prompt... "
    } else {
        " Prompt (Alt+P to edit) "
    };
    let prompt_content = if config.prompt.is_empty() {
        "(Empty prompt)"
    } else {
        &config.prompt
    };
    let prompt_lines = if editing_prompt {
        let mut display_text = config.prompt.clone();
        let cursor_pos = app.edit.edit_cursor_pos.min(display_text.len());
        display_text.insert(cursor_pos, '|');
        vec![
            Line::from(display_text),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    format!(" [{} chars] ", config.prompt.len()),
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(
                    " [Press ⎋ to finish] ",
                    Style::default().fg(Color::Yellow),
                ),
            ]),
        ]
    } else {
        vec![
            Line::from(prompt_content),
            Line::from(vec![Span::styled(
                format!(" [{} chars] ", config.prompt.len()),
                Style::default().fg(Color::DarkGray),
            )]),
        ]
    };
    f.render_widget(
        Paragraph::new(prompt_lines)
            .block(
                Block::default()
                    .title(prompt_title)
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(if editing_prompt {
                        Color::Green
                    } else {
                        Color::DarkGray
                    })),
            )
            .wrap(Wrap { trim: true }),
        regions[3],
    );

let param_header_lines: Vec<Line> = if editing_param && selected_idx < config.params_to_test.len() {
        let p = &config.params_to_test[selected_idx];
        if !p.variants.is_empty() {
            let selected_variant_idx = if editing_param_field < -1 {
                ((editing_param_field + 2) as isize).max(0) as usize
            } else {
                0
            };
            let selected_variant_idx = selected_variant_idx.min(p.variants.len().saturating_sub(1));
            let selected_name = p.variants.get(selected_variant_idx).map(|s: &String| s.as_str()).unwrap_or("");
            let mut lines = vec![Line::from(vec![
                Span::raw(" Select parameters to vary: ("),
                Span::styled("Press E to edit", Style::default().fg(Color::Yellow)),
                Span::raw(")  "),
                Span::styled(" [←/→: cycle] ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("Editing: {}", selected_name),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" ({}/{})", selected_variant_idx + 1, p.variants.len()),
                    Style::default().fg(Color::DarkGray),
                ),
            ])];
            if editing_param_field >= 0 && p.variants.is_empty() {
                // Show validation warnings only for numeric params
                if p.min >= p.max {
                    lines.push(Line::from(Span::styled(
                        " ⚠ min must be less than max",
                        Style::default().fg(Color::Red),
                    )));
                }
                if p.step <= 0.0 {
                    lines.push(Line::from(Span::styled(
                        " ⚠ step must be positive",
                        Style::default().fg(Color::Red),
                    )));
                }
                if p.step >= (p.max - p.min) && (p.max - p.min) > 0.001 {
                    lines.push(Line::from(Span::styled(
                        " ⚠ step ≥ range — only 2 values will be tested",
                        Style::default().fg(Color::Yellow),
                    )));
                }
            }
            lines
        } else {
            let field_names = ["Min", "Max", "Step"];
            let active_field_name = if editing_param_field >= 0 && editing_param_field < 3 {
                field_names[editing_param_field as usize]
            } else {
                "Min"
            };
            let mut lines = vec![Line::from(vec![
                Span::raw(" Select parameters to vary: ("),
                Span::styled("Press E to edit", Style::default().fg(Color::Yellow)),
                Span::raw(")  "),
                Span::styled(" [Tab: Min → Max → Step] ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("Editing: {}", active_field_name),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
            ])];
            // Validation warnings
            if p.min >= p.max {
                lines.push(Line::from(Span::styled(
                    " ⚠ min must be less than max",
                    Style::default().fg(Color::Red),
                )));
            }
            if p.step <= 0.0 {
                lines.push(Line::from(Span::styled(
                    " ⚠ step must be positive",
                    Style::default().fg(Color::Red),
                )));
            }
            if p.step >= (p.max - p.min) && (p.max - p.min) > 0.001 {
                lines.push(Line::from(Span::styled(
                    " ⚠ step ≥ range — only 2 values will be tested",
                    Style::default().fg(Color::Yellow),
                )));
            }
            lines
        }
    } else {
        vec![Line::from(vec![
            Span::raw(" Select parameters to vary:"),
            Span::styled(" (Space to toggle)", Style::default().fg(Color::DarkGray)),
        ])]
    };
    f.render_widget(Paragraph::new(param_header_lines), regions[5]);

    let is_spec_off = config
        .params_to_test
        .iter()
        .find(|p| p.name == "spec_type")
        .map(|p| p.min as usize == 0)
        .unwrap_or(true);

    let data_rows: Vec<Row> = config
        .params_to_test
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let marker = if i == selected_idx { ">" } else { " " };
            let is_selected = i == selected_idx;
            let checkbox = if p.name == "spec_type" {
                " - "
            } else if p.name == "draft_tokens" && is_spec_off {
                " - "
            } else if p.enabled {
                "[X]"
            } else {
                "[ ]"
            };
            let name = p.name.replace("_", " ");
            let desc_str = if p.name == "spec_type" {
                let base_idx = (p.min as usize).min(p.variants.len().saturating_sub(1));
                format!("▶ {}", p.variants[base_idx])
            } else if p.name == "draft_tokens" && is_spec_off {
                "(Disabled - Spec type is Off)".to_string()
            } else if editing_param && is_selected && editing_param_field < -1 {
                let selected_variant_idx = ((editing_param_field + 2) as isize).max(0) as usize;
                let selected_variant_idx = selected_variant_idx.min(p.variants.len().saturating_sub(1));
                let variant_names: Vec<String> = p.variants.iter().enumerate().map(|(i, v)| {
                    if i == selected_variant_idx {
                        format!("▶{}", v)
                    } else {
                        format!(" {} ", v)
                    }
                }).collect();
                variant_names.join("│")
            } else if !p.variants.is_empty() {
                let base_idx = (p.min as isize).max(0) as usize;
                let base_idx = base_idx.min(p.variants.len().saturating_sub(1));
                p.variants.iter().enumerate().map(|(i, v)| {
                    if i == base_idx {
                        format!("[{}]", v)
                    } else {
                        format!("({})", v)
                    }
                }).collect::<Vec<_>>().join("│")
            } else if editing_param && is_selected && editing_param_field >= 0 {
                let cursor_pos = param_edit_cursor_pos.min(param_edit_buffer.len());
                let before: String = param_edit_buffer.chars().take(cursor_pos).collect();
                let after: String = param_edit_buffer.chars().skip(cursor_pos).collect();
                let cursor_char = if editing_param && is_selected {
                    "|"
                } else {
                    ""
                };
                let fields: Vec<String> = (0..=2)
                    .map(|f| {
                        if f == editing_param_field {
                            format!("[{}{}{}]", before, after, cursor_char)
                        } else {
                            match f {
                                0 => format!("{:.2}", p.min),
                                1 => format!("{:.2}", p.max),
                                _ => format!("{:.2}", p.step),
                            }
                        }
                    })
                    .collect();
                format!("[{} {} {}]", fields[0], fields[1], fields[2])
            } else {
                match p.name.as_str() {
                    "flash_attn" => "(On/Off)".to_string(),
                    "threads" => format!(
                        "{} to {}, step {}",
                        p.min as u32, p.max as u32, p.step as u32
                    ),
                    "top_k" => format!(
                        "{} to {}, step {}",
                        p.min as i64, p.max as i64, p.step as i64
                    ),
                    "expert_count" => format!(
                        "{} to {}, step {}",
                        p.min as i32, p.max as i32, p.step as i32
                    ),
                    _ => format!("{:.2} to {:.2}, step {:.2}", p.min, p.max, p.step),
                }
            };
            let row_style = if is_selected {
                Style::default().fg(Color::Black).bg(Color::Yellow)
            } else {
                Style::default().fg(Color::White)
            };
            let desc_style = if p.name == "draft_tokens" && is_spec_off {
                if is_selected {
                    Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM)
                } else {
                    Style::default().fg(Color::DarkGray)
                }
            } else if editing_param && is_selected && (editing_param_field >= 0 || editing_param_field < -1) {
                Style::default().fg(Color::Blue)
            } else {
                Style::default().fg(Color::Gray)
            };
            Row::new(vec![
                Cell::from(Span::styled(marker, Style::default().fg(Color::Yellow))),
                Cell::from(Span::styled(
                    checkbox,
                    if p.name == "spec_type" || (p.name == "draft_tokens" && is_spec_off) {
                        Style::default().fg(Color::DarkGray)
                    } else if p.enabled {
                        Style::default().fg(Color::Green)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    },
                )),
                Cell::from(name),
                Cell::from(Span::styled(desc_str, desc_style)),
            ])
            .style(row_style)
        })
        .collect();
    let table = Table::new(
        data_rows,
        [
            Constraint::Length(2),
            Constraint::Length(4),
            Constraint::Length(18),
            Constraint::Fill(1),
        ],
    );
    f.render_widget(table, regions[6]);
    let total_tests = config.get_total_tests_count();
    let num_combinations = config.get_num_combinations();
    let footer_lines = vec![
        Line::from(vec![
            Span::raw(" Total tests: "),
            Span::styled(
                total_tests.to_string(),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" [Alt+M]", Style::default().fg(Color::Yellow)),
            Span::raw(" Mode "),
            Span::styled(" [Alt+N]", Style::default().fg(Color::Yellow)),
            Span::raw(" Tokens "),
            Span::styled(" [Alt+I]", Style::default().fg(Color::Yellow)),
            Span::raw(" Iters "),
            Span::styled(" [E]", Style::default().fg(Color::Yellow)),
            Span::raw(" Range"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" [↵]", Style::default().fg(Color::Yellow)),
            Span::styled(
                " START ",
                Style::default().fg(Color::Black).bg(Color::Green),
            ),
            Span::raw("  "),
            Span::styled(" [⎋]", Style::default().fg(Color::Yellow)),
            Span::raw(" Cancel "),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                format!(" Generates {} combinations ", num_combinations),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
    ];
    f.render_widget(
        Paragraph::new(footer_lines).alignment(Alignment::Center),
        regions[7],
    );
}

fn render_rpc_manager(f: &mut Frame, area: Rect, app: &mut App) {
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
        app.picker.rpc_workers_selected_idx,
        app.picker.editing_rpc_worker.is_some(),
        &app.settings_state.settings_edit_buffer,
        app.edit.edit_cursor_pos,
    );
    let available_height = rpc_area.height.saturating_sub(2);
    let max_offset = worker_lines.len().saturating_sub(available_height as usize) as u16;
    if app.picker.rpc_workers_scroll_offset > max_offset.into() {
        app.picker.rpc_workers_scroll_offset = max_offset.into();
    }
    let start_idx = app.picker.rpc_workers_scroll_offset;
    let visible_lines: Vec<Line> = worker_lines
        .iter()
        .skip(start_idx)
        .take(available_height as usize)
        .cloned()
        .collect();
    let block = Block::default()
        .title(" RPC Workers Manager ")
        .borders(Borders::ALL)
        .border_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );
    f.render_widget(Clear, rpc_area);
    f.render_widget(Paragraph::new(visible_lines).block(block), rpc_area);
    if worker_lines.len() > available_height as usize {
        render_vertical_scrollbar(
            f,
            rpc_area,
            worker_lines.len(),
            app.picker.rpc_workers_scroll_offset,
            1,
            2,
        );
    }
}

fn render_about_overlay(f: &mut Frame, area: Rect) {
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
        .border_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );
    f.render_widget(Clear, about_area);
    f.render_widget(
        Paragraph::new(about_lines)
            .block(block)
            .alignment(Alignment::Center),
        about_area,
    );
}

fn render_max_concurrent_picker(f: &mut Frame, area: Rect, app: &App, value: &str) {
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
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )));
    picker_lines.push(Line::from(""));
    picker_lines.push(Line::from(vec![Span::raw(
        "This divides the context length per loaded model: ",
    )]));
    picker_lines.push(Line::from(vec![
        Span::styled(ctx_len.to_string(), Style::default().fg(Color::Yellow)),
        Span::raw(" / "),
        Span::styled(
            format!("{}", if entered > 0 { entered } else { 1 }),
            Style::default().fg(Color::Cyan),
        ),
        Span::raw(" = "),
        Span::styled(
            format!("{}", per_model),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" tokens per model"),
    ]));
    picker_lines.push(Line::from(""));
    picker_lines.push(Line::from(vec![
        Span::raw("Value: "),
        Span::styled(value, Style::default().fg(Color::Black).bg(Color::Yellow)),
    ]));
    picker_lines.push(Line::from(""));
    picker_lines.push(Line::from(vec![
        Span::styled(
            "  [↵] confirm  ",
            Style::default().fg(Color::Black).bg(Color::Yellow),
        ),
        Span::raw("  "),
        Span::styled(
                "  [⎋] cancel  ",
            Style::default().fg(Color::Black).bg(Color::DarkGray),
        ),
    ]));
    f.render_widget(Clear, picker_area);
    f.render_widget(
        Paragraph::new(picker_lines).block(
            Block::default()
                .title(" Max Concurrent Predictions ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        ),
        picker_area,
    );
}

fn render_dashboard_picker(
    f: &mut Frame,
    area: Rect,
    _app: &App,
    enabled: bool,
    port: &str,
    auth_key: &str,
    tls_enabled: bool,
    tls_cert: &str,
    tls_key: &str,
    selected_field: i32,
    editing: bool,
    edit_buffer: &str,
) {
    let w = 60u16;
    let h = 19u16;
    let picker_area = Rect {
        x: (area.width - w) / 2,
        y: (area.height - h) / 2,
        width: w,
        height: h,
    };
    let mut picker_lines: Vec<Line> = Vec::new();
    picker_lines.push(Line::from(Span::styled(
        " Dashboard ",
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )));
    picker_lines.push(Line::from(""));
    let enabled_marker = if selected_field == -1i32 { "> " } else { "  " };
    picker_lines.push(Line::from(vec![
        Span::styled(enabled_marker, Style::default().fg(Color::Yellow)),
        Span::styled("Enabled: ", Style::default().fg(Color::Yellow)),
        Span::styled(
            if enabled { "On" } else { "Off" },
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    picker_lines.push(Line::from(""));
    let port_marker = if selected_field == 0i32 { "> " } else { "  " };
    let port_val = if editing && selected_field == 0i32 {
        format!("{}|", edit_buffer)
    } else {
        port.to_string()
    };
    picker_lines.push(Line::from(vec![
        Span::styled(port_marker, Style::default().fg(Color::Yellow)),
        Span::styled("Port: ", Style::default().fg(Color::Yellow)),
        Span::styled(port_val, Style::default().fg(Color::White)),
    ]));
    picker_lines.push(Line::from(""));
    let auth_marker = if selected_field == 1i32 { "> " } else { "  " };
    let auth_val = if editing && selected_field == 1i32 {
        format!("{}|", edit_buffer)
    } else if auth_key.is_empty() {
        "(none)".to_string()
    } else {
        auth_key.to_string()
    };
    picker_lines.push(Line::from(vec![
        Span::styled(auth_marker, Style::default().fg(Color::Yellow)),
        Span::styled("Auth Key: ", Style::default().fg(Color::Yellow)),
        Span::styled(auth_val, Style::default().fg(Color::White)),
    ]));
    picker_lines.push(Line::from(""));
    let tls_enabled_marker = if selected_field == 2i32 { "> " } else { "  " };
    picker_lines.push(Line::from(vec![
        Span::styled(tls_enabled_marker, Style::default().fg(Color::Yellow)),
        Span::styled("TLS: ", Style::default().fg(Color::Yellow)),
        Span::styled(
            if tls_enabled { "On" } else { "Off" },
            Style::default()
                .fg(if tls_enabled {
                    Color::Green
                } else {
                    Color::DarkGray
                })
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    picker_lines.push(Line::from(""));
    let tls_cert_marker = if selected_field == 3i32 { "> " } else { "  " };
    let tls_cert_val = if editing && selected_field == 3i32 {
        format!("{}|", edit_buffer)
    } else if tls_cert.is_empty() {
        "(auto-generated)".to_string()
    } else {
        tls_cert.to_string()
    };
    picker_lines.push(Line::from(vec![
        Span::styled(tls_cert_marker, Style::default().fg(Color::Yellow)),
        Span::styled("TLS Cert: ", Style::default().fg(Color::Yellow)),
        Span::styled(tls_cert_val, Style::default().fg(Color::White)),
    ]));
    picker_lines.push(Line::from(""));
    let tls_key_marker = if selected_field == 4i32 { "> " } else { "  " };
    let tls_key_val = if editing && selected_field == 4i32 {
        format!("{}|", edit_buffer)
    } else if tls_key.is_empty() {
        "(auto-generated)".to_string()
    } else {
        tls_key.to_string()
    };
    picker_lines.push(Line::from(vec![
        Span::styled(tls_key_marker, Style::default().fg(Color::Yellow)),
        Span::styled("TLS Key: ", Style::default().fg(Color::Yellow)),
        Span::styled(tls_key_val, Style::default().fg(Color::White)),
    ]));
    picker_lines.push(Line::from(""));
    picker_lines.push(Line::from(vec![Span::styled(
        "[⎋] close  ",
        Style::default().fg(Color::Black).bg(Color::DarkGray),
    )]));
    f.render_widget(Clear, picker_area);
    f.render_widget(
        Paragraph::new(picker_lines).block(
            Block::default()
                .title(" Dashboard ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        ),
        picker_area,
    );
}

fn render_dashboard_url(
    f: &mut Frame,
    area: Rect,
    app: &App,
    host: &str,
    port: &str,
    auth_key: &str,
    ws_enabled: bool,
    tls_enabled: bool,
) {
    let modal_area = Rect {
        x: 0,
        y: 0,
        width: area.width,
        height: area.height,
    };
    f.render_widget(Clear, modal_area);
    let w = 60u16;
    let h = 18u16;
    let picker_area = Rect {
        x: (area.width - w) / 2,
        y: (area.height - h) / 2,
        width: w,
        height: h,
    };
    let host_val = crate::models::format_host(host);
    let backend_str = format!("{}", app.settings.backend);
    let threads_str = app.settings.threads.to_string();
    let threads_batch_str = app.settings.threads_batch.to_string();
    let mode_str = format!("{}", app.server_mode);
    let api_str = if app.settings.api_endpoint_enabled {
        "True"
    } else {
        "False"
    };
    let rpc_workers_count = app.config.rpc_workers.iter().filter(|w| w.selected).count();
    let rpc_str = if rpc_workers_count > 0 {
        format!("{} active", rpc_workers_count)
    } else {
        "None".to_string()
    };
    let mut url = format!("{}://{}:{}/dashboard", if tls_enabled { "https" } else { "http" }, host, port);
    if !auth_key.is_empty() {
        url.push_str(&format!("?auth={}", auth_key));
    }
    let mut picker_lines: Vec<Line> = Vec::new();
    picker_lines.push(Line::from(Span::styled(
        " Server Dashboard ",
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )));
    picker_lines.push(Line::from(""));
    picker_lines.push(Line::from(vec![
        Span::styled("Host: ", Style::default().fg(Color::Yellow)),
        Span::styled(host_val, Style::default().fg(Color::White)),
    ]));
    picker_lines.push(Line::from(""));
    picker_lines.push(Line::from(vec![
        Span::styled("Backend: ", Style::default().fg(Color::Yellow)),
        Span::styled(&backend_str, Style::default().fg(Color::White)),
    ]));
    picker_lines.push(Line::from(vec![
        Span::styled("Threads: ", Style::default().fg(Color::Yellow)),
        Span::styled(&threads_str, Style::default().fg(Color::White)),
    ]));
    picker_lines.push(Line::from(vec![
        Span::styled("Threads Batch: ", Style::default().fg(Color::Yellow)),
        Span::styled(&threads_batch_str, Style::default().fg(Color::White)),
    ]));
    picker_lines.push(Line::from(vec![
        Span::styled("Mode: ", Style::default().fg(Color::Yellow)),
        Span::styled(&mode_str, Style::default().fg(Color::White)),
    ]));
    picker_lines.push(Line::from(vec![
        Span::styled("API Endpoint: ", Style::default().fg(Color::Yellow)),
        Span::styled(api_str, Style::default().fg(Color::White)),
    ]));
    picker_lines.push(Line::from(vec![
        Span::styled("RPC Workers: ", Style::default().fg(Color::Yellow)),
        Span::styled(&rpc_str, Style::default().fg(Color::White)),
    ]));
    picker_lines.push(Line::from(vec![
        Span::styled("Dashboard: ", Style::default().fg(Color::Yellow)),
        Span::styled(
            if ws_enabled { "Enabled" } else { "Disabled" },
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    picker_lines.push(Line::from(""));
    picker_lines.push(Line::from(Span::styled(
        &url,
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )));
    picker_lines.push(Line::from(""));
    picker_lines.push(Line::from(vec![
        Span::styled(
            "[↵] copy URL  ",
            Style::default().fg(Color::Black).bg(Color::DarkGray),
        ),
        Span::styled(
            "[⎋] close",
            Style::default().fg(Color::Black).bg(Color::DarkGray),
        ),
    ]));
    f.render_widget(Clear, picker_area);
    f.render_widget(
        Paragraph::new(picker_lines).block(
            Block::default()
                .title(" Server Dashboard ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        ),
        picker_area,
    );
}

fn render_bench_tune_output(f: &mut Frame, area: Rect, app: &App, result_idx: usize) {
    let results = app.bench_tune.bench_tune_results.clone();
    if let Some(result) = results.get(result_idx) {
        let modal_area = Rect {
            x: 0,
            y: 0,
            width: area.width,
            height: area.height,
        };
        f.render_widget(Clear, modal_area);
        let p_str = if format_bench_params(&result.params, false).is_empty() {
            "Baseline".to_string()
        } else {
            format_bench_params(&result.params, false).join(", ")
        };
        let main_title = Line::from(vec![
            Span::styled(" BenchTune Result: ", Style::default().fg(Color::Yellow)),
            Span::styled(
                p_str,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]);
        let settings = result.base_settings.as_ref();
        let param_rows: Vec<Row> = vec![
            Row::new(vec![
                Cell::from(Span::styled(
                    "temperature",
                    Style::default().fg(Color::Yellow),
                )),
                Cell::from(Span::styled(
                    settings
                        .map(|s| format!("{:.2}", s.temperature))
                        .unwrap_or_else(|| {
                            result
                                .params
                                .temperature
                                .map(|v| format!("{:.2}", v))
                                .unwrap_or_else(|| "-".to_string())
                        }),
                    Style::default().fg(Color::Cyan),
                )),
            ]),
            Row::new(vec![
                Cell::from(Span::styled("top_p", Style::default().fg(Color::Yellow))),
                Cell::from(Span::styled(
                    settings
                        .map(|s| format!("{:.2}", s.top_p))
                        .unwrap_or_else(|| {
                            result
                                .params
                                .top_p
                                .map(|v| format!("{:.2}", v))
                                .unwrap_or_else(|| "-".to_string())
                        }),
                    Style::default().fg(Color::Cyan),
                )),
            ]),
            Row::new(vec![
                Cell::from(Span::styled("top_k", Style::default().fg(Color::Yellow))),
                Cell::from(Span::styled(
                    settings.map(|s| s.top_k.to_string()).unwrap_or_else(|| {
                        result
                            .params
                            .top_k
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| "-".to_string())
                    }),
                    Style::default().fg(Color::Cyan),
                )),
            ]),
            Row::new(vec![
                Cell::from(Span::styled(
                    "repeat_penalty",
                    Style::default().fg(Color::Yellow),
                )),
                Cell::from(Span::styled(
                    settings
                        .map(|s| format!("{:.2}", s.repeat_penalty))
                        .unwrap_or_else(|| {
                            result
                                .params
                                .repeat_penalty
                                .map(|v| format!("{:.2}", v))
                                .unwrap_or_else(|| "-".to_string())
                        }),
                    Style::default().fg(Color::Cyan),
                )),
            ]),
            Row::new(vec![
                Cell::from(Span::styled(
                    "context_length",
                    Style::default().fg(Color::Yellow),
                )),
                Cell::from(Span::styled(
                    settings
                        .map(|s| s.context_length.to_string())
                        .unwrap_or_else(|| {
                            result
                                .params
                                .context_length
                                .map(|v| v.to_string())
                                .unwrap_or_else(|| "-".to_string())
                        }),
                    Style::default().fg(Color::Cyan),
                )),
            ]),
            Row::new(vec![
                Cell::from(Span::styled(
                    "batch_size",
                    Style::default().fg(Color::Yellow),
                )),
                Cell::from(Span::styled(
                    settings
                        .map(|s| s.batch_size.to_string())
                        .unwrap_or_else(|| {
                            result
                                .params
                                .batch_size
                                .map(|v| v.to_string())
                                .unwrap_or_else(|| "-".to_string())
                        }),
                    Style::default().fg(Color::Cyan),
                )),
            ]),
            Row::new(vec![
                Cell::from(Span::styled(
                    "flash_attn",
                    Style::default().fg(Color::Yellow),
                )),
                Cell::from(Span::styled(
                    settings
                        .map(|s| {
                            if s.flash_attn {
                                "on".to_string()
                            } else {
                                "off".to_string()
                            }
                        })
                        .unwrap_or_else(|| {
                            result
                                .params
                                .flash_attn
                                .map(|v| {
                                    if v {
                                        "on".to_string()
                                    } else {
                                        "off".to_string()
                                    }
                                })
                                .unwrap_or_else(|| "-".to_string())
                        }),
                    Style::default().fg(Color::Cyan),
                )),
            ]),
            Row::new(vec![
                Cell::from(Span::styled("threads", Style::default().fg(Color::Yellow))),
                Cell::from(Span::styled(
                    settings.map(|s| s.threads.to_string()).unwrap_or_else(|| {
                        result
                            .params
                            .threads
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| "-".to_string())
                    }),
                    Style::default().fg(Color::Cyan),
                )),
            ]),
            Row::new(vec![
                Cell::from(Span::styled(
                    "expert_count",
                    Style::default().fg(Color::Yellow),
                )),
                Cell::from(Span::styled(
                    settings
                        .map(|s| s.expert_count.to_string())
                        .unwrap_or_else(|| {
                            result
                                .params
                                .expert_count
                                .map(|v| v.to_string())
                                .unwrap_or_else(|| "-".to_string())
                        }),
                    Style::default().fg(Color::Cyan),
                )),
            ]),
            Row::new(vec![
                Cell::from(Span::styled(
                    "spec_type",
                    Style::default().fg(Color::Yellow),
                )),
                Cell::from(Span::styled(
                    settings
                        .as_ref()
                        .map(|s| {
                            if s.spec_type.is_empty() {
                                "-".to_string()
                            } else {
                                s.spec_type.clone()
                            }
                        })
                        .unwrap_or_else(|| {
                            result
                                .params
                                .spec_type
                                .as_ref()
                                .map(|s| {
                                    if s.is_empty() {
                                        "-".to_string()
                                    } else {
                                        s.clone()
                                    }
                                })
                                .unwrap_or_else(|| "-".to_string())
                        }),
                    Style::default().fg(Color::Cyan),
                )),
            ]),
            Row::new(vec![
                Cell::from(Span::styled(
                    "draft_tokens",
                    Style::default().fg(Color::Yellow),
                )),
                Cell::from(Span::styled(
                    settings
                        .map(|s| s.draft_tokens.to_string())
                        .unwrap_or_else(|| {
                            result
                                .params
                                .draft_tokens
                                .map(|v| v.to_string())
                                .unwrap_or_else(|| "-".to_string())
                        }),
                    Style::default().fg(Color::Cyan),
                )),
            ]),
        ];
        let params_table = Table::new(param_rows, [Constraint::Fill(1), Constraint::Fill(1)])
            .header(Row::new(vec![
                Cell::from(Span::styled(
                    "Parameter",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )),
                Cell::from(Span::styled(
                    "Value",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )),
            ]))
            .block(
                Block::default()
                    .title(" Parameters ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            );
        let output_idx = app
            .bench_tune
            .bench_tune_output_index
            .min(result.outputs.len().saturating_sub(1));
        let metrics_for_output = result
            .per_iteration_metrics
            .get(output_idx)
            .unwrap_or(&result.metrics);
        let metric_rows: Vec<Row> = vec![
            Row::new(vec![
                Cell::from(Span::styled(
                    "prompt_tps",
                    Style::default().fg(Color::Green),
                )),
                Cell::from(Span::styled(
                    format!("{:.2}", metrics_for_output.prompt_tps),
                    Style::default().fg(Color::Cyan),
                )),
            ]),
            Row::new(vec![
                Cell::from(Span::styled("gen_tps", Style::default().fg(Color::Green))),
                Cell::from(Span::styled(
                    format!("{:.2}", metrics_for_output.generation_tps),
                    Style::default().fg(Color::Cyan),
                )),
            ]),
            Row::new(vec![
                Cell::from(Span::styled(
                    "combined_tps",
                    Style::default().fg(Color::Green),
                )),
                Cell::from(Span::styled(
                    format!("{:.2}", metrics_for_output.combined_tps),
                    Style::default().fg(Color::Cyan),
                )),
            ]),
            Row::new(vec![
                Cell::from(Span::styled(
                    "latency/token",
                    Style::default().fg(Color::Green),
                )),
                Cell::from(Span::styled(
                    format!("{:.2} ms", metrics_for_output.latency_per_token),
                    Style::default().fg(Color::Cyan),
                )),
            ]),
            Row::new(vec![
                Cell::from(Span::styled(
                    "first_token",
                    Style::default().fg(Color::Green),
                )),
                Cell::from(Span::styled(
                    format!("{:.2} ms", metrics_for_output.first_token_time),
                    Style::default().fg(Color::Cyan),
                )),
            ]),
        ];
        let metrics_table = Table::new(metric_rows, [Constraint::Fill(1), Constraint::Fill(1)])
            .header(Row::new(vec![
                Cell::from(Span::styled(
                    "Metric",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )),
                Cell::from(Span::styled(
                    "Value",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )),
            ]))
            .block(
                Block::default()
                    .title(format!(
                        " Metrics (Iter {}/{}) ",
                        output_idx + 1,
                        result.outputs.len()
                    ))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            );
        let output_lines: Vec<Line> = if !result.outputs.is_empty() {
            result.outputs[output_idx]
                .lines()
                .map(|l: &str| Line::from(l.to_string()))
                .collect()
        } else {
            vec![Line::from("No output captured.")]
        };
        let title_area = Rect {
            x: 0,
            y: 0,
            width: area.width,
            height: 1,
        };
        f.render_widget(
            Paragraph::new(main_title).alignment(Alignment::Center),
            title_area,
        );
        let top_height = 11;
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
        let output_y = 1 + top_height;
        let cmd_text = result.server_command.as_deref().unwrap_or("-");
        let command_height = if !cmd_text.is_empty() && cmd_text != "-" && area.height >= 18 {
            let available_width = area.width.saturating_sub(4); // border + padding
            let wrapped_lines: usize = cmd_text
                .lines()
                .map(|line| {
                    let line_width = line.width() as u16;
                    if line_width <= available_width {
                        1
                    } else {
                        ((line_width as f64 / available_width as f64).ceil()) as usize
                    }
                })
                .sum();
            let required_height = wrapped_lines as u16 + 2; // border top + bottom
            let max_height = area.height.saturating_sub(1 + top_height + 1 + output_y + 1); // leave room for output + controls
            required_height.min(max_height).max(2)
        } else {
            0
        };
        let command_y = output_y;
        if command_height > 0 {
            let command_area = Rect {
                x: 0,
                y: command_y,
                width: area.width,
                height: command_height,
            };
            let command_block = Block::default()
                .title(" Server Command ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow));
            f.render_widget(
                Paragraph::new(cmd_text.to_string())
                    .block(command_block)
                    .wrap(Wrap { trim: true }),
                command_area,
            );
        }
        let output_y = output_y + command_height;
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
                let scroll = app.bench_tune.bench_tune_output_scroll as u16;
                f.render_widget(
                    Paragraph::new(output_lines)
                        .block(output_block)
                        .wrap(Wrap { trim: false })
                        .scroll((scroll, 0)),
                    output_area,
                );
            } else {
                f.render_widget(
                    Paragraph::new(output_lines).block(output_block),
                    output_area,
                );
            }
        }
        let mut absolute_idx = 0;
        let mut total_outputs = 0;
        for (r_idx, r) in results.iter().enumerate() {
            if r_idx < result_idx {
                absolute_idx += r.outputs.len();
            }
            total_outputs += r.outputs.len();
        }
        absolute_idx += output_idx + 1;
        let controls = Line::from(vec![
            Span::styled(
                "  [⎋] close  ",
                Style::default().fg(Color::Black).bg(Color::Yellow),
            ),
            Span::raw("  "),
            Span::styled("[j/k] scroll  ", Style::default().fg(Color::Yellow)),
            Span::raw("  "),
            Span::styled("[←] prev   ", Style::default().fg(Color::Yellow)),
            Span::styled(
                format!("({}/{})", absolute_idx, total_outputs),
                Style::default().fg(Color::Yellow),
            ),
            Span::raw("  "),
            Span::styled("[→] next  ", Style::default().fg(Color::Yellow)),
        ]);
        let controls_area = Rect {
            x: 0,
            y: area.height.saturating_sub(1),
            width: area.width,
            height: 1,
        };
        f.render_widget(
            Paragraph::new(controls).alignment(Alignment::Center),
            controls_area,
        );
    }
}

fn render_search_input(f: &mut Frame, area: Rect, buffer: &str, cursor_pos: usize) {
    let w: u16 = 60;
    let h: u16 = 7;
    let popup_area = Rect {
        x: (area.width - w) / 2,
        y: (area.height - h) / 2,
        width: w,
        height: h,
    };
    let clamped_pos = cursor_pos.min(buffer.len());
    let before: String = buffer.chars().take(clamped_pos).collect();
    let after: String = buffer.chars().skip(clamped_pos).collect();
    let picker_lines: Vec<Line> = vec![
        Line::from(Span::styled(
            " Search Query ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("Search: ", Style::default().fg(Color::Yellow)),
            Span::styled(before, Style::default().fg(Color::White)),
            Span::styled(
                "|",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(after, Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "  [↵] search  ",
                Style::default().fg(Color::Black).bg(Color::Yellow),
            ),
            Span::raw("  "),
            Span::styled(
            "  [⎋] cancel  ",
                Style::default().fg(Color::Black).bg(Color::DarkGray),
            ),
        ]),
    ];
    f.render_widget(Clear, popup_area);
    f.render_widget(
        Paragraph::new(picker_lines).block(
            Block::default()
                .title(" Search Input ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        ),
        popup_area,
    );
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

fn render_yarn_rope_picker(
    f: &mut Frame,
    area: Rect,
    app: &App,
    scale: &str,
    freq_base: &str,
    freq_scale: &str,
    selected_field: i32,
    editing: bool,
    edit_buffer: &str,
    edit_cursor_pos: usize,
) {
    let w: u16 = 60;
    let h: u16 = 14;
    let picker_area = Rect {
        x: (area.width - w) / 2,
        y: (area.height - h) / 2,
        width: w,
        height: h,
    };
    let mut picker_lines: Vec<Line> = vec![
        Line::from(Span::styled(
            " Yarn RoPE Params ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            " [↑/↓] Select  [↵] Edit  [⎋] Done  [Space] Enable/Disable Yarn RoPE ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    let fields = [
        (
            "Yarn RoPE",
            format!("{}", app.settings.rope_yarn_enabled),
            -1,
        ),
        ("rope_scale", scale.to_string(), 0),
        ("rope_freq_base", freq_base.to_string(), 1),
        ("rope_freq_scale", freq_scale.to_string(), 2),
    ];

    for (name, val, field_idx) in fields.iter() {
        let marker = if *field_idx == selected_field {
            "> "
        } else {
            "  "
        };
        let is_selected = *field_idx == selected_field;

        let display_val = if editing && is_selected {
            let cursor_pos = edit_cursor_pos.min(edit_buffer.len());
            let before: String = edit_buffer.chars().take(cursor_pos).collect();
            let after: String = edit_buffer.chars().skip(cursor_pos).collect();
            format!("{}|{}", before, after)
        } else {
            val.clone()
        };

        let style = if is_selected {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        picker_lines.push(Line::from(vec![
            Span::styled(marker, Style::default().fg(Color::Yellow)),
            Span::styled(format!("{}: ", name), Style::default().fg(Color::Cyan)),
            Span::styled(display_val, style),
        ]));
    }

    let rope_scale_val = scale.parse::<f32>().unwrap_or(1.0);
    let freq_base_val = freq_base.parse::<f32>().unwrap_or(0.0);
    let freq_scale_val = freq_scale.parse::<f32>().unwrap_or(1.0);
    let rope_scale_display = rope_scale_val;
    let ctx = app.settings.context_length;
    let effective_ctx = (ctx as f64 * rope_scale_display as f64) as u32;
    let ctx_display = if rope_scale_display > 1.001 {
        format!(
            "{} * {:.2} = {} tokens",
            ctx, rope_scale_display, effective_ctx
        )
    } else {
        format!("{} tokens (no scaling)", ctx)
    };

    picker_lines.push(Line::from(""));
    picker_lines.push(Line::from(Span::styled(
        format!(
            "  scale={:.2} base={:.2} scale_f={:.2}",
            rope_scale_display, freq_base_val, freq_scale_val
        ),
        Style::default().fg(Color::DarkGray),
    )));
    picker_lines.push(Line::from(Span::styled(
        format!("  Effective context: {}", ctx_display),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )));

    f.render_widget(Clear, picker_area);
    f.render_widget(
        Paragraph::new(picker_lines).block(
            Block::default()
                .title(" Yarn RoPE Params ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        ),
        picker_area,
    );
}

fn render_spec_type_picker(
    f: &mut Frame,
    area: Rect,
    _app: &App,
    entries: &[String],
    selected: usize,
) {
    let w = 50u16;
    let h = (entries.len() as u16) + 6;
    let picker_area = Rect {
        x: (area.width - w) / 2,
        y: (area.height - h) / 2,
        width: w,
        height: h,
    };
    let mut picker_lines: Vec<Line> = vec![
        Line::from(Span::styled(
            " Speculative Decoding Type ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
        " [↑/↓] Select  [↵] Apply  [⎋] Cancel ",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
    ];

    for (i, entry) in entries.iter().enumerate() {
        let marker = if i == selected { "> " } else { "  " };
        let style = if i == selected {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        picker_lines.push(Line::from(vec![
            Span::styled(marker, Style::default().fg(Color::Yellow)),
            Span::styled(entry, style),
        ]));
    }

    f.render_widget(Clear, picker_area);
    f.render_widget(
        Paragraph::new(picker_lines).block(
            Block::default()
                .title(" Speculative Decoding Type ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        ),
        picker_area,
    );
}
