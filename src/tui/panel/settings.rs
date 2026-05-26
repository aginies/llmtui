use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

/// Render the LLM Settings panel.
#[allow(clippy::too_many_arguments)]
pub fn render_all(app: &mut crate::tui::app::App, area: Rect) -> (Vec<Line<'static>>, usize, usize, usize) {
    let settings = &app.settings;
    let cached = &app.model_settings_cache;
    let selected = app.settings_state.settings_selected_idx;

    let mut selected_content_line = 0;
    let mut total_count = 0;

    let edit_buf = &app.settings_state.settings_edit_buffer;
    let editing = !edit_buf.is_empty();
    let hash = app.settings_fingerprint();

    // Cache hit with same selection: track cached total_count, but render_settings
    // always runs after (setting selected_content_line correctly).
    let mut cache_hit_total_count = 0;
    if let Some(c) = &app.settings_state.settings_render_cache
        && c.hash == hash
        && c.selected == selected
    {
        cache_hit_total_count = c.lines.len();
    }

    // Build lines -- render_settings sets selected_content_line for the current selection.
    // Always runs, regardless of cache hit state.
    let mut lines = Vec::new();
    let mut selected_line_idx = 0;
    render_settings(
        &mut lines, &mut total_count, &mut selected_line_idx, &mut selected_content_line,
        settings, cached, selected, edit_buf, editing,
    );

    // On cache hit, use cached lines (faster). On miss, update cache.
    let (lines_to_return, final_total_count) = if let Some(c) = &app.settings_state.settings_render_cache
        && c.hash == hash
        && c.selected == selected
    {
        // Cache hit: use cached total_count
        (c.lines.clone(), cache_hit_total_count.max(total_count))
    } else {
        // Cache miss: store and use built lines
        app.settings_state.settings_render_cache = Some(crate::tui::app::SettingsRenderCache {
            hash,
            selected,
            lines: lines.clone(),
        });
        (lines, total_count)
    };

    let settings_height = lines_to_return.len();

    // Scroll clamp (always executes)
    let available_height = area.height.saturating_sub(2);
    if selected_content_line < app.settings_state.settings_scroll_offset {
        app.settings_state.settings_scroll_offset = selected_content_line;
    } else if available_height > 0 && (selected_content_line - app.settings_state.settings_scroll_offset) >= (available_height as usize) {
        app.settings_state.settings_scroll_offset = (selected_content_line).saturating_sub(available_height as usize).saturating_add(1);
    }
    let max_offset = settings_height.saturating_sub(available_height as usize);
    if app.settings_state.settings_scroll_offset > max_offset {
        app.settings_state.settings_scroll_offset = max_offset;
    }

    (lines_to_return, final_total_count, settings_height, selected_content_line)
}

#[allow(clippy::too_many_arguments)]
fn render_settings(
    lines: &mut Vec<Line<'static>>,
    total_count: &mut usize,
    selected_line_idx: &mut usize,
    selected_content_line: &mut usize,
    settings: &crate::models::ModelSettings,
    cached: &crate::models::ModelSettings,
    selected: usize,
    edit_buf: &str,
    editing: bool,
) {
    // ── Loading ──────────────────────────────────────────────
    lines.push(Line::from(vec![
        Span::styled("--- Loading ---", Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
    ]));

    let loading_names = ["Prompt", "Context", "Keep in memory (mlock)"];
    let loading_vals = vec![
        format!("{}", settings.system_prompt_preset_name),
        format!("{}", settings.context_length),
        format!("{}", settings.mlock),
    ];

    for (i, val) in loading_vals.into_iter().enumerate() {
        if *total_count == selected {
            *selected_line_idx = lines.len();
        }
        add_setting(lines, total_count, settings, cached, selected_line_idx, selected_content_line, i as usize, loading_names[i], &val, selected, edit_buf, editing);
    }

    // ── GPU Offload ──────────────────────────────────────────
    lines.push(Line::from(vec![
        Span::styled("--- GPU Offload ---", Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
    ]));

    let gpu_names = ["GPU Layers", "Flash Attention", "KV Cache Offload", "Cache Type K", "Cache Type V", "Active Experts"];
    let gpu_vals = vec![
        format!("{}", match settings.gpu_layers_mode {
            crate::models::GpuLayersMode::Auto => "Auto".to_string(),
            crate::models::GpuLayersMode::Specific(n) => n.to_string(),
            crate::models::GpuLayersMode::All => "All".to_string(),
        }),
        format!("{}", settings.flash_attn),
        format!("{}", settings.kv_cache_offload),
        settings.cache_type_k.map(|v| v.to_string()).unwrap_or_else(|| "Disabled".to_string()),
        settings.cache_type_v.map(|v| v.to_string()).unwrap_or_else(|| "Disabled".to_string()),
        if settings.expert_count > 0 {
            format!("{}", settings.expert_count)
        } else {
            "Disabled".to_string()
        },
    ];

    for (i, val) in gpu_vals.into_iter().enumerate() {
        if *total_count == selected {
            *selected_line_idx = lines.len();
        }
        add_setting(lines, total_count, settings, cached, selected_line_idx, selected_content_line, 3 + i as usize, gpu_names[i], &val, selected, edit_buf, editing);
    }

    // ── Evaluation ───────────────────────────────────────────
    lines.push(Line::from(vec![
        Span::styled("--- Evaluation ---", Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
    ]));

    let eval_names = ["Eval Batch", "Unified KV", "Max Concurrent Pred"];
    let eval_vals = vec![
        format!("{}", settings.batch_size),
        format!("{}", settings.uniform_cache),
        settings.max_concurrent_predictions.map(|v| v.to_string()).unwrap_or_else(|| "Off".to_string()),
    ];

    for (i, val) in eval_vals.into_iter().enumerate() {
        if *total_count == selected {
            *selected_line_idx = lines.len();
        }
        add_setting(lines, total_count, settings, cached, selected_line_idx, selected_content_line, 9 + i as usize, eval_names[i], &val, selected, edit_buf, editing);
    }

    // ── Sampling ─────────────────────────────────────────────
    lines.push(Line::from(vec![
        Span::styled("--- Sampling ---", Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
    ]));

    let sampling_names = ["Seed", "Temp", "Top-k", "Top-p", "Min P", "Max Tokens"];
    let sampling_vals = vec![
        format!("{}", settings.seed),
        format!("{:.2}", settings.temperature),
        format!("{}", settings.top_k),
        format!("{:.2}", settings.top_p),
        format!("{:.2}", settings.min_p),
        settings.max_tokens.map(|v| v.to_string()).unwrap_or_else(|| "Disabled".to_string()),
    ];

    for (i, val) in sampling_vals.into_iter().enumerate() {
        if *total_count == selected {
            *selected_line_idx = lines.len();
        }
        add_setting(lines, total_count, settings, cached, selected_line_idx, selected_content_line, 12 + i as usize, sampling_names[i], &val, selected, edit_buf, editing);
    }

    // ── Repetition ───────────────────────────────────────────
    lines.push(Line::from(vec![
        Span::styled("--- Repetition ---", Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
    ]));

    let rep_names = ["Repeat Penalty", "Repeat Last N", "Presence Penalty", "Freq Penalty"];
    let rep_vals = vec![
        format!("{:.2}", settings.repeat_penalty),
        format!("{}", settings.repeat_last_n),
        settings.presence_penalty.map(|v| format!("{:.2}", v)).unwrap_or_else(|| "Off".to_string()),
        settings.frequency_penalty.map(|v| format!("{:.2}", v)).unwrap_or_else(|| "Off".to_string()),
    ];

    for (i, val) in rep_vals.into_iter().enumerate() {
        if *total_count == selected {
            *selected_line_idx = lines.len();
        }
        add_setting(lines, total_count, settings, cached, selected_line_idx, selected_content_line, 18 + i as usize, rep_names[i], &val, selected, edit_buf, editing);
    }

    // ── Tags ─────────────────────────────────────────────────
    lines.push(Line::from(vec![
        Span::styled("--- Tags ---", Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
    ]));

    let tags_val = if settings.tags.is_empty() {
        "None".to_string()
    } else {
        settings.tags.join(", ")
    };
    if *total_count == selected {
        *selected_line_idx = lines.len();
    }
    add_setting(lines, total_count, settings, cached, selected_line_idx, selected_content_line, 22, "Tags (Enter to edit)", &tags_val, selected, edit_buf, editing);

    // ── Backend ──────────────────────────────────────────────
    lines.push(Line::from(vec![
        Span::styled("--- Backend ---", Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
    ]));

    let backend_vals = vec![settings.get_active_backend_version_display().to_string()];
    for i in 0..backend_vals.len() {
        if *total_count == selected {
            *selected_line_idx = lines.len();
        }
        add_setting(lines, total_count, settings, cached, selected_line_idx, selected_content_line, 23 + i as usize, "LLama.cpp Version", &backend_vals[i], selected, edit_buf, editing);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn add_setting(
    lines: &mut Vec<Line<'static>>,
    total_count: &mut usize,
    settings: &crate::models::ModelSettings,
    cached: &crate::models::ModelSettings,
    selected_line_idx: &mut usize,
    selected_content_line: &mut usize,
    idx: usize,
    name: &str,
    val: &str,
    selected: usize,
    edit_buf: &str,
    editing: bool,
) {
    let current_line = lines.len();
    let marker = if idx == selected { "> " } else { "  " };
    let name_style = Style::default().fg(Color::Yellow);

    if idx == selected {
        *selected_line_idx = current_line;
        *selected_content_line = current_line;
    }

    let dirty = match idx {
        0 => settings.system_prompt_preset_name != cached.system_prompt_preset_name,
        1 => settings.context_length != cached.context_length,
        2 => settings.mlock != cached.mlock,
        3 => settings.gpu_layers_mode != cached.gpu_layers_mode,
        4 => settings.flash_attn != cached.flash_attn,
        5 => settings.kv_cache_offload != cached.kv_cache_offload,
        6 => settings.cache_type_k != cached.cache_type_k,
        7 => settings.cache_type_v != cached.cache_type_v,
        8 => settings.expert_count != cached.expert_count,
        9 => settings.batch_size != cached.batch_size,
        10 => settings.uniform_cache != cached.uniform_cache,
        11 => settings.max_concurrent_predictions != cached.max_concurrent_predictions,
        12 => settings.seed != cached.seed,
        13 => (settings.temperature - cached.temperature).abs() > 0.001,
        14 => settings.top_k != cached.top_k,
        15 => (settings.top_p - cached.top_p).abs() > 0.001,
        16 => (settings.min_p - cached.min_p).abs() > 0.001,
        17 => settings.max_tokens != cached.max_tokens,
        18 => (settings.repeat_penalty - cached.repeat_penalty).abs() > 0.001,
        19 => settings.repeat_last_n != cached.repeat_last_n,
        20 => match (settings.presence_penalty, cached.presence_penalty) {
            (Some(v1), Some(v2)) => (v1 - v2).abs() > 0.001,
            (None, None) => false,
            _ => true,
        },
        21 => match (settings.frequency_penalty, cached.frequency_penalty) {
            (Some(v1), Some(v2)) => (v1 - v2).abs() > 0.001,
            (None, None) => false,
            _ => true,
        },
        22 => settings.tags != cached.tags,
        23 => settings.get_active_backend_version() != cached.get_active_backend_version(),
        _ => settings.is_dirty(cached),
    };

    let (display_val, val_style) = if idx == selected && editing {
        (edit_buf.to_string(), Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD))
    } else if idx == selected {
        (val.to_string(), Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD))
    } else if dirty {
        (format!("{}*", val), Style::default().fg(Color::Red))
    } else {
        (val.to_string(), Style::default().fg(Color::White))
    };

    lines.push(Line::from(vec![
        Span::styled(marker, Style::default().fg(Color::Yellow)),
        Span::styled(format!("{name}: "), name_style),
        Span::styled(display_val, val_style),
    ]));
    *total_count += 1;
}
