use ratatui::style::Color;

use crate::tui::format_size;

/// A single key-value pair for model info, rendered in two columns.
/// `label` is an i18n key resolved at render time.
#[derive(Clone, Debug)]
pub struct ModelInfoPair {
    pub label: &'static str,
    pub value: String,
    pub value_style: Color,
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
) -> Vec<ModelInfoPair> {
    let mut pairs: Vec<ModelInfoPair> = Vec::new();

    let path = model.path.to_string_lossy().to_string();
    let size = format_size(model.file_size);

    pairs.push(ModelInfoPair {
        label: "model_info.path",
        value: path,
        value_style: Color::White,
    });

    pairs.push(ModelInfoPair {
        label: "model_info.size",
        value: size,
        value_style: Color::White,
    });

    // Use cached metadata if available, otherwise just show basic info.
    // Parsing GGUF here would block the render loop.
    if let Some(meta) = cached {
        if !meta.arch.is_empty() {
            pairs.push(ModelInfoPair {
                label: "model_info.arch",
                value: meta.arch.clone(),
                value_style: Color::Cyan,
            });
        }

        if meta.arch == "mtp" {
            pairs.push(ModelInfoPair {
                label: "model_info.mtp",
                value: format!("{} drafts", meta.draft_tokens),
                value_style: Color::Magenta,
            });
        }

        if !meta.domain.is_empty() {
            pairs.push(ModelInfoPair {
                label: "model_info.domain",
                value: meta.domain.clone(),
                value_style: Color::White,
            });
        }

        if !meta.quantization.is_empty() {
            pairs.push(ModelInfoPair {
                label: "model_info.quant",
                value: meta.quantization.clone(),
                value_style: Color::Cyan,
            });
        }

        if !meta.file_type.is_empty() {
            pairs.push(ModelInfoPair {
                label: "model_info.format",
                value: meta.file_type.clone(),
                value_style: Color::White,
            });
        }

        if !meta.model_parameters.is_empty() {
            pairs.push(ModelInfoPair {
                label: "model_info.parameters",
                value: meta.model_parameters.clone(),
                value_style: Color::White,
            });
        }

        if !meta.tokenizer.is_empty() {
            pairs.push(ModelInfoPair {
                label: "model_info.tokenizer",
                value: meta.tokenizer.clone(),
                value_style: Color::Cyan,
            });
        }

        // Show n_ctx_train from GGUF as "Context".
        if meta.n_ctx_train > 0 {
            pairs.push(ModelInfoPair {
                label: "model_info.context",
                value: format!("{} tokens", meta.n_ctx_train),
                value_style: Color::White,
            });
        }

        let mut all_capabilities: Vec<String> = meta.capabilities.clone();
        if let Some(pipeline) = crate::models::arch_to_pipeline_tag(&meta.arch) {
            let pipeline_str = pipeline.to_string();
            if !all_capabilities.contains(&pipeline_str) {
                all_capabilities.push(pipeline_str);
            }
        }
        for cap in &model.capabilities {
            if !all_capabilities.contains(cap) {
                all_capabilities.push(cap.clone());
            }
        }
        if let Some(ref pipeline_tag) = model.pipeline_tag
            && !all_capabilities.contains(pipeline_tag)
        {
            all_capabilities.push(pipeline_tag.clone());
        }
        if !all_capabilities.is_empty() {
            pairs.push(ModelInfoPair {
                label: "model_info.capabilities",
                value: all_capabilities.join(", "),
                value_style: Color::Green,
            });
        }
    }

    pairs
}
