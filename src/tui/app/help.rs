use super::types::{ActivePanel, App};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

impl App {
    pub fn panel_help_lines(&self) -> Vec<Line<'static>> {
        let y = Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD);

        match self.ui.active_panel {
            ActivePanel::Models => vec![
                Line::from(Span::styled(crate::t!("panel.title.models"), y)),
                Line::from(""),
                Line::from(crate::t!("panel.help.models.description")),
                Line::from(""),
                Line::from(vec![
                    Span::styled("j / k / Arrow keys", y),
                    Span::raw(format!("  {}", crate::t!("panel.help.models.navigate"))),
                ]),
                Line::from(vec![
                    Span::styled("↵ / l", y),
                    Span::raw(format!("  {}", crate::t!("panel.help.models.load"))),
                ]),
                Line::from(vec![
                    Span::styled("u", y),
                    Span::raw(format!("  {}", crate::t!("panel.help.models.unload"))),
                ]),
                Line::from(vec![
                    Span::styled("^D / Del", y),
                    Span::raw(format!("  {}", crate::t!("panel.help.models.delete"))),
                ]),
                Line::from(""),
                Line::from(crate::t!("panel.help.models.search_mode")),
                Line::from(vec![
                    Span::styled("↵", y),
                    Span::raw(format!(
                        "  {}",
                        crate::t!("panel.help.models.search_execute")
                    )),
                ]),
                Line::from(vec![
                    Span::styled("⎋", y),
                    Span::raw(format!("  {}", crate::t!("panel.help.models.search_exit"))),
                ]),
                Line::from(vec![
                    Span::styled("l", y),
                    Span::raw(format!("  {}", crate::t!("panel.help.models.view_files"))),
                ]),
                Line::from(vec![
                    Span::styled("S", y),
                    Span::raw(format!("  {}", crate::t!("panel.help.models.cycle_sort"))),
                ]),
                Line::from(vec![
                    Span::styled("B", y),
                    Span::raw(format!("  {}", crate::t!("panel.help.models.back_page"))),
                ]),
                Line::from(vec![
                    Span::styled("Down at bottom", y),
                    Span::raw(format!("  {}", crate::t!("panel.help.models.load_more"))),
                ]),
                Line::from(vec![
                    Span::styled("R", y),
                    Span::raw(format!("  {}", crate::t!("panel.help.models.readme"))),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Shift+← / →", y),
                    Span::raw(format!("  {}", crate::t!("panel.help.models.resize"))),
                ]),
                Line::from(vec![
                    Span::styled("Mouse drag on border", y),
                    Span::raw(format!("  {}", crate::t!("panel.help.models.resize_mouse"))),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Shift+a", y),
                    Span::raw(format!("  {}", crate::t!("panel.help.models.about"))),
                ]),
            ],
            ActivePanel::Log => vec![
                Line::from(Span::styled(crate::t!("panel.title.log"), y)),
                Line::from(""),
                Line::from(crate::t!("panel.help.log.description")),
                Line::from(""),
                Line::from(vec![
                    Span::styled("j / k / Arrow keys", y),
                    Span::raw(format!("  {}", crate::t!("panel.help.log.scroll"))),
                ]),
                Line::from(vec![
                    Span::styled("f", y),
                    Span::raw(format!("  {}", crate::t!("panel.help.log.toggle_follow"))),
                ]),
                Line::from(vec![
                    Span::styled("g", y),
                    Span::raw(format!("  {}", crate::t!("panel.help.log.jump_top"))),
                ]),
                Line::from(vec![
                    Span::styled("G", y),
                    Span::raw(format!("  {}", crate::t!("panel.help.log.jump_bottom"))),
                ]),
                Line::from(vec![
                    Span::styled("↵", y),
                    Span::raw(format!("  {}", crate::t!("panel.help.log.expand"))),
                ]),
                Line::from(vec![
                    Span::styled("⎋", y),
                    Span::raw(format!("  {}", crate::t!("panel.help.log.collapse"))),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Shift+a", y),
                    Span::raw(format!("  {}", crate::t!("panel.help.models.about"))),
                ]),
            ],
            ActivePanel::ServerSettings => {
                vec![
                    Line::from(Span::styled(crate::t!("panel.title.server"), y)),
                    Line::from(""),
                    Line::from(crate::t!("panel.help.server.description")),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("j / k", y),
                        Span::raw(format!("  {}", crate::t!("panel.help.server.select"))),
                    ]),
                    Line::from(vec![
                        Span::styled("↵", y),
                        Span::raw(format!("  {}", crate::t!("panel.help.server.toggle"))),
                    ]),
                    Line::from(vec![
                        Span::styled("Left / Right", y),
                        Span::raw(format!("  {}", crate::t!("panel.help.server.adjust"))),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("Host", y),
                        Span::raw(format!("  {}", crate::t!("panel.help.server.host_desc"))),
                    ]),
                    Line::from(vec![
                        Span::styled("Backend", y),
                        Span::raw(format!("  {}", crate::t!("panel.help.server.backend_desc"))),
                    ]),
                    Line::from(vec![
                        Span::styled("Threads", y),
                        Span::raw(format!("  {}", crate::t!("panel.help.server.threads_desc"))),
                    ]),
                    Line::from(vec![
                        Span::styled("Threads Batch", y),
                        Span::raw(format!(
                            "  {}",
                            crate::t!("panel.help.server.threads_batch_desc")
                        )),
                    ]),
                    Line::from(vec![
                        Span::styled("Mode", y),
                        Span::raw(format!("  {}", crate::t!("panel.help.server.mode_desc"))),
                    ]),
                    Line::from(vec![
                        Span::styled("API Endpoint", y),
                        Span::raw(format!(
                            "  {}",
                            crate::t!("panel.help.server.api_endpoint_desc")
                        )),
                    ]),
                    Line::from(vec![
                        Span::styled("API Port", y),
                        Span::raw(self.get_api_port_str()),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("⇥ Panels", y),
                        Span::raw(format!(
                            "  {}",
                            crate::t!("panel.help.server.switch_panels")
                        )),
                    ]),
                    Line::from(vec![
                        Span::styled("Shift+a", y),
                        Span::raw(format!("  {}", crate::t!("panel.help.models.about"))),
                    ]),
                ]
            }
            ActivePanel::LlmSettings => vec![
                Line::from(Span::styled(crate::t!("panel.title.llm"), y)),
                Line::from(""),
                Line::from(crate::t!("panel.help.llm.description")),
                Line::from(""),
                Line::from(vec![
                    Span::styled("j / k", y),
                    Span::raw(format!("  {}", crate::t!("panel.help.llm.navigate"))),
                ]),
                Line::from(vec![
                    Span::styled("↵", y),
                    Span::raw(format!("  {}", crate::t!("panel.help.llm.apply"))),
                ]),
                Line::from(vec![
                    Span::styled("Left / Right", y),
                    Span::raw(format!("  {}", crate::t!("panel.help.llm.adjust"))),
                ]),
                Line::from(vec![
                    Span::styled("0-9, -, .", y),
                    Span::raw(format!(
                        "  {}  ·  ^F7/8/9 {}",
                        crate::t!("panel.help.llm.type_numeric")
                            .split("·")
                            .next()
                            .unwrap_or("")
                            .trim(),
                        crate::t!("panel.help.llm.navigate")
                    )),
                ]),
                Line::from(vec![
                    Span::styled("⎋", y),
                    Span::raw(format!("  {}", crate::t!("panel.help.llm.cancel_edit"))),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("^S", y),
                    Span::raw(format!("  {}", crate::t!("panel.help.llm.save"))),
                ]),
                Line::from(vec![
                    Span::styled("^R", y),
                    Span::raw(format!("  {}", crate::t!("panel.help.llm.reset"))),
                ]),
                Line::from(vec![
                    Span::styled("^E", y),
                    Span::raw(format!("  {}", crate::t!("panel.help.llm.toggle"))),
                ]),
                Line::from(vec![
                    Span::styled("^X", y),
                    Span::raw(format!("  {}", crate::t!("panel.help.llm.expert_mode"))),
                ]),
                Line::from(""),
                Line::from(vec![Span::styled("--- Loading ---", y)]),
                Line::from(vec![
                    Span::styled("Context", y),
                    Span::raw(format!("  {}", crate::t!("field.help.context_length"))),
                ]),
                Line::from(vec![
                    Span::styled("Prompt", y),
                    Span::raw(format!(
                        "  {}",
                        crate::t!("field.help.system_prompt_preset_name")
                    )),
                ]),
                Line::from(vec![
                    Span::styled("Keep in memory", y),
                    Span::raw(format!("  {}", crate::t!("field.help.mlock"))),
                ]),
                Line::from(""),
                Line::from(vec![Span::styled("--- GPU Offload ---", y)]),
                Line::from(vec![
                    Span::styled("GPU Layers", y),
                    Span::raw(format!("  {}", crate::t!("field.help.gpu_layers_mode"))),
                ]),
                Line::from(vec![
                    Span::styled("Auto Chat Template", y),
                    Span::raw(format!("  {}", crate::t!("field.help.auto_chat_template"))),
                ]),
                Line::from(vec![
                    Span::styled("Flash Attention", y),
                    Span::raw(format!("  {}", crate::t!("field.help.flash_attn"))),
                ]),
                Line::from(vec![
                    Span::styled("KV Cache Offload", y),
                    Span::raw(format!("  {}", crate::t!("field.help.kv_cache_offload"))),
                ]),
                Line::from(vec![
                    Span::styled("Cache Type K / V", y),
                    Span::raw(format!("  {}", crate::t!("field.help.cache_type_k"))),
                ]),
                Line::from(vec![
                    Span::styled("Active Experts", y),
                    Span::raw(format!("  {}", crate::t!("field.help.expert_count"))),
                ]),
                Line::from(""),
                Line::from(vec![Span::styled("--- Evaluation ---", y)]),
                Line::from(vec![
                    Span::styled("Eval Batch", y),
                    Span::raw(format!("  {}", crate::t!("field.help.batch_size"))),
                ]),
                Line::from(vec![
                    Span::styled("Unified KV", y),
                    Span::raw(format!("  {}", crate::t!("field.help.uniform_cache"))),
                ]),
                Line::from(vec![
                    Span::styled("Max Concurrent Pred", y),
                    Span::raw(format!(
                        "  {}",
                        crate::t!("field.help.max_concurrent_predictions")
                    )),
                ]),
                Line::from(vec![
                    Span::styled("Cache Reuse", y),
                    Span::raw(format!("  {}", crate::t!("field.help.cache_reuse"))),
                ]),
                Line::from(vec![
                    Span::styled("SWA Full Cache", y),
                    Span::raw(format!("  {}", crate::t!("field.help.swa_full"))),
                ]),
                Line::from(""),
                Line::from(vec![Span::styled("--- Sampling ---", y)]),
                Line::from(vec![
                    Span::styled("Seed", y),
                    Span::raw(format!("  {}", crate::t!("field.help.seed"))),
                ]),
                Line::from(vec![
                    Span::styled("Temp", y),
                    Span::raw(format!("  {}", crate::t!("field.help.temperature"))),
                ]),
                Line::from(vec![
                    Span::styled("Top-k", y),
                    Span::raw(format!("  {}", crate::t!("field.help.top_k"))),
                ]),
                Line::from(vec![
                    Span::styled("Top-p", y),
                    Span::raw(format!("  {}", crate::t!("field.help.top_p"))),
                ]),
                Line::from(vec![
                    Span::styled("Min P", y),
                    Span::raw(format!("  {}", crate::t!("field.help.min_p"))),
                ]),
                Line::from(vec![
                    Span::styled("Max Tokens", y),
                    Span::raw(format!("  {}", crate::t!("field.help.max_tokens"))),
                ]),
                Line::from(""),
                Line::from(vec![Span::styled("--- Repetition ---", y)]),
                Line::from(vec![
                    Span::styled("Repeat Penalty", y),
                    Span::raw(format!("  {}", crate::t!("field.help.repeat_penalty"))),
                ]),
                Line::from(vec![
                    Span::styled("Repeat Last N", y),
                    Span::raw(format!("  {}", crate::t!("field.help.repeat_last_n"))),
                ]),
                Line::from(vec![
                    Span::styled("Presence Penalty", y),
                    Span::raw(format!("  {}", crate::t!("field.help.presence_penalty"))),
                ]),
                Line::from(vec![
                    Span::styled("Freq Penalty", y),
                    Span::raw(format!("  {}", crate::t!("field.help.frequency_penalty"))),
                ]),
                Line::from(""),
                Line::from(vec![Span::styled("--- Tags ---", y)]),
                Line::from(vec![
                    Span::styled("Tags", y),
                    Span::raw(format!("  {}", crate::t!("field.help.tags"))),
                ]),
                Line::from(""),
                Line::from(vec![Span::styled("--- Backend ---", y)]),
                Line::from(vec![
                    Span::styled("LLama.cpp Version", y),
                    Span::raw(format!("  {}", crate::t!("field.help.backend_version"))),
                ]),
                Line::from(""),
                Line::from(vec![Span::styled("--- Yarn RoPE ---", y)]),
                Line::from(vec![
                    Span::styled("Yarn RoPE", y),
                    Span::raw(format!("  {}", crate::t!("field.help.rope_yarn_enabled"))),
                ]),
                Line::from(vec![
                    Span::styled("Yarn Params", y),
                    Span::raw(format!("  {}", crate::t!("field.help.yarn_params"))),
                ]),
                Line::from(""),
                Line::from(vec![Span::styled("--- Speculative Decoding ---", y)]),
                Line::from(vec![
                    Span::styled("Spec Type", y),
                    Span::raw(format!("  {}", crate::t!("field.help.spec_type"))),
                ]),
                Line::from(vec![
                    Span::styled("Spec Draft N Max", y),
                    Span::raw(format!("  {}", crate::t!("field.help.draft_tokens"))),
                ]),
                Line::from(""),
                Line::from(vec![Span::styled("--- Ultra Expert: Loading ---", y)]),
                Line::from(vec![
                    Span::styled("Threads Batch", y),
                    Span::raw(format!("  {}", crate::t!("field.help.threads_batch"))),
                ]),
                Line::from(vec![
                    Span::styled("UBatch Size", y),
                    Span::raw(format!("  {}", crate::t!("field.help.ubatch_size"))),
                ]),
                Line::from(vec![
                    Span::styled("Keep", y),
                    Span::raw(format!("  {}", crate::t!("field.help.keep"))),
                ]),
                Line::from(vec![
                    Span::styled("MMap", y),
                    Span::raw(format!("  {}", crate::t!("field.help.mmap"))),
                ]),
                Line::from(vec![
                    Span::styled("NUMA", y),
                    Span::raw(format!("  {}", crate::t!("field.help.numa"))),
                ]),
                Line::from(""),
                Line::from(vec![Span::styled("--- Ultra Expert: GPU ---", y)]),
                Line::from(vec![
                    Span::styled("Split Mode", y),
                    Span::raw(format!("  {}", crate::t!("field.help.split_mode"))),
                ]),
                Line::from(vec![
                    Span::styled("Tensor Split", y),
                    Span::raw(format!("  {}", crate::t!("field.help.tensor_split"))),
                ]),
                Line::from(vec![
                    Span::styled("Main GPU", y),
                    Span::raw(format!("  {}", crate::t!("field.help.main_gpu"))),
                ]),
                Line::from(vec![
                    Span::styled("Fit", y),
                    Span::raw(format!("  {}", crate::t!("field.help.fit"))),
                ]),
                Line::from(vec![
                    Span::styled("LoRA", y),
                    Span::raw(format!("  {}", crate::t!("field.help.lora"))),
                ]),
                Line::from(vec![
                    Span::styled("LoRA Scaled", y),
                    Span::raw(format!("  {}", crate::t!("field.help.lora_scaled"))),
                ]),
                Line::from(vec![
                    Span::styled("RPC", y),
                    Span::raw(format!("  {}", crate::t!("field.help.rpc"))),
                ]),
                Line::from(vec![
                    Span::styled("Embedding", y),
                    Span::raw(format!("  {}", crate::t!("field.help.embedding"))),
                ]),
                Line::from(""),
                Line::from(vec![Span::styled("--- Ultra Expert: Sampling ---", y)]),
                Line::from(vec![
                    Span::styled("Typical P", y),
                    Span::raw(format!("  {}", crate::t!("field.help.typical_p"))),
                ]),
                Line::from(vec![
                    Span::styled("Mirostat", y),
                    Span::raw(format!("  {}", crate::t!("field.help.mirostat"))),
                ]),
                Line::from(vec![
                    Span::styled("Mirostat LR", y),
                    Span::raw(format!("  {}", crate::t!("field.help.mirostat_lr"))),
                ]),
                Line::from(vec![
                    Span::styled("Mirostat Ent", y),
                    Span::raw(format!("  {}", crate::t!("field.help.mirostat_ent"))),
                ]),
                Line::from(vec![
                    Span::styled("Ignore EOS", y),
                    Span::raw(format!("  {}", crate::t!("field.help.ignore_eos"))),
                ]),
                Line::from(vec![
                    Span::styled("Samplers", y),
                    Span::raw(format!("  {}", crate::t!("field.help.samplers"))),
                ]),
                Line::from(""),
                Line::from(vec![Span::styled("--- Ultra Expert: DRY ---", y)]),
                Line::from(vec![
                    Span::styled("DRY Multiplier", y),
                    Span::raw(format!("  {}", crate::t!("field.help.dry_multiplier"))),
                ]),
                Line::from(vec![
                    Span::styled("DRY Base", y),
                    Span::raw(format!("  {}", crate::t!("field.help.dry_base"))),
                ]),
                Line::from(vec![
                    Span::styled("DRY Allowed Length", y),
                    Span::raw(format!("  {}", crate::t!("field.help.dry_allowed_length"))),
                ]),
                Line::from(vec![
                    Span::styled("DRY Penalty Last N", y),
                    Span::raw(format!("  {}", crate::t!("field.help.dry_penalty_last_n"))),
                ]),
                Line::from(""),
                Line::from(vec![Span::styled("--- Ultra Expert: Server ---", y)]),
                Line::from(vec![
                    Span::styled("Cache Prompt", y),
                    Span::raw(format!("  {}", crate::t!("field.help.cache_prompt"))),
                ]),
                Line::from(vec![
                    Span::styled("WebUI", y),
                    Span::raw(format!("  {}", crate::t!("field.help.webui"))),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Shift+a", y),
                    Span::raw(format!("  {}", crate::t!("panel.help.models.about"))),
                ]),
            ],
            ActivePanel::ActiveModel => vec![
                Line::from(Span::styled(crate::t!("panel.title.active"), y)),
                Line::from(""),
                Line::from(crate::t!("panel.help.active.description")),
                Line::from(""),
                Line::from(crate::t!("panel.help.active.show_metrics")),
                Line::from(""),
                Line::from(vec![
                    Span::styled("⇥ Panels", y),
                    Span::raw(format!(
                        "  {}",
                        crate::t!("panel.help.server.switch_panels")
                    )),
                ]),
                Line::from(vec![
                    Span::styled("Shift+a", y),
                    Span::raw(format!("  {}", crate::t!("panel.help.models.about"))),
                ]),
            ],
            ActivePanel::ModelInfo => vec![
                Line::from(Span::styled(crate::t!("panel.title.model_info"), y)),
                Line::from(""),
                Line::from("GGUF metadata for the selected model."),
                Line::from(""),
                Line::from("Displays file name, size, architecture, layers, and training context."),
                Line::from(""),
                Line::from(vec![
                    Span::styled("^G", y),
                    Span::raw("  GGUF filename explanation (works from any panel)"),
                ]),
                Line::from(""),
                Line::from(vec![Span::styled("--- GGUF Naming Reference ---", y)]),
                Line::from(vec![
                    Span::styled("Q4/Q5/Q6/Q8", y),
                    Span::raw("  Quantization bit depth (4-8 bits)"),
                ]),
                Line::from(vec![
                    Span::styled("K", y),
                    Span::raw("  K-quant: block-wise quantization with double-quantized scales"),
                ]),
                Line::from(vec![
                    Span::styled("S/M/L", y),
                    Span::raw("  Size variant: Small < Medium < Large (quality vs file size)"),
                ]),
                Line::from(vec![
                    Span::styled("UD", y),
                    Span::raw("  Unsloth Dynamic 2.0: KL-divergence calibrated quantization"),
                ]),
                Line::from(vec![
                    Span::styled("35B", y),
                    Span::raw("  Total parameters (e.g., 35 Billion)"),
                ]),
                Line::from(vec![
                    Span::styled("A3B", y),
                    Span::raw("  Active parameters in MoE (only 3B computed per token)"),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("⇥ Panels", y),
                    Span::raw(format!(
                        "  {}",
                        crate::t!("panel.help.server.switch_panels")
                    )),
                ]),
                Line::from(vec![
                    Span::styled("Shift+a", y),
                    Span::raw(format!("  {}", crate::t!("panel.help.models.about"))),
                ]),
            ],
            ActivePanel::Profiles => vec![
                Line::from(Span::styled(crate::t!("panel.title.profiles"), y)),
                Line::from(""),
                Line::from(crate::t!("panel.help.profiles.description")),
                Line::from(""),
                Line::from(vec![
                    Span::styled("j / k", y),
                    Span::raw(format!("  {}", crate::t!("panel.help.profiles.select"))),
                ]),
                Line::from(vec![
                    Span::styled("↵", y),
                    Span::raw(format!("  {}", crate::t!("panel.help.profiles.apply"))),
                ]),
                Line::from(vec![
                    Span::styled("s", y),
                    Span::raw(format!("  {}", crate::t!("panel.help.profiles.save"))),
                ]),
                Line::from(vec![
                    Span::styled("d", y),
                    Span::raw(format!("  {}", crate::t!("panel.help.profiles.delete"))),
                ]),
                Line::from(vec![
                    Span::styled("⎋", y),
                    Span::raw(format!("  {}", crate::t!("panel.help.profiles.back"))),
                ]),
            ],
            ActivePanel::SystemPromptPresets => vec![
                Line::from(Span::styled("SYSTEM PROMPT PRESETS", y)),
                Line::from(""),
                Line::from(crate::t!("panel.help.presets.description")),
                Line::from(""),
                Line::from(vec![
                    Span::styled("j / k", y),
                    Span::raw(format!("  {}", crate::t!("panel.help.presets.select"))),
                ]),
                Line::from(vec![
                    Span::styled("↵", y),
                    Span::raw(format!("  {}", crate::t!("panel.help.presets.apply"))),
                ]),
                Line::from(vec![
                    Span::styled("e", y),
                    Span::raw(format!("  {}", crate::t!("panel.help.presets.edit"))),
                ]),
                Line::from(vec![
                    Span::styled("n", y),
                    Span::raw(format!("  {}", crate::t!("panel.help.presets.create"))),
                ]),
                Line::from(vec![
                    Span::styled("⎋", y),
                    Span::raw(format!("  {}", crate::t!("panel.help.profiles.back"))),
                ]),
            ],
            ActivePanel::SearchReadme => vec![
                Line::from(Span::styled(crate::t!("panel.title.readme"), y)),
                Line::from(""),
                Line::from(crate::t!("panel.help.readme.description")),
                Line::from(""),
                Line::from(vec![
                    Span::styled("j / k / Arrow keys", y),
                    Span::raw(format!("  {}", crate::t!("panel.help.readme.scroll"))),
                ]),
                Line::from(vec![
                    Span::styled("h / l", y),
                    Span::raw(format!(
                        "  {}",
                        crate::t!("panel.help.readme.scroll_horizontal")
                    )),
                ]),
                Line::from(vec![
                    Span::styled("↵", y),
                    Span::raw(format!("  {}", crate::t!("panel.help.readme.expand"))),
                ]),
                Line::from(vec![
                    Span::styled("⎋", y),
                    Span::raw(format!("  {}", crate::t!("panel.help.readme.collapse"))),
                ]),
            ],
            ActivePanel::Downloads => vec![
                Line::from(Span::styled(crate::t!("panel.title.downloads"), y)),
                Line::from(""),
                Line::from(crate::t!("panel.help.downloads.description")),
                Line::from(""),
                Line::from(vec![
                    Span::styled("j / k / Arrow keys", y),
                    Span::raw(format!("  {}", crate::t!("panel.help.downloads.select"))),
                ]),
                Line::from(vec![
                    Span::styled("p", y),
                    Span::raw(format!("  {}", crate::t!("panel.help.downloads.pause"))),
                ]),
                Line::from(vec![
                    Span::styled("Alt+C", y),
                    Span::raw(format!("  {}", crate::t!("panel.help.downloads.cancel"))),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("⇥ Panels", y),
                    Span::raw(format!(
                        "  {}",
                        crate::t!("panel.help.server.switch_panels")
                    )),
                ]),
            ],
        }
    }
}
