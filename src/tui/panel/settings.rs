use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

/// Render the LLM Settings panel (Loading + GPU + Evaluation + Sampling + Repetition).
/// Returns (lines, total_count, settings_height, selected_line_idx).
pub fn render_all(settings: &crate::models::ModelSettings, cached: &crate::models::ModelSettings, selected: usize, edit_buf: &str, editing: bool, vram_mib: u64, total_layers: u32, _n_ctx_train: u32, _max_threads: u32) -> (Vec<Line<'static>>, usize, usize, usize) {
    let mut lines = Vec::new();
    let mut total_count = 0;
    let mut selected_line_idx = 0;

    // VRAM estimate header
    lines.push(Line::from(vec![
        Span::styled("VRAM estimate: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled(crate::models::format_mib(vram_mib), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
    ]));
    lines.push(Line::from(""));

    // ── Loading ──────────────────────────────────────────────
    lines.push(Line::from(vec![
        Span::styled("--- Loading ---", Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
    ]));

    let loading_names = vec!["Context", "Prompt", "Reasoning Mode"];
    let loading_vals = vec![
        format!("{}", settings.context_length),
        format!("{}", settings.system_prompt_preset_name),
        format!("{}", settings.reasoning_mode),
    ];

    for (i, val) in loading_vals.into_iter().enumerate() {
        if total_count == selected {
            selected_line_idx = lines.len();
        }
        add_setting(&mut lines, &mut total_count, settings, cached, &loading_names[i], &val, selected, edit_buf, editing);
    }
    // ── GPU Offload ──────────────────────────────────────────
    lines.push(Line::from(vec![
        Span::styled("--- GPU Offload ---", Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
    ]));

    let gpu_names = vec!["GPU Layers", "Flash Attention", "KV Cache Offload", "Cache Type K", "Cache Type V"];
    let gpu_vals = vec![
        if settings.gpu_layers < 0 {
            format!("all ({total_layers} layers)",)
        } else {
            format!("{} / {total_layers} layers", settings.gpu_layers)
        },
        format!("{}", settings.flash_attn),
        format!("{}", settings.kv_cache_offload),
        format!("{}", settings.cache_type_k),
        format!("{}", settings.cache_type_v),
    ];

    for (i, val) in gpu_vals.into_iter().enumerate() {
        if total_count == selected {
            selected_line_idx = lines.len();
        }
        add_setting(&mut lines, &mut total_count, settings, cached, &gpu_names[i], &val, selected, edit_buf, editing);
    }

    // ── Evaluation ───────────────────────────────────────────
    lines.push(Line::from(vec![
        Span::styled("--- Evaluation ---", Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
    ]));

    let eval_names = vec!["Eval Batch", "Unified KV"];
    let eval_vals = vec![
        format!("{}", settings.batch_size),
        format!("{}", settings.uniform_cache),
    ];

    for (i, val) in eval_vals.into_iter().enumerate() {
        if total_count == selected {
            selected_line_idx = lines.len();
        }
        add_setting(&mut lines, &mut total_count, settings, cached, &eval_names[i], &val, selected, edit_buf, editing);
    }

    // ── Sampling ─────────────────────────────────────────────
    lines.push(Line::from(vec![
        Span::styled("--- Sampling ---", Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
    ]));

    let sampling_names = vec!["Seed", "Temp", "Top-k", "Top-p", "Min P", "Max Tokens"];
    let sampling_vals = vec![
        format!("{}", settings.seed),
        format!("{:.2}", settings.temperature),
        format!("{}", settings.top_k),
        format!("{:.2}", settings.top_p),
        format!("{:.2}", settings.min_p),
        format!("{}", settings.max_tokens),
    ];

    for (i, val) in sampling_vals.into_iter().enumerate() {
        if total_count == selected {
            selected_line_idx = lines.len();
        }
        add_setting(&mut lines, &mut total_count, settings, cached, &sampling_names[i], &val, selected, edit_buf, editing);
    }

    // ── Repetition Control ───────────────────────────────────
    lines.push(Line::from(vec![
        Span::styled("--- Repetition ---", Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
    ]));

    let rep_names = vec!["Rep. Penalty", "Rep. Last N", "Presence", "Frequency"];
    let rep_vals = vec![
        format!("{:.2}", settings.repeat_penalty),
        format!("{}", settings.repeat_last_n),
        format!("{:.2}", settings.presence_penalty),
        format!("{:.2}", settings.frequency_penalty),
    ];

    for (i, val) in rep_vals.into_iter().enumerate() {
        if total_count == selected {
            selected_line_idx = lines.len();
        }
        add_setting(&mut lines, &mut total_count, settings, cached, &rep_names[i], &val, selected, edit_buf, editing);
    }

    let height = lines.len();
    (lines, total_count, height, selected_line_idx)
}

pub fn add_setting(lines: &mut Vec<Line<'static>>, total_count: &mut usize, settings: &crate::models::ModelSettings, cached: &crate::models::ModelSettings, name: &str, val: &str, selected: usize, edit_buf: &str, editing: bool) {
    let current_idx = *total_count;
    let marker = if current_idx == selected { "> " } else { "  " };
    let name_style = Style::default().fg(Color::Yellow);

    // Compute dirty flag from current_idx into the dirty array
    let dirty = match current_idx {
        0 => settings.context_length != cached.context_length,
        1 => settings.system_prompt_preset_name != cached.system_prompt_preset_name,
        2 => settings.reasoning_mode != cached.reasoning_mode,
        3 => settings.gpu_layers != cached.gpu_layers,
        4 => settings.flash_attn != cached.flash_attn,
        5 => settings.kv_cache_offload != cached.kv_cache_offload,
        6 => settings.cache_type_k != cached.cache_type_k,
        7 => settings.cache_type_v != cached.cache_type_v,
        8 => settings.batch_size != cached.batch_size,
        9 => settings.uniform_cache != cached.uniform_cache,
        10 => settings.seed != cached.seed,
        11 => (settings.temperature - cached.temperature).abs() > 0.001,
        12 => settings.top_k != cached.top_k,
        13 => (settings.top_p - cached.top_p).abs() > 0.001,
        14 => (settings.min_p - cached.min_p).abs() > 0.001,
        15 => settings.max_tokens != cached.max_tokens,
        16 => (settings.repeat_penalty - cached.repeat_penalty).abs() > 0.001,
        17 => settings.repeat_last_n != cached.repeat_last_n,
        18 => (settings.presence_penalty - cached.presence_penalty).abs() > 0.001,
        19 => (settings.frequency_penalty - cached.frequency_penalty).abs() > 0.001,
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
