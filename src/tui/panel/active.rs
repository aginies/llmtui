use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::tui::app::App;
use crate::tui::format_size;
use crate::models::{strip_gguf, ModelState};

pub fn render(f: &mut Frame, area: Rect, app: &mut App) {
    let mut title_spans = if app.is_panel_visible(4) {
        vec![Span::raw(" Active Model (F5) ")]
    } else {
        vec![Span::raw(" Active Model(s) (F5) ")]
    };
    if app.metrics.total_vram_used > 0 {
        title_spans.push(Span::styled("[ ", Style::default().fg(Color::White)));
        title_spans.push(Span::styled("Total VRAM: ", Style::default().fg(Color::Yellow)));
        title_spans.push(Span::styled(format_size(app.metrics.total_vram_used), Style::default().fg(Color::Cyan)));
        title_spans.push(Span::styled(" / ", Style::default().fg(Color::White)));
        title_spans.push(Span::styled(format_size(app.metrics.gpu_mem_total), Style::default().fg(Color::Cyan)));
        title_spans.push(Span::styled(" ]", Style::default().fg(Color::White)));
    }

    let block = Block::default()
        .title(Line::from(title_spans))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if app.active_panel == crate::tui::app::ActivePanel::ActiveModel { Color::Green } else { Color::DarkGray }));

    let mut lines = Vec::new();

    // Get currently loaded model or the one being loaded
    let model = app.selected_model();
    let state = model.and_then(|m| app.model_states.get(&m.display_name));

    // Robust check for Benchmarking - prioritize global flag
    if app.bench_tune_running {
        if let Some(m) = model {
            lines.push(Line::from(vec![
                Span::styled(" Model:  ", Style::default().fg(Color::Yellow)),
                Span::styled(strip_gguf(&m.name), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            ]));
            
            lines.push(Line::from(vec![
                Span::styled(" Status: ", Style::default().fg(Color::Yellow)),
                Span::styled("BENCHMARKING", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            ]));

            if let Some(progress) = &app.bench_tune_progress {
                match progress {
                    crate::models::BenchTuneProgress::Running { current, total, progress: p, current_params } => {
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
                            Span::styled(format!("{}/{}", current, total), Style::default().fg(Color::White)),
                        ]));
                        
                        let p_str = crate::tui::format_bench_params(current_params, false).join(", ");

                        lines.push(Line::from(vec![
                            Span::styled(" Current: ", Style::default().fg(Color::Yellow)),
                            Span::styled(p_str, Style::default().fg(Color::Cyan)),
                        ]));
                    }
                    crate::models::BenchTuneProgress::Completed { total_tests, successful_tests, elapsed } => {
                        let elapsed_str = format!("{}s", elapsed.as_secs());
                        lines.push(Line::from(vec![
                            Span::styled(" Results: ", Style::default().fg(Color::Yellow)),
                            Span::styled(format!("{}/{} tests successful", successful_tests, total_tests), Style::default().fg(Color::White)),
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
                    Span::styled("Starting first test...", Style::default().fg(Color::DarkGray)),
                ]));
            }
        }
    } else {
        match state {
            Some(ModelState::Loaded { .. }) => {
                let m = model.unwrap();
                let pct = if app.metrics.ctx_max > 0 {
                    (app.metrics.ctx_used as f64 / app.metrics.ctx_max as f64 * 100.0).ceil() as usize
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
                let token_str = format!("{}/{} ({:.0}%)", app.metrics.ctx_used, app.metrics.ctx_max, pct as f64 / 100.0 * 100.0);
                
                lines.push(Line::from(vec![
                    Span::styled(" Model:  ", Style::default().fg(Color::Yellow)),
                    Span::styled(strip_gguf(&m.name), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
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
                
                let mut tps_parts = Vec::new();
                tps_parts.push(Span::styled(" [ ", Style::default().fg(Color::White)));
                tps_parts.push(Span::styled("Tokens/s: ", Style::default().fg(Color::Yellow)));
                tps_parts.push(Span::styled(tps_str, tps_style));
                if !latency_str.is_empty() {
                    tps_parts.push(Span::styled(latency_str, Style::default().fg(Color::DarkGray)));
                }
                tps_parts.push(Span::styled(" (prompt: ", Style::default().fg(Color::DarkGray)));
                tps_parts.push(Span::styled(prompt_str, prompt_style));
                tps_parts.push(Span::styled(")", Style::default().fg(Color::DarkGray)));
                tps_parts.push(Span::styled(" ]", Style::default().fg(Color::White)));
                tps_parts.push(Span::styled("  [ ", Style::default().fg(Color::White)));
                tps_parts.push(Span::styled("Context: ", Style::default().fg(Color::Yellow)));
                tps_parts.push(Span::styled(bar_only, Style::default().fg(Color::Cyan)));
                tps_parts.push(Span::styled(" ", Style::default().fg(Color::Cyan)));
                tps_parts.push(Span::styled(token_str, Style::default().fg(Color::Cyan)));
                tps_parts.push(Span::styled(" ]", Style::default().fg(Color::White)));
                
                lines.push(Line::from(tps_parts));

                lines.push(Line::from(vec![
                    Span::styled(" [ ", Style::default().fg(Color::White)),
                    Span::styled("Tokens Generated: ", Style::default().fg(Color::Yellow)),
                    Span::styled(format!("{:.1}", app.metrics.throughput), Style::default().fg(Color::Green)),
                    Span::styled(" t/s, Decoded: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(format!("{}", app.metrics.decoded_tokens), Style::default().fg(Color::Cyan)),
                    Span::styled(" ]", Style::default().fg(Color::White)),
                ]));

                lines.push(Line::from(vec![
                    Span::styled(" [ ", Style::default().fg(Color::White)),
                    Span::styled("CPU: ", Style::default().fg(Color::Yellow)),
                    Span::styled(format!("{:.1}%", app.metrics.cpu_usage), Style::default().fg(Color::Cyan)),
                    Span::styled(" ]", Style::default().fg(Color::White)),
                    Span::styled("  [ ", Style::default().fg(Color::White)),
                    Span::styled("RAM: ", Style::default().fg(Color::Yellow)),
                    Span::styled(format_size(app.metrics.ram_used), Style::default().fg(Color::Cyan)),
                    Span::styled(" ]", Style::default().fg(Color::White)),
                    Span::styled("  [ ", Style::default().fg(Color::White)),
                    Span::styled("VRAM: ", Style::default().fg(Color::Yellow)),
                    Span::styled(format_size(app.metrics.gpu_mem_used), Style::default().fg(Color::Cyan)),
                    Span::styled(" / ", Style::default().fg(Color::White)),
                    Span::styled(format_size(app.metrics.gpu_mem_total), Style::default().fg(Color::Cyan)),
                    Span::styled(" ]", Style::default().fg(Color::White)),
                ]));
            }
            Some(ModelState::Benchmarking) => {
                let m = model.unwrap();
                lines.push(Line::from(vec![
                    Span::styled(" Model:  ", Style::default().fg(Color::Yellow)),
                    Span::styled(strip_gguf(&m.name), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                ]));
                
                lines.push(Line::from(vec![
                    Span::styled(" Status: ", Style::default().fg(Color::Yellow)),
                    Span::styled("BENCHMARKING", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                ]));
                // ... logic for when bench_tune_running is false but state is still Benchmarking ...
                // This shouldn't happen much with the new check above.
            }
            Some(ModelState::Loading) => {
                let m = model.unwrap();
                lines.push(Line::from(vec![
                    Span::styled(" Model:  ", Style::default().fg(Color::Yellow)),
                    Span::styled(strip_gguf(&m.name), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                ]));
                
                let status_content = if app.loading_progress <= 0.0 {
                    let spinners = ["⠋", "⠙", "⠹", "⠸"];
                    format!("LOADING {}", spinners[app.loading_spinner])
                } else {
                    "LOADING".to_string()
                };
                lines.push(Line::from(vec![
                    Span::styled(" Status: ", Style::default().fg(Color::Yellow)),
                    Span::styled(status_content, Style::default().fg(Color::Yellow)),
                ]));

                let overhead = 2 + 5;
                let bar_width = area.width.saturating_sub(overhead as u16 + 2) as usize;
                
                if app.loading_progress > 0.0 && app.loading_progress <= 1.0 {
                    let filled = (app.loading_progress * bar_width as f32) as usize;
                    let bar = format!(
                        "[{}{}] {:.0}%",
                        "█".repeat(filled),
                        "░".repeat(bar_width.saturating_sub(filled)),
                        app.loading_progress * 100.0
                    );
                    lines.push(Line::from(vec![
                        Span::styled(bar, Style::default().fg(Color::Yellow)),
                    ]));
                } else {
                    // Show empty progress bar with spinner
                    let bar = format!(
                        "[{}] 0%",
                        "░".repeat(bar_width)
                    );
                    lines.push(Line::from(vec![
                        Span::styled(bar, Style::default().fg(Color::DarkGray)),
                    ]));
                }

                let mut detail_parts = Vec::new();
                if let (Some(loaded), Some(total)) = (app.load_progress.layers_loaded, app.load_progress.layers_total) {
                    detail_parts.push(format!("({}/{})", loaded, total));
                }
                if app.load_progress.tensors_loaded > 0 {
                    detail_parts.push(format!("{} tensors", app.load_progress.tensors_loaded));
                }
                let total_gpu: f64 = app.load_progress.buffers.iter()
                    .filter(|b| b.device != "CPU_Mapped" && b.device != "CPU_Cached")
                    .map(|b| b.buffer_size_mib)
                    .sum();
                if total_gpu > 0.0 {
                    detail_parts.push(format!("{} VRAM", format_size((total_gpu * 1024.0 * 1024.0) as u64)));
                }

                let phase = app.loading_phases.iter().next().map(|p| p.label()).unwrap_or("Loading...");
                let detail = detail_parts.join(", ");
                lines.push(Line::from(vec![
                    Span::styled("  ", Style::default()),
                    Span::styled(phase, Style::default().fg(Color::Cyan)),
                    Span::raw(" "),
                    Span::styled(detail, Style::default().fg(Color::Magenta)),
                ]));
            }
            Some(ModelState::Available) => {
                let m = model.unwrap();
                lines.push(Line::from(vec![
                    Span::styled(" Model:  ", Style::default().fg(Color::Yellow)),
                    Span::styled(strip_gguf(&m.name), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                ]));
                lines.push(Line::from("Model not loaded."));
            }
            _ => {
                if app.server_handle.is_some() {
                     lines.push(Line::from(vec![
                        Span::styled(" Model:  ", Style::default().fg(Color::Yellow)),
                        Span::styled("llama-server", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                        Span::styled(" (no active model selected)", Style::default().fg(Color::DarkGray)),
                    ]));
                } else {
                    lines.push(Line::from(vec![
                        Span::styled(" No active model ", Style::default().fg(Color::DarkGray)),
                    ]));
                    lines.push(Line::from(vec![
                        Span::styled(" (select a model and press Enter to load)", Style::default().fg(Color::DarkGray)),
                    ]));
                }
            }
        }
    }

    let paragraph = Paragraph::new(lines).block(block);
    f.render_widget(paragraph, area);
}
