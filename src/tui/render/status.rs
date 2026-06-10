use super::App;
use super::{Color, Line, Modifier, Span, Style};
use crate::tui::app::{ActivePanel, GlobalMode, ModelsMode};
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
        Style::default().fg(Color::DarkGray),
    ));

    // Expert mode indicator removed from top bar

    if let Some(handle) = &app.server.server_handle {
        let label = if app.server_mode == crate::models::ServerMode::Bench {
            crate::t!("status.benchmarking").to_string()
        } else {
            format!("{} {}", handle.port, app.server_mode)
        };
        parts.push(Span::styled(
            format!("● {}", label),
            Style::default().fg(Color::Green),
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
                        Style::default().fg(Color::Yellow),
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
                        Style::default().fg(Color::Green),
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
                        Style::default().fg(Color::Yellow),
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
                        Style::default().fg(Color::Yellow),
                    ));
                }
                crate::models::BenchTuneProgress::Error { error } => {
                    parts.push(Span::styled(
                        format!("● {}", crate::t_fmt!("status.bench_tune_error", error)),
                        Style::default().fg(Color::Red),
                    ));
                }
            }
        } else {
            parts.push(Span::styled(
                format!("● {}", crate::t!("status.bench_tune_ready")),
                Style::default().fg(Color::Yellow),
            ));
        }
    } else {
        parts.push(Span::styled(
            format!("○ {}", crate::t!("status.server")),
            Style::default().fg(Color::DarkGray),
        ));
    }

    if matches!(app.ui.global_mode, GlobalMode::HostPicker { .. }) {
        parts.push(Span::raw("  "));
        parts.push(Span::styled(
            crate::t!("status.host_picker"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
    }
    if matches!(app.ui.global_mode, GlobalMode::RpcManager) {
        parts.push(Span::raw("  "));
        parts.push(Span::styled(
            crate::t!("status.rpc_manager"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
    }
    if matches!(app.ui.global_mode, GlobalMode::About) {
        parts.push(Span::raw("  "));
        parts.push(Span::styled(
            crate::t!("status.about"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
    }
    if let GlobalMode::BenchTuneSetup { editing_prompt, .. } = &app.ui.global_mode {
        parts.push(Span::raw("  "));
        if *editing_prompt {
            parts.push(Span::styled(
                crate::t!("status.editing_prompt"),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            parts.push(Span::styled(
                crate::t!("status.bench_setup"),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
        }
    }

    match &app.models_mode {
        ModelsMode::Search {
            query: _, sort_by, ..
        } => {
            parts.push(Span::raw("  "));
            if app.ui.active_panel == ActivePanel::Models {
                parts.push(Span::styled(
                    crate::t!("status.search"),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ));
            } else {
                let panel_label = match app.ui.active_panel {
                    ActivePanel::Log => crate::t!("status.log"),
                    ActivePanel::ServerSettings => crate::t!("status.server_panel"),
                    ActivePanel::LlmSettings => crate::t!("status.llm"),
                    ActivePanel::Profiles => crate::t!("status.profiles"),
                    ActivePanel::SystemPromptPresets => crate::t!("status.presets"),
                    ActivePanel::SearchReadme => crate::t!("status.readme"),
                    _ => crate::t!("status.search"),
                };
                parts.push(Span::styled(
                    panel_label,
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ));
            }
            parts.push(Span::raw(" "));
            parts.push(Span::styled(
                sort_by.label(),
                Style::default().fg(Color::Magenta),
            ));
        }
        ModelsMode::Files { model_id, .. } => {
            parts.push(Span::raw("  "));
            if app.ui.active_panel == ActivePanel::Models {
                parts.push(Span::styled(
                    crate::t!("status.files"),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ));
            } else {
                let panel_label = match app.ui.active_panel {
                    ActivePanel::Log => crate::t!("status.log"),
                    ActivePanel::ServerSettings => crate::t!("status.server_panel"),
                    ActivePanel::LlmSettings => crate::t!("status.llm"),
                    ActivePanel::Profiles => crate::t!("status.profiles"),
                    ActivePanel::SystemPromptPresets => crate::t!("status.presets"),
                    ActivePanel::SearchReadme => crate::t!("status.readme"),
                    _ => crate::t!("status.files"),
                };
                parts.push(Span::styled(
                    panel_label,
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ));
            }
            parts.push(Span::raw(" "));
            parts.push(Span::styled(model_id, Style::default().fg(Color::Cyan)));
        }
        ModelsMode::List { .. } => {
            parts.push(Span::raw("  "));
            let panel_label = match app.ui.active_panel {
                ActivePanel::Models => crate::t!("status.models"),
                ActivePanel::Log => crate::t!("status.log"),
                ActivePanel::ServerSettings => crate::t!("status.server_panel"),
                ActivePanel::LlmSettings => crate::t!("status.llm"),
                ActivePanel::Profiles => crate::t!("status.profiles"),
                ActivePanel::SystemPromptPresets => crate::t!("status.presets"),
                ActivePanel::SearchReadme => crate::t!("status.readme"),
                ActivePanel::Downloads => crate::t!("status.downloads"),
                _ => crate::t!("status.app"),
            };
            parts.push(Span::styled(
                panel_label,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
        }
        ModelsMode::BenchTune => {
            parts.push(Span::raw("  "));
            parts.push(Span::styled(
                crate::t!("status.benchtune"),
                Style::default()
                    .fg(Color::Yellow)
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
