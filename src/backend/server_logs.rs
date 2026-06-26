/// Parsed metrics from a single llama-server log line.
///
/// These metrics are extracted from llama.cpp's print_timing log output.
/// They are parsed from stdout and applied as fallback when the /metrics API returns 0.
#[derive(Debug, Clone, Default)]
pub struct ServerLogMetrics {
    /// Number of tokens in the prompt being evaluated.
    pub prompt_tokens: Option<u64>,
    /// Progress of prompt evaluation (0.0-1.0).
    pub prompt_progress: Option<f64>,
    /// Elapsed prompt evaluation time in milliseconds.
    pub prompt_elapsed_ms: Option<f64>,
    /// Prompt evaluation throughput in tokens per second.
    pub prompt_tps_eval: Option<f64>,
    /// Number of tokens currently in the KV cache.
    pub ctx_used: Option<u32>,
    /// Number of decoded tokens from print_timing logs.
    pub decoded_tokens: Option<u64>,
    /// Generation tokens per second parsed from llama.cpp log output (e.g., "tg = 64.45 t/s").
    pub gen_tps: Option<f64>,
}

/// Parse all metrics from a llama-server log line.
///
/// `prev_line` is needed for the "tokens per second" cross-line pattern.
/// Returns `(parsed metrics, is_generation_start)`.
/// `is_generation_start` is true when the line contains "n_decoded =",
/// which signals the start of generation and resets prompt progress in the caller.
pub fn parse_log_line(line: &str, prev_line: Option<&str>) -> (ServerLogMetrics, bool) {
    let mut metrics = ServerLogMetrics::default();
    let mut is_generation = false;

    let is_prompt_processing = line.contains("prompt processing");
    let is_generation_line = line.contains("n_decoded =");

    if is_generation_line {
        is_generation = true;
    }

    // Parse prompt processing metrics
    if is_prompt_processing {
        // n_tokens = X
        if let Some(tokens_part) = line.split("n_tokens =").last() {
            let val_str = tokens_part.split(',').next().unwrap_or(tokens_part).trim();
            if let Ok(tokens) = val_str.parse::<u64>() {
                metrics.prompt_tokens = Some(tokens);
            }
        }
        // progress = Y
        if let Some(progress_part) = line.split("progress =").last() {
            let val_str = progress_part.split(',').next().unwrap_or(progress_part).trim();
            if let Ok(progress) = val_str.parse::<f64>() {
                metrics.prompt_progress = Some(progress);
            }
        }
        // t = Z (elapsed seconds) - try multiple patterns since log format may vary
        let mut parsed = false;
        // Pattern 1: "t = X.XX"
        if let Some(t_part) = line.split("t =").last() {
            let val_str = t_part.split_whitespace().find(|s| !s.is_empty()).unwrap_or("").trim();
            if let Ok(t) = val_str.parse::<f64>() {
                metrics.prompt_elapsed_ms = Some(t * 1000.0);
                parsed = true;
            }
        }
        // Pattern 2: "t=X.XX" (no space)
        if !parsed {
            if let Some(t_part) = line.split("t=").last() {
                let val_str = t_part
                    .split(|c: char| c.is_whitespace() || c == '\u{2560}')
                    .find(|s| !s.is_empty())
                    .unwrap_or("")
                    .trim();
                if let Ok(t) = val_str.parse::<f64>() {
                    metrics.prompt_elapsed_ms = Some(t * 1000.0);
                    parsed = true;
                }
            }
        }
        // Pattern 3: Parse from end of line (elapsed time always at end before "s /")
        if !parsed {
            if let Some(slash_part) = line.rsplit(" s /").next() {
                let val_str = slash_part
                    .split_whitespace()
                    .rfind(|s| !s.is_empty())
                    .unwrap_or("")
                    .trim();
                if let Ok(t) = val_str.parse::<f64>() {
                    metrics.prompt_elapsed_ms = Some(t * 1000.0);
                }
            }
        }
        // TPS: "X.XX tokens per second" on same line or previous line
        let tps_str = line
            .contains("tokens per second")
            .then(|| {
                line.split("tokens per second")
                    .next()
                    .and_then(|p| p.split('/').next_back())
                    .map(|s| s.trim())
            })
            .flatten()
            .or(prev_line.and_then(|prev| {
                prev.split("tokens per second")
                    .next()
                    .and_then(|p| p.split('/').next_back())
                    .map(|s| s.trim())
            }));
        if let Some(tps_str) = tps_str {
            if let Ok(tps) = tps_str.parse::<f64>() {
                metrics.prompt_tps_eval = Some(tps);
            }
        }
    }

    // n_tokens = → ctx_used (for generation lines only, not prompt processing)
    if !is_prompt_processing && line.contains("n_tokens =") {
        if let Some(tokens_part) = line.split("n_tokens =").last() {
            let val_str = tokens_part.split(',').next().unwrap_or(tokens_part).trim();
            if let Ok(tokens) = val_str.parse::<u32>() {
                metrics.ctx_used = Some(tokens);
            }
        }
    }

    // n_decoded = → decoded_tokens
    if line.contains("n_decoded =") {
        if let Some(decoded_part) = line.split("n_decoded =").last() {
            let val_str = decoded_part
                .split(',')
                .next()
                .unwrap_or(decoded_part)
                .trim();
            if let Ok(tokens) = val_str.parse::<u64>() {
                metrics.decoded_tokens = Some(tokens);
            }
        }
    }

    // tg = → gen_tps
    if line.contains("tg =") {
        if let Some(tg_part) = line.split("tg =").last() {
            let val_str = tg_part.trim().split(' ').next().unwrap_or(tg_part).trim();
            if let Ok(tg) = val_str.parse::<f64>() {
                metrics.gen_tps = Some(tg);
            }
        }
    }

    (metrics, is_generation)
}
