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
    let block = Block::default()
        .title(" Active Model(s) (F5) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if app.active_panel == crate::tui::app::ActivePanel::ActiveModel { Color::Green } else { Color::DarkGray }));

    let mut lines = Vec::new();

    // Get currently loaded model or the one being loaded
    let model = app.selected_model();
    let state = model.and_then(|m| app.model_states.get(&m.display_name));

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
                Span::styled("  [ ", Style::default().fg(Color::White)),
                Span::styled("TPS: ", Style::default().fg(Color::Yellow)),
                Span::styled(format!("{:.1}", app.metrics.tps), Style::default().fg(Color::Green)),
                Span::styled(" (in: ", Style::default().fg(Color::DarkGray)),
                Span::styled(format!("{:.1}", app.metrics.prompt_tps), Style::default().fg(Color::Green)),
                Span::styled(")", Style::default().fg(Color::DarkGray)),
                Span::styled(" ]", Style::default().fg(Color::White)),
            ]));

            lines.push(Line::from(vec![
                Span::styled(" Context: ", Style::default().fg(Color::Yellow)),
                Span::styled(bar_only, Style::default().fg(Color::Cyan)),
                Span::styled(" ", Style::default().fg(Color::Cyan)),
                Span::styled(token_str, Style::default().fg(Color::Cyan)),
                Span::styled("  [ ", Style::default().fg(Color::White)),
                Span::styled("CPU: ", Style::default().fg(Color::Yellow)),
                Span::styled(format!("{:.1}%", app.metrics.cpu_usage), Style::default().fg(Color::Cyan)),
                Span::styled(" ]  [ ", Style::default().fg(Color::White)),
                Span::styled("RAM: ", Style::default().fg(Color::Yellow)),
                Span::styled(format_size(app.metrics.ram_used), Style::default().fg(Color::Cyan)),
                Span::styled(" ]  [ ", Style::default().fg(Color::White)),
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
                Span::styled(" (see log for output)", Style::default().fg(Color::DarkGray)),
            ]));
        }
        Some(ModelState::Loading) => {
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
                    detail_parts.push(format!("{} VRAM", format_size((total_gpu * 1024.0 * 1024.0) as u64)));
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

    let paragraph = Paragraph::new(lines).block(block);
    f.render_widget(paragraph, area);
}
