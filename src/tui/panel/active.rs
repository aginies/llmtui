use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::models::{ModelState, strip_gguf};
use crate::tui::app::{App, LoadingPhase};
use crate::tui::format_size;

pub fn render(f: &mut Frame, area: Rect, app: &mut App) {
    // Get the actually loaded model(s) or the one currently being loaded
    let mut loaded_models = Vec::new();
    for (name, state) in &app.model_states {
        if !matches!(state, ModelState::Available) {
            loaded_models.push((name.clone(), state.clone()));
        }
    }

    // If no model is active in app.model_states, fallback to selected model
    // but only if it's actually in a non-available state.
    if loaded_models.is_empty()
        && let Some(m) = app.selected_model()
        && let Some(state) = app.model_states.get(&m.display_name)
        && !matches!(state, ModelState::Available)
    {
        loaded_models.push((m.display_name.clone(), state.clone()));
    }

    let mut title_spans = if loaded_models.len() == 1 {
        vec![Span::raw(crate::t!("panel.title.active_active"))]
    } else {
        vec![Span::raw(crate::t!("panel.title.active_multi_active"))]
    };

    if app.metrics.total_vram_used > 0 {
        title_spans.push(Span::styled("[ ", Style::default().fg(Color::White)));
        title_spans.push(Span::styled(
            "Total VRAM: ",
            Style::default().fg(Color::Yellow),
        ));
        title_spans.push(Span::styled(
            format_size(app.metrics.total_vram_used),
            Style::default().fg(Color::Cyan),
        ));
        title_spans.push(Span::styled(" / ", Style::default().fg(Color::White)));
        title_spans.push(Span::styled(
            format_size(app.metrics.gpu_mem_total),
            Style::default().fg(Color::Cyan),
        ));
        title_spans.push(Span::styled(" ]", Style::default().fg(Color::White)));
    }

    let block = Block::default()
        .title(Line::from(title_spans))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(
            if app.ui.active_panel == crate::tui::app::ActivePanel::ActiveModel {
                Color::Green
            } else {
                Color::DarkGray
            },
        ));

    let mut lines = Vec::new();

    // Robust check for Benchmarking - prioritize global flag
    if app.bench_tune.bench_tune_running {
        let display_name = if let Some(m) = app.selected_model() {
            strip_gguf(&m.name).to_string()
        } else {
            "Benchmarking".to_string()
        };

        lines.push(Line::from(vec![
            Span::styled(" Model:  ", Style::default().fg(Color::Yellow)),
            Span::styled(
                display_name,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));

        lines.push(Line::from(vec![
            Span::styled(" Status: ", Style::default().fg(Color::Yellow)),
            Span::styled(
                "BENCHMARKING",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));

        if let Some(progress) = &app.bench_tune.bench_tune_progress {
            match progress {
                crate::models::BenchTuneProgress::Running {
                    current,
                    total,
                    progress: p,
                    current_params,
                } => {
                    let label = " Progress: ";
                    let overhead = label.len() + 2 + 6;
                    let bar_width = area.width.saturating_sub(overhead as u16 + 2) as usize;
                    let filled = (*p as f64 / 100.0 * bar_width as f64) as usize;
                    let bar = format!(
                        "[{}{}] {:.0}%",
                        "█".repeat(filled),
                        "░".repeat(bar_width.saturating_sub(filled)),
                        p
                    );
                    lines.push(Line::from(vec![
                        Span::styled(label, Style::default().fg(Color::Yellow)),
                        Span::styled(bar, Style::default().fg(Color::Yellow)),
                    ]));
                    lines.push(Line::from(vec![
                        Span::styled(" Test: ", Style::default().fg(Color::Yellow)),
                        Span::styled(
                            format!("{}/{}", current, total),
                            Style::default().fg(Color::White),
                        ),
                    ]));

                    let p_str = crate::tui::format_bench_params(current_params, false).join(", ");

                    lines.push(Line::from(vec![
                        Span::styled(" Current: ", Style::default().fg(Color::Yellow)),
                        Span::styled(p_str, Style::default().fg(Color::Cyan)),
                    ]));
                }
                crate::models::BenchTuneProgress::Completed {
                    total_tests,
                    successful_tests,
                    elapsed,
                } => {
                    let elapsed_str = format!("{}s", elapsed.as_secs());
                    lines.push(Line::from(vec![
                        Span::styled(" Results: ", Style::default().fg(Color::Yellow)),
                        Span::styled(
                            format!("{}/{} tests successful", successful_tests, total_tests),
                            Style::default().fg(Color::White),
                        ),
                    ]));
                    lines.push(Line::from(vec![
                        Span::styled(" Total Time: ", Style::default().fg(Color::Yellow)),
                        Span::styled(elapsed_str, Style::default().fg(Color::White)),
                    ]));
                }
                crate::models::BenchTuneProgress::PartiallyCompleted {
                    total_tests,
                    successful_tests,
                    failed_tests,
                    elapsed,
                } => {
                    let elapsed_str = format!("{}s", elapsed.as_secs());
                    lines.push(Line::from(vec![
                        Span::styled(" Results: ", Style::default().fg(Color::Yellow)),
                        Span::styled(
                            format!("{}/{} tests successful", successful_tests, total_tests),
                            Style::default().fg(Color::Yellow),
                        ),
                    ]));
                    lines.push(Line::from(vec![
                        Span::styled(" Failed: ", Style::default().fg(Color::Yellow)),
                        Span::styled(
                            format!("{} test(s)", failed_tests),
                            Style::default().fg(Color::Red),
                        ),
                    ]));
                    lines.push(Line::from(vec![
                        Span::styled(" Total Time: ", Style::default().fg(Color::Yellow)),
                        Span::styled(elapsed_str, Style::default().fg(Color::White)),
                    ]));
                }
                crate::models::BenchTuneProgress::Cancelled {
                    total_tests,
                    successful_tests,
                    failed_tests,
                    elapsed,
                } => {
                    let elapsed_str = format!("{}s", elapsed.as_secs());
                    lines.push(Line::from(vec![
                        Span::styled(" Results: ", Style::default().fg(Color::Yellow)),
                        Span::styled(
                            format!("{}/{} tests successful", successful_tests, total_tests),
                            Style::default().fg(Color::Yellow),
                        ),
                    ]));
                    lines.push(Line::from(vec![
                        Span::styled(" Failed: ", Style::default().fg(Color::Yellow)),
                        Span::styled(
                            format!("{} test(s)", failed_tests),
                            Style::default().fg(Color::Red),
                        ),
                    ]));
                    lines.push(Line::from(vec![
                        Span::styled(" Total Time: ", Style::default().fg(Color::Yellow)),
                        Span::styled(elapsed_str, Style::default().fg(Color::White)),
                    ]));
                }
                crate::models::BenchTuneProgress::Error { error } => {
                    lines.push(Line::from(vec![
                        Span::styled(" Error: ", Style::default().fg(Color::Red)),
                        Span::styled(error, Style::default().fg(Color::White)),
                    ]));
                }
            }
        } else {
            lines.push(Line::from(vec![
                Span::styled(" Info:   ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    "Starting first test...",
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }
    } else if let Some((name, state)) = loaded_models.first() {
        match state {
            ModelState::Loaded { .. } => {
                let display_used = app.metrics.ctx_used.max(2049);
                let pct = if app.metrics.ctx_max > 0 {
                    (display_used as f64 / app.metrics.ctx_max as f64 * 100.0).ceil() as usize
                } else {
                    0
                };
                let bar_width = 20usize;
                let filled = (pct as f64 / 100.0 * bar_width as f64) as usize;
                let bar_only = format!(
                    "{}{}",
                    "█".repeat(filled),
                    "░".repeat(bar_width.saturating_sub(filled)),
                );
                let token_str = format!(
                    "{}/{} ({:.0}%)",
                    display_used, app.metrics.ctx_max, pct as f64
                );

                lines.push(Line::from(vec![
                    Span::styled(" Model:  ", Style::default().fg(Color::Yellow)),
                    Span::styled(
                        strip_gguf(name),
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));

                let tps_style = if app.metrics.tps > 30.0 {
                    Style::default().fg(Color::Green)
                } else if app.metrics.tps > 15.0 {
                    Style::default().fg(Color::Yellow)
                } else if app.metrics.tps > 0.0 {
                    Style::default().fg(Color::Red)
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                let prompt_style = if app.metrics.prompt_tps > 100.0 {
                    Style::default().fg(Color::Green)
                } else if app.metrics.prompt_tps > 50.0 {
                    Style::default().fg(Color::Yellow)
                } else if app.metrics.prompt_tps > 0.0 {
                    Style::default().fg(Color::Red)
                } else {
                    Style::default().fg(Color::DarkGray)
                };

                let tps_str = format!("{:.1}", app.metrics.tps);
                let prompt_str = format!("{:.1}", app.metrics.prompt_tps);

                let latency_str = if app.metrics.latency_per_token_ms > 0.0 {
                    format!("  {:.0}ms/tok", app.metrics.latency_per_token_ms)
                } else {
                    String::new()
                };

                let gen_tps_style = if app.metrics.gen_tps > 30.0 {
                    Style::default().fg(Color::Green)
                } else if app.metrics.gen_tps > 15.0 {
                    Style::default().fg(Color::Yellow)
                } else if app.metrics.gen_tps > 0.0 {
                    Style::default().fg(Color::Red)
                } else {
                    Style::default().fg(Color::DarkGray)
                };

                let tps_parts = vec![
                    Span::styled(" [ ", Style::default().fg(Color::White)),
                    Span::styled("Tokens/s: ", Style::default().fg(Color::Yellow)),
                    Span::styled(tps_str, tps_style),
                    if !latency_str.is_empty() {
                        Span::styled(latency_str, Style::default().fg(Color::DarkGray))
                    } else {
                        Span::styled(" ".repeat(10), Style::default().fg(Color::DarkGray))
                    },
                    Span::styled(" (prompt: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(prompt_str, prompt_style),
                    Span::styled(")", Style::default().fg(Color::DarkGray)),
                    Span::styled(" ]", Style::default().fg(Color::White)),
                    Span::styled("  [ ", Style::default().fg(Color::White)),
                    Span::styled("Decoded: ", Style::default().fg(Color::Yellow)),
                    Span::styled(
                        format!("{}", app.metrics.decoded_tokens),
                        Style::default().fg(Color::Cyan),
                    ),
                    Span::styled("  ", Style::default().fg(Color::White)),
                    Span::styled("Gen: ", Style::default().fg(Color::Yellow)),
                    Span::styled(format!("{:.1}", app.metrics.gen_tps), gen_tps_style),
                    Span::styled(" t/s", Style::default().fg(Color::DarkGray)),
                    Span::styled(" ]", Style::default().fg(Color::White)),
                ];

                lines.push(Line::from(tps_parts));

                let context_parts = vec![
                    Span::styled(" [ ", Style::default().fg(Color::White)),
                    Span::styled("Context: ", Style::default().fg(Color::Yellow)),
                    Span::styled(bar_only, Style::default().fg(Color::Cyan)),
                    Span::styled(" ", Style::default().fg(Color::Cyan)),
                    Span::styled(token_str, Style::default().fg(Color::Cyan)),
                    Span::styled(" ]", Style::default().fg(Color::White)),
                ];

                lines.push(Line::from(context_parts));

                lines.push(Line::from(vec![
                    Span::styled(" [ ", Style::default().fg(Color::White)),
                    Span::styled("CPU: ", Style::default().fg(Color::Yellow)),
                    Span::styled(
                        format!("{:.1}%", app.metrics.cpu_usage),
                        Style::default().fg(Color::Cyan),
                    ),
                    Span::styled(" ]", Style::default().fg(Color::White)),
                    Span::styled("  [ ", Style::default().fg(Color::White)),
                    Span::styled("RAM: ", Style::default().fg(Color::Yellow)),
                    Span::styled(
                        format_size(app.metrics.ram_used),
                        Style::default().fg(Color::Cyan),
                    ),
                    Span::styled(" ]", Style::default().fg(Color::White)),
                    Span::styled("  [ ", Style::default().fg(Color::White)),
                    Span::styled("VRAM: ", Style::default().fg(Color::Yellow)),
                    Span::styled(
                        format_size(app.metrics.gpu_mem_used),
                        Style::default().fg(Color::Cyan),
                    ),
                    Span::styled(" / ", Style::default().fg(Color::White)),
                    Span::styled(
                        format_size(app.metrics.gpu_mem_total),
                        Style::default().fg(Color::Cyan),
                    ),
                    Span::styled(" ]", Style::default().fg(Color::White)),
                ]));
            }
            ModelState::Benchmarking => {
                lines.push(Line::from(vec![
                    Span::styled(" Model:  ", Style::default().fg(Color::Yellow)),
                    Span::styled(
                        strip_gguf(name),
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));

                lines.push(Line::from(vec![
                    Span::styled(" Status: ", Style::default().fg(Color::Yellow)),
                    Span::styled(
                        "BENCHMARKING",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
            }
            ModelState::Loading => {
                lines.push(Line::from(vec![
                    Span::styled(" Model:  ", Style::default().fg(Color::Yellow)),
                    Span::styled(
                        strip_gguf(name),
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));

                let status_content = if app.loading.loading_progress <= 0.0 {
                    let spinners = ["⠋", "⠙", "⠹", "⠸"];
                    format!("LOADING {}", spinners[app.loading.loading_spinner])
                } else {
                    "LOADING".to_string()
                };
                lines.push(Line::from(vec![
                    Span::styled(" Status: ", Style::default().fg(Color::Yellow)),
                    Span::styled(status_content, Style::default().fg(Color::Yellow)),
                ]));

                let overhead = 2 + 5;
                let bar_width = area.width.saturating_sub(overhead as u16 + 2) as usize;

                if app.loading.loading_progress > 0.0 && app.loading.loading_progress <= 1.0 {
                    let filled = (app.loading.loading_progress * bar_width as f32) as usize;
                    let bar = format!(
                        "[{}{}] {:.0}%",
                        "█".repeat(filled),
                        "░".repeat(bar_width.saturating_sub(filled)),
                        app.loading.loading_progress * 100.0
                    );
                    lines.push(Line::from(vec![Span::styled(
                        bar,
                        Style::default().fg(Color::Yellow),
                    )]));
                } else {
                    // Show empty progress bar with spinner
                    let bar = format!("[{}] 0%", "░".repeat(bar_width));
                    lines.push(Line::from(vec![Span::styled(
                        bar,
                        Style::default().fg(Color::DarkGray),
                    )]));
                }

                let mut detail_parts = Vec::new();
                if let (Some(loaded), Some(total)) = (
                    app.loading.load_progress.layers_loaded,
                    app.loading.load_progress.layers_total,
                ) {
                    detail_parts.push(format!("({}/{})", loaded, total));
                }
                if app.loading.load_progress.tensors_loaded > 0 {
                    detail_parts.push(format!(
                        "{} tensors",
                        app.loading.load_progress.tensors_loaded
                    ));
                }
                let total_gpu: f64 = app
                    .loading
                    .load_progress
                    .buffers
                    .iter()
                    .filter(|b| b.device != "CPU_Mapped" && b.device != "CPU_Cached")
                    .map(|b| b.buffer_size_mib)
                    .sum();
                if total_gpu > 0.0 {
                    detail_parts.push(format!(
                        "{} VRAM",
                        format_size((total_gpu * 1024.0 * 1024.0) as u64)
                    ));
                }

                let phase = app
                    .loading
                    .loading_phases
                    .iter()
                    .next()
                    .map(|p: &LoadingPhase| p.label())
                    .unwrap_or("Loading...");
                let detail = detail_parts.join(", ");
                lines.push(Line::from(vec![
                    Span::styled("  ", Style::default()),
                    Span::styled(phase, Style::default().fg(Color::Cyan)),
                    Span::raw(" "),
                    Span::styled(detail, Style::default().fg(Color::Magenta)),
                ]));
            }
            ModelState::Failed { error } => {
                lines.push(Line::from(vec![
                    Span::styled(" Model:  ", Style::default().fg(Color::Yellow)),
                    Span::styled(
                        strip_gguf(name),
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
                lines.push(Line::from(vec![
                    Span::styled(" Status: ", Style::default().fg(Color::Yellow)),
                    Span::styled(
                        "FAILED",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    ),
                ]));
                lines.push(Line::from(vec![
                    Span::styled(" Error:  ", Style::default().fg(Color::Red)),
                    Span::styled(error, Style::default().fg(Color::White)),
                ]));
            }
            ModelState::Available => unreachable!(),
        }
    } else {
        if app.server.server_handle.is_some() {
            lines.push(Line::from(vec![
                Span::styled(" Model:  ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    "llama-server",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    crate::t!("active.no_model_loaded"),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        } else {
            lines.push(Line::from(vec![Span::styled(
                crate::t!("active.no_model"),
                Style::default().fg(Color::DarkGray),
            )]));
            lines.push(Line::from(vec![Span::styled(
                crate::t!("active.no_model_hint"),
                Style::default().fg(Color::DarkGray),
            )]));
        }
    }

    let paragraph = Paragraph::new(lines).block(block);
    f.render_widget(paragraph, area);
}
