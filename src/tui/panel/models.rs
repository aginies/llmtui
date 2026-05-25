use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Row, Table, TableState, List, ListItem, ListState, Paragraph},
};

use crate::tui::app::{App, ModelsMode};
use crate::tui::{format_size, format_number};
use crate::models::SearchSort;

pub fn render_download_panel(
    f: &mut Frame,
    area: Rect,
    downloads: &[crate::models::DownloadState],
    total_speed: f64,
    scroll_state: &mut TableState,
    is_focused: bool,
) {
    if downloads.is_empty() {
        return;
    }

    let total_speed_str = format_speed(total_speed);
    let count = downloads.len();
    let title = if count == 1 {
        format!(" Downloading ({}) ", total_speed_str)
    } else {
        format!(" {} Downloads ({}) ", count, total_speed_str)
    };
    
    let border_color = if is_focused { Color::Green } else { Color::Yellow };
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    // Show all active downloads in a table
    let rows: Vec<Row> = downloads
        .iter()
        .map(|d| {
            // Re-implement basic progress formatting locally since I removed them from models.rs
            let progress_pct = if d.total_bytes == 0 { 0.0 } else { d.downloaded_bytes as f64 / d.total_bytes as f64 * 100.0 };
            let progress_str = format!("{:.1}%", progress_pct);
            let speed_str = format_speed(d.bytes_per_second);
            
            let status = match &d.status {
                crate::models::DownloadStatus::Downloading => "Downloading...",
                crate::models::DownloadStatus::Paused => "Paused",
                crate::models::DownloadStatus::Complete => "Complete",
                crate::models::DownloadStatus::Cancelled => "Cancelled",
                crate::models::DownloadStatus::Error(e) => e.as_str(),
            };

            let status_color = match &d.status {
                crate::models::DownloadStatus::Downloading => Color::Yellow,
                crate::models::DownloadStatus::Paused => Color::White,
                crate::models::DownloadStatus::Complete => Color::Green,
                crate::models::DownloadStatus::Cancelled => Color::Red,
                crate::models::DownloadStatus::Error(_) => Color::Red,
            };

            Row::new(vec![
                Cell::from(d.model_id.as_str()),
                Cell::from(d.filename.as_str()),
                Cell::from(progress_str),
                Cell::from(speed_str),
                Cell::from(format_eta(d)),
                Cell::from(status).style(Style::default().fg(status_color)),
            ])
        })
        .collect();

    let headers = vec![
        Cell::from("Model").style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Cell::from("File").style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Cell::from("Progress").style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Cell::from("Speed").style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Cell::from("ETA").style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Cell::from("Status").style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
    ];

    let widths = [
        Constraint::Ratio(3, 10),
        Constraint::Ratio(3, 10),
        Constraint::Ratio(8, 100),
        Constraint::Ratio(1, 10),
        Constraint::Ratio(3, 25),
        Constraint::Ratio(1, 10),
    ];

    let table = Table::new(rows, widths)
        .header(Row::new(headers))
        .block(block)
        .row_highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
        .highlight_symbol(">> ");

    f.render_stateful_widget(table, area, scroll_state);
}

fn format_speed(bytes_per_second: f64) -> String {
    format!("{}/s", crate::tui::format_size(bytes_per_second as u64))
}

fn format_eta(d: &crate::models::DownloadState) -> String {
    if d.total_bytes == 0 || d.downloaded_bytes >= d.total_bytes {
        return "—".to_string();
    }

    let remaining = (d.total_bytes as f64 - d.downloaded_bytes as f64) as u64;
    if d.bytes_per_second > 0.0 {
        let secs = remaining as f64 / d.bytes_per_second;
        format_time_remaining(secs as u64)
    } else {
        "calculating...".to_string()
    }
}

/// Highlight occurrences of each word in `query` within `text` (case-insensitive).
fn highlight_query<'a>(text: &'a str, query: &str) -> Line<'a> {
    let words: Vec<String> = query.trim().split_whitespace().map(|w| w.to_lowercase()).collect();
    if words.is_empty() || text.is_empty() {
        return Line::from(text);
    }
    let lower_text = text.to_lowercase();
    // Build a set of character positions that match any query word
    let mut highlight = vec![false; text.len()];
    for word in &words {
        let mut pos = 0;
        while let Some(idx) = lower_text[pos..].find(word) {
            let start = pos + idx;
            let end = start + word.len();
            for i in start..end {
                highlight[i] = true;
            }
            pos = end;
        }
    }
    let mut spans = Vec::new();
    let mut start = 0;
    let mut in_highlight = highlight[0];
    for i in 1..=text.len() {
        if i == text.len() || highlight[i] != in_highlight {
            let style = if in_highlight {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            spans.push(Span::styled(&text[start..i], style));
            start = i;
            if i < text.len() {
                in_highlight = highlight[i];
            }
        }
    }
    Line::from(spans)
}

fn format_time_remaining(total_secs: u64) -> String {
    let days = total_secs / 86400;
    let hours = (total_secs % 86400) / 3600;
    let minutes = (total_secs % 3600) / 60;
    let secs = total_secs % 60;

    if days > 0 {
        format!("{days}d {hours}h {minutes}m")
    } else if hours > 0 {
        format!("{hours}h {minutes}m {secs}s")
    } else if minutes > 0 {
        format!("{minutes}m {secs}s")
    } else {
        format!("{secs}s")
    }
}

pub fn render(f: &mut Frame, area: Rect, app: &mut App) {
    match &app.models_mode {
        ModelsMode::List => {
            let title = if app.is_panel_visible(0) {
                format!(" Models [1] ")
            } else {
                format!(" Models ")
            };

            let border_color = if app.active_panel == crate::tui::app::ActivePanel::Models {
                Color::Green
            } else {
                Color::Yellow
            };
            let block = Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color));

            let inner_area = block.inner(area);
            f.render_widget(block, area);

            let (list_area, filter_area) = if app.filtering_local || !app.local_filter.is_empty() {
                let chunks = ratatui::layout::Layout::default()
                    .direction(ratatui::layout::Direction::Vertical)
                    .constraints([
                        Constraint::Length(1),
                        Constraint::Fill(1),
                    ])
                    .split(inner_area);
                (chunks[1], Some(chunks[0]))
            } else {
                (inner_area, None)
            };

            if let Some(fa) = filter_area {
                let filter_text = if app.filtering_local {
                    Line::from(vec![
                        Span::styled(" Filter: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                        Span::styled(&app.local_filter, Style::default().fg(Color::Black).bg(Color::Yellow)),
                        Span::styled("_", Style::default().fg(Color::Black).bg(Color::Yellow)),
                    ])
                } else {
                    Line::from(vec![
                        Span::styled(" Filter: ", Style::default().fg(Color::DarkGray)),
                        Span::styled(&app.local_filter, Style::default().fg(Color::Cyan)),
                    ])
                };
                f.render_widget(ratatui::widgets::Paragraph::new(filter_text), fa);
            }

            let filtered_indices = app.get_filtered_model_indices();
            let list_items: Vec<ListItem> = filtered_indices
                .iter()
                .map(|&idx| {
                    let model = &app.models[idx];
                    let model_state = app.model_states.get(&model.display_name);
                    let is_selected = Some(idx) == app.selected_model_idx;

                    let selector = if is_selected { "> " } else { "  " };
                    let status = match model_state {
                        Some(crate::models::ModelState::Loaded { .. }) => "[loaded] ",
                        Some(crate::models::ModelState::Loading) => "[loading] ",
                        Some(crate::models::ModelState::Benchmarking) => "[benchmarking] ",
                        _ => "",
                    };

                    let name_style = if is_selected {
                        Style::default().fg(Color::Black).bg(Color::Green).add_modifier(Modifier::BOLD)
                    } else {
                        match model_state {
                            Some(crate::models::ModelState::Loaded { .. }) => Style::default().fg(Color::Green),
                            Some(crate::models::ModelState::Loading) | Some(crate::models::ModelState::Benchmarking) => Style::default().fg(Color::Yellow),
                            _ => Style::default().fg(Color::White),
                        }
                    };

                    ListItem::new(Line::from(vec![
                        Span::styled(selector, Style::default().fg(Color::Yellow)),
                        Span::styled(status, Style::default().fg(Color::Yellow)),
                        Span::styled(&model.display_name, name_style),
                    ]))
                })
                .collect();

            let mut list_state = ListState::default();
            if let Some(idx) = app.selected_model_idx {
                if let Some(pos) = filtered_indices.iter().position(|&i| i == idx) {
                    list_state.select(Some(pos));
                }
            }

            let list = List::new(list_items);
            f.render_stateful_widget(list, list_area, &mut list_state);
        }
        ModelsMode::Search { query, results, sort_by, loading, has_more, .. } => {
            let sort_label = sort_by.label();
            let title = if app.is_panel_visible(0) {
                format!(" Search [1]: {} [{}] ({} results)", query, sort_label, results.len())
            } else {
                format!(" Search: {} [{}] ({} results)", query, sort_label, results.len())
            };
            let border_color = if app.active_panel == crate::tui::app::ActivePanel::Models {
                Color::Green
            } else {
                Color::Magenta
            };
            let block = Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color));

            let headers = vec![
                Cell::from("Model").style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Cell::from(if *sort_by == SearchSort::Downloads { "⬇" } else { "Dl" }).style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Cell::from(if *sort_by == SearchSort::Likes { "♥" } else { "Lk" }).style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Cell::from("License").style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            ];

            let downloaded_names: Vec<String> = app.models.iter()
                .map(|m| m.name.to_lowercase())
                .collect();

            let mut rows: Vec<Row> = results
                .iter()
                .map(|result| {
                    let license = result.license.as_deref().unwrap_or("—");
                    let model_basename = result.model_id.rsplit('/').next().unwrap_or("").to_lowercase();
                    let is_downloaded = downloaded_names.iter().any(|n| n.starts_with(&model_basename));

                    let marker = if is_downloaded { "✓" } else { " " };
                    let marker_span = Span::styled(format!("[{}] ", marker), Style::default().fg(Color::Green));
                    let highlighted = highlight_query(&result.model_id, query);
                    let mut model_spans: Vec<Span> = vec![marker_span];
                    model_spans.extend(highlighted.spans.iter().cloned());

                    Row::new(vec![
                        Cell::from(Line::from(model_spans)),
                        Cell::from(format_number(result.downloads)),
                        Cell::from(format_number(result.likes)),
                        Cell::from(license.to_string()),
                    ])
                })
                .collect();

            // Add informational rows
            if *loading {
                rows.push(Row::new(vec![
                    Cell::from("Loading more results...").style(Style::default().fg(Color::Yellow)),
                    Cell::from(""),
                    Cell::from(""),
                    Cell::from(""),
                ]));
            } else if results.is_empty() {
                rows.push(Row::new(vec![
                    Cell::from("No results found for this query.").style(Style::default().fg(Color::Red)),
                    Cell::from(""),
                    Cell::from(""),
                    Cell::from(""),
                ]));
            } else if !has_more {
                rows.push(Row::new(vec![
                    Cell::from("No more results").style(Style::default().fg(Color::DarkGray)),
                    Cell::from(""),
                    Cell::from(""),
                    Cell::from(""),
                ]));
            }

            let widths = [
                Constraint::Percentage(60),
                Constraint::Percentage(15),
                Constraint::Percentage(10),
                Constraint::Percentage(15),
            ];

            let table = Table::new(rows, widths)
                .header(Row::new(headers))
                .block(block)
                .row_highlight_style(Style::default().fg(Color::Black).bg(Color::Green).add_modifier(Modifier::BOLD))
                .highlight_symbol("> ");

            app.search_table_state.select(app.search_results_idx);
            f.render_stateful_widget(table, area, &mut app.search_table_state);
        }
        ModelsMode::Files { model_id, files, selected_idx, selected_result: _, .. } => {
            let title = format!(" {} - GGUF files ", model_id);
            let border_color = if app.active_panel == crate::tui::app::ActivePanel::Models {
                Color::Green
            } else {
                Color::Magenta
            };
            let block = Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color));

            let inner_area = block.inner(area);
            f.render_widget(block, area);

            // Render Files Table
            let rows: Vec<Row> = files
                .iter()
                .map(|(filename, size, _url)| {
                    let name = filename.rsplit('/').next().unwrap_or(filename);
                    Row::new(vec![
                        Cell::from(name.to_string()),
                        Cell::from(format_size(*size)),
                    ])
                })
                .collect();

            let headers = vec![
                Cell::from("File").style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Cell::from("Size").style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            ];

            let widths = [
                Constraint::Percentage(80),
                Constraint::Percentage(20),
            ];

            let table = Table::new(rows, widths)
                .header(Row::new(headers))
                .row_highlight_style(Style::default().fg(Color::Black).bg(Color::Green).add_modifier(Modifier::BOLD))
                .highlight_symbol("> ");

            app.files_table_state.select(*selected_idx);

            f.render_stateful_widget(table, inner_area, &mut app.files_table_state);
        }
        ModelsMode::BenchTune => {
            let title = " BenchTune ".to_string();
            let border_color = if app.active_panel == crate::tui::app::ActivePanel::Models {
                Color::Green
            } else {
                Color::Yellow
            };
            let block = Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color));

            let inner_area = block.inner(area);
            f.render_widget(block, area);

            // Show bench_tune progress or results
            let mut lines: Vec<Line> = Vec::new();

            if let Some(progress) = &app.bench_tune_progress {
                match progress {
                    crate::models::BenchTuneProgress::Running { current, total, progress: p, current_params } => {
                        lines.push(Line::from(format!("Progress: {}/{} ({:.0}%)", current, total, p)));
                        lines.push(Line::from(""));
                        lines.push(Line::from(Span::styled("Current parameters:", Style::default().add_modifier(Modifier::BOLD))));
                        
                        let p_parts = crate::tui::format_bench_params(current_params, true);

                        if p_parts.is_empty() {
                            lines.push(Line::from("  (Baseline)"));
                        } else {
                            for part in p_parts {
                                lines.push(Line::from(part));
                            }
                        }
                    }
                    crate::models::BenchTuneProgress::Completed { total_tests, successful_tests, elapsed } => {
                        let elapsed_str = format!("{}s", elapsed.as_secs());
                        lines.push(Line::from(vec![
                            Span::raw("Status: "),
                            Span::styled("COMPLETED", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                            Span::raw(format!(" ({} tests in {})", total_tests, elapsed_str)),
                        ]));
                        
                        let success_style = if *successful_tests == *total_tests {
                            Style::default().fg(Color::Green)
                        } else if *successful_tests > 0 {
                            Style::default().fg(Color::Yellow)
                        } else {
                            Style::default().fg(Color::Red)
                        };
                        
                        lines.push(Line::from(vec![
                            Span::raw("Success: "),
                            Span::styled(format!("{}/{}", successful_tests, total_tests), success_style.add_modifier(Modifier::BOLD)),
                        ]));

                        if *successful_tests < *total_tests {
                            lines.push(Line::from(Span::styled(
                                format!("Warning: {} test(s) failed. Check Log (F6) for details.", total_tests - successful_tests),
                                Style::default().fg(Color::Red)
                            )));
                        }
                        
                        if !app.bench_tune_results.is_empty() {
                            lines.push(Line::from(""));
                            lines.push(Line::from(Span::styled(" Benchmark results (sorted by generation speed):", Style::default().add_modifier(Modifier::BOLD))));
                            lines.push(Line::from(Span::styled(" (Press [Enter] to view details of selected result)", Style::default().fg(Color::DarkGray))));
                            lines.push(Line::from(""));
                            
                            use ratatui::widgets::{Row, Cell};
                            
                            let header = Row::new(vec![
                                Cell::from(" # "),
                                Cell::from("Gen t/s"),
                                Cell::from("Inf t/s"),
                                Cell::from("Params"),
                            ]).style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));

                            let mut rows = Vec::new();
                            for (i, result) in app.bench_tune_results.iter().enumerate() {
                                let p_str = crate::tui::format_bench_params(&result.params, false).join(",");
                                
                                let mut style = Style::default().fg(Color::White);
                                if i == 0 {
                                    style = style.fg(Color::Green);
                                }
                                
                                rows.push(Row::new(vec![
                                    Cell::from(format!(" {:<2} ", i + 1)),
                                    Cell::from(format!("{:.2}", result.metrics.generation_tps)),
                                    Cell::from(format!("{:.2}", result.metrics.prompt_tps)),
                                    Cell::from(p_str),
                                ]).style(style));
                            }

                            // Use TableState for scrolling and selection
                            let mut state = ratatui::widgets::TableState::default();
                            state.select(Some(app.bench_tune_result_row));

                            let table = ratatui::widgets::Table::new(rows, [
                                ratatui::layout::Constraint::Length(4),
                                ratatui::layout::Constraint::Length(10),
                                ratatui::layout::Constraint::Length(10),
                                ratatui::layout::Constraint::Fill(1),
                            ])
                            .header(header)
                            .block(Block::default().borders(Borders::NONE))
                            .row_highlight_style(Style::default().bg(Color::Rgb(60, 60, 60)).add_modifier(Modifier::BOLD))
                            .highlight_symbol("> ");
                            
                            let header_height = lines.len() as u16;
                            let table_area = Rect {
                                x: inner_area.x,
                                y: inner_area.y + header_height,
                                width: inner_area.width,
                                height: inner_area.height.saturating_sub(header_height),
                            };
                            
                            f.render_widget(Paragraph::new(lines.clone()), inner_area);
                            f.render_stateful_widget(table, table_area, &mut state);
                            return;
                        }
                    }
                    crate::models::BenchTuneProgress::Error { error } => {
                        lines.push(Line::from(format!("Error: {}", error)));
                    }
                }
            } else {
                lines.push(Line::from("Benchmark tuning not started."));
            }

            let paragraph = ratatui::widgets::Paragraph::new(lines).block(Block::default());
            f.render_widget(paragraph, inner_area);
        }
    }
}


