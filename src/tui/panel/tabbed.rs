use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
};

use super::info;
use super::info::ModelInfoPair;
use super::settings;
use crate::tui::app::{ActivePanel, App};

const SERVER_SETTINGS_HEIGHT: u16 = 8;

pub fn render_settings_only(f: &mut Frame, area: Rect, app: &mut App) {
    if area.height < 2 || area.width < 10 {
        return;
    }

    let is_focused = app.active_panel == crate::tui::app::ActivePanel::LlmSettings;

    // Split area: top for Server Settings, rest for LLM Settings
    let server_area = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: SERVER_SETTINGS_HEIGHT.min(area.height),
    };

    let llm_area = Rect {
        x: area.x,
        y: area.y + server_area.height,
        width: area.width,
        height: area.height.saturating_sub(server_area.height),
    };

    // ── Server Settings box (always shown) ───────────────────
    render_server_settings(f, server_area, app);

    // ── LLM Settings ─────────────────────────────────────────
    let (settings_lines, _count, settings_height, _selected_line_idx) = settings::render_all(app, llm_area);

    let available_height = llm_area.height.saturating_sub(2);

    // Build visible settings lines with scroll offset applied
    let start_idx = app.settings_scroll_offset as usize;
    let visible_lines: Vec<Line<'static>> = settings_lines
        .iter()
        .skip(start_idx)
        .take(available_height as usize)
        .cloned()
        .collect();

    let border_color = if is_focused { Color::Green } else { Color::Rgb(255, 165, 0) };
    let vram_text = crate::models::format_mib(app.vram_estimate);
    let block = Block::default()
        .title(Line::from(vec![
            Span::raw(" LLM Settings (F4) [4] "),
            Span::styled(format!("(VRAM ~= {}) ", vram_text), Style::default().fg(Color::Yellow)),
        ]))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let paragraph = Paragraph::new(visible_lines).block(block);
    f.render_widget(paragraph, llm_area);

    // Render scrollbar if settings overflow
    if settings_height > available_height as usize {
        let scrollbar_area = Rect {
            x: llm_area.right().saturating_sub(1),
            y: llm_area.top(),
            width: 1,
            height: llm_area.height,
        };

        let mut scrollbar_state = ScrollbarState::new(settings_height)
            .position(app.settings_scroll_offset as usize);

        f.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓")),
            scrollbar_area,
            &mut scrollbar_state,
        );
    }
}
fn render_server_settings(f: &mut Frame, area: Rect, app: &mut App) {
    if area.height < 2 || area.width < 10 {
        return;
    }

    let is_focused = app.active_panel == crate::tui::app::ActivePanel::ServerSettings;
    let border_color = if is_focused { Color::Green } else { Color::Rgb(255, 165, 0) };
    let selected = app.server_settings_selected_idx;
    let server_running = app.server_handle.is_some();

    let host_val = crate::models::format_host(&app.settings.host);

    let backend_name = format!("{} (v{})", app.settings.backend, app.settings.get_active_backend_version_display());
    let threads_val = format!("{}", app.settings.threads);
    let threads_batch_val = format!("{}", app.settings.threads_batch);
    let mode_val = format!("{}", app.server_mode);
    let api_enabled = if app.settings.api_endpoint_enabled { "True" } else { "False" };
    let rpc_workers_count = app.config.rpc_workers.iter().filter(|w| w.selected).count();
    let rpc_workers_val = if rpc_workers_count > 0 {
        format!("{} active", rpc_workers_count)
    } else {
        "None".to_string()
    };

    let mut lines = Vec::new();
    let mut count = 0;
    let mut selected_line_idx = 0;
    let mut selected_content_line = 0;
    settings::add_setting(&mut lines, &mut count, &app.settings, &app.settings, &mut selected_line_idx, &mut selected_content_line, 0, "Host", host_val, selected, "", false);
    settings::add_setting(&mut lines, &mut count, &app.settings, &app.settings, &mut selected_line_idx, &mut selected_content_line, 1, "Backend", &backend_name, selected, "", false);
    settings::add_setting(&mut lines, &mut count, &app.settings, &app.settings, &mut selected_line_idx, &mut selected_content_line, 2, "Threads", &threads_val, selected, "", false);
    settings::add_setting(&mut lines, &mut count, &app.settings, &app.settings, &mut selected_line_idx, &mut selected_content_line, 3, "Threads Batch", &threads_batch_val, selected, "", false);
    settings::add_setting(&mut lines, &mut count, &app.settings, &app.settings, &mut selected_line_idx, &mut selected_content_line, 4, "Mode", &mode_val, selected, "", false);
    settings::add_setting(&mut lines, &mut count, &app.settings, &app.settings, &mut selected_line_idx, &mut selected_content_line, 5, "API Endpoint", api_enabled, selected, "", server_running);
    settings::add_setting(&mut lines, &mut count, &app.settings, &app.settings, &mut selected_line_idx, &mut selected_content_line, 6, "RPC Workers", &rpc_workers_val, selected, "", false);

    let total_settings = lines.len();
    let available_height = area.height.saturating_sub(2);

    if selected_content_line < app.server_settings_scroll_offset {
        app.server_settings_scroll_offset = selected_content_line;
    } else if available_height > 0 && (selected_content_line - app.server_settings_scroll_offset) >= (available_height as usize) {
        app.server_settings_scroll_offset = (selected_content_line).saturating_sub(available_height as usize).saturating_add(1);
    }

    let max_offset = total_settings.saturating_sub(available_height as usize);
    if app.server_settings_scroll_offset > max_offset {
        app.server_settings_scroll_offset = max_offset;
    }

    let start_idx = app.server_settings_scroll_offset as usize;
    let visible_lines: Vec<Line> = lines
        .into_iter()
        .skip(start_idx)
        .take(available_height as usize)
        .collect();

    let block = Block::default()
        .title(" Server Settings (F2) [2] ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let paragraph = Paragraph::new(visible_lines).block(block);
    f.render_widget(paragraph, area);

    if total_settings > available_height as usize {
        let scrollbar_area = Rect {
            x: area.right().saturating_sub(1),
            y: area.top() + 1,
            width: 1,
            height: area.height.saturating_sub(2),
        };
        let mut scrollbar_state = ScrollbarState::new(total_settings)
            .position(app.server_settings_scroll_offset as usize);
        f.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓")),
            scrollbar_area,
            &mut scrollbar_state,
        );
    }
}

pub fn render_server_only(f: &mut Frame, area: Rect, app: &mut App) {
    render_server_settings(f, area, app);
}

pub fn render_llm_only(f: &mut Frame, area: Rect, app: &mut App) {
    let is_focused = app.active_panel == ActivePanel::LlmSettings;
    let border_color = if is_focused { Color::Green } else { Color::Rgb(255, 165, 0) };
    let vram_text = crate::models::format_mib(app.vram_estimate);
    let block = Block::default()
        .title(Line::from(vec![
            Span::raw(" LLM Settings (F4) [4] "),
            Span::styled(format!("(VRAM ~= {}) ", vram_text), Style::default().fg(Color::Yellow)),
        ]))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let (all_lines, _count, settings_height, _selected_line_idx) = settings::render_all(app, area);
    
    let available_height = area.height.saturating_sub(2);
    let start_idx = app.settings_scroll_offset as usize;
    let visible_lines: Vec<Line> = all_lines
        .iter()
        .skip(start_idx)
        .take(available_height as usize)
        .cloned()
        .collect();

    let paragraph = Paragraph::new(visible_lines).block(block);
    f.render_widget(paragraph, area);

    // Render scrollbar if settings overflow
    if settings_height > available_height as usize {
        let scrollbar_area = Rect {
            x: area.right().saturating_sub(1),
            y: area.top(),
            width: 1,
            height: area.height,
        };
        
        let mut scrollbar_state = ScrollbarState::new(settings_height)
            .position(app.settings_scroll_offset as usize);
        
        f.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓")),
            scrollbar_area,
            &mut scrollbar_state,
        );
    }
}

fn empty_info() -> Vec<Line<'static>> {
    vec![Line::from(Span::styled(
        "Select a model to view info",
        Style::default().fg(Color::DarkGray),
    ))]
}

pub fn get_info_lines(app: &App, width: u16) -> Vec<Line<'static>> {
    let mut info_lines: Vec<Line<'static>> = match &app.models_mode {
        crate::tui::app::ModelsMode::Search { results, .. } => {
            if let Some(idx) = app.search_results_idx {
                if idx < results.len() {
                    let r = &results[idx];
                    render_search_result_info(r, None)
                } else {
                    empty_info()
                }
            } else {
                empty_info()
            }
        }
        crate::tui::app::ModelsMode::Files { selected_result, files, selected_idx, .. } => {
            let mut lines = if let Some(r) = selected_result {
                // If a file is selected, pass its info to override the repo size and extract params
                let file_info = selected_idx.and_then(|idx| files.get(idx).map(|(f, s, _)| (f.clone(), *s)));
                render_search_result_info(r, file_info)
            } else {
                Vec::new()
            };
            // Add GGUF file name for the selected file
            if let Some(idx) = selected_idx
                && let Some((filename, _, _url)) = files.get(*idx) {
                    lines.push(Line::from(vec![
                        Span::styled("File: ", Style::default().fg(Color::Yellow)),
                        Span::styled(filename.clone(), Style::default().fg(Color::White)),
                    ]));
                }
            lines
        }
        _ => {
            match app.selected_model() {
                Some(model) => {
                    let key = model.path.to_string_lossy().to_string();
                    let cached_meta = app.gguf_metadata_cache.get(&key);
                    let gpu_mem_total_mib = app.metrics.gpu_mem_total / (1024 * 1024);
                    let pairs = info::render_model_lines(model, cached_meta, app.vram_estimate, &app.model_settings_cache, gpu_mem_total_mib);
                    let mut lines = render_model_info_lines(&pairs, width);
                    // Hint when GGUF metadata was not available.
                    if cached_meta.is_none() {
                        lines.push(Line::from(vec![
                            Span::styled(
                                "GGUF metadata not available — check log for errors",
                                Style::default().fg(Color::DarkGray),
                            ),
                        ]));
                    }
                    lines
                },
                None => empty_info(),
            }
        }
    };

    // Add HuggingFace link for search results
    if let crate::tui::app::ModelsMode::Search { results, .. } = &app.models_mode {
        if let Some(idx) = app.search_results_idx
            && idx < results.len() {
                let r = &results[idx];
                let link_line = Line::from(vec![
                    Span::styled("  https://huggingface.co/", Style::default().fg(Color::DarkGray)),
                    Span::styled(r.model_id.clone(), Style::default().fg(Color::DarkGray)),
                ]);
                info_lines.push(link_line);
            }
    } else if let crate::tui::app::ModelsMode::Files { selected_result, .. } = &app.models_mode
        && let Some(r) = selected_result {
            let link_line = Line::from(vec![
                Span::styled("  https://huggingface.co/", Style::default().fg(Color::DarkGray)),
                Span::styled(r.model_id.clone(), Style::default().fg(Color::DarkGray)),
            ]);
            info_lines.push(link_line);
        }

    info_lines
}

pub fn render_info_only(f: &mut Frame, area: Rect, app: &mut App) {
    if area.height < 2 || area.width < 10 {
        return;
    }

    let info_lines = get_info_lines(app, area.width);
    render_info_paragraph(f, area, info_lines);
}

/// Render the info paragraph with a block and borders.
fn render_info_paragraph(f: &mut Frame, area: Rect, lines: Vec<Line<'static>>) {
    let block = Block::default()
        .title(" Model Info (F3)")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue));

    let paragraph = Paragraph::new(lines).block(block);
    f.render_widget(paragraph, area);
}

/// Convert ModelInfoPairs into 2-column Lines.
/// Path is rendered full-width on its own line.
fn render_model_info_lines(pairs: &[ModelInfoPair], _width: u16) -> Vec<Line<'static>> {
    if pairs.is_empty() {
        return empty_info();
    }

    let mut lines: Vec<Line<'static>> = Vec::new();

    // First pair (path) spans full width
    if let Some(first) = pairs.first() {
        let label = format!("{}: ", first.label);
        let value = first.value.clone();
        lines.push(Line::from(vec![
            Span::styled(label, Style::default().fg(Color::Yellow)),
            Span::styled(value, Style::default().fg(first.value_style)),
        ]));
    }

    // Remaining pairs in 2 columns
    let remaining: Vec<&ModelInfoPair> = pairs.iter().skip(1).collect();
    for chunk in remaining.chunks(2) {
        let left = chunk[0];

        if let Some(right) = chunk.get(1) {
            // Two columns: pad left label to align with right label
            let left_label = format!("{}: ", left.label);
            let right_label = format!("{}: ", right.label);
            
            lines.push(Line::from(vec![
                Span::styled(format!("{:<12}", left_label), Style::default().fg(Color::Yellow)),
                Span::styled(format!("{:<15}", left.value), Style::default().fg(left.value_style)),
                Span::raw("  "),
                Span::styled(format!("{:<12}", right_label), Style::default().fg(Color::Yellow)),
                Span::styled(right.value.clone(), Style::default().fg(right.value_style)),
            ]));
        } else {
            // Single item in last row
            let label = format!("{}: ", left.label);
            lines.push(Line::from(vec![
                Span::styled(label, Style::default().fg(Color::Yellow)),
                Span::styled(left.value.clone(), Style::default().fg(left.value_style)),
            ]));
        }
    }

    lines
}

fn extract_params_from_filename(filename: &str) -> String {
    let stem = crate::models::strip_gguf(filename.rsplit('/').next().unwrap_or(filename));
    
    // Look for patterns like "30B", "8B", "4B", "14B", "70B", "405B", "30B-A3B"
    // Search from end to find the size token
    let upper = stem.to_uppercase();
    for i in (0..stem.len()).rev() {
        let ch = upper.chars().nth(i).unwrap();
        if ch.is_ascii_digit() || ch == '.' {
            continue;
        }
        if ch == 'B' || ch == 'A' {
            // Extract the token ending at position i
            let start = if let Some(dash_pos) = stem[..=i].rfind('-') {
                dash_pos + 1
            } else {
                0
            };
            let token = &stem[start..=i];
            let token_upper = token.to_uppercase();
            // Check if it matches a size pattern (digits with optional .decimal, optionally followed by A3B)
            if token_upper.ends_with("A3B") {
                return token.to_string();
            }
            if token_upper.ends_with('B') {
                let digits_part = &token_upper[..token_upper.len()-1];
                if digits_part.chars().all(|c| c.is_ascii_digit() || c == '.') && !digits_part.is_empty() {
                    return token.to_string();
                }
            }
            break;
        }
    }
    
    "N/A".to_string()
}

fn render_search_result_info(r: &crate::models::SearchResult, file_info: Option<(String, u64)>) -> Vec<Line<'static>> {
    let size_str = file_info.as_ref().map(|(_, size)| crate::tui::format_size(*size))
        .or_else(|| r.size.map(crate::tui::format_size));
    
    // Extract params from filename if available, otherwise use repo-level params
    let params_str = if let Some((filename, _)) = &file_info {
        extract_params_from_filename(filename)
    } else {
        r.parameters.as_deref().unwrap_or("N/A").to_string()
    };
    let cap_str: String = if r.capabilities.is_empty() {
        "N/A".to_string()
    } else {
        r.capabilities.iter().take(5).map(|c| c.as_str()).collect::<Vec<_>>().join(", ")
    };
    let pipeline_str: String = r.pipeline_tag.as_deref().unwrap_or("N/A").to_string();
    let tag_str: String = r.tags.iter().take(3).cloned().collect::<Vec<_>>().join(", ");

    let mut lines = vec![
        Line::from(vec![
            Span::styled("Model: ", Style::default().fg(Color::Yellow)),
            Span::styled(r.model_id.clone(), Style::default().fg(Color::White)),
        ]),
    ];
    if let Some(s) = size_str.clone() {
        lines.push(Line::from(vec![
            Span::styled("Size: ", Style::default().fg(Color::Yellow)),
            Span::styled(s, Style::default().fg(Color::White)),
        ]));
    }
    lines.push(Line::from(vec![
        Span::styled("Params: ", Style::default().fg(Color::Yellow)),
        Span::styled(params_str.clone(), Style::default().fg(Color::White)),
        Span::raw(" | "),
        Span::styled("Type: ", Style::default().fg(Color::Yellow)),
        Span::styled(cap_str.clone(), Style::default().fg(Color::White)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Pipeline: ", Style::default().fg(Color::Yellow)),
        Span::styled(pipeline_str.to_string(), Style::default().fg(Color::White)),
        Span::raw(" | "),
        Span::styled("Downloads: ", Style::default().fg(Color::Yellow)),
        Span::styled(format!("{}", r.downloads), Style::default().fg(Color::White)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Likes: ", Style::default().fg(Color::Yellow)),
        Span::styled(format!("{}", r.likes), Style::default().fg(Color::White)),
        Span::raw(" | "),
        Span::styled("Trending: ", Style::default().fg(Color::Yellow)),
        Span::styled(format!("{}", r.trending_score), Style::default().fg(Color::White)),
    ]));
    let license: String = r.license.as_deref().unwrap_or("—").to_string();
    lines.push(Line::from(vec![
        Span::styled("License: ", Style::default().fg(Color::Yellow)),
        Span::styled(license, Style::default().fg(Color::White)),
    ]));
    if let Some(created) = &r.created_at {
        lines.push(Line::from(vec![
            Span::styled("Created: ", Style::default().fg(Color::Yellow)),
            Span::styled(created.clone(), Style::default().fg(Color::White)),
        ]));
    }
    lines.push(Line::from(vec![
        Span::styled("Tags: ", Style::default().fg(Color::Yellow)),
        Span::styled(tag_str, Style::default().fg(Color::White)),
    ]));
    lines
}
