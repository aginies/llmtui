use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Row, Table, TableState, List, ListItem, ListState, Paragraph},
};

use crate::tui::app::{App, ModelsMode};
use crate::models::SearchSort;

fn format_size(size: u64) -> String {
    let kb = 1024.0;
    let mb = kb * 1024.0;
    let gb = mb * 1024.0;
    let tb = gb * 1024.0;

    let s = size as f64;
    if s < kb {
        format!("{} B", size)
    } else if s < mb {
        format!("{:.1} KB", s / kb)
    } else if s < gb {
        format!("{:.1} MB", s / mb)
    } else if s < tb {
        format!("{:.1} GB", s / gb)
    } else {
        format!("{:.1} TB", s / tb)
    }
}

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

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    // Version picker mode
    if let ModelsMode::VersionPicker { releases, selected_idx, .. } = &app.models_mode {
        render_version_picker(
            f,
            area,
            releases,
            *selected_idx,
            &app.cached_cpu_versions,
            &app.cached_vulkan_versions,
            &app.cached_rocm_versions,
            app.version_picker_show_cached,
            app.picker_backend,
            app.version_picker_scroll_offset,
        );
        return;
    }

    let (block, items) = match &app.models_mode {
        ModelsMode::List => {
            let title = format!(" Models ({} models) ", app.models.len());
            let border_color = if app.active_panel == crate::tui::app::ActivePanel::Models {
                Color::Green
            } else {
                Color::Rgb(255, 165, 0)
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
        ModelsMode::Search { query, results, sort_by, .. } => {
            let sort_label = sort_by.label();
            let title = format!(" Search: {} [{}]", query, sort_label);
            let block = Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Magenta));

            let headers = vec![
                Cell::from("Model").style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Cell::from(if *sort_by == SearchSort::Downloads { "⬇ Downloads" } else { "Downloads" }).style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Cell::from(if *sort_by == SearchSort::Likes { "♥ Likes" } else { "Likes" }).style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            ];

            let rows: Vec<Row> = results
                .iter()
                .enumerate()
                .map(|(i, result)| {
                    let is_selected = Some(i) == app.search_results_idx;

                    let row_style = if is_selected {
                        Style::default().bg(Color::Green).add_modifier(Modifier::REVERSED)
                    } else {
                        Style::default()
                    };

                    Row::new(vec![
                        Cell::from(result.model_id.as_str()),
                        Cell::from(format_number(result.downloads)),
                        Cell::from(format_number(result.likes)),
                    ]).style(row_style)
                })
                .collect();

            let widths = [
                Constraint::Fill(1),
                Constraint::Length(10),
                Constraint::Length(10),
            ];

            let table = Table::new(rows, widths).header(Row::new(headers)).block(block);
            let mut table_state = TableState::default();
            table_state.select(app.search_results_idx);
            return f.render_stateful_widget(table, area, &mut table_state);
        }
        ModelsMode::Files { model_id, files, selected_idx, selected_result, .. } => {
            let title = format!(" {} - GGUF files ", model_id);
            let block = Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Magenta));

            let inner_area = block.inner(area);
            f.render_widget(block, area);

            // Split inner area into metadata (top) and files (bottom)
            let chunks = ratatui::layout::Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .constraints([
                    Constraint::Length(4), // Metadata
                    Constraint::Min(0),    // Files
                ])
                .split(inner_area);

            // Render Metadata
            if let Some(result) = selected_result {
                let params_str = result.parameters.as_deref().unwrap_or("N/A");
                let cap_str = if result.capabilities.is_empty() {
                    "N/A".to_string()
                } else {
                    result.capabilities.join(", ")
                };

                let meta_items = vec![
                    Line::from(vec![
                        Span::styled("Model: ", Style::default().fg(Color::Yellow)),
                        Span::styled(model_id, Style::default().fg(Color::White)),
                    ]),
                    Line::from(vec![
                        Span::styled("Params: ", Style::default().fg(Color::Yellow)),
                        Span::styled(params_str, Style::default().fg(Color::White)),
                        Span::raw(" | "),
                        Span::styled("Capabilities: ", Style::default().fg(Color::Yellow)),
                        Span::styled(cap_str, Style::default().fg(Color::White)),
                    ]),
                    Line::from(vec![
                        Span::styled("Downloads: ", Style::default().fg(Color::Yellow)),
                        Span::styled(format_number(result.downloads), Style::default().fg(Color::White)),
                        Span::raw(" | "),
                        Span::styled("Likes: ", Style::default().fg(Color::Yellow)),
                        Span::styled(format_number(result.likes), Style::default().fg(Color::White)),
                    ]),
                ];
                f.render_widget(Paragraph::new(meta_items), chunks[0]);
            }

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
            let mut table_state = TableState::default();
            table_state.select(*selected_idx);

            let file_chunks = ratatui::layout::Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .constraints([
                    ratatui::layout::Constraint::Fill(1),
                    ratatui::layout::Constraint::Length(1),
                ])
                .split(chunks[1]);

            f.render_stateful_widget(table, file_chunks[0], &mut table_state);

            let hint = Line::from(Span::styled(
                "ENTER to download | UP | DOWN | ESC to back",
                Style::default().fg(Color::Cyan),
            ));
            f.render_widget(Paragraph::new(hint), file_chunks[1]);
            return;
        }
        _ => (
            Block::default().borders(Borders::ALL),
            Vec::new(),
        ),
    };

    let mut list_state = ListState::default();
    list_state.select(match &app.models_mode {
        ModelsMode::Files { selected_idx, .. } => *selected_idx,
        _ => app.selected_model_idx,
    });

    let list = List::new(items).block(block);
    f.render_stateful_widget(list, area, &mut list_state);
}

pub fn render_version_picker(
    f: &mut Frame,
    area: Rect,
    releases: &[crate::models::LlamaCppRelease],
    selected_idx: usize,
    cpu_versions: &[String],
    vulkan_versions: &[String],
    rocm_versions: &[String],
    show_cached: bool,
    picker_backend: crate::models::Backend,
    scroll_offset: u16,
) {
    let current_version = if selected_idx < releases.len() {
        Some(&releases[selected_idx])
    } else {
        None
    };

    let title = if let Some(v) = current_version {
        format!(" llama.cpp Releases (v{}) ", v.tag)
    } else {
        " llama.cpp Releases ".to_string()
    };

    let _block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let chunks = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            ratatui::layout::Constraint::Fill(1),
            ratatui::layout::Constraint::Length(1),
        ])
        .split(area);

    // Backend toggle header
    let cpu_label = if picker_backend == crate::models::Backend::Cpu {
        format!("[CPU] {}", "CPU".to_string())
    } else {
        "CPU".to_string()
    };
    let vulkan_label = if picker_backend == crate::models::Backend::Vulkan {
        format!("[Vulkan] {}", "Vulkan".to_string())
    } else {
        "Vulkan".to_string()
    };
    let rocm_label = if picker_backend == crate::models::Backend::Rocrm {
        format!("[ROCm] {}", "ROCm".to_string())
    } else {
        "ROCm".to_string()
    };
    let backend_line = Line::from(vec![
        Span::styled(cpu_label, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw(" | "),
        Span::styled(vulkan_label, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw(" | "),
        Span::styled(rocm_label, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
    ]);
    f.render_widget(Paragraph::new(backend_line), chunks[0]);

    // Build rows for releases
    let mut rows: Vec<Row> = Vec::new();
    for (i, r) in releases.iter().enumerate() {
 let tag_style = if i == selected_idx {
            Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else if r.is_prerelease {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default().fg(Color::White)
        };

        let tag = &r.tag;
        let marker = if i == selected_idx { ">" } else { " " };
        let marker_style = if i == selected_idx {
            Style::default().fg(Color::Black).bg(Color::Yellow)
        } else {
            Style::default().fg(Color::Yellow)
        };

        let backend_label = match picker_backend {
            crate::models::Backend::Cpu => "CPU".to_string(),
            crate::models::Backend::Vulkan => "Vulkan".to_string(),
            crate::models::Backend::Rocrm => "ROCm".to_string(),
        };

        let size_hint = r.size.map(|s| format_size(s)).unwrap_or_default();
        let name = if r.name.len() > 40 {
            format!("{}...", &r.name[..37])
        } else {
            r.name.clone()
        };

        let row = Row::new(vec![
            Cell::from(Span::styled(marker, marker_style)),
            Cell::from(Span::styled(tag, tag_style)),
            Cell::from(Span::styled(backend_label, Style::default().fg(Color::Cyan))),
            Cell::from(Span::styled(name, Style::default().fg(Color::White))),
            Cell::from(Span::styled(size_hint, Style::default().fg(Color::DarkGray))),
        ]);
        rows.push(row);
    }

    // Cached versions section
    if show_cached {
        let cached_versions = match picker_backend {
            crate::models::Backend::Cpu => cpu_versions,
            crate::models::Backend::Vulkan => vulkan_versions,
            crate::models::Backend::Rocrm => rocm_versions,
        };
        if !cached_versions.is_empty() {
            rows.push(Row::new(vec![
                Cell::from(""),
                Cell::from(""),
                Cell::from(""),
                Cell::from(""),
                Cell::from(""),
            ]));
            let cached_header = Row::new(vec![
                Cell::from(""),
                Cell::from(""),
                Cell::from(Span::styled("--- Cached ---", Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD))),
                Cell::from(""),
                Cell::from(""),
            ]);
            rows.push(cached_header);
            for cv in cached_versions {
                rows.push(Row::new(vec![
                    Cell::from(" "),
                    Cell::from(Span::styled(cv, Style::default().fg(Color::Green))),
                    Cell::from(""),
                    Cell::from(""),
                    Cell::from(""),
                ]));
            }
        }
    }

    // Create table
    let widths = [
        Constraint::Length(2),
        Constraint::Length(10),
        Constraint::Length(8),
        Constraint::Fill(1),
        Constraint::Length(12),
    ];

    let table = Table::new(rows, widths);
    let mut table_state = TableState::default();
    table_state.select(Some(selected_idx));

  // Apply scroll offset
    let table_area = chunks[0];
    let _table_height = table_area.height as usize;
    if scroll_offset > 0 {
        let current = table_state.selected();
        if let Some(s) = current {
            table_state.select(Some(s.saturating_sub(scroll_offset as usize)));
        }
    }

    f.render_stateful_widget(table, table_area, &mut table_state);

    // Hint line
    let hint = Line::from(Span::styled(
        "TAB to switch backend | ENTER to select | UP | DOWN | R refresh | C cached | ESC to back",
        Style::default().fg(Color::Cyan),
    ));
    f.render_widget(Paragraph::new(hint), chunks[1]);
}
