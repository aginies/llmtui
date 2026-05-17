use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Row, Table, TableState, List, ListItem, ListState},
};

use crate::tui::app::{App, ModelsMode};
use crate::tui::format_size;
use crate::models::SearchSort;

fn format_number(n: u64) -> String {
    if n >= 1_000_000_000 {
        format!("{:.1}B", n as f64 / 1_000_000_000.0)
    } else if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        format!("{}", n)
    }
}

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
        format!("Downloading ({}) ", total_speed_str)
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
            let progress = d.formatted_progress();
            let speed = d.formatted_speed();
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
                Cell::from(progress),
                Cell::from(speed),
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
    if bytes_per_second < 1024.0 {
        format!("{:.0} B/s", bytes_per_second)
    } else if bytes_per_second < 1024.0 * 1024.0 {
        format!("{:.1} KB/s", bytes_per_second / 1024.0)
    } else {
        format!("{:.1} MB/s", bytes_per_second / (1024.0 * 1024.0))
    }
}

pub fn render(f: &mut Frame, area: Rect, app: &mut App) {
    let (block, items) = match &app.models_mode {
        ModelsMode::List => {
            let title = if app.is_panel_visible(0) {
                format!(" Models (F1) ")
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

            let list_items: Vec<ListItem> = app
                .models
                .iter()
                .enumerate()
                .map(|(i, model)| {
                    let is_loaded = app.is_model_loaded(&model.display_name);
                    let is_loading = matches!(app.model_states.get(&model.display_name), Some(crate::models::ModelState::Loading));
                    let is_selected = Some(i) == app.selected_model_idx;

                    let selector = if is_selected { "> " } else { "  " };
                    let status = if is_loaded { 
                        "[loaded] " 
                    } else if is_loading {
                        "[loading] "
                    } else { 
                        "" 
                    };

                    let name_style = if is_selected {
                        Style::default().fg(Color::Black).bg(Color::Green).add_modifier(Modifier::BOLD)
                    } else if is_loaded {
                        Style::default().fg(Color::Green)
                    } else if is_loading {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default().fg(Color::White)
                    };

                    ListItem::new(Line::from(vec![
                        Span::styled(selector, Style::default().fg(Color::Yellow)),
                        Span::styled(status, Style::default().fg(Color::Yellow)),
                        Span::styled(&model.display_name, name_style),
                    ]))
                })
                .collect();

            (block, list_items)
        }
        ModelsMode::Search { query, results, sort_by, loading, has_more, .. } => {
            let sort_label = sort_by.label();
            let title = if app.is_panel_visible(0) {
                format!(" Search (F1): {} [{}]", query, sort_label)
            } else {
                format!(" Search: {} [{}]", query, sort_label)
            };
            let block = Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Magenta));

            let headers = vec![
                Cell::from("Model").style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Cell::from(if *sort_by == SearchSort::Downloads { "⬇" } else { "Dl" }).style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Cell::from(if *sort_by == SearchSort::Likes { "♥" } else { "Lk" }).style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Cell::from("License").style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            ];

            let mut rows: Vec<Row> = results
                .iter()
                .enumerate()
                .map(|(i, result)| {
                    let is_selected = Some(i) == app.search_results_idx;

                    let row_style = if is_selected {
                        Style::default().bg(Color::Green).add_modifier(Modifier::REVERSED)
                    } else {
                        Style::default()
                    };

                    let license = result.license.as_deref().unwrap_or("—");

                    Row::new(vec![
                        Cell::from(result.model_id.as_str()),
                        Cell::from(format_number(result.downloads)),
                        Cell::from(format_number(result.likes)),
                        Cell::from(license),
                    ]).style(row_style)
                })
                .collect();

            // Add loading indicator row
            if *loading {
                rows.push(Row::new(vec![
                    Cell::from(""),
                    Cell::from(""),
                    Cell::from(""),
                    Cell::from("  Loading more results...").style(Style::default().fg(Color::Yellow)),
                ]));
            } else if !has_more && !results.is_empty() {
                rows.push(Row::new(vec![
                    Cell::from(""),
                    Cell::from(""),
                    Cell::from(""),
                    Cell::from("  No more results").style(Style::default().fg(Color::DarkGray)),
                ]));
            }

            let widths = [
                Constraint::Fill(1),
                Constraint::Length(8),
                Constraint::Length(5),
                Constraint::Length(11),
            ];

            let table = Table::new(rows, widths).header(Row::new(headers)).block(block);
            app.search_table_state.select(app.search_results_idx);
            return f.render_stateful_widget(table, area, &mut app.search_table_state);
        }
        ModelsMode::Files { model_id, files, selected_idx, selected_result: _, .. } => {
            let title = format!(" {} - GGUF files ", model_id);
            let block = Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Magenta));

            let inner_area = block.inner(area);
            f.render_widget(block, area);

            // Render Files Table
            let rows: Vec<Row> = files
                .iter()
                .enumerate()
                .map(|(i, (filename, size, _url))| {
                    let is_selected = Some(i) == *selected_idx;
                    let name = filename.rsplit('/').next().unwrap_or(filename);

                    let row_style = if is_selected {
                        Style::default().bg(Color::Green).add_modifier(Modifier::REVERSED)
                    } else {
                        Style::default()
                    };

                    Row::new(vec![
                        Cell::from(name),
                        Cell::from(format_size(*size)),
                    ]).style(row_style)
                })
                .collect();

            let headers = vec![
                Cell::from("File").style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Cell::from("Size").style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            ];

            let widths = [
                Constraint::Fill(1),
                Constraint::Length(12),
            ];

            let table = Table::new(rows, widths).header(Row::new(headers));
            app.files_table_state.select(*selected_idx);

            f.render_stateful_widget(table, inner_area, &mut app.files_table_state);
            return;
        }
    };

    let mut list_state = ListState::default();
    list_state.select(match &app.models_mode {
        ModelsMode::Files { selected_idx, .. } => *selected_idx,
        _ => app.selected_model_idx,
    });

    let list = List::new(items).block(block);
    f.render_stateful_widget(list, area, &mut list_state);
}


