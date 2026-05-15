use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

/// Render the LLM Settings panel (Loading + GPU + Evaluation + Sampling + Repetition).
/// Returns (lines, total_count, settings_height, selected_line_idx).
pub fn render_all(settings: &crate::models::ModelSettings, cached: &crate::models::ModelSettings, selected: usize, edit_buf: &str, editing: bool, _vram_mib: u64, total_layers: u32, _n_ctx_train: u32, _max_threads: u32, on_version_change: Option<&dyn Fn(&crate::models::ModelSettings)>) -> (Vec<Line<'static>>, usize, usize, usize) {
    let mut lines = Vec::new();
    let mut total_count = 0;
    let mut selected_line_idx = 0;

    // ── Loading ──────────────────────────────────────────────
    lines.push(Line::from(vec![
        Span::styled("--- Loading ---", Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
    ]));

    let loading_names = vec!["Context", "Prompt", "Keep in memory (mlock)"];
    let loading_vals = vec![
        format!("{}", settings.context_length),
        format!("{}", settings.system_prompt_preset_name),
        format!("{}", settings.mlock),
    ];

    for (i, val) in loading_vals.into_iter().enumerate() {
        if total_count == selected {
            selected_line_idx = lines.len();
        }
        add_setting(&mut lines, &mut total_count, settings, cached, &loading_names[i], &val, selected, edit_buf, editing, on_version_change);
    }
    // ── GPU Offload ──────────────────────────────────────────
    lines.push(Line::from(vec![
        Span::styled("--- GPU Offload ---", Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
    ]));

    let gpu_names = vec!["GPU Layers", "Flash Attention", "KV Cache Offload", "Cache Type K", "Cache Type V", "Active Experts"];
    let gpu_vals = vec![
        if settings.gpu_layers < 0 {
            format!("all ({total_layers} layers)",)
        } else {
            format!("{} / {total_layers} layers", settings.gpu_layers)
        },
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
        add_setting(&mut lines, &mut total_count, settings, cached, &gpu_names[i], &val, selected, edit_buf, editing, on_version_change);
    }

    // ── Evaluation ───────────────────────────────────────────
    lines.push(Line::from(vec![
        Span::styled("--- Evaluation ---", Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
    ]));

    let eval_names = vec!["Eval Batch", "Unified KV", "Max Concurrent Pred"];
    let eval_vals = vec![
        format!("{}", settings.batch_size),
        format!("{}", settings.uniform_cache),
        format!("{}", settings.max_concurrent_predictions),
    ];

    for (i, val) in eval_vals.into_iter().enumerate() {
        if total_count == selected {
            selected_line_idx = lines.len();
        }
        add_setting(&mut lines, &mut total_count, settings, cached, &eval_names[i], &val, selected, edit_buf, editing, on_version_change);
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
        settings.max_tokens.map(|v| v.to_string()).unwrap_or_else(|| "Disabled".to_string()),
    ];

    for (i, val) in sampling_vals.into_iter().enumerate() {
        if total_count == selected {
            selected_line_idx = lines.len();
        }
        add_setting(&mut lines, &mut total_count, settings, cached, &sampling_names[i], &val, selected, edit_buf, editing, on_version_change);
    }

    // ── Server ───────────────────────────────────────────────
    lines.push(Line::from(vec![
        Span::styled("--- Server ---", Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
    ]));

    let server_names = vec!["API Key"];
    let server_vals = vec![
        match &settings.server_api_key {
            Some(key) if !key.is_empty() => "********",
            _ => "None",
        },
    ];

    for (i, val) in server_vals.into_iter().enumerate() {
        if total_count == selected {
            selected_line_idx = lines.len();
        }
        add_setting(&mut lines, &mut total_count, settings, cached, &server_names[i], &val, selected, edit_buf, editing, on_version_change);
    }

    // ── Backend ──────────────────────────────────────────────
    lines.push(Line::from(vec![
        Span::styled("--- Backend ---", Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
    ]));

    let backend_names = vec!["LLama.cpp Version"];
    let cpu_ver = settings.llama_cpp_version_cpu.as_deref().unwrap_or("latest");
    let vulk_ver = settings.llama_cpp_version_vulkan.as_deref().unwrap_or("latest");
    let rocm_ver = settings.llama_cpp_version_rocm.as_deref().unwrap_or("latest");
    let backend_vals = vec![
        format!("CPU {} | Vulkan {} | ROCm {}", cpu_ver, vulk_ver, rocm_ver),
    ];

    for (i, val) in backend_vals.into_iter().enumerate() {
        if total_count == selected {
            selected_line_idx = lines.len();
        }
        add_setting(&mut lines, &mut total_count, settings, cached, &backend_names[i], &val, selected, edit_buf, editing, on_version_change);
    }

    let height = lines.len();
    (lines, total_count, height, selected_line_idx)
}

pub fn add_setting(lines: &mut Vec<Line<'static>>, total_count: &mut usize, settings: &crate::models::ModelSettings, cached: &crate::models::ModelSettings, name: &str, val: &str, selected: usize, edit_buf: &str, editing: bool, on_version_change: Option<&dyn Fn(&crate::models::ModelSettings)>) {
    let current_idx = *total_count;
    let marker = if current_idx == selected { "> " } else { "  " };
    let name_style = Style::default().fg(Color::Yellow);

    // Compute dirty flag from current_idx into the dirty array
    let dirty = match current_idx {
        0 => settings.context_length != cached.context_length,
        1 => settings.system_prompt_preset_name != cached.system_prompt_preset_name,
        2 => settings.mlock != cached.mlock,
        3 => settings.gpu_layers != cached.gpu_layers,
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
        22 => settings.llama_cpp_version_cpu != cached.llama_cpp_version_cpu || settings.llama_cpp_version_vulkan != cached.llama_cpp_version_vulkan || settings.llama_cpp_version_rocm != cached.llama_cpp_version_rocm,
        23 => settings.server_api_key != cached.server_api_key,
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

    // If version field and we have a callback, apply it
    if current_idx == 22 {
        if let Some(callback) = on_version_change {
            callback(settings);
        }
    }

    lines.push(Line::from(vec![
        Span::styled(marker, Style::default().fg(Color::Yellow)),
        Span::styled(format!("{name}: "), name_style),
        Span::styled(display_val, val_style),
    ]));
    *total_count += 1;
}
