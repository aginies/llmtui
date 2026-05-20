use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

/// Render the LLM Settings panel (Loading + GPU + Evaluation + Sampling + Repetition).
/// Returns (lines, total_count, settings_height, selected_line_idx).
#[allow(clippy::too_many_arguments)]
pub fn render_all(app: &mut crate::tui::app::App, area: Rect) -> (Vec<Line<'static>>, usize, usize, usize) {
    let settings = &app.settings;
    let cached = &app.model_settings_cache;
    let selected = app.settings_selected_idx;
    let edit_buf = &app.settings_edit_buffer;
    let editing = !edit_buf.is_empty();
    let hash = app.settings_fingerprint();

    // Cache hit: return a clone of the cached lines.
    if let Some(c) = &app.settings_render_cache
        && c.hash == hash
        && c.selected == selected {
            // Even on cache hit, we need to update scrolling if area changed
            let available_height = area.height.saturating_sub(2);
            let selected_line_idx = c.selected_line_idx;
            let settings_height = c.lines.len();

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

            return (c.lines.clone(), 22, settings_height, selected_line_idx);
        }

    let mut lines = Vec::new();
    let mut total_count = 0;
    let mut selected_line_idx = 0;

    // ── Loading ──────────────────────────────────────────────
    lines.push(Line::from(vec![
        Span::styled("--- Loading ---", Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
    ]));

    let loading_names = ["Context", "Prompt", "Keep in memory (mlock)"];
    let loading_vals = vec![
        format!("{}", settings.context_length),
        format!("{}", settings.system_prompt_preset_name),
        format!("{}", settings.mlock),
    ];

    for (i, val) in loading_vals.into_iter().enumerate() {
        if total_count == selected {
            selected_line_idx = lines.len();
        }
        add_setting(&mut lines, &mut total_count, settings, cached, loading_names[i], &val, selected, edit_buf, editing);
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
        if total_count == selected {
            selected_line_idx = lines.len();
        }
        add_setting(&mut lines, &mut total_count, settings, cached, gpu_names[i], &val, selected, edit_buf, editing);
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
        if total_count == selected {
            selected_line_idx = lines.len();
        }
        add_setting(&mut lines, &mut total_count, settings, cached, eval_names[i], &val, selected, edit_buf, editing);
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
        if total_count == selected {
            selected_line_idx = lines.len();
        }
        add_setting(&mut lines, &mut total_count, settings, cached, sampling_names[i], &val, selected, edit_buf, editing);
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
        if total_count == selected {
            selected_line_idx = lines.len();
        }
        add_setting(&mut lines, &mut total_count, settings, cached, rep_names[i], &val, selected, edit_buf, editing);
    }

    // ── Backend ──────────────────────────────────────────────
    lines.push(Line::from(vec![
        Span::styled("--- Backend ---", Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
    ]));

    let backend_names = ["LLama.cpp Version"];
    let backend_vals = vec![
        match settings.backend {
            crate::models::Backend::Cpu => settings.llama_cpp_version_cpu.as_deref().unwrap_or("latest"),
            crate::models::Backend::Vulkan => settings.llama_cpp_version_vulkan.as_deref().unwrap_or("latest"),
            crate::models::Backend::Rocm => settings.llama_cpp_version_rocm.as_deref().unwrap_or("latest"),
            crate::models::Backend::RocmLemonade => settings.llama_cpp_version_rocm_lemonade.as_deref().unwrap_or("latest"),
            crate::models::Backend::Cuda => settings.llama_cpp_version_cuda.as_deref().unwrap_or("latest"),
        }.to_string(),
    ];

    for (i, val) in backend_vals.into_iter().enumerate() {
        if total_count == selected {
            selected_line_idx = lines.len();
        }
        add_setting(&mut lines, &mut total_count, settings, cached, backend_names[i], &val, selected, edit_buf, editing);
    }

    let settings_height = lines.len();
    
    // Update cache
    app.settings_render_cache = Some(crate::tui::app::SettingsRenderCache {
        hash,
        selected,
        selected_line_idx,
        lines: lines.clone(),
    });

    // Handle scrolling
    let available_height = area.height.saturating_sub(2);
    if selected_line_idx < (app.settings_scroll_offset as usize) {
        app.settings_scroll_offset = selected_line_idx as u16;
    } else if available_height > 0 && (selected_line_idx - app.settings_scroll_offset as usize) >= (available_height as usize) {
        app.settings_scroll_offset = (selected_line_idx as u16).saturating_sub(available_height).saturating_add(1);
    }

    let max_offset = settings_height.saturating_sub(available_height as usize) as u16;
    if app.settings_scroll_offset > max_offset {
        app.settings_scroll_offset = max_offset;
    }

    (lines, total_count, settings_height, selected_line_idx)
}

#[allow(clippy::too_many_arguments)]
pub fn add_setting(lines: &mut Vec<Line<'static>>, total_count: &mut usize, settings: &crate::models::ModelSettings, cached: &crate::models::ModelSettings, name: &str, val: &str, selected: usize, edit_buf: &str, editing: bool, ) {
    let current_idx = *total_count;
    let marker = if current_idx == selected { "> " } else { "  " };
    let name_style = Style::default().fg(Color::Yellow);

    // Compute dirty flag from current_idx into the dirty array
    let dirty = match current_idx {
        0 => settings.context_length != cached.context_length,
        1 => settings.system_prompt_preset_name != cached.system_prompt_preset_name,
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
        22 => match settings.backend {
            crate::models::Backend::Cpu => settings.llama_cpp_version_cpu != cached.llama_cpp_version_cpu,
            crate::models::Backend::Vulkan => settings.llama_cpp_version_vulkan != cached.llama_cpp_version_vulkan,
            crate::models::Backend::Rocm => settings.llama_cpp_version_rocm != cached.llama_cpp_version_rocm,
            crate::models::Backend::RocmLemonade => settings.llama_cpp_version_rocm_lemonade != cached.llama_cpp_version_rocm_lemonade,
            crate::models::Backend::Cuda => settings.llama_cpp_version_cuda != cached.llama_cpp_version_cuda,
        },
        _ => false,
    };

    let (display_val, val_style) = if current_idx == selected && editing {
        (edit_buf.to_string(), Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD))
    } else if current_idx == selected {
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
