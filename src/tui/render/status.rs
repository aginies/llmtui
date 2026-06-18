use super::App;
use super::{Line, Modifier, Span, Style};
use crate::tui::app::{GlobalMode, ModelsMode};
use crate::tui::colors::*;
use crate::tui::render::hints::render_hints;
use ratatui::layout::Rect;

pub fn render_status_bar<'a>(app: &'a App, panel_area: Rect) -> Line<'a> {
    let mut parts = Vec::new();

    let mode_name = match &app.models_mode {
        ModelsMode::List { .. } => crate::t!("status.list").to_string(),
        ModelsMode::Search { results, .. } => format!("{} {}", crate::t!("status.search"), crate::t_fmt!("status.search_count", results.len())),
        ModelsMode::Files { files, .. } => crate::t_fmt!("status.files", files.len()),
        ModelsMode::BenchTune => crate::t!("status.bench_tune").to_string(),
    };
    parts.push(Span::styled(
        format!("[Mode: {}] ", mode_name),
        Style::default().fg(WHITE),
    ));

    // Expert mode indicator removed from top bar

    if let Some(handle) = &app.server.server_handle {
        let inner = if app.server_mode == crate::models::ServerMode::Bench {
            crate::t!("status.benchmarking").to_string()
        } else if app.settings.api_endpoint_enabled {
            let tls = if app.server.running_server_tls.unwrap_or(false) {
                " TLS:On"
            } else {
                ""
            };
            format!("api:{} llm:{}{}", app.settings.api_endpoint_port, handle.port, tls)
        } else {
            format!("{} {}", handle.port, app.server_mode)
        };
        let mut content = format!("[{}]", inner);
        if app.config.default.web_search_enabled {
            content.push_str(" SEARXNG");
        }
        parts.push(Span::styled(
            content,
            Style::default().fg(GREEN),
        ));
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
                    parts.push(Span::styled(
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
                    parts.push(Span::styled(
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
                    parts.push(Span::styled(
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
                    parts.push(Span::styled(
                        format!("● {}", progress_str),
                        Style::default().fg(YELLOW),
                    ));
                }
                crate::models::BenchTuneProgress::Error { error } => {
                    parts.push(Span::styled(
                        format!("● {}", crate::t_fmt!("status.bench_tune_error", error)),
                        Style::default().fg(RED),
                    ));
                }
            }
        } else {
            parts.push(Span::styled(
                format!("● {}", crate::t!("status.bench_tune_ready")),
                Style::default().fg(YELLOW),
            ));
        }
    } else {
        parts.push(Span::styled(
            "[ N/A ]",
            Style::default().fg(GREEN),
        ));
    }

    if matches!(app.ui.global_mode, GlobalMode::HostPicker { .. }) {
        parts.push(Span::raw("  "));
        parts.push(Span::styled(
            crate::t!("status.host_picker"),
            Style::default()
                .fg(YELLOW)
                .add_modifier(Modifier::BOLD),
        ));
    }
    if matches!(app.ui.global_mode, GlobalMode::RpcManager) {
        parts.push(Span::raw("  "));
        parts.push(Span::styled(
            crate::t!("status.rpc_manager"),
            Style::default()
                .fg(YELLOW)
                .add_modifier(Modifier::BOLD),
        ));
    }
    if matches!(app.ui.global_mode, GlobalMode::About) {
        parts.push(Span::raw("  "));
        parts.push(Span::styled(
            crate::t!("status.about"),
            Style::default()
                .fg(YELLOW)
                .add_modifier(Modifier::BOLD),
        ));
    }
    if let GlobalMode::BenchTuneSetup { editing_prompt, .. } = &app.ui.global_mode {
        parts.push(Span::raw("  "));
        if *editing_prompt {
            parts.push(Span::styled(
                crate::t!("status.editing_prompt"),
                Style::default()
                    .fg(CYAN)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            parts.push(Span::styled(
                crate::t!("status.bench_setup"),
                Style::default()
                    .fg(YELLOW)
                    .add_modifier(Modifier::BOLD),
            ));
        }
    }

    match &app.models_mode {
        ModelsMode::Search {
            query: _, sort_by, ..
        } => {
            parts.push(Span::raw("  "));
            parts.push(Span::styled(
                sort_by.label(),
                Style::default().fg(CYAN),
            ));
        }
        ModelsMode::Files { model_id, .. } => {
            parts.push(Span::raw("  "));
            parts.push(Span::styled(model_id, Style::default().fg(CYAN)));
        }
        ModelsMode::List { .. } => {}
        ModelsMode::BenchTune => {
            parts.push(Span::raw("  "));
            parts.push(Span::styled(
                crate::t!("status.benchtune"),
                Style::default()
                    .fg(YELLOW)
                    .add_modifier(Modifier::BOLD),
            ));
        }
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
