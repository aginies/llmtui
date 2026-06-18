use super::App;
use super::{Line, Modifier, Span, Style};
use crate::tui::app::{ActivePanel, GlobalMode, ModelsMode};
use crate::tui::colors::*;
use crate::tui::render::hints::render_hints_line;
use ratatui::layout::Rect;

pub fn render_status_bar(app: &App, panel_area: Rect) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    // Line 1: mode + server status
    let mode_name: String = match &app.models_mode {
        ModelsMode::List { sort_by } => format!("{} | sort:{}", crate::t!("status.list"), sort_by.label().to_lowercase()),
        ModelsMode::Search { results, sort_by, .. } => format!("{} {} | sort:{}", crate::t!("status.search"), crate::t_fmt!("status.search_count", results.len()), sort_by.label().to_lowercase()),
        ModelsMode::Files { model_id, .. } => format!("{} | {}", crate::t!("status.files"), model_id),
        ModelsMode::BenchTune => crate::t!("status.bench_tune").to_string(),
    };
    let mut status_parts: Vec<Span<'static>> = Vec::new();
    
    status_parts.push(Span::styled(format!("[Mode: {}] ", mode_name), Style::default().fg(WHITE)));

    if app.is_settings_dirty() {
        status_parts.push(Span::raw("  "));
        status_parts.push(Span::styled(
            crate::t!("hints.unsaved_watermark").to_string(),
            Style::default()
                .fg(ratatui::style::Color::Rgb(255, 130, 130))
                .add_modifier(Modifier::BOLD),
        ));
    }

    if let Some(handle) = &app.server.server_handle {
        let inner: String = if app.server_mode == crate::models::ServerMode::Bench {
            crate::t!("status.benchmarking").to_string()
        } else if app.settings.api_endpoint_enabled {
            let tls = if app.server.running_server_tls.unwrap_or(false) {
                " TLS:On"
            } else {
                ""
            };
            format!("API:{} llama.cpp:{}{}", app.settings.api_endpoint_port, handle.port, tls)
        } else {
            format!("{} {}", handle.port, app.server_mode)
        };
        let mut content = format!("[{}]", inner);
        if app.config.default.web_search_enabled {
            content.push_str(" SEARXNG");
        }
        status_parts.push(Span::styled(content, Style::default().fg(GREEN)));
    } else if app.server_mode == crate::models::ServerMode::BenchTune {
        if let Some(progress) = &app.bench_tune.bench_tune_progress {
            match progress {
                crate::models::BenchTuneProgress::Running {
                    current,
                    total,
                    progress,
                    current_params: _,
                } => {
                    let progress_str = crate::t_fmt!(
                        "status.bench_tune_progress",
                        current,
                        total,
                        format!("{:.0}", progress)
                    );
                    status_parts.push(Span::styled(
                        format!("● {}", progress_str),
                        Style::default().fg(YELLOW),
                    ));
                }
                crate::models::BenchTuneProgress::Completed {
                    total_tests,
                    successful_tests,
                    elapsed,
                } => {
                    let elapsed_str = format!("{}s", elapsed.as_secs());
                    let progress_str = crate::t_fmt!(
                        "status.bench_tune_complete",
                        total_tests,
                        successful_tests,
                        elapsed_str
                    );
                    status_parts.push(Span::styled(
                        format!("● {}", progress_str),
                        Style::default().fg(GREEN),
                    ));
                }
                crate::models::BenchTuneProgress::PartiallyCompleted {
                    total_tests,
                    successful_tests,
                    failed_tests,
                    elapsed,
                } => {
                    let elapsed_str = format!("{}s", elapsed.as_secs());
                    let progress_str = crate::t_fmt!(
                        "status.bench_tune_partial",
                        total_tests,
                        successful_tests,
                        failed_tests,
                        elapsed_str
                    );
                    status_parts.push(Span::styled(
                        format!("● {}", progress_str),
                        Style::default().fg(YELLOW),
                    ));
                }
                crate::models::BenchTuneProgress::Cancelled {
                    total_tests,
                    successful_tests,
                    failed_tests,
                    elapsed,
                } => {
                    let elapsed_str = format!("{}s", elapsed.as_secs());
                    let progress_str = crate::t_fmt!(
                        "status.bench_tune_cancelled",
                        total_tests,
                        successful_tests,
                        failed_tests,
                        elapsed_str
                    );
                    status_parts.push(Span::styled(
                        format!("● {}", progress_str),
                        Style::default().fg(YELLOW),
                    ));
                }
                crate::models::BenchTuneProgress::Error { error } => {
                    status_parts.push(Span::styled(
                        format!("● {}", crate::t_fmt!("status.bench_tune_error", error)),
                        Style::default().fg(RED),
                    ));
                }
            }
        } else {
            status_parts.push(Span::styled(
                format!("● {}", crate::t!("status.bench_tune_ready")),
                Style::default().fg(YELLOW),
            ));
        }
    } else {
        status_parts.push(Span::styled(
            "[ N/A ]".to_string(),
            Style::default().fg(GREEN),
        ));
    }

    if matches!(app.ui.global_mode, GlobalMode::HostPicker { .. }) {
        status_parts.push(Span::raw("  "));
        status_parts.push(Span::styled(
            crate::t!("status.host_picker").to_string(),
            Style::default()
                .fg(YELLOW)
                .add_modifier(Modifier::BOLD),
        ));
    }
    if matches!(app.ui.global_mode, GlobalMode::RpcManager) {
        status_parts.push(Span::raw("  "));
        status_parts.push(Span::styled(
            crate::t!("status.rpc_manager").to_string(),
            Style::default()
                .fg(YELLOW)
                .add_modifier(Modifier::BOLD),
        ));
    }
    if matches!(app.ui.global_mode, GlobalMode::About) {
        status_parts.push(Span::raw("  "));
        status_parts.push(Span::styled(
            crate::t!("status.about").to_string(),
            Style::default()
                .fg(YELLOW)
                .add_modifier(Modifier::BOLD),
        ));
    }
    if let GlobalMode::BenchTuneSetup { editing_prompt, .. } = &app.ui.global_mode {
        status_parts.push(Span::raw("  "));
        if *editing_prompt {
            status_parts.push(Span::styled(
                crate::t!("status.editing_prompt").to_string(),
                Style::default()
                    .fg(CYAN)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            status_parts.push(Span::styled(
                crate::t!("status.bench_setup").to_string(),
                Style::default()
                    .fg(YELLOW)
                    .add_modifier(Modifier::BOLD),
            ));
        }
        }

    // Panel visibility indicator right-aligned at end of line
    let indicator = render_panel_visibility(app);
    let indicator_width: usize = indicator.iter().map(|s| s.width()).sum::<usize>() + 2; // brackets + spaces
    let existing_width: usize = status_parts.iter().map(|s| s.width()).sum();
    let available = panel_area.width.saturating_sub(2) as usize - existing_width;
    let padding = available.saturating_sub(indicator_width).saturating_sub(6);
    if padding > 0 {
        status_parts.push(Span::raw(" ".repeat(padding)));
    }
    status_parts.push(Span::styled("[", Style::default().fg(DIM_GRAY)));
    for (i, span) in indicator.iter().enumerate() {
        if i > 0 {
            status_parts.push(Span::raw(" "));
        }
        status_parts.push(span.clone());
    }
    status_parts.push(Span::styled("]", Style::default().fg(DIM_GRAY)));

    lines.push(Line::from(status_parts));
    lines.push(render_hints_line(app, panel_area));
    lines
}

fn render_panel_visibility(app: &App) -> Vec<Span<'static>> {
    let panels: [(u8, &str, ActivePanel); 6] = [
        (0, "Mdl", ActivePanel::Models),
        (1, "Srv", ActivePanel::ServerSettings),
        (2, "Info", ActivePanel::ModelInfo),
        (3, "LLM", ActivePanel::LlmSettings),
        (4, "Act", ActivePanel::ActiveModel),
        (5, "Log", ActivePanel::Log),
    ];

    let mut parts: Vec<Span<'static>> = Vec::new();

    for (bit, name, panel) in panels {
        let visible = app.is_panel_visible(bit);
        let focused = app.ui.active_panel == panel;

        if visible {
            let style = if focused {
                Style::default().fg(YELLOW).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(GREEN)
            };
            parts.push(Span::styled(name.to_string(), style));
        } else {
            parts.push(Span::styled("_", Style::default().fg(DIM_GRAY)));
        }
    }

    if app.download.downloading {
        parts.push(Span::styled("↓", Style::default().fg(DIM_GRAY)));
    }

    parts
}
