use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::tui::app::App;

fn strip_gguf(name: &str) -> &str {
    name.strip_suffix(".gguf").unwrap_or(name)
}

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let mut title_spans = if app.is_panel_visible(4) {
        vec![Span::raw(" Active Model (F5) ")]
    } else {
        vec![Span::raw(" Active Model(s) ")]
    };
    if app.metrics.total_vram_used > 0 {
        title_spans.push(Span::styled("[ ", Style::default().fg(Color::White)));
        title_spans.push(Span::styled("Total VRAM: ", Style::default().fg(Color::Yellow)));
        title_spans.push(Span::styled(format_mem(app.metrics.total_vram_used), Style::default().fg(Color::Cyan)));
        title_spans.push(Span::styled(" / ", Style::default().fg(Color::White)));
        title_spans.push(Span::styled(format_mem(app.metrics.gpu_mem_total), Style::default().fg(Color::Cyan)));
        title_spans.push(Span::styled(" ]", Style::default().fg(Color::White)));
    }

    let block = Block::default()
        .title(Line::from(title_spans))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));

    let model = app.selected_model();
    let status = model.and_then(|m| app.model_states.get(&m.display_name));
    
    let mut lines = Vec::new();

    match status {
        Some(crate::models::ModelState::Loaded { .. }) => {
            let m = model.unwrap();
            lines.push(Line::from(vec![
                Span::styled(" Model:  ", Style::default().fg(Color::Yellow)),
                         Span::styled(strip_gguf(&m.name), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                Span::raw("  "),
                Span::styled("✓", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            ]));

            // Metrics row 1: Performance and Context
            let pct = if app.metrics.ctx_max > 0 {
                (app.metrics.ctx_used as f64 / app.metrics.ctx_max as f64 * 100.0).ceil() as usize
            } else {
                0
            };
            let bar_width = 20usize;
            let filled = (pct as f64 / 100.0 * bar_width as f64) as usize;
            let bar_only = format!(
                "{}{}{}",
                "█".repeat(filled.saturating_sub(1)),
                "█",
                "░".repeat(bar_width.saturating_sub(filled)),
            );
            let token_str = format!("{}/{} ({:.0}%)", app.metrics.ctx_used, app.metrics.ctx_max, pct as f64 / 100.0 * 100.0);
            lines.push(Line::from(vec![
                Span::styled(" [ ", Style::default().fg(Color::White)),
                Span::styled("TPS: ", Style::default().fg(Color::Yellow)),
                Span::styled(format!("{:.1}", app.metrics.tps), Style::default().fg(Color::Green)),
                Span::styled(" (in: ", Style::default().fg(Color::DarkGray)),
                Span::styled(format!("{:.1}", app.metrics.prompt_tps), Style::default().fg(Color::Green)),
                Span::styled(")", Style::default().fg(Color::DarkGray)),
                Span::styled(" ]  [ ", Style::default().fg(Color::White)),
                Span::styled(bar_only, Style::default().fg(Color::Cyan)),
                Span::styled(" ", Style::default().fg(Color::Cyan)),
                Span::styled("tokens", Style::default().fg(Color::Cyan)),
                Span::styled(" ", Style::default().fg(Color::Cyan)),
                Span::styled(token_str, Style::default().fg(Color::Cyan)),
                Span::styled(" ]", Style::default().fg(Color::White)),
            ]));

            // Metrics row 2: System + VRAM
            lines.push(Line::from(vec![
                Span::styled(" [ ", Style::default().fg(Color::White)),
                Span::styled("CPU: ", Style::default().fg(Color::Yellow)),
                Span::styled(format!("{:.1}%", app.metrics.cpu_usage), Style::default().fg(Color::Cyan)),
                Span::styled(" ]  [ ", Style::default().fg(Color::White)),
                Span::styled("RAM: ", Style::default().fg(Color::Yellow)),
                Span::styled(format_mem(app.metrics.ram_used), Style::default().fg(Color::Cyan)),
                Span::styled(" ]  [ ", Style::default().fg(Color::White)),
                Span::styled("VRAM: ", Style::default().fg(Color::Yellow)),
                Span::styled(format_mem(app.metrics.gpu_mem_used), Style::default().fg(Color::Cyan)),
                Span::styled(" / ", Style::default().fg(Color::White)),
                Span::styled(format_mem(app.metrics.gpu_mem_total), Style::default().fg(Color::Cyan)),
                Span::styled(" ]", Style::default().fg(Color::White)),
            ]));
        }
        Some(crate::models::ModelState::Loading) => {
            let m = model.unwrap();
            lines.push(Line::from(vec![
                Span::styled(" Model:  ", Style::default().fg(Color::Yellow)),
                Span::styled(strip_gguf(&m.name), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            ]));
            lines.push(Line::from(vec![
                Span::styled(" Status: ", Style::default().fg(Color::Yellow)),
                Span::styled("LOADING", Style::default().fg(Color::Yellow)),
            ]));

            // Show loading progress with details
            if app.loading_progress > 0.0 && app.loading_progress < 1.0 {
                let bar_width = area.width.saturating_sub(10) as usize;
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

                // Build detail line from available data
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
                    detail_parts.push(format!("{} VRAM", format_mem((total_gpu * 1024.0 * 1024.0) as u64)));
                }

                let phase = app.loading_phases.last().map(|p| p.label()).unwrap_or("Loading...");
                if detail_parts.is_empty() {
                    lines.push(Line::from(vec![
                        Span::styled("  ", Style::default()),
                        Span::styled(phase, Style::default().fg(Color::Cyan)),
                    ]));
                } else {
                    let detail = detail_parts.join(", ");
                    lines.push(Line::from(vec![
                        Span::styled("  ", Style::default()),
                        Span::styled(phase, Style::default().fg(Color::Cyan)),
                        Span::raw(" "),
                        Span::styled(detail, Style::default().fg(Color::Magenta)),
                    ]));
                }
            }
        }
        Some(crate::models::ModelState::Failed { error }) => {
            let m = model.unwrap();
            lines.push(Line::from(vec![
               Span::styled(" Model:  ", Style::default().fg(Color::Yellow)),
                        Span::styled(strip_gguf(&m.name), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                    ]));
                    lines.push(Line::from(vec![
                        Span::styled(" Status: ", Style::default().fg(Color::Yellow)),
                        Span::styled(error, Style::default().fg(Color::Red)),
            ]));
        }
        _ => {
            // Only show the global last_error_message if it's a Router/Server crash 
            // and no model is selected or the selected model isn't loaded.
            if let Some(error) = &app.last_error_message {
                if error.contains("Router Crash") {
                    lines.push(Line::from(vec![
                        Span::styled(" Status: ", Style::default().fg(Color::Yellow)),
                        Span::styled(error, Style::default().fg(Color::Red)),
                    ]));
                } else {
                    lines.push(Line::from(vec![
                        Span::styled(" (no active metrics for selected model)", Style::default().fg(Color::DarkGray)),
                    ]));
                }
            } else {
                lines.push(Line::from(vec![
                    Span::styled(" (no active metrics for selected model)", Style::default().fg(Color::DarkGray)),
                ]));
            }
        }
    }

    let paragraph = Paragraph::new(lines).block(block);
    f.render_widget(paragraph, area);
}

fn format_mem(bytes: u64) -> String {
    if bytes == 0 {
        "0 B".to_string()
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}
