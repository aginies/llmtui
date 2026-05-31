use crate::models::ModelSettings;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

// ── Function pointer types (zero-cost, no dynamic dispatch) ──────────────────

pub type DisplayFn = fn(&ModelSettings) -> String;
pub type DirtyFn = fn(&ModelSettings, &ModelSettings) -> bool;
pub type AdjustFn = fn(&mut ModelSettings, i32, u32);  // u32 = context_limit (0 = no limit)
pub type ApplyEditFn = fn(&mut ModelSettings, &str);
pub type CtrlEToggleFn = fn(&mut ModelSettings);

// ── Edit kinds ───────────────────────────────────────────────────────────────

pub enum EditKind {
    /// Direct text entry (digits, decimals, etc.)
    Direct,
    /// Toggles a boolean on Enter
    Toggle,
    /// Opens a modal (picker, etc.)
    Modal,
}

// ── SettingField ─────────────────────────────────────────────────────────────

pub struct SettingField {
    #[allow(dead_code)]
    pub id: &'static str,
    pub name: &'static str,
    pub section: &'static str,
    pub display: DisplayFn,
    pub dirty: DirtyFn,
    pub adjust: AdjustFn,
    pub apply_edit: ApplyEditFn,
    pub ctrl_e_toggle: Option<CtrlEToggleFn>,
    #[allow(dead_code)]
    pub edit_kind: EditKind,
}

impl SettingField {
    pub fn name(&self) -> &str {
        self.name
    }

    pub fn display(&self, settings: &ModelSettings) -> String {
        (self.display)(settings)
    }

    pub fn is_dirty(&self, settings: &ModelSettings, cached: &ModelSettings) -> bool {
        (self.dirty)(settings, cached)
    }

    pub fn adjust(&self, settings: &mut ModelSettings, delta: i32, context_limit: u32) {
        (self.adjust)(settings, delta, context_limit);
    }

    pub fn apply_edit(&self, settings: &mut ModelSettings, buf: &str) {
        (self.apply_edit)(settings, buf);
    }

    pub fn ctrl_e_toggle(&self, settings: &mut ModelSettings) {
        if let Some(toggle) = self.ctrl_e_toggle {
            toggle(settings);
        }
    }

    /// Returns true if this field starts a new section (different from the previous field's section).
    pub fn is_new_section(&self, prev_section: Option<&str>) -> bool {
        Some(self.section) != prev_section
    }
}

// ── Helper constructors ──────────────────────────────────────────────────────

fn field(
    id: &'static str,
    name: &'static str,
    section: &'static str,
    display: DisplayFn,
    dirty: DirtyFn,
    adjust: AdjustFn,
    apply_edit: ApplyEditFn,
    edit_kind: EditKind,
) -> SettingField {
    SettingField {
        id,
        name,
        section,
        display,
        dirty,
        adjust,
        apply_edit,
        ctrl_e_toggle: None,
        edit_kind,
    }
}

// ── GPU Layers special adjust (needs total_layers from App, so partial) ──────

fn gpu_layers_adjust(settings: &mut ModelSettings, delta: i32, _context_limit: u32) {
    use crate::models::GpuLayersMode;
    settings.gpu_layers_mode = match (delta, &settings.gpu_layers_mode) {
        (1, GpuLayersMode::Auto) => GpuLayersMode::Specific(1),
        (1, GpuLayersMode::Specific(n)) => GpuLayersMode::Specific(n + 1),
        (1, GpuLayersMode::All) => GpuLayersMode::Auto,
        (-1, GpuLayersMode::Auto) => GpuLayersMode::Auto,
        (-1, GpuLayersMode::Specific(n)) if *n == 0 => GpuLayersMode::Auto,
        (-1, GpuLayersMode::Specific(n)) if *n == 1 => GpuLayersMode::Specific(0),
        (-1, GpuLayersMode::Specific(n)) => GpuLayersMode::Specific(n - 1),
        (-1, GpuLayersMode::All) => GpuLayersMode::All,
        _ => settings.gpu_layers_mode.clone(),
    };
}

fn gpu_layers_apply(settings: &mut ModelSettings, buf: &str) {
    use crate::models::GpuLayersMode;
    if let Ok(v) = buf.parse::<i32>() {
        settings.gpu_layers_mode = if v < 0 {
            GpuLayersMode::All
        } else {
            GpuLayersMode::Specific(v as u32)
        };
    }
}

// ── Standard fields (28 fields) ─────────────────────────────────────────────

pub fn standard_fields() -> Vec<SettingField> {
    vec![
        // ── Loading ───────────────────────────────────────────────────────────
        field(
            "system_prompt_preset_name",
            "Prompt",
            "Loading",
            |s| s.system_prompt_preset_name.clone(),
            |s, c| s.system_prompt_preset_name != c.system_prompt_preset_name,
            |_, _, _| {},
            |_, _| {},
            EditKind::Modal,
        ),
        field(
            "context_length",
            "Context",
            "Loading",
            |s| s.context_length.to_string(),
            |s, c| s.context_length != c.context_length,
            |s, delta, ctx_limit| {
                let mut val = (s.context_length as i32 + delta * 128).max(128) as u32;
                if ctx_limit > 0 {
                    val = val.min(ctx_limit);
                }
                s.context_length = val;
            },
            |s, buf| {
                if let Ok(v) = buf.parse::<u32>() {
                    let mut val = v.max(128);
                    if s.context_length > 0 {
                        val = val.min(s.context_length);
                    }
                    s.context_length = val;
                }
            },
            EditKind::Direct,
        ),
        field(
            "mlock",
            "Keep in memory (mlock)",
            "Loading",
            |s| s.mlock.to_string(),
            |s, c| s.mlock != c.mlock,
            |s, _, _| {
                s.mlock = !s.mlock;
            },
            |_, _| {},
            EditKind::Toggle,
        ),

        // ── GPU Offload ───────────────────────────────────────────────────────
        field(
            "gpu_layers_mode",
            "GPU Layers",
            "GPU Offload",
            |s| match s.gpu_layers_mode {
                crate::models::GpuLayersMode::Auto => "Auto".to_string(),
                crate::models::GpuLayersMode::Specific(n) => n.to_string(),
                crate::models::GpuLayersMode::All => "All".to_string(),
            },
            |s, c| s.gpu_layers_mode != c.gpu_layers_mode,
            gpu_layers_adjust,
            gpu_layers_apply,
            EditKind::Direct,
        ),
        field(
            "flash_attn",
            "Flash Attention",
            "GPU Offload",
            |s| s.flash_attn.to_string(),
            |s, c| s.flash_attn != c.flash_attn,
            |s, _, _| {
                s.flash_attn = !s.flash_attn;
            },
            |_, _| {},
            EditKind::Toggle,
        ),
        field(
            "kv_cache_offload",
            "KV Cache Offload",
            "GPU Offload",
            |s| s.kv_cache_offload.to_string(),
            |s, c| s.kv_cache_offload != c.kv_cache_offload,
            |s, _, _| {
                s.kv_cache_offload = !s.kv_cache_offload;
            },
            |_, _| {},
            EditKind::Toggle,
        ),
        field(
            "cache_type_k",
            "Cache Type K",
            "GPU Offload",
            |s| s.cache_type_k
                .map(|v| v.to_string())
                .unwrap_or_else(|| "Disabled".to_string()),
            |s, c| s.cache_type_k != c.cache_type_k,
            |s, delta, _| {
                use crate::models::CacheTypeK;
                let mut val = s.cache_type_k.unwrap_or(CacheTypeK::F16);
                val = if delta > 0 { val.next() } else { val.prev() };
                s.cache_type_k = Some(val);
            },
            |s, buf| {
                use crate::models::CacheTypeK;
                if let Ok(n) = buf.parse::<u8>() {
                    s.cache_type_k = Some(CacheTypeK::from_u8(n));
                }
            },
            EditKind::Direct,
        ),
        field(
            "cache_type_v",
            "Cache Type V",
            "GPU Offload",
            |s| s.cache_type_v
                .map(|v| v.to_string())
                .unwrap_or_else(|| "Disabled".to_string()),
            |s, c| s.cache_type_v != c.cache_type_v,
            |s, delta, _| {
                use crate::models::CacheTypeV;
                let mut val = s.cache_type_v.unwrap_or(CacheTypeV::F16);
                val = if delta > 0 { val.next() } else { val.prev() };
                s.cache_type_v = Some(val);
            },
            |s, buf| {
                use crate::models::CacheTypeV;
                if let Ok(n) = buf.parse::<u8>() {
                    s.cache_type_v = Some(CacheTypeV::from_u8(n));
                }
            },
            EditKind::Direct,
        ),
        field(
            "expert_count",
            "Active Experts",
            "GPU Offload",
            |s| if s.expert_count > 0 {
                s.expert_count.to_string()
            } else {
                "Disabled".to_string()
            },
            |s, c| s.expert_count != c.expert_count,
            |s, delta, _| {
                s.expert_count = (s.expert_count as i32 + delta).clamp(-1, 99);
            },
            |s, buf| {
                if let Ok(v) = buf.parse::<i32>() {
                    s.expert_count = v.clamp(-1, 99);
                }
            },
            EditKind::Direct,
        ),

        // ── Evaluation ────────────────────────────────────────────────────────
        field(
            "batch_size",
            "Eval Batch",
            "Evaluation",
            |s| s.batch_size.to_string(),
            |s, c| s.batch_size != c.batch_size,
            |s, delta, _| {
                s.batch_size = (s.batch_size as i32 + delta * 64).max(1) as u32;
            },
            |s, buf| {
                if let Ok(v) = buf.parse::<u32>() {
                    s.batch_size = v.max(1);
                }
            },
            EditKind::Direct,
        ),
        field(
            "uniform_cache",
            "Unified KV",
            "Evaluation",
            |s| s.uniform_cache.to_string(),
            |s, c| s.uniform_cache != c.uniform_cache,
            |s, _, _| {
                s.uniform_cache = !s.uniform_cache;
            },
            |_, _| {},
            EditKind::Toggle,
        ),
        field(
            "max_concurrent_predictions",
            "Max Concurrent Pred",
            "Evaluation",
            |s| s.max_concurrent_predictions
                .map(|v| v.to_string())
                .unwrap_or_else(|| "Off".to_string()),
            |s, c| s.max_concurrent_predictions != c.max_concurrent_predictions,
            |s, delta, _| {
                match s.max_concurrent_predictions {
                    Some(n) => s.max_concurrent_predictions = Some(((n as i32) + delta).clamp(1, 10) as u32),
                    None => s.max_concurrent_predictions = Some(1),
                }
            },
            |s, buf| {
                if let Ok(v) = buf.parse::<u32>() {
                    s.max_concurrent_predictions = Some(v.clamp(1, 10));
                }
            },
            EditKind::Direct,
        ),

        // ── Sampling ──────────────────────────────────────────────────────────
        field(
            "seed",
            "Seed",
            "Sampling",
            |s| s.seed.to_string(),
            |s, c| s.seed != c.seed,
            |s, delta, _| {
                s.seed = (s.seed + delta).max(-1);
            },
            |s, buf| {
                if let Ok(v) = buf.parse::<i32>() {
                    s.seed = v;
                }
            },
            EditKind::Direct,
        ),
        field(
            "temperature",
            "Temp",
            "Sampling",
            |s| format!("{:.2}", s.temperature),
            |s, c| (s.temperature - c.temperature).abs() > 0.001,
            |s, delta, _| {
                s.temperature = ((s.temperature * 100.0 + delta as f32 * 5.0) / 100.0).clamp(0.0, 2.0);
            },
            |s, buf| {
                if let Ok(v) = buf.parse::<i32>() {
                    s.temperature = (v as f32 / 100.0).clamp(0.0, 2.0);
                }
            },
            EditKind::Direct,
        ),
        field(
            "top_k",
            "Top-k",
            "Sampling",
            |s| s.top_k.to_string(),
            |s, c| s.top_k != c.top_k,
            |s, delta, _| {
                s.top_k = (s.top_k + delta).max(1);
            },
            |s, buf| {
                if let Ok(v) = buf.parse::<i32>() {
                    s.top_k = v.max(0);
                }
            },
            EditKind::Direct,
        ),
        field(
            "top_p",
            "Top-p",
            "Sampling",
            |s| format!("{:.2}", s.top_p),
            |s, c| (s.top_p - c.top_p).abs() > 0.001,
            |s, delta, _| {
                s.top_p = ((s.top_p * 100.0 + delta as f32 * 5.0) / 100.0).clamp(0.0, 1.0);
            },
            |s, buf| {
                if let Ok(v) = buf.parse::<i32>() {
                    s.top_p = (v as f32 / 100.0).clamp(0.0, 1.0);
                }
            },
            EditKind::Direct,
        ),
        field(
            "min_p",
            "Min P",
            "Sampling",
            |s| format!("{:.2}", s.min_p),
            |s, c| (s.min_p - c.min_p).abs() > 0.001,
            |s, delta, _| {
                s.min_p = ((s.min_p * 100.0 + delta as f32 * 5.0) / 100.0).clamp(0.0, 1.0);
            },
            |s, buf| {
                if let Ok(v) = buf.parse::<i32>() {
                    s.min_p = (v as f32 / 100.0).clamp(0.0, 1.0);
                }
            },
            EditKind::Direct,
        ),
        field(
            "max_tokens",
            "Max Tokens",
            "Sampling",
            |s| s.max_tokens
                .map(|v| v.to_string())
                .unwrap_or_else(|| "Disabled".to_string()),
            |s, c| s.max_tokens != c.max_tokens,
            |s, delta, _| {
                let current = s.max_tokens.unwrap_or(2048);
                s.max_tokens = Some((current as i32 + delta * 16).max(16) as u32);
            },
            |s, buf| {
                if let Ok(v) = buf.parse::<i32>() {
                    s.max_tokens = if v == 0 {
                        None
                    } else {
                        Some(v as u32)
                    };
                }
            },
            EditKind::Direct,
        ),

        // ── Repetition ────────────────────────────────────────────────────────
        field(
            "repeat_penalty",
            "Repeat Penalty",
            "Repetition",
            |s| format!("{:.2}", s.repeat_penalty),
            |s, c| (s.repeat_penalty - c.repeat_penalty).abs() > 0.001,
            |s, delta, _| {
                s.repeat_penalty = ((s.repeat_penalty * 100.0 + delta as f32 * 5.0) / 100.0).clamp(1.0, 2.0);
            },
            |s, buf| {
                if let Ok(v) = buf.parse::<i32>() {
                    s.repeat_penalty = (v as f32 / 100.0).clamp(0.0, 2.0);
                }
            },
            EditKind::Direct,
        ),
        field(
            "repeat_last_n",
            "Repeat Last N",
            "Repetition",
            |s| s.repeat_last_n.to_string(),
            |s, c| s.repeat_last_n != c.repeat_last_n,
            |s, delta, _| {
                s.repeat_last_n = (s.repeat_last_n as i32 + delta).max(0);
            },
            |s, buf| {
                if let Ok(v) = buf.parse::<i32>() {
                    s.repeat_last_n = v.max(0);
                }
            },
            EditKind::Direct,
        ),
        field(
            "presence_penalty",
            "Presence Penalty",
            "Repetition",
            |s| s.presence_penalty
                .map(|v| format!("{:.2}", v))
                .unwrap_or_else(|| "Off".to_string()),
            |s, c| match (s.presence_penalty, c.presence_penalty) {
                (Some(v1), Some(v2)) => (v1 - v2).abs() > 0.001,
                (None, None) => false,
                _ => true,
            },
            |s, delta, _| {
                let current = s.presence_penalty.unwrap_or(0.0);
                s.presence_penalty = Some(((current * 100.0 + delta as f32 * 5.0) / 100.0).clamp(-2.0, 2.0));
            },
            |s, buf| {
                if let Ok(v) = buf.parse::<i32>() {
                    s.presence_penalty = Some((v as f32 / 100.0).clamp(0.0, 1.0));
                }
            },
            EditKind::Direct,
        ),
        field(
            "frequency_penalty",
            "Freq Penalty",
            "Repetition",
            |s| s.frequency_penalty
                .map(|v| format!("{:.2}", v))
                .unwrap_or_else(|| "Off".to_string()),
            |s, c| match (s.frequency_penalty, c.frequency_penalty) {
                (Some(v1), Some(v2)) => (v1 - v2).abs() > 0.001,
                (None, None) => false,
                _ => true,
            },
            |s, delta, _| {
                let current = s.frequency_penalty.unwrap_or(0.0);
                s.frequency_penalty = Some(((current * 100.0 + delta as f32 * 5.0) / 100.0).clamp(-2.0, 2.0));
            },
            |s, buf| {
                if let Ok(v) = buf.parse::<i32>() {
                    s.frequency_penalty = Some((v as f32 / 100.0).clamp(0.0, 1.0));
                }
            },
            EditKind::Direct,
        ),

        // ── Tags ──────────────────────────────────────────────────────────────
        field(
            "tags",
            "Tags (Enter to edit)",
            "Tags",
            |s| if s.tags.is_empty() {
                "None".to_string()
            } else {
                s.tags.join(", ")
            },
            |s, c| s.tags != c.tags,
            |_, _, _| {},
            |_, _| {},
            EditKind::Modal,
        ),

        // ── Backend ───────────────────────────────────────────────────────────
        field(
            "backend_version",
            "LLama.cpp Version",
            "Backend",
            |s| s.get_active_backend_version_display().to_string(),
            |s, c| s.get_active_backend_version() != c.get_active_backend_version(),
            |_, _, _| {},
            |_, _| {},
            EditKind::Modal,
        ),

        // ── Yarn RoPE ─────────────────────────────────────────────────────────
        field(
            "rope_yarn_enabled",
            "Yarn RoPE",
            "Yarn RoPE",
            |s| s.rope_yarn_enabled.to_string(),
            |s, c| s.rope_yarn_enabled != c.rope_yarn_enabled,
            |s, _, _| {
                s.rope_yarn_enabled = !s.rope_yarn_enabled;
            },
            |_, _| {},
            EditKind::Toggle,
        ),
        field(
            "yarn_params",
            "Yarn Params",
            "Yarn RoPE",
            |s| format!(
                "scale={:.2} base={:.2} scale_f={:.2}",
                s.rope_scale, s.rope_freq_base, s.rope_freq_scale
            ),
            |s, c| s.rope_scale != c.rope_scale
                || s.rope_freq_base != c.rope_freq_base
                || s.rope_freq_scale != c.rope_freq_scale,
            |_, _, _| {},
            |_, _| {},
            EditKind::Modal,
        ),

        // ── MTP ───────────────────────────────────────────────────────────────
        field(
            "is_mtp",
            "Enable MTP",
            "MTP (Multi-Token Prediction)",
            |s| s.is_mtp.to_string(),
            |s, c| s.is_mtp != c.is_mtp,
            |s, _, _| {
                s.is_mtp = !s.is_mtp;
            },
            |_, _| {},
            EditKind::Toggle,
        ),
        field(
            "draft_tokens",
            "Draft Tokens",
            "MTP (Multi-Token Prediction)",
            |s| s.draft_tokens.to_string(),
            |s, c| s.draft_tokens != c.draft_tokens,
            |s, delta, _| {
                s.draft_tokens = (s.draft_tokens as i32 + delta).max(0).min(16) as u32;
            },
            |s, buf| {
                if let Ok(v) = buf.parse::<u32>() {
                    s.draft_tokens = v.min(16);
                }
            },
            EditKind::Direct,
        ),
    ]
}

// ── Expert fields (27 fields) ───────────────────────────────────────────────

pub fn expert_fields() -> Vec<SettingField> {
    vec![
        // ── Loading (expert) ──────────────────────────────────────────────────
        field(
            "threads_batch",
            "Threads Batch",
            "Loading (expert)",
            |s| s.threads_batch.to_string(),
            |s, c| s.threads_batch != c.threads_batch,
            |s, delta, _| {
                s.threads_batch = (s.threads_batch as i32 + delta).max(1) as u32;
            },
            |s, buf| {
                if let Ok(v) = buf.parse::<u32>() {
                    s.threads_batch = v.max(1);
                }
            },
            EditKind::Direct,
        ),
        field(
            "ubatch_size",
            "UBatch Size",
            "Loading (expert)",
            |s| s.ubatch_size.to_string(),
            |s, c| s.ubatch_size != c.ubatch_size,
            |s, delta, _| {
                s.ubatch_size = (s.ubatch_size as i32 + delta * 64).max(1) as u32;
            },
            |s, buf| {
                if let Ok(v) = buf.parse::<u32>() {
                    s.ubatch_size = v.max(1);
                }
            },
            EditKind::Direct,
        ),
        field(
            "keep",
            "Keep",
            "Loading (expert)",
            |s| s.keep.to_string(),
            |s, c| s.keep != c.keep,
            |s, delta, _| {
                s.keep = s.keep + delta;
            },
            |s, buf| {
                if let Ok(v) = buf.parse::<i32>() {
                    s.keep = v;
                }
            },
            EditKind::Direct,
        ),
        field(
            "swa_full",
            "SWA Full",
            "Loading (expert)",
            |s| s.swa_full.to_string(),
            |s, c| s.swa_full != c.swa_full,
            |s, _, _| {
                s.swa_full = !s.swa_full;
            },
            |_, _| {},
            EditKind::Toggle,
        ),
        field(
            "mmap",
            "MMap",
            "Loading (expert)",
            |s| s.mmap.to_string(),
            |s, c| s.mmap != c.mmap,
            |s, _, _| {
                s.mmap = !s.mmap;
            },
            |_, _| {},
            EditKind::Toggle,
        ),
        field(
            "numa",
            "NUMA",
            "Loading (expert)",
            |s| s.numa.to_string(),
            |s, c| s.numa != c.numa,
            |s, delta, _| {
                use crate::models::NumMode;
                let mut val = s.numa;
                val = match (delta, val) {
                    (1, NumMode::None) => NumMode::Distribute,
                    (1, NumMode::Distribute) => NumMode::Isolate,
                    (1, NumMode::Isolate) => NumMode::Numactl,
                    (1, NumMode::Numactl) => NumMode::None,
                    (-1, NumMode::None) => NumMode::Numactl,
                    (-1, NumMode::Distribute) => NumMode::None,
                    (-1, NumMode::Isolate) => NumMode::Distribute,
                    (-1, NumMode::Numactl) => NumMode::Isolate,
                    _ => val,
                };
                s.numa = val;
            },
            |_, _| {},
            EditKind::Toggle,
        ),

        // ── GPU (expert) ──────────────────────────────────────────────────────
        field(
            "split_mode",
            "Split Mode",
            "GPU (expert)",
            |s| s.split_mode.to_string(),
            |s, c| s.split_mode != c.split_mode,
            |s, delta, _| {
                use crate::models::SplitMode;
                let mut val = s.split_mode;
                val = match (delta, val) {
                    (1, SplitMode::None) => SplitMode::Layer,
                    (1, SplitMode::Layer) => SplitMode::Row,
                    (1, SplitMode::Row) => SplitMode::Tensor,
                    (1, SplitMode::Tensor) => SplitMode::None,
                    (-1, SplitMode::None) => SplitMode::Tensor,
                    (-1, SplitMode::Layer) => SplitMode::None,
                    (-1, SplitMode::Row) => SplitMode::Layer,
                    (-1, SplitMode::Tensor) => SplitMode::Row,
                    _ => val,
                };
                s.split_mode = val;
            },
            |_, _| {},
            EditKind::Toggle,
        ),
        field(
            "tensor_split",
            "Tensor Split",
            "GPU (expert)",
            |s| s.tensor_split.clone(),
            |s, c| s.tensor_split != c.tensor_split,
            |_, _, _| {},
            |_, _| {},
            EditKind::Modal,
        ),
        field(
            "main_gpu",
            "Main GPU",
            "GPU (expert)",
            |s| s.main_gpu.to_string(),
            |s, c| s.main_gpu != c.main_gpu,
            |s, delta, _| {
                s.main_gpu = s.main_gpu + delta;
            },
            |s, buf| {
                if let Ok(v) = buf.parse::<i32>() {
                    s.main_gpu = v;
                }
            },
            EditKind::Direct,
        ),
        field(
            "fit",
            "Fit",
            "GPU (expert)",
            |s| s.fit.to_string(),
            |s, c| s.fit != c.fit,
            |s, _, _| {
                s.fit = !s.fit;
            },
            |_, _| {},
            EditKind::Toggle,
        ),
        field(
            "lora",
            "LoRA",
            "GPU (expert)",
            |s| s.lora
                .as_ref()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| "Disabled".to_string()),
            |s, c| s.lora != c.lora,
            |_, _, _| {},
            |_, _| {},
            EditKind::Modal,
        ),
        field(
            "lora_scaled",
            "LoRA Scaled",
            "GPU (expert)",
            |s| s.lora_scaled
                .as_ref()
                .map(|(p, _)| p.to_string_lossy().to_string())
                .unwrap_or_else(|| "Disabled".to_string()),
            |s, c| s.lora_scaled != c.lora_scaled,
            |_, _, _| {},
            |_, _| {},
            EditKind::Modal,
        ),
        field(
            "rpc",
            "RPC",
            "GPU (expert)",
            |s| s.rpc.clone(),
            |s, c| s.rpc != c.rpc,
            |_, _, _| {},
            |_, _| {},
            EditKind::Modal,
        ),
        field(
            "embedding",
            "Embedding",
            "GPU (expert)",
            |s| s.embedding.to_string(),
            |s, c| s.embedding != c.embedding,
            |s, _, _| {
                s.embedding = !s.embedding;
            },
            |_, _| {},
            EditKind::Toggle,
        ),

        // ── Sampling (expert) ─────────────────────────────────────────────────
        field(
            "typical_p",
            "Typical P",
            "Sampling (expert)",
            |s| format!("{:.2}", s.typical_p),
            |s, c| (s.typical_p - c.typical_p).abs() > 0.001,
            |s, delta, _| {
                s.typical_p = ((s.typical_p * 100.0 + delta as f32 * 5.0) / 100.0).clamp(0.0, 1.0);
            },
            |s, buf| {
                if let Ok(v) = buf.parse::<i32>() {
                    s.typical_p = (v as f32 / 100.0).clamp(0.0, 1.0);
                }
            },
            EditKind::Direct,
        ),
        field(
            "mirostat",
            "Mirostat",
            "Sampling (expert)",
            |s| s.mirostat.to_string(),
            |s, c| s.mirostat != c.mirostat,
            |s, delta, _| {
                use crate::models::Mirostat;
                let mut val = s.mirostat;
                val = match (delta, val) {
                    (1, Mirostat::Off) => Mirostat::Mirostat,
                    (1, Mirostat::Mirostat) => Mirostat::Mirostat2,
                    (1, Mirostat::Mirostat2) => Mirostat::Off,
                    (-1, Mirostat::Off) => Mirostat::Mirostat2,
                    (-1, Mirostat::Mirostat) => Mirostat::Off,
                    (-1, Mirostat::Mirostat2) => Mirostat::Mirostat,
                    _ => val,
                };
                s.mirostat = val;
            },
            |_, _| {},
            EditKind::Toggle,
        ),
        field(
            "mirostat_lr",
            "Mirostat LR",
            "Sampling (expert)",
            |s| format!("{:.2}", s.mirostat_lr),
            |s, c| (s.mirostat_lr - c.mirostat_lr).abs() > 0.001,
            |s, delta, _| {
                s.mirostat_lr = ((s.mirostat_lr * 100.0 + delta as f32 * 5.0) / 100.0).clamp(0.0, 1.0);
            },
            |s, buf| {
                if let Ok(v) = buf.parse::<i32>() {
                    s.mirostat_lr = (v as f32 / 100.0).clamp(0.0, 1.0);
                }
            },
            EditKind::Direct,
        ),
        field(
            "mirostat_ent",
            "Mirostat Ent",
            "Sampling (expert)",
            |s| format!("{:.2}", s.mirostat_ent),
            |s, c| (s.mirostat_ent - c.mirostat_ent).abs() > 0.001,
            |s, delta, _| {
                s.mirostat_ent = ((s.mirostat_ent * 100.0 + delta as f32 * 5.0) / 100.0).clamp(0.0, 10.0);
            },
            |s, buf| {
                if let Ok(v) = buf.parse::<i32>() {
                    s.mirostat_ent = (v as f32 / 100.0).clamp(0.0, 10.0);
                }
            },
            EditKind::Direct,
        ),
        field(
            "ignore_eos",
            "Ignore EOS",
            "Sampling (expert)",
            |s| s.ignore_eos.to_string(),
            |s, c| s.ignore_eos != c.ignore_eos,
            |s, _, _| {
                s.ignore_eos = !s.ignore_eos;
            },
            |_, _| {},
            EditKind::Toggle,
        ),
        field(
            "samplers",
            "Samplers",
            "Sampling (expert)",
            |s| s.samplers.0.clone(),
            |s, c| s.samplers.0 != c.samplers.0,
            |_, _, _| {},
            |_, _| {},
            EditKind::Modal,
        ),

        // ── DRY (expert) ──────────────────────────────────────────────────────
        field(
            "dry_multiplier",
            "DRY Multiplier",
            "DRY (expert)",
            |s| format!("{:.2}", s.dry_multiplier),
            |s, c| (s.dry_multiplier - c.dry_multiplier).abs() > 0.001,
            |s, delta, _| {
                s.dry_multiplier = ((s.dry_multiplier * 100.0 + delta as f32 * 5.0) / 100.0).clamp(0.0, 10.0);
            },
            |s, buf| {
                if let Ok(v) = buf.parse::<i32>() {
                    s.dry_multiplier = (v as f32 / 100.0).clamp(0.0, 10.0);
                }
            },
            EditKind::Direct,
        ),
        field(
            "dry_base",
            "DRY Base",
            "DRY (expert)",
            |s| format!("{:.2}", s.dry_base),
            |s, c| (s.dry_base - c.dry_base).abs() > 0.001,
            |s, delta, _| {
                s.dry_base = ((s.dry_base * 100.0 + delta as f32 * 5.0) / 100.0).clamp(0.0, 10.0);
            },
            |s, buf| {
                if let Ok(v) = buf.parse::<i32>() {
                    s.dry_base = (v as f32 / 100.0).clamp(0.0, 10.0);
                }
            },
            EditKind::Direct,
        ),
        field(
            "dry_allowed_length",
            "DRY Allowed Length",
            "DRY (expert)",
            |s| s.dry_allowed_length.to_string(),
            |s, c| s.dry_allowed_length != c.dry_allowed_length,
            |s, delta, _| {
                s.dry_allowed_length = s.dry_allowed_length + delta;
            },
            |s, buf| {
                if let Ok(v) = buf.parse::<i32>() {
                    s.dry_allowed_length = v;
                }
            },
            EditKind::Direct,
        ),
        field(
            "dry_penalty_last_n",
            "DRY Penalty Last N",
            "DRY (expert)",
            |s| s.dry_penalty_last_n.to_string(),
            |s, c| s.dry_penalty_last_n != c.dry_penalty_last_n,
            |s, delta, _| {
                s.dry_penalty_last_n = s.dry_penalty_last_n + delta;
            },
            |s, buf| {
                if let Ok(v) = buf.parse::<i32>() {
                    s.dry_penalty_last_n = v;
                }
            },
            EditKind::Direct,
        ),

        // ── Server (expert) ───────────────────────────────────────────────────
        field(
            "cache_prompt",
            "Cache Prompt",
            "Server (expert)",
            |s| s.cache_prompt.to_string(),
            |s, c| s.cache_prompt != c.cache_prompt,
            |s, _, _| {
                s.cache_prompt = !s.cache_prompt;
            },
            |_, _| {},
            EditKind::Toggle,
        ),
        field(
            "cache_reuse",
            "Cache Reuse",
            "Server (expert)",
            |s| s.cache_reuse.to_string(),
            |s, c| s.cache_reuse != c.cache_reuse,
            |s, delta, _| {
                s.cache_reuse = (s.cache_reuse as i32 + delta).max(0) as u32;
            },
            |s, buf| {
                if let Ok(v) = buf.parse::<u32>() {
                    s.cache_reuse = v;
                }
            },
            EditKind::Direct,
        ),
        field(
            "webui",
            "WebUI",
            "Server (expert)",
            |s| s.webui.to_string(),
            |s, c| s.webui != c.webui,
            |s, _, _| {
                s.webui = !s.webui;
            },
            |_, _| {},
            EditKind::Toggle,
        ),
    ]
}

// ── Simple helper for the server settings panel (tabbed.rs) ──────────────────

/// Render a single setting line for the server settings panel.
#[allow(clippy::too_many_arguments)]
pub fn add_setting(
    lines: &mut Vec<Line<'static>>,
    total_count: &mut usize,
    _settings: &crate::models::ModelSettings,
    _cached: &crate::models::ModelSettings,
    _selected_line_idx: &mut usize,
    _selected_content_line: &mut usize,
    _idx: usize,
    name: &str,
    val: &str,
    selected: usize,
    _edit_buf: &str,
    _editing: bool,
) {
    if *total_count == selected {
        lines.push(Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Yellow)),
            Span::styled(format!("{name}: "), Style::default().fg(Color::Yellow)),
            Span::styled(val.to_string(), Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::styled("  ", Style::default().fg(Color::Yellow)),
            Span::styled(format!("{name}: "), Style::default().fg(Color::Yellow)),
            Span::styled(val.to_string(), Style::default().fg(Color::White)),
        ]));
    }
    *total_count += 1;
}
