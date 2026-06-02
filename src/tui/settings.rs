use crate::config::Profile;
use crate::models::{
    CacheQuantType, GpuLayersMode, Mirostat, ModelSettings, NumMode, SplitMode,
};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

// ── Function pointer types (zero-cost, no dynamic dispatch) ──────────────────

pub type DisplayFn = fn(&ModelSettings) -> String;
pub type DirtyFn = fn(&ModelSettings, &ModelSettings) -> bool;
pub type AdjustFn = fn(&mut ModelSettings, i32, u32); // u32 = context_limit (0 = no limit)
pub type ApplyEditFn = fn(&mut ModelSettings, &str);
pub type CtrlEToggleFn = fn(&mut ModelSettings);

// ── Edit kinds ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
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
    pub is_expert: bool,
    pub is_ultra: bool,
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

// ── Helper constructors (generated from macro) ───────────────────────────────

/// Generate a field constructor function.
/// Variants:
///   - `field` / `expert_field` / `ultra_field` — no ctrl_e_toggle
///   - `field_with_toggle` / `expert_field_with_toggle` / `ultra_field_with_toggle` — with ctrl_e_toggle
macro_rules! make_field_fn {
    ($fn:ident, $expert:expr, $ultra:expr, toggle) => {
        fn $fn(
            id: &'static str,
            name: &'static str,
            section: &'static str,
            display: DisplayFn,
            dirty: DirtyFn,
            adjust: AdjustFn,
            apply_edit: ApplyEditFn,
            ctrl_e_toggle: CtrlEToggleFn,
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
                ctrl_e_toggle: Some(ctrl_e_toggle),
                edit_kind,
                is_expert: $expert,
                is_ultra: $ultra,
            }
        }
    };
    ($fn:ident, $expert:expr, $ultra:expr, @none) => {
        fn $fn(
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
                is_expert: $expert,
                is_ultra: $ultra,
            }
        }
    };
}

make_field_fn!(field, false, false, @none);
make_field_fn!(expert_field, true, false, @none);
make_field_fn!(ultra_field, true, true, @none);
make_field_fn!(field_with_toggle, false, false, toggle);
make_field_fn!(expert_field_with_toggle, true, false, toggle);
make_field_fn!(ultra_field_with_toggle, true, true, toggle);

// ── Shared adjustment and toggle logic ───────────────────────────────────────

fn gpu_layers_adjust(settings: &mut ModelSettings, delta: i32, _context_limit: u32) {
    settings.gpu_layers_mode = match (delta, &settings.gpu_layers_mode) {
        (1, GpuLayersMode::Auto) => GpuLayersMode::Specific(1),
        (1, GpuLayersMode::Specific(n)) => GpuLayersMode::Specific(n + 1),
        (1, GpuLayersMode::All) => GpuLayersMode::Auto,
        (-1, GpuLayersMode::Auto) => GpuLayersMode::Auto,
        (-1, GpuLayersMode::Specific(n)) if *n == 0 => GpuLayersMode::Auto,
        (-1, GpuLayersMode::Specific(n)) if *n == 1 => GpuLayersMode::Specific(0),
        (-1, GpuLayersMode::Specific(n)) => GpuLayersMode::Specific(n - 1),
        (-1, GpuLayersMode::All) => GpuLayersMode::All,
        _ => settings.gpu_layers_mode,
    };
}

fn gpu_layers_apply(settings: &mut ModelSettings, buf: &str) {
    if let Ok(v) = buf.parse::<i32>() {
        settings.gpu_layers_mode = if v < 0 {
            GpuLayersMode::All
        } else {
            GpuLayersMode::Specific(v as u32)
        };
    }
}

fn toggle_mlock(settings: &mut ModelSettings) {
    settings.mlock = !settings.mlock;
}
fn toggle_flash_attn(settings: &mut ModelSettings) {
    settings.flash_attn = !settings.flash_attn;
}

fn toggle_fit(settings: &mut ModelSettings) {
    settings.fit = !settings.fit;
}
fn toggle_kv_cache_offload(settings: &mut ModelSettings) {
    settings.kv_cache_offload = !settings.kv_cache_offload;
}
fn toggle_uniform_cache(settings: &mut ModelSettings) {
    settings.uniform_cache = !settings.uniform_cache;
}
fn toggle_mtp(settings: &mut ModelSettings) {
    if settings.spec_type.is_empty() {
        settings.spec_type = "draft-mtp".to_string();
    } else {
        settings.spec_type = String::new();
    }
}

fn toggle_rope_yarn_enabled(settings: &mut ModelSettings) {
    settings.rope_yarn_enabled = !settings.rope_yarn_enabled;
}
fn toggle_ignore_eos(settings: &mut ModelSettings) {
    settings.ignore_eos = !settings.ignore_eos;
}
fn toggle_max_tokens(settings: &mut ModelSettings) {
    settings.max_tokens = settings.max_tokens.map_or(Some(2048), |_| None);
}
fn toggle_max_concurrent_predictions(settings: &mut ModelSettings) {
    settings.max_concurrent_predictions = settings
        .max_concurrent_predictions
        .map_or(Some(1), |_| None);
}
fn toggle_cache_type_k(settings: &mut ModelSettings) {
    settings.cache_type_k = settings.cache_type_k.map_or(Some(CacheQuantType::F16), |_| None);
}
fn toggle_cache_type_v(settings: &mut ModelSettings) {
    settings.cache_type_v = settings.cache_type_v.map_or(Some(CacheQuantType::F16), |_| None);
}
fn toggle_expert_count(settings: &mut ModelSettings) {
    settings.expert_count = match settings.expert_count {
        0 => 1,
        -1 => 0,
        _ => -1,
    };
}
fn toggle_presence_penalty(settings: &mut ModelSettings) {
    settings.presence_penalty = settings.presence_penalty.map_or(Some(0.0), |_| None);
}
fn toggle_frequency_penalty(settings: &mut ModelSettings) {
    settings.frequency_penalty = settings.frequency_penalty.map_or(Some(0.0), |_| None);
}

// ── Diff macros for profile settings comparison ──────────────────────────────

macro_rules! diff_int {
    ($parts:expr, $s:expr, $c:expr, $field:ident, $label:literal) => {
        if let Some(v) = $s.$field && v != $c.$field {
            $parts.push(format!("{}={}", $label, v));
        }
    };
}
macro_rules! diff_float {
    ($parts:expr, $s:expr, $c:expr, $field:ident, $label:literal) => {
        if let Some(v) = $s.$field && (v - $c.$field).abs() > 0.001 {
            $parts.push(format!("{}={:.2}", $label, v));
        }
    };
}
macro_rules! diff_bool {
    ($parts:expr, $s:expr, $c:expr, $field:ident, $label:literal) => {
        if let Some(v) = $s.$field && v != $c.$field {
            $parts.push(format!("{}={}", $label, v));
        }
    };
}
macro_rules! diff_string {
    ($parts:expr, $s:expr, $c:expr, $field:ident, $label:literal) => {
        if let Some(v) = &$s.$field && v != &$c.$field {
            $parts.push(format!("{}={}", $label, v));
        }
    };
}
macro_rules! diff_enum {
    ($parts:expr, $s:expr, $c:expr, $field:ident, $label:literal) => {
        if let Some(ref v) = $s.$field && *v != $c.$field {
            $parts.push(format!("{}={}", $label, v));
        }
    };
}
macro_rules! diff_option {
    ($parts:expr, $s:expr, $c:expr, $field:ident, $label:literal) => {
        if $s.$field != $c.$field {
            if let Some(ref v) = $s.$field {
                $parts.push(format!("{}={}", $label, v));
            }
        }
    };
}
macro_rules! diff_option_float {
    ($parts:expr, $s:expr, $c:expr, $field:ident, $label:literal) => {
        if let Some(v) = $s.$field {
            let current_val = $c.$field.unwrap_or(0.0);
            if (v - current_val).abs() > 0.001 {
                $parts.push(format!("{}={:.2}", $label, v));
            }
        }
    };
}

// ── All Fields (Interleaved for context-aware expert mode) ────────────────────

macro_rules! make_cache_type_field {
    ($field:ident, $name:literal, $toggle:ident) => {
        expert_field_with_toggle(
            stringify!($field),
            $name,
            "GPU Offload",
            |s| s.$field.map(|v| v.to_string()).unwrap_or_else(|| "Disabled".to_string()),
            |s, c| s.$field != c.$field,
            |s, delta, _| {
                let mut val = s.$field.unwrap_or(CacheQuantType::F16);
                val = if delta > 0 { val.next() } else { val.prev() };
                s.$field = Some(val);
            },
            |s, buf| {
                if let Ok(n) = buf.parse::<u8>() {
                    s.$field = Some(CacheQuantType::from_u8(n));
                }
            },
            $toggle,
            EditKind::Direct,
        )
    };
}

pub fn all_fields() -> Vec<SettingField> {
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
                    s.context_length = v.max(128);
                }
            },
            EditKind::Direct,
        ),
        expert_field_with_toggle(
            "rope_yarn_enabled",
            "Yarn RoPE",
            "Loading",
            |s| s.rope_yarn_enabled.to_string(),
            |s, c| s.rope_yarn_enabled != c.rope_yarn_enabled,
            |_, _, _| {},
            |_, _| {},
            toggle_rope_yarn_enabled,
            EditKind::Toggle,
        ),
        expert_field(
            "yarn_params",
            "Yarn Params",
            "Loading",
            |s| {
                format!(
                    "scale={:.2} base={:.2} scale_f={:.2}",
                    s.rope_scale, s.rope_freq_base, s.rope_freq_scale
                )
            },
            |s, c| {
                s.rope_scale != c.rope_scale
                    || s.rope_freq_base != c.rope_freq_base
                    || s.rope_freq_scale != c.rope_freq_scale
            },
            |_, _, _| {},
            |_, _| {},
            EditKind::Modal,
        ),
        ultra_field(
            "threads_batch",
            "Threads Batch",
            "Loading",
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
        ultra_field(
            "ubatch_size",
            "UBatch Size",
            "Loading",
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
        ultra_field(
            "keep",
            "Keep",
            "Loading",
            |s| s.keep.to_string(),
            |s, c| s.keep != c.keep,
            |s, delta, _| {
                s.keep = (s.keep + delta).max(0);
            },
            |s, buf| {
                if let Ok(v) = buf.parse::<i32>() {
                    s.keep = v;
                }
            },
            EditKind::Direct,
        ),
        field_with_toggle(
            "mlock",
            "Keep in memory (mlock)",
            "Loading",
            |s| s.mlock.to_string(),
            |s, c| s.mlock != c.mlock,
            |_, _, _| {},
            |_, _| {},
            toggle_mlock,
            EditKind::Toggle,
        ),
        expert_field(
            "numa",
            "NUMA",
            "Loading",
            |s| s.numa.to_string(),
            |s, c| s.numa != c.numa,
            |s, delta, _| {
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
        // ── GPU Offload ───────────────────────────────────────────────────────
        field(
            "gpu_layers_mode",
            "GPU Layers",
            "GPU Offload",
            |s| match s.gpu_layers_mode {
                GpuLayersMode::Auto => "Auto".to_string(),
                GpuLayersMode::Specific(n) => n.to_string(),
                GpuLayersMode::All => "All".to_string(),
            },
            |s, c| s.gpu_layers_mode != c.gpu_layers_mode,
            gpu_layers_adjust,
            gpu_layers_apply,
            EditKind::Direct,
        ),
        ultra_field(
            "split_mode",
            "Split Mode",
            "GPU Offload",
            |s| s.split_mode.to_string(),
            |s, c| s.split_mode != c.split_mode,
            |s, delta, _| {
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
        ultra_field(
            "tensor_split",
            "Tensor Split",
            "GPU Offload",
            |s| s.tensor_split.clone(),
            |s, c| s.tensor_split != c.tensor_split,
            |_, _, _| {},
            |_, _| {},
            EditKind::Modal,
        ),
        expert_field(
            "main_gpu",
            "Main GPU",
            "GPU Offload",
            |s| s.main_gpu.to_string(),
            |s, c| s.main_gpu != c.main_gpu,
            |s, delta, _| {
                s.main_gpu = (s.main_gpu + delta).max(0);
            },
            |s, buf| {
                if let Ok(v) = buf.parse::<i32>() {
                    s.main_gpu = v;
                }
            },
            EditKind::Direct,
        ),
        field_with_toggle(
            "fit",
            "Fit",
            "GPU Offload",
            |s| s.fit.to_string(),
            |s, c| s.fit != c.fit,
            |_, _, _| {},
            |_, _| {},
            toggle_fit,
            EditKind::Toggle,
        ),
        field_with_toggle(
            "flash_attn",
            "Flash Attention",
            "GPU Offload",
            |s| s.flash_attn.to_string(),
            |s, c| s.flash_attn != c.flash_attn,
            |_, _, _| {},
            |_, _| {},
            toggle_flash_attn,
            EditKind::Toggle,
        ),
        field_with_toggle(
            "kv_cache_offload",
            "KV Cache Offload",
            "GPU Offload",
            |s| s.kv_cache_offload.to_string(),
            |s, c| s.kv_cache_offload != c.kv_cache_offload,
            |_, _, _| {},
            |_, _| {},
            toggle_kv_cache_offload,
            EditKind::Toggle,
        ),
        // ── Cache type fields ──────────────────────────────────────────────────

        make_cache_type_field!(cache_type_k, "Cache Type K", toggle_cache_type_k),
        make_cache_type_field!(cache_type_v, "Cache Type V", toggle_cache_type_v),
        expert_field_with_toggle(
            "expert_count",
            "Active Experts",
            "GPU Offload",
            |s| {
                if s.expert_count > 0 {
                    s.expert_count.to_string()
                } else if s.expert_count == -1 {
                    "Auto".to_string()
                } else {
                    "Disabled".to_string()
                }
            },
            |s, c| s.expert_count != c.expert_count,
            |s, delta, _| {
                s.expert_count = (s.expert_count + delta).clamp(-1, 99);
            },
            |s, buf| {
                if let Ok(v) = buf.parse::<i32>() {
                    s.expert_count = v.clamp(-1, 99);
                }
            },
            toggle_expert_count,
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
        field_with_toggle(
            "uniform_cache",
            "Unified KV",
            "Evaluation",
            |s| s.uniform_cache.to_string(),
            |s, c| s.uniform_cache != c.uniform_cache,
            |_, _, _| {},
            |_, _| {},
            toggle_uniform_cache,
            EditKind::Toggle,
        ),
        expert_field_with_toggle(
            "max_concurrent_predictions",
            "Max Concurrent Pred",
            "Evaluation",
            |s| {
                s.max_concurrent_predictions
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "Off".to_string())
            },
            |s, c| s.max_concurrent_predictions != c.max_concurrent_predictions,
            |s, delta, _| match s.max_concurrent_predictions {
                Some(n) => {
                    s.max_concurrent_predictions = Some(((n as i32) + delta).clamp(1, 10) as u32)
                }
                None => s.max_concurrent_predictions = Some(1),
            },
            |s, buf| {
                if let Ok(v) = buf.parse::<u32>() {
                    s.max_concurrent_predictions = Some(v.clamp(1, 10));
                }
            },
            toggle_max_concurrent_predictions,
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
                s.temperature =
                    ((s.temperature * 100.0 + delta as f32 * 5.0) / 100.0).clamp(0.0, 2.0);
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
        ultra_field(
            "typical_p",
            "Typical P",
            "Sampling",
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
        ultra_field(
            "mirostat",
            "Mirostat",
            "Sampling",
            |s| s.mirostat.to_string(),
            |s, c| s.mirostat != c.mirostat,
            |s, delta, _| {
                let mut val = s.mirostat;
                val = match (delta, val) {
                    (1, Mirostat::Off) => Mirostat::V1,
                    (1, Mirostat::V1) => Mirostat::Mirostat2,
                    (1, Mirostat::Mirostat2) => Mirostat::Off,
                    (-1, Mirostat::Off) => Mirostat::Mirostat2,
                    (-1, Mirostat::V1) => Mirostat::Off,
                    (-1, Mirostat::Mirostat2) => Mirostat::V1,
                    _ => val,
                };
                s.mirostat = val;
            },
            |_, _| {},
            EditKind::Toggle,
        ),
        ultra_field(
            "mirostat_lr",
            "Mirostat LR",
            "Sampling",
            |s| format!("{:.2}", s.mirostat_lr),
            |s, c| (s.mirostat_lr - c.mirostat_lr).abs() > 0.001,
            |s, delta, _| {
                s.mirostat_lr =
                    ((s.mirostat_lr * 100.0 + delta as f32 * 5.0) / 100.0).clamp(0.0, 1.0);
            },
            |s, buf| {
                if let Ok(v) = buf.parse::<i32>() {
                    s.mirostat_lr = (v as f32 / 100.0).clamp(0.0, 1.0);
                }
            },
            EditKind::Direct,
        ),
        ultra_field(
            "mirostat_ent",
            "Mirostat Ent",
            "Sampling",
            |s| format!("{:.2}", s.mirostat_ent),
            |s, c| (s.mirostat_ent - c.mirostat_ent).abs() > 0.001,
            |s, delta, _| {
                s.mirostat_ent =
                    ((s.mirostat_ent * 100.0 + delta as f32 * 5.0) / 100.0).clamp(0.0, 10.0);
            },
            |s, buf| {
                if let Ok(v) = buf.parse::<i32>() {
                    s.mirostat_ent = (v as f32 / 100.0).clamp(0.0, 10.0);
                }
            },
            EditKind::Direct,
        ),
        ultra_field_with_toggle(
            "ignore_eos",
            "Ignore EOS",
            "Sampling",
            |s| s.ignore_eos.to_string(),
            |s, c| s.ignore_eos != c.ignore_eos,
            |_, _, _| {},
            |_, _| {},
            toggle_ignore_eos,
            EditKind::Toggle,
        ),
        ultra_field(
            "samplers",
            "Samplers",
            "Sampling",
            |s| s.samplers.0.clone(),
            |s, c| s.samplers.0 != c.samplers.0,
            |_, _, _| {},
            |_, _| {},
            EditKind::Modal,
        ),
        field_with_toggle(
            "max_tokens",
            "Max Tokens",
            "Sampling",
            |s| {
                s.max_tokens
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "Disabled".to_string())
            },
            |s, c| s.max_tokens != c.max_tokens,
            |s, delta, _| {
                let current = s.max_tokens.unwrap_or(2048);
                s.max_tokens = Some((current as i32 + delta * 16).max(16) as u32);
            },
            |s, buf| {
                if let Ok(v) = buf.parse::<i32>() {
                    s.max_tokens = if v == 0 { None } else { Some(v as u32) };
                }
            },
            toggle_max_tokens,
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
                s.repeat_penalty =
                    ((s.repeat_penalty * 100.0 + delta as f32 * 5.0) / 100.0).clamp(1.0, 2.0);
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
                s.repeat_last_n = (s.repeat_last_n + delta).max(0);
            },
            |s, buf| {
                if let Ok(v) = buf.parse::<i32>() {
                    s.repeat_last_n = v.max(0);
                }
            },
            EditKind::Direct,
        ),
        expert_field_with_toggle(
            "presence_penalty",
            "Presence Penalty",
            "Repetition",
            |s| {
                s.presence_penalty
                    .map(|v| {
                        if (v - 0.0).abs() < 0.001 {
                            "Off".to_string()
                        } else {
                            format!("{:.2}", v)
                        }
                    })
                    .unwrap_or_else(|| "Off".to_string())
            },
            |s, c| match (s.presence_penalty, c.presence_penalty) {
                (Some(v1), Some(v2)) => (v1 - v2).abs() > 0.001,
                (None, None) => false,
                _ => true,
            },
            |s, delta, _| {
                let current = s.presence_penalty.unwrap_or(0.0);
                s.presence_penalty =
                    Some(((current * 100.0 + delta as f32 * 5.0) / 100.0).clamp(-2.0, 2.0));
            },
            |s, buf| {
                if let Ok(v) = buf.parse::<i32>() {
                    s.presence_penalty = Some((v as f32 / 100.0).clamp(0.0, 1.0));
                }
            },
            toggle_presence_penalty,
            EditKind::Direct,
        ),
        expert_field_with_toggle(
            "frequency_penalty",
            "Freq Penalty",
            "Repetition",
            |s| {
                s.frequency_penalty
                    .map(|v| {
                        if (v - 0.0).abs() < 0.001 {
                            "Off".to_string()
                        } else {
                            format!("{:.2}", v)
                        }
                    })
                    .unwrap_or_else(|| "Off".to_string())
            },
            |s, c| match (s.frequency_penalty, c.frequency_penalty) {
                (Some(v1), Some(v2)) => (v1 - v2).abs() > 0.001,
                (None, None) => false,
                _ => true,
            },
            |s, delta, _| {
                let current = s.frequency_penalty.unwrap_or(0.0);
                s.frequency_penalty =
                    Some(((current * 100.0 + delta as f32 * 5.0) / 100.0).clamp(-2.0, 2.0));
            },
            |s, buf| {
                if let Ok(v) = buf.parse::<i32>() {
                    s.frequency_penalty = Some((v as f32 / 100.0).clamp(0.0, 1.0));
                }
            },
            toggle_frequency_penalty,
            EditKind::Direct,
        ),
        // ── DRY ───────────────────────────────────────────────────────────────
        ultra_field(
            "dry_multiplier",
            "DRY Multiplier",
            "DRY",
            |s| format!("{:.2}", s.dry_multiplier),
            |s, c| (s.dry_multiplier - c.dry_multiplier).abs() > 0.001,
            |s, delta, _| {
                s.dry_multiplier =
                    ((s.dry_multiplier * 100.0 + delta as f32 * 5.0) / 100.0).clamp(0.0, 10.0);
            },
            |s, buf| {
                if let Ok(v) = buf.parse::<i32>() {
                    s.dry_multiplier = (v as f32 / 100.0).clamp(0.0, 10.0);
                }
            },
            EditKind::Direct,
        ),
        ultra_field(
            "dry_base",
            "DRY Base",
            "DRY",
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
        ultra_field(
            "dry_allowed_length",
            "DRY Allowed Length",
            "DRY",
            |s| s.dry_allowed_length.to_string(),
            |s, c| s.dry_allowed_length != c.dry_allowed_length,
            |s, delta, _| {
                s.dry_allowed_length = (s.dry_allowed_length + delta).max(0);
            },
            |s, buf| {
                if let Ok(v) = buf.parse::<i32>() {
                    s.dry_allowed_length = v;
                }
            },
            EditKind::Direct,
        ),
        ultra_field(
            "dry_penalty_last_n",
            "DRY Penalty Last N",
            "DRY",
            |s| s.dry_penalty_last_n.to_string(),
            |s, c| s.dry_penalty_last_n != c.dry_penalty_last_n,
            |s, delta, _| {
                s.dry_penalty_last_n = (s.dry_penalty_last_n + delta).max(0);
            },
            |s, buf| {
                if let Ok(v) = buf.parse::<i32>() {
                    s.dry_penalty_last_n = v;
                }
            },
            EditKind::Direct,
        ),
        // ── Speculative Decoding ─────────────────────────────────────────────
        expert_field_with_toggle(
            "is_mtp",
            "MTP",
            "Speculative",
            |s| {
                if s.spec_type.is_empty() {
                    "Off".to_string()
                } else {
                    s.spec_type.clone()
                }
            },
            |s, c| s.spec_type != c.spec_type,
            |_, _, _| {},
            |_, _| {},
            toggle_mtp,
            EditKind::Modal,
        ),
        expert_field(
            "spec_type",
            "Spec Type",
            "Speculative",
            |s| {
                if s.spec_type.is_empty() {
                    "Off".to_string()
                } else {
                    s.spec_type.clone()
                }
            },
            |s, c| s.spec_type != c.spec_type,
            |_, _, _| {},
            |_, _| {},
            EditKind::Modal,
        ),
        expert_field(
            "draft_tokens",
            "Spec Draft N Max",
            "Speculative",
            |s| s.draft_tokens.to_string(),
            |s, c| s.draft_tokens != c.draft_tokens,
            |s, delta, _| {
                s.draft_tokens = (s.draft_tokens as i32 + delta).clamp(0, 16) as u32;
            },
            |s, buf| {
                if let Ok(v) = buf.parse::<u32>() {
                    s.draft_tokens = v.min(16);
                }
            },
            EditKind::Direct,
        ),
        // ── Tags ──────────────────────────────────────────────────────────────
        field(
            "tags",
            "Tags (Enter to edit)",
            "Tags",
            |s| {
                if s.tags.is_empty() {
                    "None".to_string()
                } else {
                    s.tags.join(", ")
                }
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
    ]
}

pub fn filtered_fields(expert_mode: bool) -> Vec<SettingField> {
    all_fields()
        .into_iter()
        .filter(|f| {
            if !expert_mode {
                !f.is_expert
            } else {
                !f.is_ultra // In expert mode, hide ultra experts
            }
        })
        .collect()
}

// ── Simple helper for the server settings panel (tabbed.rs) ──────────────────

/// Render a single setting line for the server settings panel.
#[allow(clippy::too_many_arguments)]
pub fn add_setting(
    lines: &mut Vec<Line<'static>>,
    total_count: &mut usize,
    _settings: &ModelSettings,
    _cached: &ModelSettings,
    selected_line_idx: &mut usize,
    selected_content_line: &mut usize,
    idx: usize,
    name: &str,
    val: &str,
    selected: usize,
    _edit_buf: &str,
    _editing: bool,
    disabled: bool,
) {
    let current_line = lines.len();
    let name_style = if disabled {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().fg(Color::Yellow)
    };
    let val_style = if disabled {
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::DIM)
    } else {
        Style::default().fg(Color::White)
    };
    if idx == selected {
        *selected_line_idx = current_line;
        *selected_content_line = current_line;
        lines.push(Line::from(vec![
            Span::styled(
                "> ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(if disabled {
                        Modifier::DIM
                    } else {
                        Modifier::BOLD
                    }),
            ),
            Span::styled(format!("{name}: "), name_style),
            Span::styled(
                val.to_string(),
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::styled("  ", name_style),
            Span::styled(format!("{name}: "), name_style),
            Span::styled(val.to_string(), val_style),
        ]));
    }
    *total_count += 1;
}

/// Build a list of setting names that differ between a profile and the current settings.
pub fn profile_settings_parts(profile: &Profile, current: &ModelSettings) -> Vec<String> {
    let mut parts = Vec::new();
    let s = &profile.settings;

    // ── Integers ──────────────────────────────────────────────────────────
    diff_int!(parts, s, current, context_length, "ctx");
    diff_int!(parts, s, current, threads, "threads");
    diff_int!(parts, s, current, threads_batch, "threads_batch");
    diff_int!(parts, s, current, batch_size, "batch");
    diff_int!(parts, s, current, ubatch_size, "ubatch");
    diff_int!(parts, s, current, parallel, "parallel");
    diff_option!(parts, s, current, max_concurrent_predictions, "concurrent");
    diff_int!(parts, s, current, cache_reuse, "cache_reuse");
    diff_option!(parts, s, current, max_tokens, "max_tokens");
    diff_int!(parts, s, current, draft_tokens, "draft_tokens");
    diff_int!(parts, s, current, ws_server_port, "ws_port");

    diff_int!(parts, s, current, keep, "keep");
    diff_int!(parts, s, current, main_gpu, "main_gpu");
    diff_int!(parts, s, current, expert_count, "expert_count");
    diff_int!(parts, s, current, seed, "seed");
    diff_int!(parts, s, current, top_k, "top_k");
    diff_int!(parts, s, current, repeat_last_n, "repeat_last_n");
    diff_int!(parts, s, current, dry_allowed_length, "dry_allowed");
    diff_int!(parts, s, current, dry_penalty_last_n, "dry_penalty_last_n");

    // ── Floats ────────────────────────────────────────────────────────────
    diff_float!(parts, s, current, temperature, "temp");
    diff_float!(parts, s, current, top_p, "top_p");
    diff_float!(parts, s, current, min_p, "min_p");
    diff_float!(parts, s, current, typical_p, "typical_p");
    diff_float!(parts, s, current, mirostat_lr, "mirostat_lr");
    diff_float!(parts, s, current, mirostat_ent, "mirostat_ent");
    diff_float!(parts, s, current, repeat_penalty, "rep_pen");
    diff_option_float!(parts, s, current, presence_penalty, "pres_pen");
    diff_option_float!(parts, s, current, frequency_penalty, "freq_pen");
    diff_float!(parts, s, current, dry_multiplier, "dry_mult");
    diff_float!(parts, s, current, dry_base, "dry_base");
    diff_float!(parts, s, current, rope_scale, "rope_scale");
    diff_float!(parts, s, current, rope_freq_base, "rope_freq_base");
    diff_float!(parts, s, current, rope_freq_scale, "rope_freq_scale");

    // ── Bools ─────────────────────────────────────────────────────────────
    diff_bool!(parts, s, current, swa_full, "swa_full");
    diff_bool!(parts, s, current, mlock, "mlock");
    diff_bool!(parts, s, current, mmap, "mmap");
    diff_bool!(parts, s, current, uniform_cache, "uniform_cache");
    diff_bool!(parts, s, current, kv_cache_offload, "kv_cache_offload");
    diff_bool!(parts, s, current, fit, "fit");
    diff_bool!(parts, s, current, embedding, "embedding");
    diff_bool!(parts, s, current, flash_attn, "flash_attn");
    diff_bool!(parts, s, current, jinja, "jinja");
    diff_bool!(parts, s, current, ignore_eos, "ignore_eos");
    diff_bool!(parts, s, current, rope_yarn_enabled, "yarn_enabled");
    diff_bool!(parts, s, current, cache_prompt, "cache_prompt");
    diff_bool!(parts, s, current, webui, "webui");
    diff_bool!(parts, s, current, ws_server_enabled, "ws_enabled");
    diff_bool!(parts, s, current, ws_server_tls_enabled, "ws_tls");

    // ── Strings ───────────────────────────────────────────────────────────
    diff_string!(parts, s, current, system_prompt_preset_name, "preset");
    diff_string!(parts, s, current, tensor_split, "tensor_split");
    diff_string!(parts, s, current, rpc, "rpc");
    diff_option!(parts, s, current, chat_template, "chat_template");
    diff_option!(parts, s, current, chat_template_kwargs, "chat_template_kwargs");
    diff_option!(parts, s, current, ws_server_auth_key, "ws_key");
    diff_option!(parts, s, current, ws_server_tls_cert, "ws_cert");
    diff_option!(parts, s, current, ws_server_tls_key, "ws_key_file");
    diff_option!(parts, s, current, llama_cpp_version_cpu, "llama_cpp_cpu");
    diff_option!(parts, s, current, llama_cpp_version_vulkan, "llama_cpp_vulkan");
    diff_option!(parts, s, current, llama_cpp_version_rocm, "llama_cpp_rocm");
    diff_option!(parts, s, current, llama_cpp_version_rocm_lemonade, "llama_cpp_rocm_lemonade");
    diff_option!(parts, s, current, llama_cpp_version_cuda, "llama_cpp_cuda");
    diff_string!(parts, s, current, spec_type, "spec_type");

    // ── Enums ─────────────────────────────────────────────────────────────
    diff_enum!(parts, s, current, numa, "numa");
    diff_enum!(parts, s, current, split_mode, "split_mode");
    diff_enum!(parts, s, current, mirostat, "mirostat");
    diff_enum!(parts, s, current, samplers, "samplers");
    diff_enum!(parts, s, current, rope_scaling, "rope_scaling");
    diff_enum!(parts, s, current, cache_type, "cache_type");
    diff_option!(parts, s, current, cache_type_k, "cache_type_k");
    diff_option!(parts, s, current, cache_type_v, "cache_type_v");

    // ── Special (custom display) ──────────────────────────────────────────
    if let Some(v) = s.gpu_layers_mode && v != current.gpu_layers_mode {
        let display = match v {
            crate::models::GpuLayersMode::Auto => "Auto".to_string(),
            crate::models::GpuLayersMode::Specific(n) => n.to_string(),
            crate::models::GpuLayersMode::All => "All".to_string(),
        };
        parts.push(format!("gpu_layers={}", display));
    }

    parts
}
