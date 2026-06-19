use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Gauge, Paragraph},
};

use crate::tui::colors::*;
use crate::models::{ModelState, model_filename};
use crate::tui::app::App;
use crate::tui::format_size;

/// Build a VRAM usage bar for the panel title.
/// Returns a Line with the bar + percentage, or a shortened variant for narrow panels.
fn vram_bar_line(used: u64, total: u64, panel_width: u16) -> Option<Line<'static>> {
    if total == 0 || used == 0 {
        return None;
    }

    let pct = (used as f64 / total as f64 * 100.0) as usize;
    let usage_str = format_size(used);
    let total_str = format_size(total);

    // Narrow terminal: just show percentage
    if panel_width < 20 {
        return Some(Line::from(vec![
            Span::styled(format!("{pct}%"), vram_color(pct)),
        ]));
    }

    // Medium terminal: show bar + percentage, no byte values
    if panel_width < 30 {
        let bar = vram_progress_bar(10, pct);
        return Some(Line::from(vec![
            Span::styled(bar, vram_color(pct)),
            Span::raw(" "),
            Span::styled(format!("{pct}%"), vram_color(pct)),
        ]));
    }

    // Wide terminal: full bar + percentage + byte values
    let bar = vram_progress_bar(10, pct);
    Some(Line::from(vec![
        Span::styled(bar, vram_color(pct)),
        Span::raw(" "),
        Span::styled(format!("{pct}%"), vram_color(pct)),
        Span::raw(" "),
        Span::styled(format!("{usage_str}/{total_str}"), CYAN),
    ]))
}

/// Build a VRAM progress bar of given width at the given percentage.
fn vram_progress_bar(width: usize, pct: usize) -> String {
    let filled = width * pct / 100;
    let empty = width - filled;
    format!(
        "[{}{}]",
        "█".repeat(filled),
        "░".repeat(empty),
    )
}

/// Get the appropriate color for a VRAM usage percentage.
fn vram_color(pct: usize) -> Color {
    if pct < 60 {
        VRAM_GREEN
    } else if pct < 80 {
        VRAM_YELLOW
    } else {
        VRAM_RED
    }
}

pub fn render(f: &mut Frame, area: Rect, app: &mut App) {
    // Use cached hint instead of scanning model_states every render.
    let mut loaded_models = if app.pending.active_model_hint_dirty {
        app.pending.active_model_hint_dirty = false;
        let mut result = Vec::new();
        for (name, state) in &app.model_states {
            if !matches!(state, ModelState::Available) {
                result.push((name.clone(), state.clone()));
            }
        }
        // In router mode, prefer the selected model, loaded or not.
        // Otherwise fall back to first loaded model.
        let hint = if app.server_mode == crate::models::ServerMode::Router {
            if let Some(selected) = app.selected_model() {
                let state = app.model_states.get(&selected.display_name).cloned().unwrap_or(ModelState::Available);
                Some((selected.display_name.clone(), state))
            } else {
                result.first().cloned()
            }
        } else {
            result.first().cloned()
        };
        app.active_model_hint = hint;
        if app.server_mode == crate::models::ServerMode::Router {
            if let Some(h) = &app.active_model_hint {
                vec![h.clone()]
            } else {
                result
            }
        } else {
            result
        }
    } else if let Some(hint) = &app.active_model_hint {
        vec![hint.clone()]
    } else {
        // Fallback scan if hint is not yet set
        let mut result = Vec::new();
        for (name, state) in &app.model_states {
            if !matches!(state, ModelState::Available) {
                result.push((name.clone(), state.clone()));
            }
        }
        let hint = if app.server_mode == crate::models::ServerMode::Router {
            if let Some(selected) = app.selected_model() {
                let state = app.model_states.get(&selected.display_name).cloned().unwrap_or(ModelState::Available);
                Some((selected.display_name.clone(), state))
            } else {
                result.first().cloned()
            }
        } else {
            result.first().cloned()
        };
        app.active_model_hint = hint;
        if app.server_mode == crate::models::ServerMode::Router {
            if let Some(h) = &app.active_model_hint {
                vec![h.clone()]
            } else {
                result
            }
        } else {
            result
        }
    };

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
        title_spans.push(Span::styled("[ ", Style::default().fg(WHITE)));
        title_spans.push(Span::styled(
            "Total VRAM: ",
            Style::default().fg(YELLOW),
        ));

        if let Some(bar_line) = vram_bar_line(
            app.metrics.total_vram_used,
            app.metrics.gpu_mem_total,
            area.width,
        ) {
            for span in bar_line.spans.iter() {
                title_spans.push(Span::styled(
                    span.content.clone(),
                    span.style,
                ));
            }
        } else {
            title_spans.push(Span::styled(
                format_size(app.metrics.total_vram_used),
                Style::default().fg(CYAN),
            ));
            title_spans.push(Span::styled(" / ", Style::default().fg(WHITE)));
            title_spans.push(Span::styled(
                format_size(app.metrics.gpu_mem_total),
                Style::default().fg(CYAN),
            ));
        }

        title_spans.push(Span::styled(" ]", Style::default().fg(WHITE)));
    }

    let is_active_focused = app.ui.active_panel == crate::tui::app::ActivePanel::ActiveModel;
    let (border_type, border_color) = if is_active_focused {
        (BorderType::Double, LIGHT_GREEN)
    } else {
        (BorderType::Rounded, LIGHT_GRAY)
    };

    let title_color = if is_active_focused { GREEN } else { YELLOW };
    let block = Block::default()
        .title(Line::from(title_spans))
        .title_style(Style::default().fg(title_color))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .border_type(border_type);

    let mut lines = Vec::new();

    // Robust check for Benchmarking - prioritize global flag
    if app.bench_tune.bench_tune_running {
   let display_name = if let Some(m) = app.selected_model() {
             model_filename(&m.name)
         } else {
             "Benchmarking".to_string()
         };

                lines.push(Line::from(vec![
                     Span::styled(" Model:  ", Style::default().fg(YELLOW)),
                     Span::styled(
                         display_name,
                         Style::default()
                             .fg(WHITE)
                             .add_modifier(Modifier::BOLD),
                     ),
                 ]));

        lines.push(Line::from(vec![
            Span::styled(" Status: ", Style::default().fg(YELLOW)),
            Span::styled(
                "BENCHMARKING",
                Style::default()
                    .fg(YELLOW)
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
                    let bar_width = area.width.saturating_sub(16) as usize;
                    let filled = (*p as f64 / 100.0 * bar_width as f64) as usize;
                    let bar = format!(
                        "[{}{}] {:.0}%",
                        "█".repeat(filled),
                        "░".repeat(bar_width.saturating_sub(filled)),
                        p
                    );
                    lines.push(Line::from(vec![
                        Span::styled(" Progress: ", Style::default().fg(YELLOW)),
                        Span::styled(bar, Style::default().fg(YELLOW)),
                    ]));
                    lines.push(Line::from(vec![
                        Span::styled(" Test: ", Style::default().fg(YELLOW)),
                        Span::styled(
                            format!("{}/{}", current, total),
                            Style::default().fg(WHITE),
                        ),
                    ]));

                    let p_str = crate::tui::format_bench_params(current_params, false).join(", ");

                    lines.push(Line::from(vec![
                        Span::styled(" Current: ", Style::default().fg(YELLOW)),
                        Span::styled(p_str, Style::default().fg(CYAN)),
                    ]));
                }
                crate::models::BenchTuneProgress::Completed {
                    total_tests,
                    successful_tests,
                    elapsed,
                } => {
                    let elapsed_str = format!("{}s", elapsed.as_secs());
                    lines.push(Line::from(vec![
                        Span::styled(" Results: ", Style::default().fg(YELLOW)),
                        Span::styled(
                            format!("{}/{} tests successful", successful_tests, total_tests),
                            Style::default().fg(WHITE),
                        ),
                    ]));
                    lines.push(Line::from(vec![
                        Span::styled(" Total Time: ", Style::default().fg(YELLOW)),
                        Span::styled(elapsed_str, Style::default().fg(WHITE)),
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
                        Span::styled(" Results: ", Style::default().fg(YELLOW)),
                        Span::styled(
                            format!("{}/{} tests successful", successful_tests, total_tests),
                            Style::default().fg(YELLOW),
                        ),
                    ]));
                    lines.push(Line::from(vec![
                        Span::styled(" Failed: ", Style::default().fg(YELLOW)),
                        Span::styled(
                            format!("{} test(s)", failed_tests),
                            Style::default().fg(RED),
                        ),
                    ]));
                    lines.push(Line::from(vec![
                        Span::styled(" Total Time: ", Style::default().fg(YELLOW)),
                        Span::styled(elapsed_str, Style::default().fg(WHITE)),
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
                        Span::styled(" Results: ", Style::default().fg(YELLOW)),
                        Span::styled(
                            format!("{}/{} tests successful", successful_tests, total_tests),
                            Style::default().fg(YELLOW),
                        ),
                    ]));
                    lines.push(Line::from(vec![
                        Span::styled(" Failed: ", Style::default().fg(YELLOW)),
                        Span::styled(
                            format!("{} test(s)", failed_tests),
                            Style::default().fg(RED),
                        ),
                    ]));
                    lines.push(Line::from(vec![
                        Span::styled(" Total Time: ", Style::default().fg(YELLOW)),
                        Span::styled(elapsed_str, Style::default().fg(WHITE)),
                    ]));
                }
                crate::models::BenchTuneProgress::Error { error } => {
                    lines.push(Line::from(vec![
                        Span::styled(" Error: ", Style::default().fg(RED)),
                        Span::styled(error, Style::default().fg(WHITE)),
                    ]));
                }
            }
        } else {
            lines.push(Line::from(vec![
                Span::styled(" Info:   ", Style::default().fg(YELLOW)),
                Span::styled(
                    "Starting first test...",
                    Style::default().fg(DIM_GRAY),
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
                let token_str = format!(
                    "{}/{} ({:.0}%)",
                    display_used, app.metrics.ctx_max, pct as f64
                );

                lines.push(Line::from(vec![
                    Span::styled(" Model:  ", Style::default().fg(YELLOW)),
                    Span::styled(
                        model_filename(name),
                        Style::default()
                            .fg(WHITE)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));

                let tps_style = Style::default().fg(CYAN);
                let prompt_style = Style::default().fg(CYAN);

                let tps_str = format!("{:.1}", app.metrics.tps);
                let prompt_str = format!("{:.1}", app.metrics.prompt_tps);

                let latency_str = if app.metrics.latency_per_token_ms > 0.0 {
                    format!("  {:.0}ms/tok", app.metrics.latency_per_token_ms)
                } else {
                    String::new()
                };

                let gen_tps_style = Style::default().fg(CYAN);

                let tps_parts = vec![
                    Span::styled(" [ ", Style::default().fg(WHITE)),
                    Span::styled("Tokens/s: ", Style::default().fg(YELLOW)),
                    Span::styled(tps_str, tps_style),
                    if !latency_str.is_empty() {
                        Span::styled(latency_str, Style::default().fg(CYAN))
                    } else {
                        Span::styled(" ".repeat(10), Style::default().fg(DIM_GRAY))
                    },
                    Span::styled(" (prompt: ", Style::default().fg(YELLOW)),
                    Span::styled(prompt_str, prompt_style),
                    Span::styled(")", Style::default().fg(YELLOW)),
                    Span::styled(" ]", Style::default().fg(WHITE)),
                    Span::styled("  [ ", Style::default().fg(WHITE)),
                    Span::styled("Decoded: ", Style::default().fg(YELLOW)),
                    Span::styled(
                        format!("{}", app.metrics.decoded_tokens),
                        Style::default().fg(CYAN),
                    ),
                    Span::styled("  ", Style::default().fg(WHITE)),
                    Span::styled("Gen: ", Style::default().fg(YELLOW)),
                    Span::styled(format!("{:.1}", app.metrics.gen_tps), gen_tps_style),
                    Span::styled(" t/s", Style::default().fg(CYAN)),
                    Span::styled(" ]", Style::default().fg(WHITE)),
                ];

                lines.push(Line::from(tps_parts));

             let mut context_parts = vec![
                      Span::styled(" [ ", Style::default().fg(WHITE)),
                      Span::styled("Context: ", Style::default().fg(YELLOW)),
                      Span::styled(token_str, Style::default().fg(CYAN)),
                      Span::styled(" ]", Style::default().fg(WHITE)),
                  ];

                if app.metrics.prompt_progress > 0.0 {
                    let bar_width = 20usize;
                    let filled = (app.metrics.prompt_progress * bar_width as f64) as usize;
                    let prompt_bar = format!(
                        "{}{}",
                        "█".repeat(filled),
                        "░".repeat(bar_width.saturating_sub(filled)),
                    );
                    let prompt_token_str = format!(
                        "{} tokens ({:.0} t/s)",
                        app.metrics.prompt_tokens,
                        app.metrics.prompt_tps_eval,
                    );
                    context_parts.push(Span::styled(" ", Style::default().fg(WHITE)));
                    context_parts.push(Span::styled(" [Progress: ", Style::default().fg(YELLOW)));
                    context_parts.push(Span::styled(prompt_bar, Style::default().fg(CYAN)));
                    context_parts.push(Span::styled(" ", Style::default().fg(CYAN)));
                    context_parts.push(Span::styled(format!("{}%", (app.metrics.prompt_progress * 100.0) as usize), Style::default().fg(CYAN)));
                    context_parts.push(Span::styled(" ", Style::default().fg(CYAN)));
                    context_parts.push(Span::styled(prompt_token_str, Style::default().fg(CYAN)));
                    context_parts.push(Span::styled(" ]", Style::default().fg(WHITE)));
                }

                  lines.push(Line::from(context_parts));

                lines.push(Line::from(vec![
                    Span::styled(" [ ", Style::default().fg(WHITE)),
                    Span::styled("CPU: ", Style::default().fg(YELLOW)),
                    Span::styled(
                        format!("{:.1}%", app.metrics.cpu_usage),
                        Style::default().fg(CYAN),
                    ),
                    Span::styled(" ]", Style::default().fg(WHITE)),
                    Span::styled("  [ ", Style::default().fg(WHITE)),
                    Span::styled("RAM: ", Style::default().fg(YELLOW)),
                    Span::styled(
                        format_size(app.metrics.ram_used),
                        Style::default().fg(CYAN),
                    ),
                    Span::styled(" ]", Style::default().fg(WHITE)),
                    Span::styled("  [ ", Style::default().fg(WHITE)),
                    Span::styled("VRAM: ", Style::default().fg(YELLOW)),
                    Span::styled(
                        format_size(app.metrics.gpu_mem_used),
                        Style::default().fg(CYAN),
                    ),
                    Span::styled(" / ", Style::default().fg(WHITE)),
                    Span::styled(
                        format_size(app.metrics.gpu_mem_total),
                        Style::default().fg(CYAN),
                    ),
                    Span::styled(" ]", Style::default().fg(WHITE)),
                ]));
            }
            ModelState::Benchmarking => {
                lines.push(Line::from(vec![
                    Span::styled(" Model:  ", Style::default().fg(YELLOW)),
                    Span::styled(
                        model_filename(name),
                        Style::default()
                            .fg(WHITE)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));

                lines.push(Line::from(vec![
                    Span::styled(" Status: ", Style::default().fg(YELLOW)),
                    Span::styled(
                        "BENCHMARKING",
                        Style::default()
                            .fg(YELLOW)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
            }
            ModelState::Loading => {
                lines.push(Line::from(vec![
                    Span::styled(" Model:  ", Style::default().fg(YELLOW)),
                    Span::styled(
                        model_filename(name),
                        Style::default()
                            .fg(WHITE)
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
                     Span::styled(" Status: ", Style::default().fg(YELLOW)),
                     Span::styled(status_content, Style::default().fg(YELLOW)),
                 ]));

                 if app.loading.loading_progress > 0.0 && app.loading.loading_progress <= 1.0 {
                    let ratio = app.loading.loading_progress as f64;
                    let bar_area = Rect {
                        x: area.x,
                        y: area.y + 4,
                        width: area.width.saturating_sub(2),
                        height: 1,
                    };
                    let gauge = Gauge::default()
                        .ratio(ratio)
                        .label(format!("{:.0}%", ratio * 100.0))
                        .gauge_style(Style::default().fg(YELLOW));
                    f.render_widget(gauge, bar_area);
                 } else {
                    let bar_area = Rect {
                        x: area.x,
                        y: area.y + 4,
                        width: area.width.saturating_sub(2),
                        height: 1,
                    };
                    let gauge = Gauge::default()
                        .ratio(0.0)
                        .label("0%")
                        .gauge_style(Style::default().fg(DIM_GRAY));
                    f.render_widget(gauge, bar_area);
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
                    .last_active_phase
                    .map(|p| p.label())
                    .unwrap_or("Loading...");
                let detail = detail_parts.join(", ");
                lines.push(Line::from(vec![
                    Span::styled("  ", Style::default()),
                    Span::styled(phase, Style::default().fg(CYAN)),
                    Span::raw(" "),
                    Span::styled(detail, Style::default().fg(MAGENTA)),
                ]));
            }
            ModelState::Failed { error } => {
                lines.push(Line::from(vec![
                    Span::styled(" Model:  ", Style::default().fg(YELLOW)),
                    Span::styled(
                        model_filename(name),
                        Style::default()
                            .fg(WHITE)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
                lines.push(Line::from(vec![
                    Span::styled(" Status: ", Style::default().fg(YELLOW)),
                    Span::styled(
                        "FAILED",
                        Style::default().fg(RED).add_modifier(Modifier::BOLD),
                    ),
                ]));
                lines.push(Line::from(vec![
                    Span::styled(" Error:  ", Style::default().fg(RED)),
                    Span::styled(error, Style::default().fg(WHITE)),
                ]));
            }
            ModelState::Available => {
                lines.push(Line::from(vec![
                    Span::styled(" Model:  ", Style::default().fg(YELLOW)),
                    Span::styled(
                        model_filename(name),
                        Style::default()
                            .fg(WHITE)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
                lines.push(Line::from(vec![
                    Span::styled(" Status: ", Style::default().fg(YELLOW)),
                    Span::styled(
                        "NOT LOADED",
                        Style::default()
                            .fg(DIM_GRAY)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
            }
        }
    } else {
        if app.server.server_handle.is_some() {
            lines.push(Line::from(vec![
                Span::styled(" Model:  ", Style::default().fg(YELLOW)),
                Span::styled(
                    "llama-server",
                    Style::default()
                        .fg(WHITE)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    crate::t!("active.no_model_loaded"),
                    Style::default().fg(DIM_GRAY),
                ),
            ]));
        } else {
            lines.push(Line::from(vec![Span::styled(
                crate::t!("active.no_model"),
                Style::default().fg(DIM_GRAY),
            )]));
            lines.push(Line::from(vec![Span::styled(
                crate::t!("active.no_model_hint"),
                Style::default().fg(DIM_GRAY),
            )]));
        }
    }

    let paragraph = Paragraph::new(lines).block(block);
    f.render_widget(paragraph, area);
}
