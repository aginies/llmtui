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
use crate::tui::app::App;

const SERVER_SETTINGS_HEIGHT: u16 = 5;

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
    let (settings_lines, count, settings_height, selected_line_idx) = settings::render_all(
        &app.settings,
        &app.model_settings_cache,
        app.settings_selected_idx,
        &app.settings_edit_buffer,
        !app.settings_edit_buffer.is_empty(),
        app.vram_estimate,
        app.model_total_layers,
        app.model_n_ctx_train,
        app.max_threads,
    );
    
    // Ensure selection stays in bounds
    if app.settings_selected_idx >= count {
        app.settings_selected_idx = count.saturating_sub(1);
    }
    
    let available_height = llm_area.height.saturating_sub(2);
    
    // Clamp scroll so selected item is within the visible window.
    if selected_line_idx < (app.settings_scroll_offset as usize) {
        app.settings_scroll_offset = selected_line_idx as u16;
    } else if available_height > 0 && (selected_line_idx - app.settings_scroll_offset as usize) >= (available_height as usize) {
        app.settings_scroll_offset = (selected_line_idx as u16).saturating_sub(available_height).saturating_add(1);
    }

    // Clamp scroll offset to max
    let max_offset = settings_height.saturating_sub(available_height as usize) as u16;
    if app.settings_scroll_offset > max_offset {
        app.settings_scroll_offset = max_offset;
    }
    
    // Build visible settings lines with scroll offset applied
    let start_idx = app.settings_scroll_offset as usize;
    let visible_lines: Vec<Line<'static>> = settings_lines
        .iter()
        .skip(start_idx)
        .take(available_height as usize)
        .cloned()
        .collect();

    let border_color = if is_focused { Color::Green } else { Color::Rgb(255, 165, 0) };
    let block = Block::default()
        .title(" LLM Settings ")
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

fn render_server_settings(f: &mut Frame, area: Rect, app: &App) {
    if area.height < 2 || area.width < 10 {
        return;
    }

    let is_focused = app.active_panel == crate::tui::app::ActivePanel::ServerSettings;
    let border_color = if is_focused { Color::Green } else { Color::Rgb(255, 165, 0) };
    let selected = app.server_settings_selected_idx;

    let host_val = if app.settings.host.is_empty() {
        "localhost (127.0.0.1)"
    } else if app.settings.host == "127.0.0.1" {
        "localhost (127.0.0.1)"
    } else {
        &app.settings.host
    };

    let backend_name = format!("{}", app.settings.backend);
    let parallel_val = format!("{}", app.settings.parallel);

    let mut lines = Vec::new();
    let mut count = 0;
    settings::add_setting(&mut lines, &mut count, &app.settings, &app.settings, "Host", &host_val, selected, "", false);
    settings::add_setting(&mut lines, &mut count, &app.settings, &app.settings, "Backend", &backend_name, selected, "", false);
    settings::add_setting(&mut lines, &mut count, &app.settings, &app.settings, "Parallel", &parallel_val, selected, "", false);
    lines.push(Line::from(vec![
        Span::styled("  (Enter/Arrows to change)", Style::default().fg(Color::DarkGray)),
    ]));

    let block = Block::default()
        .title(" Server Settings ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let paragraph = Paragraph::new(lines).block(block);
    f.render_widget(paragraph, area);
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
            if let Some(idx) = selected_idx {
                if let Some((filename, _, _url)) = files.get(*idx) {
                    lines.push(Line::from(vec![
                        Span::styled("File: ", Style::default().fg(Color::Yellow)),
                        Span::styled(filename.clone(), Style::default().fg(Color::White)),
                    ]));
                }
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
        if let Some(idx) = app.search_results_idx {
            if idx < results.len() {
                let r = &results[idx];
                let link_line = Line::from(vec![
                    Span::styled("  https://huggingface.co/", Style::default().fg(Color::DarkGray)),
                    Span::styled(r.model_id.clone(), Style::default().fg(Color::DarkGray)),
                ]);
                info_lines.push(link_line);
            }
        }
    } else if let crate::tui::app::ModelsMode::Files { selected_result, .. } = &app.models_mode {
        if let Some(r) = selected_result {
            let link_line = Line::from(vec![
                Span::styled("  https://huggingface.co/", Style::default().fg(Color::DarkGray)),
                Span::styled(r.model_id.clone(), Style::default().fg(Color::DarkGray)),
            ]);
            info_lines.push(link_line);
        }
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
        .title(" Model Info ")
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
    let stem = filename
        .rsplit('/')
        .next()
        .unwrap_or(filename)
        .trim_end_matches(".gguf")
        .trim_end_matches(".GGUF");
    
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
    let size_str = file_info.as_ref().map(|(_, size)| {
        let gb = *size as f64 / (1024.0 * 1024.0 * 1024.0);
        if *size < 1024 * 1024 {
            format!("{:.1} MB", *size as f64 / 1_000_000.0)
        } else {
            format!("{:.1} GB", gb)
        }
    }).or_else(|| r.size.map(|s| {
        let gb = s as f64 / (1024.0 * 1024.0 * 1024.0);
        if s < 1024 * 1024 {
            format!("{:.1} MB", s as f64 / 1_000_000.0)
        } else {
            format!("{:.1} GB", gb)
        }
    }));
    
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
    let pipeline_str = r.pipeline_tag.as_deref().unwrap_or("N/A");
    let tag_str = r.tags.iter().take(3).map(|t| t.as_str()).collect::<Vec<_>>().join(", ");

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
        Span::styled("Tags: ", Style::default().fg(Color::Yellow)),
        Span::styled(tag_str, Style::default().fg(Color::White)),
    ]));
    lines
}
