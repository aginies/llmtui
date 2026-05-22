use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Row, Table, TableState, List, ListItem, ListState},
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
                crate::models::DownloadStatus::Complete => "Complete",
                crate::models::DownloadStatus::Error(e) => e.as_str(),
            };

            let status_color = match &d.status {
                crate::models::DownloadStatus::Downloading => Color::Yellow,
                crate::models::DownloadStatus::Complete => Color::Green,
                crate::models::DownloadStatus::Error(_) => Color::Red,
            };

            Row::new(vec![
                Cell::from(d.model_id.as_str()),
                Cell::from(d.filename.as_str()),
                Cell::from(progress_str),
                Cell::from(speed_str),
                Cell::from(status).style(Style::default().fg(status_color)),
            ])
        })
        .collect();

    let headers = vec![
        Cell::from("Model").style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Cell::from("File").style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Cell::from("Progress").style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Cell::from("Speed").style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Cell::from("Status").style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
    ];

    let widths = [
        Constraint::Length(25),
        Constraint::Length(30),
        Constraint::Length(8),
        Constraint::Length(10),
        Constraint::Length(14),
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

            let mut rows: Vec<Row> = results
                .iter()
                .map(|result| {
                    let license = result.license.as_deref().unwrap_or("—");

                    Row::new(vec![
                        Cell::from(result.model_id.clone()),
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
    }
}


