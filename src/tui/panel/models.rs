use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Cell, Paragraph, Row, Table, TableState},
};

use crate::models::{ListSort, SearchSort};
use crate::tui::app::{App, ModelsMode};
use crate::tui::colors::*;
use crate::tui::{format_context_k, format_number, format_size};
use regex::Regex;

const MARQUEE_SUFFIX: &str = "\u{25B6}";

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
        crate::t_fmt!("download.title", total_speed_str)
    } else {
        crate::t_fmt!("download.count", count, total_speed_str)
    };

    let (border_type, border_color) = if is_focused {
        (BorderType::Thick, GREEN)
    } else {
        (BorderType::Plain, GRAY)
    };
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .border_type(border_type);

    f.render_widget(block.clone(), area);
    let inner = block.inner(area);

    // Show all active downloads in a table
    let rows: Vec<Row> = downloads
        .iter()
        .map(|d| {
            // Re-implement basic progress formatting locally since I removed them from models.rs
            let progress_pct = if d.total_bytes == 0 {
                0.0
            } else {
                d.downloaded_bytes as f64 / d.total_bytes as f64 * 100.0
            };
            let progress_str = format!("{:.1}%", progress_pct);
            let speed_str = format_speed(d.bytes_per_second);

            let status = match &d.status {
                crate::models::DownloadStatus::Downloading => {
                    crate::t!("download.status.downloading")
                }
                crate::models::DownloadStatus::Pausing => crate::t!("download.status.pausing"),
                crate::models::DownloadStatus::Paused => crate::t!("download.status.paused"),
                crate::models::DownloadStatus::Complete => crate::t!("download.status.complete"),
                crate::models::DownloadStatus::Cancelled => crate::t!("download.status.cancelled"),
                crate::models::DownloadStatus::Error(e) => e.as_str(),
            };

            let status_color = match &d.status {
                crate::models::DownloadStatus::Downloading => YELLOW,
                crate::models::DownloadStatus::Pausing => YELLOW,
                crate::models::DownloadStatus::Paused => WHITE,
                crate::models::DownloadStatus::Complete => GREEN,
                crate::models::DownloadStatus::Cancelled => RED,
                crate::models::DownloadStatus::Error(_) => RED,
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
        Cell::from(crate::t!("download.headers.model")).style(
            Style::default()
                .fg(YELLOW)
                .add_modifier(Modifier::BOLD),
        ),
        Cell::from(crate::t!("download.headers.file")).style(
            Style::default()
                .fg(YELLOW)
                .add_modifier(Modifier::BOLD),
        ),
        Cell::from(crate::t!("download.headers.progress")).style(
            Style::default()
                .fg(YELLOW)
                .add_modifier(Modifier::BOLD),
        ),
        Cell::from(crate::t!("download.headers.speed")).style(
            Style::default()
                .fg(YELLOW)
                .add_modifier(Modifier::BOLD),
        ),
        Cell::from(crate::t!("download.headers.eta")).style(
            Style::default()
                .fg(YELLOW)
                .add_modifier(Modifier::BOLD),
        ),
        Cell::from(crate::t!("download.headers.status")).style(
            Style::default()
                .fg(YELLOW)
                .add_modifier(Modifier::BOLD),
        ),
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
        .row_highlight_style(
            Style::default()
                .bg(GREEN)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    f.render_stateful_widget(table, inner, scroll_state);
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
        crate::t!("download.calculating").to_string()
    }
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

pub fn scroll_text(
    text: &str,
    max_width: u16,
    state: &mut crate::tui::app::TextScrollState,
) -> String {
    state.max_offset = text.chars().count().saturating_sub(max_width as usize);

    // Short text: no scrolling needed
    if state.max_offset == 0 {
        state.cached_output = Some(text.to_string());
        state.cached_width = max_width;
        return state.cached_output.as_ref().unwrap().clone();
    }

    // Use cache if width and offset haven't changed
    if state.cached_width == max_width && state.cached_offset == state.offset {
        if let Some(ref cached) = state.cached_output {
            return cached.clone();
        }
    }

    let offset = state.offset.min(state.max_offset);
    let mut char_indices_iter = text.char_indices();
    let start_byte = char_indices_iter.nth(offset).map_or(0, |(i, _)| i);
    let end_byte = char_indices_iter
        .nth(max_width as usize)
        .map_or(text.len(), |(i, _)| i);
    let result = format!("{}{}", &text[start_byte..end_byte], MARQUEE_SUFFIX);
    state.cached_output = Some(result.clone());
    state.cached_width = max_width;
    state.cached_offset = state.offset;
    result
}

/// Highlight occurrences matched by `compiled` within `text`.
/// `lower_text` is the case-folded version of `text` used for matching.
fn highlight_query(text: &str, lower_text: &str, compiled: Option<&Regex>) -> Line<'static> {
    if compiled.is_none() || text.is_empty() {
        return Line::from(text.to_string());
    }
    let highlights = compiled
        .unwrap()
        .captures_iter(lower_text)
        .filter_map(|cap| {
            let m = cap.get(0)?;
            Some((m.start(), m.end()))
        })
        .collect::<Vec<(usize, usize)>>();
    let mut spans = Vec::new();
    let mut start = 0;
    for (hstart, hend) in &highlights {
        if *hstart > start {
            spans.push(Span::styled(
                text[start..*hstart].to_string(),
                Style::default(),
            ));
        }
        spans.push(Span::styled(
            text[*hstart..*hend].to_string(),
            Style::default()
                .fg(CYAN)
                .add_modifier(Modifier::BOLD),
        ));
        start = *hend;
    }
    if start < text.len() {
        spans.push(Span::styled(
            text[start..text.len()].to_string(),
            Style::default(),
        ));
    }
    if spans.is_empty() {
        spans.push(Span::styled(text.to_string(), Style::default()));
    }
    Line::from(spans)
}

pub fn render(f: &mut Frame, area: Rect, app: &mut App) {
    match &app.models_mode {
        ModelsMode::List { sort_by } => {
            let title = if app.is_panel_visible(0) {
                crate::t!("panel.title.models_active").to_string()
            } else {
                crate::t!("panel.title.models").to_string()
            };

            let is_models_focused = app.ui.active_panel == crate::tui::app::ActivePanel::Models;
            let (border_type, border_color) = if is_models_focused {
                (BorderType::Thick, GREEN)
            } else {
                (BorderType::Plain, GRAY)
            };
            let block = Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color))
                .border_type(border_type);

            f.render_widget(block.clone(), area);

            let inner_area = block.inner(area);
            let (table_area, filter_area) =
                if app.search.filtering_local || !app.search.local_filter.is_empty() {
                    let chunks = ratatui::layout::Layout::default()
                        .direction(ratatui::layout::Direction::Vertical)
                        .constraints([Constraint::Length(1), Constraint::Fill(1)])
                        .split(inner_area);
                    (chunks[1], Some(chunks[0]))
                } else {
                    (inner_area, None)
                };

            if let Some(fa) = filter_area {
                let filter_inner = ratatui::layout::Rect {
                    x: fa.x + 1,
                    y: fa.y,
                    width: fa.width.saturating_sub(2),
                    height: fa.height,
                };
                let filter_text = if app.search.filtering_local {
                    Line::from(vec![
                        Span::styled(
                            "Filter: ",
                            Style::default()
                                .fg(YELLOW)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            &app.search.local_filter,
                            Style::default().fg(BLACK).bg(YELLOW),
                        ),
                        Span::styled("_", Style::default().fg(BLACK).bg(YELLOW)),
                    ])
                } else {
                    Line::from(vec![
                        Span::styled("Filter: ", Style::default().fg(DARK_GRAY)),
                        Span::styled(&app.search.local_filter, Style::default().fg(CYAN)),
                    ])
                };
                f.render_widget(ratatui::widgets::Paragraph::new(filter_text), filter_inner);
            }

            let sort_ascending = sort_by.is_ascending();
            let headers = vec![
                Cell::from(if *sort_by == ListSort::Name {
                    if sort_ascending {
                        "Model \u{2191}"
                    } else {
                        "Model \u{2193}"
                    }
                } else {
                    crate::t!("models.list_headers.model")
                })
                .style(
                    Style::default()
                        .fg(YELLOW)
                        .add_modifier(Modifier::BOLD),
                ),
                Cell::from(if *sort_by == ListSort::Status {
                    if sort_ascending {
                        "Status \u{2191}"
                    } else {
                        "Status \u{2193}"
                    }
                } else {
                    crate::t!("models.list_headers.status")
                })
                .style(
                    Style::default()
                        .fg(YELLOW)
                        .add_modifier(Modifier::BOLD),
                ),
                Cell::from(if *sort_by == ListSort::Params {
                    if sort_ascending {
                        "Params \u{2191}"
                    } else {
                        "Params \u{2193}"
                    }
                } else {
                    crate::t!("models.list_headers.params")
                })
                .style(
                    Style::default()
                        .fg(YELLOW)
                        .add_modifier(Modifier::BOLD),
                ),
                Cell::from(if *sort_by == ListSort::Qual {
                    if sort_ascending {
                        "Qual \u{2191}"
                    } else {
                        "Qual \u{2193}"
                    }
                } else {
                    crate::t!("models.list_headers.quality")
                })
                .style(
                    Style::default()
                        .fg(YELLOW)
                        .add_modifier(Modifier::BOLD),
                ),
                Cell::from(
                    Line::from(if *sort_by == ListSort::Context {
                        if sort_ascending {
                            " Ctx \u{2191}"
                        } else {
                            " Ctx \u{2193}"
                        }
                    } else {
                        crate::t!("models.list_headers.context")
                    })
                    .style(
                        Style::default()
                            .fg(YELLOW)
                            .add_modifier(Modifier::BOLD),
                    )
                    .alignment(Alignment::Center),
                ),
            ];

            let ctx_cache = app.get_ctx_cache();
            let sorted_indices = app.get_sorted_model_indices().to_vec();

            let rows: Vec<Row> = sorted_indices
                .iter()
                .map(|&idx| {
                    let model = &app.models[idx];
                    let key = model.display_name.clone();
                    let model_state = app.model_states.get(&model.display_name);
                    let is_selected = Some(idx) == app.selected_model_idx;

                    let status_text = match model_state {
                        Some(crate::models::ModelState::Loaded { .. }) => {
                            Some(crate::t!("models.list_status.loaded"))
                        }
                        Some(crate::models::ModelState::Loading) => {
                            Some(crate::t!("models.list_status.loading"))
                        }
                        Some(crate::models::ModelState::Benchmarking) => {
                            Some(crate::t!("models.list_status.benchmarking"))
                        }
                        _ => None,
                    };

                    let (context_length, rope_yarn_enabled, rope_scale) = ctx_cache
                        .get(model.display_name.as_str())
                        .copied()
                        .unwrap_or((0, false, 0.0));
                    let context_str =
                        format_context_k(context_length, rope_yarn_enabled, rope_scale);

                    let filename = model
                        .display_name
                        .rsplit('/')
                        .next()
                        .unwrap_or(&model.display_name);
                    let display_name = filename.strip_suffix(".gguf").unwrap_or(filename);

                    // Fetch metadata once, reuse for params / moe / quality
                    let path_key = model.path.to_string_lossy();
                    let meta = app.search.gguf_metadata_cache.get(path_key.as_ref());
                    let params_str = meta
                        .map(|m| {
                            let mut p = m.model_parameters.clone();
                            if m.arch.contains("moe") && !p.is_empty() {
                                p = format!("{} (MoE)", p);
                            }
                            p
                        })
                        .unwrap_or_default();
                    let params_width = params_str.chars().count() as u16 + 2;

                    let name_width =
                        table_area
                            .width
                            .saturating_sub(
                                status_text.as_ref().map_or(0, |s| s.chars().count()) as u16
                            )
                            .saturating_sub(context_str.chars().count() as u16 + 4)
                            .saturating_sub(params_width)
                            .saturating_sub(4);
                    let max_offset = filename.chars().count().saturating_sub(name_width as usize);
                    let state = app.ui.text_scrolls.entry(key.clone()).or_insert_with(|| {
                        crate::tui::app::TextScrollState {
                            offset: 0,
                            last_tick: std::time::Instant::now(),
                            direction: 1,
                            hold_count: 0,
                            max_offset,
                            visible: false,
                            cached_output: None,
                            cached_width: 0,
                            cached_offset: 0,
                        }
                    });
                    state.max_offset = max_offset;
                    state.visible = true;

                    let name_display = scroll_text(display_name, name_width, state);
                    let name_style = if is_selected {
                        Style::default()
                            .fg(BLACK)
                            .bg(GREEN)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        match model_state {
                            Some(crate::models::ModelState::Loaded { .. }) => Style::default()
                                .fg(GREEN)
                                .add_modifier(Modifier::BOLD),
                            Some(crate::models::ModelState::Loading)
                            | Some(crate::models::ModelState::Benchmarking) => {
                                Style::default().fg(YELLOW)
                            }
                            _ => Style::default().fg(WHITE),
                        }
                    };

                    let status_style = match model_state {
                        Some(crate::models::ModelState::Loaded { .. }) => {
                            Style::default().fg(GREEN)
                        }
                        Some(crate::models::ModelState::Loading)
                        | Some(crate::models::ModelState::Benchmarking) => {
                            Style::default().fg(YELLOW)
                        }
                        _ => Style::default().fg(GRAY),
                    };

                    let is_moe = meta.map(|m| m.arch.contains("moe")).unwrap_or(false);
                    let params_style = if params_str.is_empty() {
                        Style::default().fg(DARK_GRAY)
                    } else if is_moe {
                        Style::default().fg(MAGENTA)
                    } else {
                        Style::default().fg(WHITE)
                    };

                    let quality_cell = meta
                        .map(|m| quality_dot(m.quality_rank))
                        .unwrap_or_else(|| quality_dot(0));

                    Row::new(vec![
                        Cell::from(Line::from(Span::styled(name_display, name_style))),
                        Cell::from(status_text.clone().unwrap_or_default()).style(status_style),
                        Cell::from(params_str).style(params_style),
                        Cell::from(quality_cell),
                        Cell::from(
                            ratatui::text::Text::from(context_str)
                                .alignment(ratatui::layout::Alignment::Right),
                        )
                        .style(Style::default().fg(CYAN)),
                    ])
                })
                .collect();

            let widths = [
                Constraint::Percentage(52),
                Constraint::Percentage(11),
                Constraint::Percentage(10),
                Constraint::Length(4),
                Constraint::Percentage(10),
            ];

            let table = Table::new(rows, widths)
                .header(Row::new(headers))
                .row_highlight_style(
                    Style::default()
                        .fg(BLACK)
                        .bg(GREEN)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("> ");

            app.ui.models_table_state.select(
                app.selected_model_idx
                    .and_then(|idx| sorted_indices.iter().position(|&i| i == idx)),
            );
            f.render_stateful_widget(table, table_area, &mut app.ui.models_table_state);
        }
        ModelsMode::Search {
            query,
            results,
            sort_by,
            loading,
            has_more,
            ..
        } => {
            let sort_label = sort_by.label();
            let display_query = app.search.search_input.as_deref().unwrap_or(query);
            let title = if app.is_panel_visible(0) {
                crate::t_fmt!(
                    "models.search_title",
                    display_query,
                    sort_label,
                    results.len()
                )
            } else {
                crate::t_fmt!(
                    "models.search_simple",
                    display_query,
                    sort_label,
                    results.len()
                )
            };
            let is_models_focused = app.ui.active_panel == crate::tui::app::ActivePanel::Models;
            let (border_type, border_color) = if is_models_focused {
                (BorderType::Thick, GREEN)
            } else {
                (BorderType::Plain, GRAY)
            };
            let block = Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color))
                .border_type(border_type);

            let headers = vec![
                Cell::from(crate::t!("models.search_headers.model")).style(
                    Style::default()
                        .fg(YELLOW)
                        .add_modifier(Modifier::BOLD),
                ),
                Cell::from(if *sort_by == SearchSort::Downloads {
                    "⬇"
                } else {
                    crate::t!("models.search_headers.downloads")
                })
                .style(
                    Style::default()
                        .fg(YELLOW)
                        .add_modifier(Modifier::BOLD),
                ),
                Cell::from(if *sort_by == SearchSort::Likes {
                    "♥"
                } else {
                    crate::t!("models.search_headers.likes")
                })
                .style(
                    Style::default()
                        .fg(YELLOW)
                        .add_modifier(Modifier::BOLD),
                ),
                Cell::from(crate::t!("models.search_headers.license")).style(
                    Style::default()
                        .fg(YELLOW)
                        .add_modifier(Modifier::BOLD),
                ),
            ];

            let query_regex = if query.trim().is_empty() {
                None
            } else {
                let pattern: String = query
                    .split_whitespace()
                    .map(|w| w.to_lowercase())
                    .collect::<Vec<_>>()
                    .join("|");
                Regex::new(&pattern).ok()
            };

            let mut rows: Vec<Row> = results
                .iter()
                .map(|result| {
                    let license = result.license.as_deref().unwrap_or("—");
                    let table_width = area.width.saturating_sub(2);
                    let col_width = table_width * 60 / 100;
                    let key = result.model_id.clone();
                    let max_offset = result
                        .model_id
                        .chars()
                        .count()
                        .saturating_sub(col_width as usize);
                    let state = app.ui.text_scrolls.entry(key).or_insert_with(|| {
                        crate::tui::app::TextScrollState {
                            offset: 0,
                            last_tick: std::time::Instant::now(),
                            direction: 1,
                            hold_count: 0,
                            max_offset,
                            visible: false,
                            cached_output: None,
                            cached_width: 0,
                            cached_offset: 0,
                        }
                    });
                    state.max_offset = max_offset;
                    state.visible = true;
                    let scrolled_raw = scroll_text(&result.model_id, col_width, state);
                    let scrolled_lower = scrolled_raw.to_lowercase();
                    let highlighted =
                        highlight_query(&scrolled_raw, &scrolled_lower, query_regex.as_ref());

                    let is_downloaded = result.downloaded;
                    let marker = if is_downloaded { "✓" } else { " " };
                    let marker_span =
                        Span::styled(format!("[{}] ", marker), Style::default().fg(GREEN));
                    let mut name_spans: Vec<Span> = vec![marker_span];
                    name_spans.extend(highlighted.spans.iter().cloned());

                    Row::new(vec![
                        Cell::from(Line::from(name_spans)),
                        Cell::from(format_number(result.downloads)),
                        Cell::from(format_number(result.likes)),
                        Cell::from(license.to_string()),
                    ])
                })
                .collect();

            // Add informational rows
            if *loading {
                rows.push(Row::new(vec![
                    Cell::from("Loading more results...").style(Style::default().fg(YELLOW)),
                    Cell::from(""),
                    Cell::from(""),
                    Cell::from(""),
                ]));
            } else if results.is_empty() {
                rows.push(Row::new(vec![
                    Cell::from("No results found for this query.")
                        .style(Style::default().fg(RED)),
                    Cell::from(""),
                    Cell::from(""),
                    Cell::from(""),
                ]));
            } else if !has_more {
                rows.push(Row::new(vec![
                    Cell::from("No more results").style(Style::default().fg(DARK_GRAY)),
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
                .row_highlight_style(
                    Style::default()
                        .fg(BLACK)
                        .bg(GREEN)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("> ");

            app.search
                .search_table_state
                .select(app.search.search_results_idx);
            f.render_stateful_widget(table, area, &mut app.search.search_table_state);
        }
        ModelsMode::Files {
            model_id,
            files,
            selected_idx,
            selected_result: _,
            ..
        } => {
            let title = crate::t_fmt!("models.gguf_files", model_id);
            let is_models_focused = app.ui.active_panel == crate::tui::app::ActivePanel::Models;
            let (border_type, border_color) = if is_models_focused {
                (BorderType::Thick, GREEN)
            } else {
                (BorderType::Plain, GRAY)
            };
            let block = Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color))
                .border_type(border_type);

            let inner_area = block.inner(area);
            f.render_widget(block, area);

            // Render Files Table
            let rows: Vec<Row> = files
                .iter()
                .map(|(filename, size, _url): &(_, _, _)| {
                    let name = filename.rsplit('/').next().unwrap_or(filename);
                    let is_downloaded = app
                        .search
                        .downloaded_filenames
                        .contains(&name.to_lowercase());
                    let marker = if is_downloaded { "✓" } else { " " };
                    let marker_span =
                        Span::styled(format!("[{}] ", marker), Style::default().fg(GREEN));
                    let table_width = inner_area.width.saturating_sub(2);
                    let col_width = table_width * 80 / 100;
                    let available = col_width.saturating_sub(4);
                    let key = filename.clone();
                    let max_offset = name.chars().count().saturating_sub(available as usize);
                    let state = app.ui.text_scrolls.entry(key).or_insert_with(|| {
                        crate::tui::app::TextScrollState {
                            offset: 0,
                            last_tick: std::time::Instant::now(),
                            direction: 1,
                            hold_count: 0,
                            max_offset,
                            visible: false,
                            cached_output: None,
                            cached_width: 0,
                            cached_offset: 0,
                        }
                    });
                    state.max_offset = max_offset;
                    state.visible = true;
                    let scrolled_raw = scroll_text(name, available, state);
                    let highlighted = highlight_query(&scrolled_raw, "", None);
                    let mut name_spans: Vec<Span> = vec![marker_span];
                    name_spans.extend(highlighted.spans.iter().cloned());
                    Row::new(vec![
                        Cell::from(Line::from(name_spans)),
                        Cell::from(format_size(*size)),
                    ])
                })
                .collect();

            let headers = vec![
                Cell::from(crate::t!("models.gguf_headers.file")).style(
                    Style::default()
                        .fg(YELLOW)
                        .add_modifier(Modifier::BOLD),
                ),
                Cell::from(crate::t!("models.gguf_headers.size")).style(
                    Style::default()
                        .fg(YELLOW)
                        .add_modifier(Modifier::BOLD),
                ),
            ];

            let widths = [Constraint::Percentage(80), Constraint::Percentage(20)];

            let table = Table::new(rows, widths)
                .header(Row::new(headers))
                .row_highlight_style(
                    Style::default()
                        .fg(BLACK)
                        .bg(GREEN)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("> ");

            app.search.files_table_state.select(*selected_idx);

            f.render_stateful_widget(table, inner_area, &mut app.search.files_table_state);
        }
        ModelsMode::BenchTune => {
           let title = crate::t!("panel.title.bench_tune").to_string();
            let is_models_focused = app.ui.active_panel == crate::tui::app::ActivePanel::Models;
            let (border_type, border_color) = if is_models_focused {
                (BorderType::Thick, GREEN)
            } else {
                (BorderType::Plain, GRAY)
            };
            let block = Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color))
                .border_type(border_type);

            let inner_area = block.inner(area);
            f.render_widget(block, area);

            // Show bench_tune progress or results
            let mut lines: Vec<Line> = Vec::new();

            if let Some(progress) = &app.bench_tune.bench_tune_progress {
                match progress {
                    crate::models::BenchTuneProgress::Running {
                        current,
                        total,
                        progress: p,
                        current_params,
                    } => {
                        lines.push(Line::from(crate::t_fmt!(
                            "models.benchtune_progress",
                            current,
                            total,
                            format!("{:.0}", p)
                        )));
                        lines.push(Line::from(""));
                        lines.push(Line::from(Span::styled(
                            crate::t!("models.benchtune_current"),
                            Style::default().add_modifier(Modifier::BOLD),
                        )));

                        let p_parts = crate::tui::format_bench_params(current_params, true);

                        if p_parts.is_empty() {
                            lines.push(Line::from(crate::t!("models.benchtune_baseline")));
                        } else {
                            for part in p_parts {
                                lines.push(Line::from(part));
                            }
                        }
                    }
                    crate::models::BenchTuneProgress::Completed {
                        total_tests,
                        successful_tests,
                        elapsed,
                    } => {
                        let elapsed_str = format!("{}s", elapsed.as_secs());
                        lines.push(Line::from(vec![
                            Span::raw(crate::t!("models.benchtune_complete")),
                            Span::styled(
                                crate::t_fmt!(
                                    "models.benchtune_complete_time",
                                    total_tests,
                                    elapsed_str
                                ),
                                Style::default()
                                    .fg(GREEN)
                                    .add_modifier(Modifier::BOLD),
                            ),
                        ]));

                        let success_style = if successful_tests == total_tests {
                            Style::default().fg(GREEN)
                        } else if *successful_tests > 0 {
                            Style::default().fg(YELLOW)
                        } else {
                            Style::default().fg(RED)
                        };

                        lines.push(Line::from(vec![
                            Span::raw(crate::t!("models.benchtune_success")),
                            Span::styled(
                                format!("{}/{}", successful_tests, total_tests),
                                success_style.add_modifier(Modifier::BOLD),
                            ),
                        ]));

                        if successful_tests < total_tests {
                            lines.push(Line::from(Span::styled(
                                crate::t_fmt!(
                                    "models.benchtune_warning",
                                    total_tests - successful_tests
                                ),
                                Style::default().fg(RED),
                            )));
                        }

                        if !app.bench_tune.bench_tune_results.is_empty() {
                            render_benchtune_results_table(f, inner_area, app, &mut lines);
                            return;
                        }
                    }
                    crate::models::BenchTuneProgress::PartiallyCompleted {
                        total_tests,
                        successful_tests,
                        failed_tests,
                        elapsed,
                    } => {
                        let elapsed_str = format!("{}s", elapsed.as_secs());
                        lines.push(Line::from(vec![
                            Span::raw("Status: "),
                            Span::styled(
                                "PARTIALLY COMPLETED",
                                Style::default()
                                    .fg(YELLOW)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::raw(format!(" ({} tests in {})", total_tests, elapsed_str)),
                        ]));

                        lines.push(Line::from(vec![
                            Span::raw("Success: "),
                            Span::styled(
                                format!("{}/{}", successful_tests, total_tests),
                                Style::default()
                                    .fg(YELLOW)
                                    .add_modifier(Modifier::BOLD),
                            ),
                        ]));
                        lines.push(Line::from(vec![
                            Span::raw("Failed: "),
                            Span::styled(
                                format!("{} test(s)", failed_tests),
                                Style::default().fg(RED).add_modifier(Modifier::BOLD),
                            ),
                        ]));
                        lines.push(Line::from(Span::styled(
                            "Check Log (F6) for failure details.".to_string(),
                            Style::default().fg(RED),
                        )));

                        if !app.bench_tune.bench_tune_results.is_empty() {
                            render_benchtune_results_table(f, inner_area, app, &mut lines);
                            return;
                        }
                    }
                    crate::models::BenchTuneProgress::Cancelled {
                        total_tests,
                        successful_tests,
                        failed_tests,
                        elapsed,
                    } => {
                        let elapsed_str = format!("{}s", elapsed.as_secs());
                        lines.push(Line::from(vec![
                            Span::raw("Status: "),
                            Span::styled(
                                "CANCELLED",
                                Style::default()
                                    .fg(YELLOW)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::raw(format!(" ({} tests in {})", total_tests, elapsed_str)),
                        ]));

                        lines.push(Line::from(vec![
                            Span::raw("Success: "),
                            Span::styled(
                                format!("{}/{}", successful_tests, total_tests),
                                Style::default()
                                    .fg(YELLOW)
                                    .add_modifier(Modifier::BOLD),
                            ),
                        ]));
                        lines.push(Line::from(vec![
                            Span::raw("Failed: "),
                            Span::styled(
                                format!("{} test(s)", failed_tests),
                                Style::default().fg(RED).add_modifier(Modifier::BOLD),
                            ),
                        ]));
                        lines.push(Line::from(Span::styled(
                            "Benchmark was cancelled by user.",
                            Style::default().fg(YELLOW),
                        )));

                        if !app.bench_tune.bench_tune_results.is_empty() {
                            render_benchtune_results_table(f, inner_area, app, &mut lines);
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

/// Return a colored emoji dot for the quality rank.
/// Emoji colors are baked into the glyph so they survive row highlights.
fn quality_dot(rank: u8) -> Cell<'static> {
    let dot = match rank {
        4 => "\u{1F7E2}", // green circle - best
        3 => "\u{1F7E1}", // yellow circle - high
        2 => "\u{1F7E0}", // orange circle - medium
        1 => "\u{1F534}", // red circle - low
        _ => "\u{26AB}",  // black circle - unknown
    };
    Cell::from(dot)
}

/// Render the benchmark results table.
/// Adds header lines to `lines`, then renders paragraph + table in `inner_area`.
fn render_benchtune_results_table(
    f: &mut Frame,
    inner_area: Rect,
    app: &mut App,
    lines: &mut Vec<Line<'static>>,
) {
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " Benchmark results (sorted by generation speed):",
        Style::default().add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(Span::styled(
        " (Press [↵] to view details of selected result)",
        Style::default().fg(DARK_GRAY),
    )));
    lines.push(Line::from(""));

    let mut rows = Vec::new();
    for (i, result) in app.bench_tune.bench_tune_results.iter().enumerate() {
        let p_str = crate::tui::format_bench_params(&result.params, false).join(",");
        let style = if i == 0 {
            Style::default().fg(GREEN)
        } else {
            Style::default().fg(WHITE)
        };
        rows.push(
            Row::new(vec![
                Cell::from(format!(" {:<2} ", i + 1)),
                Cell::from(format!("{:.2}", result.metrics.generation_tps)),
                Cell::from(format!("{:.2}", result.metrics.prompt_tps)),
                Cell::from(p_str),
            ])
            .style(style),
        );
    }

    app.bench_tune
        .bench_tune_table_state
        .select(Some(app.bench_tune.bench_tune_result_row));

    let header = Row::new(vec![
        Cell::from(" # "),
        Cell::from("Gen t/s"),
        Cell::from("Inf t/s"),
        Cell::from("Params"),
    ])
    .style(
        Style::default()
            .fg(YELLOW)
            .add_modifier(Modifier::BOLD),
    );

    let table = ratatui::widgets::Table::new(
        rows,
        [
            ratatui::layout::Constraint::Length(4),
            ratatui::layout::Constraint::Length(10),
            ratatui::layout::Constraint::Length(10),
            ratatui::layout::Constraint::Fill(1),
        ],
    )
    .header(header)
    .block(Block::default().borders(Borders::NONE))
    .row_highlight_style(
        Style::default()
            .bg(GREEN)
            .add_modifier(Modifier::BOLD),
    )
    .highlight_symbol("> ");

    let header_height = lines.len() as u16;
    let table_area = Rect {
        x: inner_area.x,
        y: inner_area.y + header_height,
        width: inner_area.width,
        height: inner_area.height.saturating_sub(header_height),
    };

    f.render_widget(Paragraph::new(lines.clone()), inner_area);
    f.render_stateful_widget(
        table,
        table_area,
        &mut app.bench_tune.bench_tune_table_state,
    );
}
