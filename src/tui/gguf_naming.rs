use std::collections::HashMap;
use std::sync::LazyLock;

/// A single segment of a GGUF filename with its explanation and optional documentation link.
#[derive(Debug, Clone, PartialEq)]
pub struct GgufSegment {
    pub label: String,
    pub value: String,
    pub description: String,
    pub link: &'static str,
}

/// Complete parsed explanation of a GGUF filename.
#[derive(Debug, Clone, PartialEq)]
pub struct GgufExplanation {
    /// Display name shown as title (e.g., "Qwen3.6-35B-A3B (Unsloth Dynamic)")
    pub model_family: String,
    pub segments: Vec<GgufSegment>,
}

/// Information about a known model family for specific explanations.
struct FamilyInfo {
    name: &'static str,
    link: &'static str,
    param_descriptions: &'static [(&'static str, &'static str)],
    moe_description: &'static str,
    moe_link: &'static str,
}

/// Known model families with specific explanations.
static KNOWN_FAMILIES: LazyLock<HashMap<&'static str, FamilyInfo>> = LazyLock::new(|| {
    [
        (
            "qwen3.6",
            FamilyInfo {
                name: "Qwen3.6 by Alibaba",
                link: "https://qwen.readthedocs.io/en/latest/",
                param_descriptions: &[
                    ("35B", "35 Billion total parameters (MoE architecture)"),
                    ("72B", "72 Billion total parameters"),
                ],
                moe_description:
                    "Mixture-of-Experts: only a fraction of parameters active per token",
                moe_link: "https://friendli.ai/blog/moe-models-comparison",
            },
        ),
        (
            "qwen3.5",
            FamilyInfo {
                name: "Qwen3.5 by Alibaba",
                link: "https://qwen.readthedocs.io/en/latest/",
                param_descriptions: &[
                    ("0.5B", "0.5 Billion parameters"),
                    ("1.5B", "1.5 Billion parameters"),
                    ("3B", "3 Billion parameters"),
                    ("4B", "4 Billion parameters"),
                    ("7B", "7 Billion parameters"),
                    ("8B", "8 Billion parameters"),
                    ("14B", "14 Billion parameters"),
                    ("30B", "30 Billion parameters"),
                    ("32B", "32 Billion parameters"),
                    ("57B", "57 Billion parameters"),
                    ("72B", "72 Billion parameters"),
                ],
                moe_description:
                    "Mixture-of-Experts: only a fraction of parameters active per token",
                moe_link: "https://friendli.ai/blog/moe-models-comparison",
            },
        ),
        (
            "qwen2.5",
            FamilyInfo {
                name: "Qwen2.5 by Alibaba",
                link: "https://qwen.readthedocs.io/en/latest/",
                param_descriptions: &[
                    ("3B", "3 Billion parameters"),
                    ("7B", "7 Billion parameters"),
                    ("14B", "14 Billion parameters"),
                    ("32B", "32 Billion parameters"),
                    ("30B", "30 Billion parameters"),
                    ("72B", "72 Billion parameters"),
                    ("1.5B", "1.5 Billion parameters"),
                    ("0.5B", "0.5 Billion parameters"),
                ],
                moe_description:
                    "Mixture-of-Experts: only a fraction of parameters active per token",
                moe_link: "https://friendli.ai/blog/moe-models-comparison",
            },
        ),
        (
            "qwen2",
            FamilyInfo {
                name: "Qwen2 by Alibaba",
                link: "https://qwen.readthedocs.io/en/latest/",
                param_descriptions: &[
                    ("1.5B", "1.5 Billion parameters"),
                    ("7B", "7 Billion parameters"),
                    ("57B", "57 Billion parameters"),
                    ("72B", "72 Billion parameters"),
                ],
                moe_description:
                    "Mixture-of-Experts: only a fraction of parameters active per token",
                moe_link: "https://friendli.ai/blog/moe-models-comparison",
            },
        ),
        (
            "llama3.1",
            FamilyInfo {
                name: "Llama 3.1 by Meta",
                link: "https://huggingface.co/Meta-Llama",
                param_descriptions: &[
                    ("8B", "8 Billion parameters"),
                    ("70B", "70 Billion parameters"),
                    ("405B", "405 Billion parameters"),
                ],
                moe_description:
                    "Mixture-of-Experts: only a fraction of parameters active per token",
                moe_link: "https://friendli.ai/blog/moe-models-comparison",
            },
        ),
        (
            "llama3",
            FamilyInfo {
                name: "Llama 3 by Meta",
                link: "https://huggingface.co/Meta-Llama",
                param_descriptions: &[
                    ("8B", "8 Billion parameters"),
                    ("70B", "70 Billion parameters"),
                ],
                moe_description:
                    "Mixture-of-Experts: only a fraction of parameters active per token",
                moe_link: "https://friendli.ai/blog/moe-models-comparison",
            },
        ),
        (
            "llama2",
            FamilyInfo {
                name: "Llama 2 by Meta",
                link: "https://huggingface.co/Meta-Llama",
                param_descriptions: &[
                    ("7B", "7 Billion parameters"),
                    ("13B", "13 Billion parameters"),
                    ("70B", "70 Billion parameters"),
                ],
                moe_description:
                    "Mixture-of-Experts: only a fraction of parameters active per token",
                moe_link: "https://friendli.ai/blog/moe-models-comparison",
            },
        ),
        (
            "mistral",
            FamilyInfo {
                name: "Mistral by Mistral AI",
                link: "https://huggingface.co/mistralai",
                param_descriptions: &[
                    ("7B", "7 Billion parameters"),
                    ("22B", "22 Billion parameters"),
                    ("24B", "24 Billion parameters"),
                ],
                moe_description:
                    "Mixture-of-Experts: only a fraction of parameters active per token",
                moe_link: "https://friendli.ai/blog/moe-models-comparison",
            },
        ),
        (
            "mistral-nemo",
            FamilyInfo {
                name: "Mistral Nemo by Mistral AI",
                link: "https://huggingface.co/mistralai",
                param_descriptions: &[
                    ("12B", "12 Billion parameters"),
                ],
                moe_description:
                    "Mixture-of-Experts: only a fraction of parameters active per token",
                moe_link: "https://friendli.ai/blog/moe-models-comparison",
            },
        ),
        (
            "phi3",
            FamilyInfo {
                name: "Phi-3 by Microsoft",
                link: "https://huggingface.co/microsoft",
                param_descriptions: &[
                    ("3.8B", "3.8 Billion parameters"),
                    ("14B", "14 Billion parameters"),
                    ("2.7B", "2.7 Billion parameters"),
                ],
                moe_description:
                    "Mixture-of-Experts: only a fraction of parameters active per token",
                moe_link: "https://friendli.ai/blog/moe-models-comparison",
            },
        ),
        (
            "gemma2",
            FamilyInfo {
                name: "Gemma 2 by Google",
                link: "https://huggingface.co/google",
                param_descriptions: &[
                    ("2B", "2 Billion parameters"),
                    ("9B", "9 Billion parameters"),
                ],
                moe_description:
                    "Mixture-of-Experts: only a fraction of parameters active per token",
                moe_link: "https://friendli.ai/blog/moe-models-comparison",
            },
        ),
        (
            "gemma",
            FamilyInfo {
                name: "Gemma by Google",
                link: "https://huggingface.co/google",
                param_descriptions: &[
                    ("2B", "2 Billion parameters"),
                    ("7B", "7 Billion parameters"),
                ],
                moe_description:
                    "Mixture-of-Experts: only a fraction of parameters active per token",
                moe_link: "https://friendli.ai/blog/moe-models-comparison",
            },
        ),
        (
            "starcoder2",
            FamilyInfo {
                name: "StarCoder2 by BigCode",
                link: "https://huggingface.co/bigcode",
                param_descriptions: &[
                    ("3B", "3 Billion parameters"),
                    ("7B", "7 Billion parameters"),
                    ("15B", "15 Billion parameters"),
                ],
                moe_description:
                    "Mixture-of-Experts: only a fraction of parameters active per token",
                moe_link: "https://friendli.ai/blog/moe-models-comparison",
            },
        ),
        (
            "codestral",
            FamilyInfo {
                name: "Codestral by Mistral AI",
                link: "https://huggingface.co/mistralai",
                param_descriptions: &[
                    ("22B", "22 Billion parameters"),
                ],
                moe_description:
                    "Mixture-of-Experts: only a fraction of parameters active per token",
                moe_link: "https://friendli.ai/blog/moe-models-comparison",
            },
        ),
    ]
    .into_iter()
    .collect()
});

/// Quantization provider info (shared across all families).
struct QuantProviderInfo {
    pub label: &'static str,
    pub description: &'static str,
    pub link: &'static str,
}

static QUANT_PROVIDERS: LazyLock<HashMap<&'static str, QuantProviderInfo>> =
    LazyLock::new(|| {
        [
            (
                "UD",
                QuantProviderInfo {
                    label: "Unsloth Dynamic 2.0",
                    description:
                        "Dynamic bit-allocation: KL-divergence calibrated, keeps sensitive \
                        layers at higher precision",
                    link: "https://unsloth.ai/docs/basics/unsloth-dynamic-2.0-ggufs",
                },
            ),
            (
                "H",
                QuantProviderInfo {
                    label: "HuggingFace",
                    description: "Standard HuggingFace quantization",
                    link: "https://huggingface.co/docs/transformers.js/models-converter",
                },
            ),
        ]
        .into_iter()
        .collect()
    });

/// Quantization level description (shared across all families).
struct QuantLevelInfo {
    #[allow(dead_code)]
    bit: u32,
    description: &'static str,
}

static QUANT_LEVELS: LazyLock<HashMap<u32, QuantLevelInfo>> = LazyLock::new(|| {
    [
        (2, QuantLevelInfo { bit: 2, description: "2-bit quantization — aggressive compression, largest quality loss" }),
        (3, QuantLevelInfo { bit: 3, description: "3-bit quantization — high compression, noticeable quality loss" }),
        (4, QuantLevelInfo { bit: 4, description: "4-bit quantization — balanced compression and quality, widely used" }),
        (5, QuantLevelInfo { bit: 5, description: "5-bit quantization — good quality with moderate compression" }),
        (6, QuantLevelInfo { bit: 6, description: "6-bit quantization — high quality with moderate compression" }),
        (7, QuantLevelInfo { bit: 7, description: "7-bit quantization — near-lossless quality" }),
        (8, QuantLevelInfo { bit: 8, description: "8-bit quantization — nearly indistinguishable from FP16" }),
    ]
    .into_iter()
    .collect()
});

const QUANTIZATION_COMMON_LINK: &str =
    "https://github.com/ggml-org/llama.cpp/discussions/2094";

/// Improved Quantization (IQ) format info.
/// IQ formats use mixed-precision internally, not simple bit depths.
struct IqQuantInfo {
    label: &'static str,
    description: &'static str,
}

static IQ_QUANTS: LazyLock<HashMap<&'static str, IqQuantInfo>> = LazyLock::new(|| {
    [
        ("IQ1_S", IqQuantInfo { label: "IQ1_S", description: "1-bit mixed precision — extreme compression, significant quality loss" }),
        ("IQ2_XXS", IqQuantInfo { label: "IQ2_XXS", description: "2-bit extreme compression — very low quality" }),
        ("IQ2_XS", IqQuantInfo { label: "IQ2_XS", description: "2-bit mixed precision — high compression, low quality" }),
        ("IQ2_S", IqQuantInfo { label: "IQ2_S", description: "2-bit small — moderate compression" }),
        ("IQ3_XXS", IqQuantInfo { label: "IQ3_XXS", description: "3-bit extreme compression — good compression with moderate quality" }),
        ("IQ3_S", IqQuantInfo { label: "IQ3_S", description: "3-bit small — balanced compression and quality" }),
        ("IQ4_NL", IqQuantInfo { label: "IQ4_NL", description: "4-bit non-linear — near-lossless quality" }),
        ("IQ4_XS", IqQuantInfo { label: "IQ4_XS", description: "4-bit extended — good quality with moderate compression" }),
    ]
    .into_iter()
    .collect()
});

/// Size variant descriptions (shared across all families).
struct SizeVariantInfo {
    label: &'static str,
    description: &'static str,
}

static SIZE_VARIANTS: LazyLock<HashMap<char, SizeVariantInfo>> = LazyLock::new(|| {
    [
        ('S', SizeVariantInfo { label: "Small", description: "Most aggressive quantization, smallest file size (S < M < L)" }),
        ('M', SizeVariantInfo { label: "Medium", description: "Balanced between quality and size" }),
        ('L', SizeVariantInfo { label: "Large", description: "Least aggressive quantization, largest file size, best quality" }),
    ]
    .into_iter()
    .collect()
});

/// Extract the filename stem (without path and without .gguf extension).
pub fn extract_stem(filename: &str) -> String {
    let name = filename.rsplit('/').next().unwrap_or(filename);
    let name = name.strip_suffix(".gguf").unwrap_or(name);
    let name = name.strip_suffix(".GGUF").unwrap_or(name);
    name.to_string()
}

/// Try to match the beginning of the stem to a known model family.
fn match_family(stem: &str) -> Option<(&'static str, &'static FamilyInfo)> {
    let stem_lower = stem.to_lowercase();
    let prefixes = [
        "qwen3.6",
        "qwen3.5",
        "qwen2.5",
        "qwen2",
        "llama-3.1",
        "llama-3",
        "llama-2",
        "llama3.1",
        "llama3",
        "llama2",
        "mistral-nemo",
        "mistral",
        "phi3",
        "gemma2",
        "gemma",
        "starcoder2",
        "codestral",
    ];
    for prefix in prefixes {
        if stem_lower.starts_with(prefix) {
            // Map to the canonical family key
            let family_key = match prefix {
                "llama-3.1" | "llama3.1" => "llama3.1",
                "llama-3" | "llama3" => "llama3",
                "llama-2" | "llama2" => "llama2",
                _ => prefix,
            };
            if let Some(info) = KNOWN_FAMILIES.get(family_key) {
                return Some((family_key, info));
            }
        }
    }
    None
}

/// Extract parameter count string from stem (e.g., "35B", "8B", "A3B").
fn extract_param_info(stem: &str) -> (Option<String>, Option<String>) {
    // Look for MoE active params like "A3B", "A4B" at the end
    // Pattern: total params followed by -AxB for MoE
    if let Some(a_pos) = stem.rfind('-') {
        let after_dash = &stem[a_pos + 1..];
        if after_dash.starts_with('A') && after_dash.len() >= 2 {
            let active_part = &after_dash[1..];
            if active_part.chars().take(active_part.len() - 1).all(|c| c.is_ascii_digit())
                && active_part.ends_with('B')
            {
                let total_str = &stem[..a_pos];
                return (Some(total_str.to_string()), Some(after_dash.to_string()));
            }
        }
    }
    // Look for plain param count like "35B", "8B"
    let upper = stem.to_uppercase();
    for i in (0..stem.len()).rev() {
        let ch = upper.chars().nth(i).unwrap();
        if ch.is_ascii_digit() || ch == '.' {
            continue;
        }
        if ch == 'B' || ch == 'A' {
            let start = if let Some(dash_pos) = stem[..=i].rfind('-') {
                dash_pos + 1
            } else {
                0
            };
            let token = &stem[start..=i];
            let token_upper = token.to_uppercase();
            if token_upper.ends_with('B') {
                let digits_part = &token_upper[..token_upper.len() - 1];
                if digits_part.chars().all(|c| c.is_ascii_digit() || c == '.')
                    && !digits_part.is_empty()
                {
                    return (Some(token.to_string()), None);
                }
            }
            break;
        }
    }
    (None, None)
}

/// Detect quantization provider (e.g., "UD" for Unsloth Dynamic).
fn extract_quant_provider(stem: &str) -> Option<(String, &'static QuantProviderInfo)> {
    let parts: Vec<&str> = stem.split('-').collect();
    for (_i, part) in parts.iter().enumerate() {
        let upper = part.to_uppercase();
        if upper == "UD" {
            if let Some(provider) = QUANT_PROVIDERS.get("UD") {
                return Some((part.to_string(), provider));
            }
        } else if upper == "H" {
            if let Some(provider) = QUANT_PROVIDERS.get("H") {
                return Some((part.to_string(), provider));
            }
        }
    }
    None
}

/// Extract quantization scheme and size variant from stem.
/// Returns (quant_bits, k_quant, size_variant, iq_format)
fn extract_quant_scheme(stem: &str) -> (Option<u32>, bool, Option<char>, Option<String>) {
    let mut quant_bits: Option<u32> = None;
    let mut k_quant = false;
    let mut size_variant: Option<char> = None;
    let mut iq_format: Option<String> = None;

    let parts: Vec<&str> = stem.split('-').collect();
    for part in parts {
        let upper = part.to_uppercase();
        // Match IQ1_S, IQ2_XXS, IQ3_XXS, IQ4_NL, etc. (Improved Quantization)
        if upper.starts_with("IQ") {
            if let Some(info) = IQ_QUANTS.get(upper.as_str()) {
                iq_format = Some(info.label.to_string());
                continue;
            }
        }
        // Match Q4_K_S, Q5_K_M, Q8_0, Q3_K_S, etc.
        if upper.starts_with('Q') && upper.chars().nth(1).map_or(false, |c| c.is_ascii_digit()) {
            // Extract digit(s) after Q
            let mut digit_str = String::new();
            for c in upper.chars().skip(1) {
                if c.is_ascii_digit() {
                    digit_str.push(c);
                } else {
                    break;
                }
            }
            if let Ok(bits) = digit_str.parse::<u32>() {
                quant_bits = Some(bits);
            }

            // Check for K-quant variant (Q4_K_S, Q5_K_M, Q6_K, etc.)
            if upper.contains("_K") {
                k_quant = true;
                // Extract size variant after _K (e.g., _K_S, _K_M, _K_L)
                // Pattern can be _K_S or _K_S (with underscore between K and size)
                if let Some(k_pos) = upper.find("_K") {
                    let after_k = &upper[k_pos + 2..];
                    // Skip any underscore before the size variant
                    let after_k = after_k.strip_prefix('_').unwrap_or(after_k);
                    if !after_k.is_empty() && after_k.chars().next().unwrap().is_ascii_alphabetic() {
                        size_variant = after_k.chars().next();
                    }
                }
            }
        }
    }
    (quant_bits, k_quant, size_variant, iq_format)
}

/// Build segments for quantization part of the filename.
fn build_quant_segments(
    bits: Option<u32>,
    k_quant: bool,
    size_variant: Option<char>,
    iq_format: Option<String>,
) -> Vec<GgufSegment> {
    let mut segments = Vec::new();

    // Handle IQ formats first (they don't use standard bit depth)
    if let Some(iq) = iq_format {
        if let Some(iq_info) = IQ_QUANTS.get(iq.as_str()) {
            segments.push(GgufSegment {
                label: iq,
                value: iq_info.label.to_string(),
                description: iq_info.description.to_string(),
                link: QUANTIZATION_COMMON_LINK,
            });
        }
        return segments;
    }

    if let Some(b) = bits {
        if let Some(level_info) = QUANT_LEVELS.get(&b) {
            segments.push(GgufSegment {
                label: format!("Q{}", b),
                value: format!("{}-bit", b),
                description: level_info.description.to_string(),
                link: QUANTIZATION_COMMON_LINK,
            });
        }
    }

    if k_quant {
        segments.push(GgufSegment {
            label: "K".to_string(),
            value: "K-quant".to_string(),
            description: "Block-wise quantization with double-quantized scales for better quality".to_string(),
            link: QUANTIZATION_COMMON_LINK,
        });
    }

    if let Some(sv) = size_variant {
        if let Some(sv_info) = SIZE_VARIANTS.get(&sv) {
            let quant_label = if let Some(b) = bits {
                format!("Q{}_", b)
            } else {
                String::new()
            };
            segments.push(GgufSegment {
                label: format!("{}{}", quant_label, sv),
                value: sv_info.label.to_string(),
                description: sv_info.description.to_string(),
                link: QUANTIZATION_COMMON_LINK,
            });
        }
    }

    segments
}

/// Strip quantization suffix from a stem to get just the model identity.
fn strip_quant_suffix(stem: &str) -> String {
    let parts: Vec<&str> = stem.split('-').collect();
    for (i, part) in parts.iter().enumerate() {
        let upper = part.to_uppercase();
        // Stop at quantization part (Q4_K_S, Q5_K_M, etc.)
        if upper.starts_with('Q') && upper.chars().nth(1).map_or(false, |c| c.is_ascii_digit()) {
            if i > 0 {
                return parts[..i].join("-");
            }
            return stem.to_string();
        }
        // Stop at IQ quantization part (IQ1_S, IQ3_XXS, etc.)
        if upper.starts_with("IQ") && IQ_QUANTS.contains_key(upper.as_str()) {
            if i > 0 {
                return parts[..i].join("-");
            }
            return stem.to_string();
        }
        // Also stop at known quantization provider markers (UD, H)
        if (upper == "UD" || upper == "H") && i + 1 < parts.len() {
            let next_upper = parts[i + 1].to_uppercase();
            // Check for Q4/Q5/Q6/Q8
            if next_upper.starts_with('Q') && next_upper.chars().nth(1).map_or(false, |c| c.is_ascii_digit()) {
                if i > 0 {
                    return parts[..i].join("-");
                }
                return stem.to_string();
            }
            // Check for IQ quantization
            if next_upper.starts_with("IQ") && IQ_QUANTS.contains_key(next_upper.as_str()) {
                if i > 0 {
                    return parts[..i].join("-");
                }
                return stem.to_string();
            }
        }
    }
    stem.to_string()
}

/// Build the model family display name.
fn build_model_display(stem: &str, provider: Option<&'static QuantProviderInfo>) -> String {
    let display = stem.to_string();

    // Try to find model family prefix
    if let Some((prefix, info)) = match_family(stem) {
        let prefix_len = prefix.len();
        let remaining = if stem.len() > prefix_len {
            &stem[prefix_len..]
        } else {
            ""
        };
        let remaining = remaining.strip_prefix('-').unwrap_or(remaining);

        // Strip quantization suffix before extracting params
        let identity = strip_quant_suffix(remaining);
        let (total_params, active_params) = extract_param_info(&identity);

        let mut name = if let Some(active) = active_params {
            if let Some(total) = total_params {
                format!("{}-{}-{} ({})", prefix, total, active, info.name)
            } else {
                format!("{}-{} ({})", prefix, active, info.name)
            }
        } else if let Some(total) = total_params {
            format!("{}-{} ({})", prefix, total, info.name)
        } else {
            format!("{} ({})", prefix, info.name)
        };

        if let Some(provider) = provider {
            name = format!("{} [{}]", name, provider.label);
        }
        name
    } else {
        if let Some(provider) = provider {
            format!("{} [{}]", display, provider.label)
        } else {
            display
        }
    }
}

/// Main parsing function — matches family, extracts params, builds segments.
pub fn parse_gguf_filename(filename: &str) -> GgufExplanation {
    let stem = extract_stem(filename);

    let family_match = match_family(&stem);
    let provider_match = extract_quant_provider(&stem);
    let (bits, k_quant, size_variant, iq_format) = extract_quant_scheme(&stem);

    let model_family = build_model_display(&stem, provider_match.as_ref().map(|(_, v)| *v));

    let mut segments = Vec::new();

    if let Some((prefix, info)) = family_match {
        let prefix_len = prefix.len();
        let remaining = if stem.len() > prefix_len {
            &stem[prefix_len..]
        } else {
            ""
        };
        let remaining = remaining.strip_prefix('-').unwrap_or(remaining);

        // Strip quantization suffix before extracting params
        let identity = strip_quant_suffix(remaining);
        let (total_params, active_params) = extract_param_info(&identity);

        // Model family segment
        segments.push(GgufSegment {
            label: prefix.to_string(),
            value: prefix.to_string(),
            description: info.name.to_string(),
            link: info.link,
        });

        // Total parameters
        if let Some(total) = &total_params {
            let desc = info
                .param_descriptions
                .iter()
                .find(|(k, _)| *k == total.as_str())
                .map(|(_, v)| v.to_string())
                .unwrap_or_else(|| format!("{} total parameters", total));
            segments.push(GgufSegment {
                label: total.clone(),
                value: total.clone(),
                description: desc,
                link: "",
            });
        }

        // Active parameters (MoE)
        if let Some(active) = &active_params {
            segments.push(GgufSegment {
                label: active.clone(),
                value: active.clone(),
                description: info.moe_description.to_string(),
                link: info.moe_link,
            });
        }

        // Quantization provider
        if let Some((prov_str, prov_info)) = &provider_match {
            segments.push(GgufSegment {
                label: prov_str.clone(),
                value: prov_info.label.to_string(),
                description: prov_info.description.to_string(),
                link: prov_info.link,
            });
        }

        // Quantization segments
        let quant_segs = build_quant_segments(bits, k_quant, size_variant, iq_format.clone());
        segments.extend(quant_segs);
    } else {
        // Generic fallback for unknown families
        segments.push(GgufSegment {
            label: stem.clone(),
            value: stem.clone(),
            description: "Unknown model family — using generic GGUF naming conventions".to_string(),
            link: "",
        });

        // Quantization provider
        if let Some((prov_str, prov_info)) = &provider_match {
            segments.push(GgufSegment {
                label: prov_str.clone(),
                value: prov_info.label.to_string(),
                description: prov_info.description.to_string(),
                link: prov_info.link,
            });
        }

        // Quantization segments
        let quant_segs = build_quant_segments(bits, k_quant, size_variant, iq_format);
        segments.extend(quant_segs);
    }

    GgufExplanation {
        model_family,
        segments,
    }
}

/// Get a GgufExplanation, using the cache if available.
pub fn get_explanation(
    filename: &str,
    cache: &mut HashMap<String, GgufExplanation>,
) -> GgufExplanation {
    let key = extract_stem(filename);
    if let Some(explanation) = cache.get(&key) {
        return explanation.clone();
    }
    let explanation = parse_gguf_filename(filename);
    cache.insert(key, explanation.clone());
    explanation
}

/// Pre-populate the cache with explanations for multiple filenames.
#[allow(dead_code)]
pub fn populate_cache(
    filenames: &[&str],
    cache: &mut HashMap<String, GgufExplanation>,
) {
    for filename in filenames {
        let key = extract_stem(filename);
        if !cache.contains_key(&key) {
            let explanation = parse_gguf_filename(filename);
            cache.insert(key, explanation);
        }
    }
}


