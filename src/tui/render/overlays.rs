use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Cell, Clear, Paragraph, Row, Table, Wrap},
};
use unicode_width::UnicodeWidthStr;

use super::App;
use super::onboarding;
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
            .border_style(Style::default().fg(Color::Yellow))
            .border_type(BorderType::Double);
        f.render_widget(Paragraph::new(text).block(block), area);
        return true;
    }

    if let GlobalMode::Confirmation {
        selected,
        kind,
        display_name,
        detail,
    } = &app.ui.global_mode
    {
        if f.area().height >= 8 {
            render_confirmation(
                f,
                f.area(),
                app,
                *selected,
                *kind,
                display_name,
                detail.as_deref(),
            );
        }
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

    if let GlobalMode::ChatTemplatePicker { entries, selected } = &app.ui.global_mode {
        render_chat_template_picker(f, f.area(), app, entries, *selected);
        return true;
    }

    if let GlobalMode::ChatTemplateFilePicker { entries, selected } = &app.ui.global_mode {
        render_chat_template_file_picker(f, f.area(), app, entries, *selected);
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
        render_dashboard_url(
            f,
            f.area(),
            app,
            host,
            port,
            auth_key,
            *ws_enabled,
            *tls_enabled,
        );
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

    if let GlobalMode::GgufNaming {
        explanation,
        filename: _,
    } = &app.ui.global_mode
    {
        render_gguf_naming_overlay(f, f.area(), explanation);
        return true;
    }

    if let GlobalMode::Onboarding { step } = &app.ui.global_mode {
        onboarding::render_onboarding(f, f.area(), app, *step);
        return true;
    }

    if let GlobalMode::WebSearchPicker {
        enabled,
        engine,
        engine_url,
        api_key,
        selected_field,
        engine_picker_selected,
        editing,
        edit_buffer,
        edit_cursor_pos: _,
    } = &app.ui.global_mode
    {
        render_web_search_picker(
            f,
            f.area(),
            app,
            *enabled,
            engine,
            engine_url,
            api_key.as_deref(),
            *selected_field,
            *engine_picker_selected,
            *editing,
            edit_buffer,
        );
        return true;
    }

    false
}

fn render_web_search_picker(
    f: &mut Frame,
    area: Rect,
    _app: &App,
    enabled: bool,
    engine: &str,
    engine_url: &str,
    api_key: Option<&str>,
    selected_field: i32,
    engine_picker_selected: usize,
    editing: bool,
    edit_buffer: &str,
) {
    let engines = ["searxng", "duckduckgo", "brave", "google", "startpage"];
    let w = 65u16;
    let h = if selected_field < -1 {
        (8.min(area.height - 4)) as u16
    } else {
        (15.min(area.height - 4)) as u16
    };
    let picker_area = Rect {
        x: (area.width - w) / 2,
        y: (area.height - h) / 2,
        width: w,
        height: h,
    };
    let mut picker_lines: Vec<Line> = Vec::new();

    if selected_field < -1 {
        picker_lines.push(Line::from(Span::styled(
            crate::t!("dialog.web_search.help"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )));
        picker_lines.push(Line::from(""));
        for (i, e) in engines.iter().enumerate() {
            let marker = if i == engine_picker_selected { "> " } else { "  " };
            let style = if i == engine_picker_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            picker_lines.push(Line::from(vec![
                Span::styled(marker, Style::default().fg(Color::Yellow)),
                Span::styled(e.to_string(), style),
            ]));
        }
        f.render_widget(Clear, picker_area);
        f.render_widget(
            Paragraph::new(picker_lines).block(
                Block::default()
                    .title(Span::styled(
                        crate::t!("dialog.web_search.title"),
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow)),
            ),
            picker_area,
        );
        return;
    }

    let enabled_marker = if selected_field == -1 { "> " } else { "  " };
    picker_lines.push(Line::from(vec![
        Span::styled(enabled_marker, Style::default().fg(Color::Yellow)),
        Span::styled(
            crate::t!("dialog.web_search.enabled"),
            Style::default().fg(Color::Yellow),
        ),
        Span::styled(
            if enabled {
                crate::t!("dialog.web_search.on")
            } else {
                crate::t!("dialog.web_search.off")
            },
            Style::default()
                .fg(if enabled {
                    Color::Green
                } else {
                    Color::DarkGray
                })
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    picker_lines.push(Line::from(""));
    let engine_marker = if selected_field == 0 { "> " } else { "  " };
    let engine_val = if engine_marker == "> " && editing {
        format!("{} ({})", engine, edit_buffer)
    } else {
        engine.to_string()
    };
    picker_lines.push(Line::from(vec![
        Span::styled(engine_marker, Style::default().fg(Color::Yellow)),
        Span::styled(
            crate::t!("dialog.web_search.engine"),
            Style::default().fg(Color::Yellow),
        ),
        Span::styled(engine_val, Style::default().fg(Color::White)),
    ]));
    picker_lines.push(Line::from(""));
    let url_marker = if selected_field == 1 { "> " } else { "  " };
    let url_val = if editing && selected_field == 1 {
        format!("{}|", edit_buffer)
    } else if engine_url.is_empty() {
        crate::t!("dialog.web_search.engine_url").to_string()
    } else {
        engine_url.to_string()
    };
    picker_lines.push(Line::from(vec![
        Span::styled(url_marker, Style::default().fg(Color::Yellow)),
        Span::styled(
            crate::t!("dialog.web_search.engine_url"),
            Style::default().fg(Color::Yellow),
        ),
        Span::styled(url_val, Style::default().fg(Color::White)),
    ]));
    picker_lines.push(Line::from(""));
    let key_marker = if selected_field == 2 { "> " } else { "  " };
    let key_val = if editing && selected_field == 2 {
        format!("{}|", edit_buffer)
    } else if let Some(k) = api_key {
        if k.is_empty() {
            String::new()
        } else {
            "****".to_string()
        }
    } else {
        String::new()
    };
    picker_lines.push(Line::from(vec![
        Span::styled(key_marker, Style::default().fg(Color::Yellow)),
        Span::styled(
            crate::t!("dialog.web_search.api_key"),
            Style::default().fg(Color::Yellow),
        ),
        Span::styled(key_val, Style::default().fg(Color::White)),
    ]));
    picker_lines.push(Line::from(""));
    picker_lines.push(Line::from(Span::styled(
        crate::t!("dialog.web_search.help"),
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )));
    f.render_widget(Clear, picker_area);
    f.render_widget(
        Paragraph::new(picker_lines).block(
            Block::default()
                .title(Span::styled(
                    crate::t!("dialog.web_search.title"),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        ),
        picker_area,
    );
}

fn render_confirmation(
    f: &mut Frame,
    area: Rect,
    app: &App,
    selected: bool,
    kind: ConfirmationKind,
    display_name: &str,
    detail: Option<&str>,
) {
    let (title, text_lines) = match kind {
        ConfirmationKind::Exit => {
            let loaded_count = app
                .model_states
                .values()
                .filter(|s| matches!(s, crate::models::ModelState::Loaded { .. }))
                .count();
            (
                crate::t!("dialog.exit.title"),
                vec![
                    Line::from(""),
                    Line::from(vec![
                        Span::raw(crate::t!("dialog.exit.message")),
                        Span::raw(" "),
                        Span::styled(
                            format!("{}", loaded_count),
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(""),
                    ]),
                    Line::from(crate::t!("dialog.exit.confirm")),
                ],
            )
        }
        ConfirmationKind::Reset => (
            crate::t!("dialog.reset.title"),
            vec![
                Line::from(""),
                Line::from(crate::t!("dialog.reset.message")),
            ],
        ),
        ConfirmationKind::Delete => (
            crate::t!("dialog.delete.title"),
            vec![
                Line::from(""),
                Line::from(vec![
                    Span::raw(crate::t!("dialog.delete.message")),
                    Span::raw(" "),
                    Span::styled(
                        display_name,
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("?"),
                ]),
                Line::from(""),
                Line::from(crate::t!("dialog.delete.confirm")),
            ],
        ),
        ConfirmationKind::Unload => (
            crate::t!("dialog.unload.title"),
            vec![
                Line::from(""),
                Line::from(vec![
                    Span::raw(crate::t!("dialog.unload.message")),
                    Span::raw(" "),
                    Span::styled(
                        display_name,
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("?"),
                ]),
            ],
        ),
        ConfirmationKind::DeleteBackend => {
            let display = if let Some(d) = detail {
                d
            } else {
                display_name
            };
            (
                crate::t!("dialog.delete_backend.title"),
                vec![
                    Line::from(""),
                    Line::from(vec![
                        Span::raw(crate::t!("dialog.delete_backend.message")),
                        Span::raw(" "),
                        Span::styled(
                            display,
                            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                        ),
                        Span::raw("?"),
                    ]),
                    Line::from(""),
                    Line::from(crate::t!("dialog.delete_backend.confirm")),
                ],
            )
        }
    };
    let mut lines = text_lines;
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(
            crate::t!("dialog.confirm_yes"),
            Style::default().fg(Color::Black).bg(if selected {
                Color::Yellow
            } else {
                Color::DarkGray
            }),
        ),
        Span::raw("    "),
        Span::styled(
            crate::t!("dialog.confirm_no"),
            Style::default().fg(Color::Black).bg(if selected {
                Color::DarkGray
            } else {
                Color::Yellow
            }),
        ),
    ]));
    let w = 70u16;
    let h = (lines.len() + 2) as u16;
    let popup_area = Rect {
        x: area.width.saturating_sub(w) / 2,
        y: area.height.saturating_sub(h) / 2,
        width: w,
        height: h,
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
        ))
        .border_type(BorderType::Double);
    f.render_widget(Clear, popup_area);
    f.render_widget(
        Paragraph::new(lines)
            .block(block)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: false }),
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
        crate::t!("dialog.host_picker.help"),
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
                .title(Span::styled(
                    crate::t!("dialog.host_picker.title"),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ))
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
        crate::t!("dialog.profile_picker.help"),
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )));
    picker_lines.push(Line::from(""));
    for (i, (name, desc)) in entries.iter().enumerate() {
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
            Span::styled(name, style),
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
            crate::t!("dialog.profile_picker.changed"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )));
        if preview_parts.is_empty() {
            picker_lines.push(Line::from(Span::styled(
                crate::t!("dialog.profile_picker.no_changes"),
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
                    .title(Span::styled(
                        crate::t!("dialog.profile_picker.title"),
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ))
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
        picker_lines.push(Line::from(Span::styled(
            format!("{} {}", crate::t!("dialog.prompt_picker.delete"), name),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )));
        picker_lines.push(Line::from(""));
        picker_lines.push(Line::from(Span::styled(
            crate::t!("dialog.prompt_picker.confirm"),
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
                    entries[selected].0.clone()
                } else {
                    crate::t!("dialog.prompt_picker.new").to_string()
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
            crate::t!("dialog.prompt_picker.edit_help"),
            Style::default().fg(Color::Cyan),
        )));
    } else {
        picker_lines.push(Line::from(Span::styled(
            crate::t!("dialog.prompt_picker.list_help"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )));
        picker_lines.push(Line::from(""));
        for (i, (name, desc)) in entries.iter().enumerate() {
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
                Span::styled(name, style),
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
                    .title(Span::styled(
                        crate::t!("dialog.prompt_picker.title"),
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ))
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
    if app.edit.tags_insert_mode {
        modal_lines.push(Line::from(Span::styled(
            crate::t!("dialog.tags.add_help"),
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        modal_lines.push(Line::from(Span::styled(
            crate::t!("dialog.tags.edit_help"),
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
            Span::styled(
                crate::t!("dialog.tags.new_label"),
                Style::default().fg(Color::Yellow),
            ),
            Span::styled(
                &app.edit.tags_edit_buffer,
                Style::default().fg(Color::Black).bg(Color::Yellow),
            ),
            Span::styled("_", Style::default().fg(Color::Black).bg(Color::Yellow)),
        ]));
    } else if app.edit.tags_selected_idx.is_some() {
        modal_lines.push(Line::from(vec![
            Span::styled(
                crate::t!("dialog.tags.edit_label"),
                Style::default().fg(Color::Yellow),
            ),
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
                .title(Span::styled(
                    crate::t!("dialog.tags.title"),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow))
                .border_type(BorderType::Double),
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
        crate::t!("dialog.backend_picker.select"),
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )));
    let gpu_models: Vec<String> = all_models.iter().filter_map(|m| m.clone()).collect();
    if !gpu_models.is_empty() {
        picker_lines.push(Line::from(vec![
            Span::raw(crate::t!("dialog.backend_picker.hardware")),
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
        let is_recommended = vendors.iter().any(|v| {
            matches!(
                (v, backend, tag),
                (GpuVendor::Amd, crate::models::Backend::Rocm, None)
                    | (GpuVendor::Amd, crate::models::Backend::RocmLemonade, None)
                    | (GpuVendor::Nvidia, crate::models::Backend::Cuda, None)
                    | (GpuVendor::Nvidia, crate::models::Backend::Vulkan, None)
                    | (GpuVendor::Intel, crate::models::Backend::Vulkan, None)
                    | (GpuVendor::Unknown, crate::models::Backend::Cpu, None)
                    | (GpuVendor::Apple, crate::models::Backend::Cpu, None)
            )
        });
        let style = if i == selected {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        let label = match backend {
            crate::models::Backend::Cpu => crate::t!("dialog.backend_picker.cpu"),
            crate::models::Backend::Vulkan => crate::t!("dialog.backend_picker.vulkan"),
            crate::models::Backend::Rocm => crate::t!("dialog.backend_picker.rocm"),
            crate::models::Backend::RocmLemonade => {
                crate::t!("dialog.backend_picker.rocm_lemonade")
            }
            crate::models::Backend::Cuda => crate::t!("dialog.backend_picker.cuda"),
            crate::models::Backend::CpuArm64 => crate::t!("dialog.backend_picker.cpu_arm64"),
            crate::models::Backend::CpuWindows => crate::t!("dialog.backend_picker.cpu_windows"),
            crate::models::Backend::VulkanWindows => {
                crate::t!("dialog.backend_picker.vulkan_windows")
            }
            crate::models::Backend::CudaWindows12_4 => crate::t!("dialog.backend_picker.cuda_124"),
            crate::models::Backend::CudaWindows13_1 => crate::t!("dialog.backend_picker.cuda_131"),
            crate::models::Backend::HipWindows => crate::t!("dialog.backend_picker.hip_windows"),
            crate::models::Backend::CpuMacosArm64 => {
                crate::t!("dialog.backend_picker.cpu_macos_arm64")
            }
            crate::models::Backend::CpuMacosX64 => {
                crate::t!("dialog.backend_picker.cpu_macos_intel")
            }
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
            line_spans.push(Span::styled(
                crate::t!("dialog.backend_picker.cached"),
                Style::default().fg(Color::Blue),
            ));
        }
        if is_recommended {
            line_spans.push(Span::raw("  "));
            line_spans.push(Span::styled(
                crate::t!("dialog.backend_picker.recommended"),
                Style::default().fg(Color::Green),
            ));
        }
        picker_lines.push(Line::from(line_spans));
    }
    f.render_widget(Clear, picker_area);
    f.render_widget(
        Paragraph::new(picker_lines).block(
            Block::default()
                .title(Span::styled(
                    crate::t!("dialog.backend_picker.title"),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ))
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
    let h = (30.min(area.height - 4)) as u16;
    let popup_area = Rect {
        x: (area.width.saturating_sub(w)) / 2,
        y: (area.height.saturating_sub(h)) / 2,
        width: w.min(area.width),
        height: h.min(area.height),
    };
    let mode_idx = bench_mode_selection.min(1);
    let mode_name = if mode_idx == 0 {
        crate::t!("dialog.bench_config.runtime_only")
    } else {
        crate::t!("dialog.bench_config.full")
    };
    let block = Block::default()
        .title(Span::styled(
            crate::t!("dialog.bench_config.title"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .border_type(BorderType::Double);
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
        Span::styled(
            crate::t!("dialog.bench_config.mode"),
            Style::default().fg(Color::Yellow),
        ),
        Span::styled(
            mode_name,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" | "),
        Span::styled(
            crate::t!("dialog.bench_config.iters"),
            Style::default().fg(Color::Yellow),
        ),
        Span::styled(
            iters_display,
            if app.edit.editing_iters {
                Style::default().fg(Color::Black).bg(Color::Cyan)
            } else {
                Style::default().fg(Color::Cyan)
            },
        ),
        Span::raw(" | "),
        Span::styled(
            crate::t!("dialog.bench_config.max_tokens"),
            Style::default().fg(Color::Yellow),
        ),
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
        crate::t!("dialog.bench_config.editing_prompt")
    } else {
        crate::t!("dialog.bench_config.prompt")
    };
    let prompt_content = if config.prompt.is_empty() {
        crate::t!("dialog.bench_config.empty_prompt").to_string()
    } else {
        config.prompt.clone()
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
                    crate::t!("dialog.bench_config.finish"),
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
                    }))
                    .border_type(BorderType::Double),
            )
            .wrap(Wrap { trim: true }),
        regions[3],
    );

    let param_header_lines: Vec<Line> = if editing_param
        && selected_idx < config.params_to_test.len()
    {
        let p = &config.params_to_test[selected_idx];
        if !p.variants.is_empty() {
            let selected_variant_idx = if editing_param_field < -1 {
                ((editing_param_field + 2) as isize).max(0) as usize
            } else {
                0
            };
            let selected_variant_idx = selected_variant_idx.min(p.variants.len().saturating_sub(1));
            let selected_name = p
                .variants
                .get(selected_variant_idx)
                .map(|s: &String| s.as_str())
                .unwrap_or("");
            let mut lines = vec![Line::from(vec![
                Span::raw(crate::t!("dialog.bench_config.select_params")),
                Span::styled(
                    crate::t!("dialog.bench_config.edit_hint"),
                    Style::default().fg(Color::Yellow),
                ),
                Span::raw(")  "),
                Span::styled(" [←/→: cycle] ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!(
                        "{} {}",
                        crate::t!("dialog.bench_config.editing"),
                        selected_name
                    ),
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
                        crate::t!("dialog.bench_config.error_min"),
                        Style::default().fg(Color::Red),
                    )));
                }
                if p.step <= 0.0 {
                    lines.push(Line::from(Span::styled(
                        crate::t!("dialog.bench_config.error_step"),
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
            let active_field_name = if (0..3).contains(&editing_param_field) {
                field_names[editing_param_field as usize]
            } else {
                "Min"
            };
            let mut lines = vec![Line::from(vec![
                Span::raw(crate::t!("dialog.bench_config.select_params")),
                Span::styled(
                    crate::t!("dialog.bench_config.edit_hint"),
                    Style::default().fg(Color::Yellow),
                ),
                Span::raw(")  "),
                Span::styled(
                    " [Tab: Min → Max → Step] ",
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!(
                        "{} {}",
                        crate::t!("dialog.bench_config.editing"),
                        active_field_name
                    ),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
            ])];
            // Validation warnings
            if p.min >= p.max {
                lines.push(Line::from(Span::styled(
                    crate::t!("dialog.bench_config.error_min"),
                    Style::default().fg(Color::Red),
                )));
            }
            if p.step <= 0.0 {
                lines.push(Line::from(Span::styled(
                    crate::t!("dialog.bench_config.error_step"),
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
            Span::raw(crate::t!("dialog.bench_config.params_label")),
            Span::styled(
                crate::t!("dialog.bench_config.toggle_hint"),
                Style::default().fg(Color::DarkGray),
            ),
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
                crate::t!("dialog.bench_config.disabled").to_string()
            } else if editing_param && is_selected && editing_param_field < -1 {
                let selected_variant_idx = ((editing_param_field + 2) as isize).max(0) as usize;
                let selected_variant_idx =
                    selected_variant_idx.min(p.variants.len().saturating_sub(1));
                let variant_names: Vec<String> = p
                    .variants
                    .iter()
                    .enumerate()
                    .map(|(i, v)| {
                        if i == selected_variant_idx {
                            format!("▶{}", v)
                        } else {
                            format!(" {} ", v)
                        }
                    })
                    .collect();
                variant_names.join("│")
            } else if !p.variants.is_empty() {
                let base_idx = (p.min as isize).max(0) as usize;
                let base_idx = base_idx.min(p.variants.len().saturating_sub(1));
                p.variants
                    .iter()
                    .enumerate()
                    .map(|(i, v)| {
                        if i == base_idx {
                            format!("[{}]", v)
                        } else {
                            format!("({})", v)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("│")
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
                    "flash_attn" => crate::t!("dialog.bench_config.on_off").to_string(),
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
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::DIM)
                } else {
                    Style::default().fg(Color::DarkGray)
                }
            } else if editing_param && is_selected && !(-1..0).contains(&editing_param_field) {
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
            Span::raw(crate::t!("dialog.bench_config.total_tests")),
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
            Span::styled(
                crate::t!("dialog.bench_config.start"),
                Style::default().fg(Color::Yellow),
            ),
            Span::styled(
                crate::t!("dialog.bench_config.start_text"),
                Style::default().fg(Color::Black).bg(Color::Green),
            ),
            Span::raw("  "),
            Span::styled(
                crate::t!("dialog.bench_config.cancel_key"),
                Style::default().fg(Color::Yellow),
            ),
            Span::raw(crate::t!("dialog.bench_config.cancel")),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            format!(
                "{} {}",
                crate::t!("dialog.bench_config.generates"),
                num_combinations
            ),
            Style::default().fg(Color::DarkGray),
        )]),
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
        .title(Span::styled(
            crate::t!("dialog.rpc.title"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .border_type(BorderType::Double);
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
    let h = 16.min(area.height - 4);
    let about_area = Rect {
        x: (area.width.saturating_sub(w)) / 2,
        y: (area.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    };
    let about_lines = panel::about::render_about();
    let block = Block::default()
        .title(Span::styled(
            crate::t!("dialog.about.title"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .border_type(BorderType::Double);
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
    let h = (10.min(area.height - 4)).max(8) as u16;
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
    picker_lines.push(Line::from(vec![Span::raw(crate::t!(
        "dialog.max_concurrent.divides"
    ))]));
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
        Span::raw(crate::t!("dialog.max_concurrent.tokens_per_model")),
    ]));
    picker_lines.push(Line::from(""));
    picker_lines.push(Line::from(vec![
        Span::raw(crate::t!("dialog.max_concurrent.value_label")),
        Span::styled(value, Style::default().fg(Color::Black).bg(Color::Yellow)),
    ]));
    picker_lines.push(Line::from(""));
    picker_lines.push(Line::from(vec![
        Span::styled(
            crate::t!("dialog.max_concurrent.confirm"),
            Style::default().fg(Color::Black).bg(Color::Yellow),
        ),
        Span::raw("  "),
        Span::styled(
            crate::t!("dialog.max_concurrent.cancel"),
            Style::default().fg(Color::Black).bg(Color::DarkGray),
        ),
    ]));
    f.render_widget(Clear, picker_area);
    f.render_widget(
        Paragraph::new(picker_lines).block(
            Block::default()
                .title(Span::styled(
                    crate::t!("dialog.max_concurrent.title"),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ))
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
    let h = (19.min(area.height - 4)) as u16;
    let picker_area = Rect {
        x: (area.width - w) / 2,
        y: (area.height - h) / 2,
        width: w,
        height: h,
    };
    let mut picker_lines: Vec<Line> = Vec::new();
    let enabled_marker = if selected_field == -1i32 { "> " } else { "  " };
    picker_lines.push(Line::from(vec![
        Span::styled(enabled_marker, Style::default().fg(Color::Yellow)),
        Span::styled(
            crate::t!("dialog.dashboard.enabled"),
            Style::default().fg(Color::Yellow),
        ),
        Span::styled(
            if enabled {
                crate::t!("dialog.dashboard.on")
            } else {
                crate::t!("dialog.dashboard.off")
            },
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
        Span::styled(
            crate::t!("dialog.dashboard.port"),
            Style::default().fg(Color::Yellow),
        ),
        Span::styled(port_val, Style::default().fg(Color::White)),
    ]));
    picker_lines.push(Line::from(""));
    let auth_marker = if selected_field == 1i32 { "> " } else { "  " };
    let auth_val = if editing && selected_field == 1i32 {
        format!("{}|", edit_buffer)
    } else if auth_key.is_empty() {
        crate::t!("dialog.dashboard.none").to_string()
    } else {
        auth_key.to_string()
    };
    picker_lines.push(Line::from(vec![
        Span::styled(auth_marker, Style::default().fg(Color::Yellow)),
        Span::styled(
            crate::t!("dialog.dashboard.auth_key"),
            Style::default().fg(Color::Yellow),
        ),
        Span::styled(auth_val, Style::default().fg(Color::White)),
    ]));
    picker_lines.push(Line::from(""));
    let tls_enabled_marker = if selected_field == 2i32 { "> " } else { "  " };
    picker_lines.push(Line::from(vec![
        Span::styled(tls_enabled_marker, Style::default().fg(Color::Yellow)),
        Span::styled(
            crate::t!("dialog.dashboard.tls"),
            Style::default().fg(Color::Yellow),
        ),
        Span::styled(
            if tls_enabled {
                crate::t!("dialog.dashboard.on")
            } else {
                crate::t!("dialog.dashboard.off")
            },
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
        crate::t!("dialog.dashboard.tls_auto").to_string()
    } else {
        tls_cert.to_string()
    };
    picker_lines.push(Line::from(vec![
        Span::styled(tls_cert_marker, Style::default().fg(Color::Yellow)),
        Span::styled(
            crate::t!("dialog.dashboard.tls_cert"),
            Style::default().fg(Color::Yellow),
        ),
        Span::styled(tls_cert_val, Style::default().fg(Color::White)),
    ]));
    picker_lines.push(Line::from(""));
    let tls_key_marker = if selected_field == 4i32 { "> " } else { "  " };
    let tls_key_val = if editing && selected_field == 4i32 {
        format!("{}|", edit_buffer)
    } else if tls_key.is_empty() {
        crate::t!("dialog.dashboard.tls_auto").to_string()
    } else {
        tls_key.to_string()
    };
    picker_lines.push(Line::from(vec![
        Span::styled(tls_key_marker, Style::default().fg(Color::Yellow)),
        Span::styled(
            crate::t!("dialog.dashboard.tls_key"),
            Style::default().fg(Color::Yellow),
        ),
        Span::styled(tls_key_val, Style::default().fg(Color::White)),
    ]));
    picker_lines.push(Line::from(""));
    picker_lines.push(Line::from(vec![Span::styled(
        crate::t!("dialog.dashboard.close"),
        Style::default().fg(Color::Black).bg(Color::DarkGray),
    )]));
    f.render_widget(Clear, picker_area);
    f.render_widget(
        Paragraph::new(picker_lines).block(
            Block::default()
                .title(Span::styled(
                    crate::t!("dialog.dashboard.title"),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow))
                .border_type(BorderType::Double),
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
    let h = (18.min(area.height - 4)) as u16;
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
    let mut url = format!(
        "{}://{}:{}/dashboard",
        if tls_enabled { "https" } else { "http" },
        host,
        port
    );
    if !auth_key.is_empty() {
        url.push_str(&format!("?auth={}", auth_key));
    }
    let mut picker_lines: Vec<Line> = Vec::new();
    picker_lines.push(Line::from(Span::styled(
        crate::t!("dialog.dashboard_url.title"),
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )));
    picker_lines.push(Line::from(""));
    picker_lines.push(Line::from(vec![
        Span::styled(
            crate::t!("dialog.dashboard_url.host"),
            Style::default().fg(Color::Yellow),
        ),
        Span::styled(host_val, Style::default().fg(Color::White)),
    ]));
    picker_lines.push(Line::from(""));
    picker_lines.push(Line::from(vec![
        Span::styled(
            crate::t!("dialog.dashboard_url.backend"),
            Style::default().fg(Color::Yellow),
        ),
        Span::styled(&backend_str, Style::default().fg(Color::White)),
    ]));
    picker_lines.push(Line::from(vec![
        Span::styled(
            crate::t!("dialog.dashboard_url.threads"),
            Style::default().fg(Color::Yellow),
        ),
        Span::styled(&threads_str, Style::default().fg(Color::White)),
    ]));
    picker_lines.push(Line::from(vec![
        Span::styled(
            crate::t!("dialog.dashboard_url.threads_batch"),
            Style::default().fg(Color::Yellow),
        ),
        Span::styled(&threads_batch_str, Style::default().fg(Color::White)),
    ]));
    picker_lines.push(Line::from(vec![
        Span::styled(
            crate::t!("dialog.dashboard_url.mode"),
            Style::default().fg(Color::Yellow),
        ),
        Span::styled(&mode_str, Style::default().fg(Color::White)),
    ]));
    picker_lines.push(Line::from(vec![
        Span::styled(
            crate::t!("dialog.dashboard_url.api_endpoint"),
            Style::default().fg(Color::Yellow),
        ),
        Span::styled(api_str, Style::default().fg(Color::White)),
    ]));
    picker_lines.push(Line::from(vec![
        Span::styled(
            crate::t!("dialog.dashboard_url.rpc_workers"),
            Style::default().fg(Color::Yellow),
        ),
        Span::styled(&rpc_str, Style::default().fg(Color::White)),
    ]));
    picker_lines.push(Line::from(vec![
        Span::styled(
            crate::t!("dialog.dashboard_url.dashboard"),
            Style::default().fg(Color::Yellow),
        ),
        Span::styled(
            if ws_enabled {
                crate::t!("dialog.dashboard.enabled")
            } else {
                crate::t!("dialog.dashboard.disabled")
            },
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
            crate::t!("dialog.dashboard_url.copy"),
            Style::default().fg(Color::Black).bg(Color::DarkGray),
        ),
        Span::styled(
            crate::t!("dialog.dashboard_url.close"),
            Style::default().fg(Color::Black).bg(Color::DarkGray),
        ),
    ]));
    f.render_widget(Clear, picker_area);
    f.render_widget(
        Paragraph::new(picker_lines).block(
            Block::default()
                .title(Span::styled(
                    crate::t!("dialog.dashboard_url.title"),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow))
                .border_type(BorderType::Double),
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
            crate::t!("dialog.bench_result.baseline").to_string()
        } else {
            format_bench_params(&result.params, false).join(", ")
        };
        let main_title = Line::from(vec![
            Span::styled(
                crate::t!("dialog.bench_result.title"),
                Style::default().fg(Color::Yellow),
            ),
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
                    crate::t!("dialog.gguf.value"),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )),
            ]))
            .block(
                Block::default()
                    .title(crate::t!("dialog.bench_result.parameters"))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan))
                    .border_type(BorderType::Double),
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
                    "prompt_processing",
                    Style::default().fg(Color::Green),
                )),
                Cell::from(Span::styled(
                    format!("{:.2} ms", metrics_for_output.prompt_processing_time),
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
                    crate::t!("dialog.gguf.value"),
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
                    .border_style(Style::default().fg(Color::Cyan))
                    .border_type(BorderType::Double),
            );
        let output_lines: Vec<Line> = if !result.outputs.is_empty() {
            result.outputs[output_idx]
                .lines()
                .map(|l: &str| Line::from(l.to_string()))
                .collect()
        } else {
            vec![Line::from(crate::t!("dialog.bench_result.no_output"))]
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
            let max_height = area
                .height
                .saturating_sub(1 + top_height + 1 + output_y + 1); // leave room for output + controls
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
                .title(crate::t!("dialog.bench_result.server_cmd"))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow))
                .border_type(BorderType::Double);
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
                .title(crate::t!("dialog.bench_result.output"))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .border_type(BorderType::Double);
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
                crate::t!("dialog.bench_result.close"),
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
    let h: u16 = (8.min(area.height.saturating_sub(4))).max(7);
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
            crate::t!("dialog.search.title"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                crate::t!("dialog.search.label"),
                Style::default().fg(Color::Yellow),
            ),
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
                crate::t!("dialog.search.execute"),
                Style::default().fg(Color::Black).bg(Color::Yellow),
            ),
            Span::raw("  "),
            Span::styled(
                crate::t!("dialog.max_concurrent.cancel"),
                Style::default().fg(Color::Black).bg(Color::DarkGray),
            ),
        ]),
    ];
    f.render_widget(Clear, popup_area);
    f.render_widget(
        Paragraph::new(picker_lines).block(
            Block::default()
                .title(crate::t!("panel.title.search_input"))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow))
                .border_type(BorderType::Double),
        ),
        popup_area,
    );
}

fn render_gguf_naming_overlay(
    f: &mut Frame,
    area: Rect,
    explanation: &crate::tui::gguf_naming::GgufExplanation,
) {
    let title_len = explanation.model_family.chars().count().max(20) as u16;
    let w = (title_len + 40).clamp(70, 100).min(area.width - 4);
    let h = (explanation.segments.len() as u16 + 10)
        .clamp(12, 35)
        .min(area.height - 4);

    let popup_area = Rect {
        x: (area.width - w) / 2,
        y: (area.height - h) / 2,
        width: w,
        height: h,
    };

    let mut lines: Vec<Line> = Vec::new();

    // Title
    lines.push(Line::from(Span::styled(
        &explanation.model_family,
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    // Table header
    let header_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let segment_header = crate::t!("dialog.gguf.segment");
    let value_header = crate::t!("dialog.gguf.value");
    let desc_header = crate::t!("dialog.gguf.description");

    // Calculate column widths
    let max_label_width = explanation
        .segments
        .iter()
        .map(|s| s.label.chars().count())
        .max()
        .unwrap_or(7) as u16;
    let max_label_width = max_label_width.max(segment_header.chars().count() as u16);

    let label_w = max_label_width + 1;
    let value_w = 10;
    let desc_w = w - label_w - value_w - 6;

    lines.push(Line::from(vec![
        Span::styled(
            format!("{:<width$}", segment_header, width = label_w as usize),
            header_style,
        ),
        Span::raw("  "),
        Span::styled(
            format!("{:<width$}", value_header, width = value_w as usize),
            header_style,
        ),
        Span::raw("  "),
        Span::styled(desc_header, header_style),
    ]));
    lines.push(Line::from(Span::styled(
        format!(
            "{}  {}  {}",
            "─".repeat(label_w as usize),
            "─".repeat(value_w as usize),
            "─".repeat(desc_w as usize)
        ),
        Style::default().fg(Color::DarkGray),
    )));

    // Segments
    for segment in &explanation.segments {
        let header_spans: Vec<Span> = vec![
            Span::styled(
                format!("{:<width$}", segment.label, width = label_w as usize),
                Style::default().fg(Color::Cyan),
            ),
            Span::raw("  "),
            Span::styled(
                format!("{:<width$}", segment.value, width = value_w as usize),
                Style::default().fg(Color::White),
            ),
            Span::raw("  "),
        ];

        let desc_text = &segment.description;
        let available_w = desc_w as usize;

        if desc_text.width() <= available_w {
            // Fits on one line
            let mut combined = header_spans.clone();
            combined.push(Span::styled(
                desc_text.clone(),
                Style::default().fg(Color::Gray),
            ));
            lines.push(Line::from(combined));
        } else {
            // Wrap description into multiple lines
            let mut remaining: &str = desc_text;
            let mut first_line = true;
            while !remaining.is_empty() {
                let mut line_spans = if first_line {
                    header_spans.clone()
                } else {
                    vec![Span::raw(
                        " ".repeat(label_w as usize + 2 + value_w as usize + 2),
                    )]
                };

                // Find how many chars fit in available_w based on display width
                let mut display_width = 0;
                let mut byte_count = 0;
                for (i, ch) in remaining.char_indices() {
                    let ch_width = ch.to_string().width();
                    if display_width + ch_width > available_w {
                        break;
                    }
                    display_width += ch_width;
                    byte_count = i + ch.len_utf8();
                }

                if byte_count == 0 {
                    // Single char exceeds width — force it
                    byte_count = remaining
                        .char_indices()
                        .next()
                        .map(|(i, _)| i + 1)
                        .unwrap_or(remaining.len());
                }

                let line_text = &remaining[..byte_count];
                line_spans.push(Span::styled(
                    line_text.to_string(),
                    Style::default().fg(Color::Gray),
                ));
                lines.push(Line::from(line_spans));

                remaining = &remaining[byte_count..];
                if remaining.is_empty() {
                    break;
                }
                // Trim leading space for continuation lines
                if let Some((i, _)) = remaining.char_indices().next()
                    && remaining[i..].starts_with(' ')
                {
                    remaining = &remaining[i + 1..];
                }
                first_line = false;
            }
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![Span::styled(
        crate::t!("dialog.gguf.close"),
        Style::default().fg(Color::Yellow),
    )]));

    let block = Block::default()
        .title(Span::styled(
            crate::t!("dialog.gguf.title"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .border_type(BorderType::Double);

    f.render_widget(Clear, popup_area);
    f.render_widget(Paragraph::new(lines).block(block), popup_area);
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
    let h: u16 = (14.min(area.height - 4)).max(8);
    let picker_area = Rect {
        x: (area.width - w) / 2,
        y: (area.height - h) / 2,
        width: w,
        height: h,
    };
    let mut picker_lines: Vec<Line> = vec![
        Line::from(Span::styled(
            crate::t!("dialog.yarn.help"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    let fields = [
        (
            crate::t!("dialog.yarn.label"),
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
        format!(
            "{} {}",
            crate::t!("dialog.yarn.effective_context"),
            ctx_display
        ),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )));

    f.render_widget(Clear, picker_area);
    f.render_widget(
        Paragraph::new(picker_lines).block(
            Block::default()
                .title(Span::styled(
                    crate::t!("dialog.yarn.title"),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ))
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
    let h = (entries.len() as u16 + 6).min(area.height - 4);
    let picker_area = Rect {
        x: (area.width - w) / 2,
        y: (area.height - h) / 2,
        width: w,
        height: h,
    };
    let mut picker_lines: Vec<Line> = vec![
        Line::from(Span::styled(
            crate::t!("dialog.profile_picker.help"),
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
                .title(Span::styled(
                    crate::t!("dialog.spec.title"),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        ),
        picker_area,
    );
}

fn render_chat_template_picker(
    f: &mut Frame,
    area: Rect,
    _app: &App,
    entries: &[String],
    selected: usize,
) {
    let w = 55u16;
    let h = (entries.len() as u16 + 8).min(area.height - 4);
    let picker_area = Rect {
        x: (area.width - w) / 2,
        y: (area.height - h) / 2,
        width: w,
        height: h,
    };
    let mut picker_lines: Vec<Line> = vec![
        Line::from(Span::styled(
            crate::t!("dialog.chat_template.help"),
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
                .title(Span::styled(
                    crate::t!("dialog.chat_template.title"),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        ),
        picker_area,
    );
}

fn render_chat_template_file_picker(
    f: &mut Frame,
    area: Rect,
    _app: &App,
    entries: &[(String, String)],
    selected: usize,
) {
    let w = 60u16;
    let h = (entries.len() as u16 + 6).min(area.height - 4);
    let picker_area = Rect {
        x: (area.width - w) / 2,
        y: (area.height - h) / 2,
        width: w,
        height: h,
    };
    let mut picker_lines: Vec<Line> = vec![
        Line::from(Span::styled(
            crate::t!("dialog.chat_template.file.help"),
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
    ];

    if entries.is_empty() {
        picker_lines.push(Line::from(Span::styled(
            crate::t!("dialog.chat_template.file.empty"),
            Style::default().fg(Color::Red),
        )));
    } else {
        for (i, (name, _path)) in entries.iter().enumerate() {
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
                Span::styled(name, style),
            ]));
        }
    }

    f.render_widget(Clear, picker_area);
    f.render_widget(
        Paragraph::new(picker_lines).block(
            Block::default()
                .title(Span::styled(
                    crate::t!("dialog.chat_template.file.title"),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow))
                .border_type(BorderType::Double),
        ),
        picker_area,
    );
}
