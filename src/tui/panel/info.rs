use ratatui::style::Color;

/// A single key-value pair for model info, rendered in two columns.
#[derive(Clone, Debug)]
pub struct ModelInfoPair {
    pub label: String,
    pub value: String,
    pub value_style: Color,
}

/// Compute the maximum context length that can fit within the given VRAM budget.
///
/// This mirrors the KV cache formula from `estimate_vram_mib` but solves
/// backwards for context length given a fixed VRAM budget.
pub fn max_context_for_vram(
    model_mib: u64,
    vram_mib: u64,
    total_layers: u32,
    hidden_size: u32,
    n_head: u32,
    n_kv_head: u32,
    gpu_layers: i32,
    flash_attn: bool,
    uniform_cache: bool,
    parallel: u32,
    cache_type_k: &str,
    cache_type_v: &str,
) -> u32 {
    let model_mib_f = model_mib as f64;
    let vram_f = vram_mib as f64;

    // How much of the model is loaded into VRAM based on GPU layers.
    let gpu_layers = if gpu_layers < 0 {
        if total_layers > 0 { total_layers as f64 } else { 32.0 }
    } else {
        gpu_layers.unsigned_abs() as f64
    };

    let model_vram = if total_layers > 0 && gpu_layers > 0.0 {
        model_mib_f * (gpu_layers / total_layers as f64).min(1.0)
    } else if gpu_layers > 0.0 {
        model_mib_f
    } else {
        0.0
    };

    // VRAM budget for KV cache
    let kv_budget = vram_f - model_vram;
    if kv_budget <= 0.0 {
        return 0;
    }

    // GQA ratio
    let gqa_ratio = if n_head > 0 {
        n_kv_head as f64 / n_head as f64
    } else {
        1.0
    };

    let flash_attn_factor = if flash_attn { 0.5 } else { 1.0 };

    let parallel = if parallel > 0 { parallel as f64 } else { 1.0 };
    let uniform_cache_factor = if uniform_cache { 1.0 / parallel } else { 1.0 };

    // KV quant factor (relative to f16 = 2 bytes)
    let kv_quant_factor = kv_quant_bytes(cache_type_k, cache_type_v) / 2.0;

    // KV cache per token per layer: 2 * hidden * 2 * gqa_ratio * flash * uniform * quant
    let kv_per_token = 2.0 * hidden_size as f64 * 2.0 * gqa_ratio * flash_attn_factor * uniform_cache_factor * kv_quant_factor;

    // Total KV budget = kv_per_token * ctx * gpu_layers
    // ctx = kv_budget / (kv_per_token * gpu_layers)
    if gpu_layers > 0.0 && kv_per_token > 0.0 {
        let ctx = kv_budget / (kv_per_token * gpu_layers);
        ctx as u32
    } else {
        0
    }
}

fn kv_quant_bytes(cache_type_k: &str, cache_type_v: &str) -> f64 {
    let k_bytes = match cache_type_k {
        "F32" => 4.0,
        "F16" | "BF16" => 2.0,
        "Q8_0" => 1.0,
        "Q5_0" | "Q5_1" => 0.625,
        "Q4_0" | "Q4_1" => 0.5,
        "Iq4Nl" => 0.5,
        _ => 2.0,
    };
    let v_bytes = match cache_type_v {
        "F32" => 4.0,
        "F16" | "BF16" => 2.0,
        "Q8_0" => 1.0,
        "Q5_0" | "Q5_1" => 0.625,
        "Q4_0" | "Q4_1" => 0.5,
        "Iq4Nl" => 0.5,
        _ => 2.0,
    };
    (k_bytes + v_bytes) / 2.0
}

/// Render model metadata as a list of (label, value) pairs.
///
/// Returns pairs suitable for 2-column rendering. The first pair (path)
/// spans the full width since it can be very long.
///
/// If `cached` is provided, the GGUF file is not re-parsed — the cached
/// metadata is used instead. This avoids expensive file I/O when switching
/// between models that have already been viewed.
pub fn render_model_lines(
    model: &crate::models::DiscoveredModel,
    cached: Option<&crate::models::GgufMetadata>,
    vram_mib: u64,
    settings: &crate::models::ModelSettings,
    gpu_mem_total_mib: u64,
) -> Vec<ModelInfoPair> {
    let mut pairs: Vec<ModelInfoPair> = Vec::new();

    let path = model.path.to_string_lossy().to_string();
    let size = format_size(model.file_size);

    pairs.push(ModelInfoPair {
        label: "Path".to_string(),
        value: path,
        value_style: Color::White,
    });

    pairs.push(ModelInfoPair {
        label: "Size".to_string(),
        value: size,
        value_style: Color::White,
    });

    // Use cached metadata if available, otherwise just show basic info.
    // Parsing GGUF here would block the render loop.
    if let Some(meta) = cached {
        if !meta.arch.is_empty() {
            pairs.push(ModelInfoPair {
                label: "Arch".to_string(),
                value: meta.arch.clone(),
                value_style: Color::Cyan,
            });
        }

        if !meta.domain.is_empty() {
            pairs.push(ModelInfoPair {
                label: "Domain".to_string(),
                value: meta.domain.clone(),
                value_style: Color::White,
            });
        }

        if !meta.capabilities.is_empty() {
            pairs.push(ModelInfoPair {
                label: "Capabilities".to_string(),
                value: meta.capabilities.join(", "),
                value_style: Color::Green,
            });
        }

        if !meta.quantization.is_empty() {
            pairs.push(ModelInfoPair {
                label: "Quant".to_string(),
                value: meta.quantization.clone(),
                value_style: Color::Cyan,
            });
        }

        if !meta.file_type.is_empty() {
            pairs.push(ModelInfoPair {
                label: "Format".to_string(),
                value: meta.file_type.clone(),
                value_style: Color::White,
            });
        }

        if !meta.model_parameters.is_empty() {
            pairs.push(ModelInfoPair {
                label: "Parameters".to_string(),
                value: meta.model_parameters.clone(),
                value_style: Color::White,
            });
        }

        if !meta.tokenizer.is_empty() {
            pairs.push(ModelInfoPair {
                label: "Tokenizer".to_string(),
                value: meta.tokenizer.clone(),
                value_style: Color::Cyan,
            });
        }

        if meta.vocab_size > 0 {
            pairs.push(ModelInfoPair {
                label: "Vocab".to_string(),
                value: format!("{} tokens", meta.vocab_size),
                value_style: Color::White,
            });
        }

        // Show n_ctx_train from GGUF as "Context".
        if meta.n_ctx_train > 0 {
            pairs.push(ModelInfoPair {
                label: "Context".to_string(),
                value: format!("{} tokens", meta.n_ctx_train),
                value_style: Color::White,
            });
        }

        // Compute and show max context possible given VRAM.
        // Use the provided vram_mib if available, otherwise compute it from
        // the model file size and settings (mirrors estimate_vram_mib).
        let effective_vram = if vram_mib > 0 {
            vram_mib
        } else if meta.hidden_size > 0 {
            let model_mib = model.file_size / (1024 * 1024);
            let hidden = Some(meta.hidden_size);
            let n_head = if meta.n_head > 0 { Some(meta.n_head) } else { None };
            let n_kv_head = if meta.n_kv_head > 0 { Some(meta.n_kv_head) } else { None };
            crate::models::estimate_vram_mib(
                model_mib, settings, meta.layers, hidden, n_head, n_kv_head, gpu_mem_total_mib
            )
        } else {
            0
        };

        if effective_vram > 0 && meta.hidden_size > 0 {
            let max_ctx = max_context_for_vram(
                model.file_size,
                effective_vram,
                meta.layers,
                meta.hidden_size,
                meta.n_head,
                meta.n_kv_head,
                settings.gpu_layers,
                settings.flash_attn,
                settings.uniform_cache,
                settings.parallel,
                &settings.cache_type_k.to_string(),
                &settings.cache_type_v.to_string(),
            );
            if max_ctx > 0 {
                pairs.push(ModelInfoPair {
                    label: "Max Context".to_string(),
                    value: format_ctx(max_ctx),
                    value_style: Color::Yellow,
                });
            }
        }
    }

    pairs
}

fn format_ctx(ctx: u32) -> String {
    if ctx >= 1_000_000 {
        format!("{:.1}M", ctx as f64 / 1_000_000.0)
    } else if ctx >= 1_000 {
        format!("{:.1}K", ctx as f64 / 1_000.0)
    } else {
        ctx.to_string()
    }
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}
