use std::path::PathBuf;
use std::time::{Duration, Instant};

use tokio::sync::{mpsc, watch};

use crate::backend::server::spawn_server;
use crate::models::{
    BenchTuneConfig, BenchTuneMetrics, BenchTuneMode, BenchTuneParamValue, BenchTuneResult,
    BenchTuneStatus, DiscoveredModel, ModelSettings, ServerMode,
};

/// Benchmark tuning constants
const HEALTH_CHECK_ITERATIONS: u32 = 120;
const HEALTH_CHECK_INTERVAL_MS: u64 = 500;
const HEALTH_CHECK_LOG_INTERVAL: u32 = 10;
const REQUEST_TIMEOUT_SECS: u64 = 120;

/// Build a BenchTuneResult from accumulated iteration metrics.
fn build_bench_result(
    params: BenchTuneParamValue,
    total_prompt_tokens: u64,
    total_generation_tokens: u64,
    total_prompt_time: Duration,
    total_generation_time: Duration,
    total_time: Duration,
    first_token_times: Vec<u128>,
    outputs: Vec<String>,
    per_iteration_metrics: Vec<BenchTuneMetrics>,
    base_settings: Option<ModelSettings>,
) -> BenchTuneResult {
    let prompt_tps = if total_prompt_time.as_secs_f64() > 0.0 {
        (total_prompt_tokens as f64) / total_prompt_time.as_secs_f64()
    } else {
        0.0
    };

    let generation_tps = if total_generation_time.as_secs_f64() > 0.0 {
        (total_generation_tokens as f64) / total_generation_time.as_secs_f64()
    } else {
        0.0
    };

    let combined_tps = if total_time.as_secs_f64() > 0.0 {
        ((total_prompt_tokens + total_generation_tokens) as f64) / total_time.as_secs_f64()
    } else {
        0.0
    };

    let avg_latency_per_token = if total_generation_tokens > 0 {
        total_generation_time.as_millis() as f64 / (total_generation_tokens as f64)
    } else {
        0.0
    };

    let avg_first_token_time = if !first_token_times.is_empty() {
        first_token_times.iter().sum::<u128>() as f64 / first_token_times.len() as f64
    } else {
        0.0
    };

    BenchTuneResult {
        params,
        metrics: BenchTuneMetrics {
            prompt_tps,
            generation_tps,
            combined_tps,
            latency_per_token: avg_latency_per_token,
            first_token_time: avg_first_token_time,
        },
        outputs,
        per_iteration_metrics,
        base_settings,
    }
}

/// Run a benchmark tuning test with multiple parameter combinations
pub async fn run_bench_tune(
    main_config: &crate::config::Config,
    config: &BenchTuneConfig,
    model: &DiscoveredModel,
    settings: &ModelSettings,
    progress_tx: mpsc::Sender<BenchTuneStatus>,
    log_tx: mpsc::Sender<String>,
    cancel_rx: &mut watch::Receiver<bool>,
) -> Result<Vec<BenchTuneResult>, Box<dyn std::error::Error + Send + Sync>> {
    let start_time = Instant::now();
    let total_tests = config.get_total_tests_count();

    // Warn on large runs
    if total_tests > 500 {
        let _ = log_tx
            .send(format!(
                "WARNING: Benchmark will run {} combinations. This may take a long time.",
                total_tests
            ))
            .await;
    }

    // Generate all parameter combinations
    let combinations = config.generate_combinations();

    // Results storage
    let mut results = Vec::new();
    let mut failed_tests: Vec<(usize, String)> = Vec::new();

    // Apply chat_template_kwargs from config to settings
    let mut settings = settings.clone();
    if let Some(kwargs) = &config.chat_template_kwargs {
        settings.chat_template_kwargs = Some(kwargs.clone());
    }

    // Create a shared HTTP client for all inference requests
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .build()?;

    // If runtime-only mode, send params in request body (no server restarts)
    if config.bench_mode == BenchTuneMode::RuntimeOnly {
        // Spawn a single server for all runtime-only iterations
        let (exit_tx, _exit_rx) = tokio::sync::mpsc::channel(1);
        let (server_handle, _command) = spawn_server(
            main_config,
            Some(model),
            &settings,
            log_tx.clone(),
            None,
            ServerMode::Normal,
            1,
            exit_tx,
        )
        .await?;

        let host = if server_handle.host == "0.0.0.0" {
            "127.0.0.1"
        } else {
            &server_handle.host
        };

        // Wait for server to be ready
        for i in 0..HEALTH_CHECK_ITERATIONS {
            if *cancel_rx.borrow() {
                let _ = crate::backend::server::kill_server(server_handle).await;
                let elapsed = start_time.elapsed();
                progress_tx
                    .send(BenchTuneStatus::Cancelled {
                        total_tests,
                        successful_tests: results.len(),
                        failed_tests: failed_tests.len(),
                        elapsed,
                    })
                    .await?;
                return Ok(results);
            }
            if crate::backend::server::check_health(host, server_handle.port).await {
                break;
            }
            if i % HEALTH_CHECK_LOG_INTERVAL == 0 && i > 0 {
                let _ = log_tx
                    .send(format!(
                        "  ... still waiting ({:.0}s)...",
                        i as f32 * (HEALTH_CHECK_INTERVAL_MS as f32 / 1000.0)
                    ))
                    .await;
            }
            tokio::time::sleep(Duration::from_millis(HEALTH_CHECK_INTERVAL_MS)).await;
        }

        let server_port = server_handle.port;
        let server_host = host.to_string();

        for (idx, combination) in combinations.iter().enumerate() {
            // Check cancellation before each test
            if *cancel_rx.borrow() {
                let _ = crate::backend::server::kill_server(server_handle).await;
                let elapsed = start_time.elapsed();
                progress_tx
                    .send(BenchTuneStatus::Cancelled {
                        total_tests,
                        successful_tests: results.len(),
                        failed_tests: failed_tests.len(),
                        elapsed,
                    })
                    .await?;
                return Ok(results);
            }

            let progress = (idx as f32 / total_tests as f32) * 100.0;
            progress_tx
                .send(BenchTuneStatus::Running {
                    current: idx + 1,
                    total: total_tests,
                    progress,
                    current_params: combination.clone(),
                })
                .await?;

            let result = run_bench_tune_runtime_only(
                combination,
                &settings,
                config.num_iterations,
                config.prompt.clone(),
                &server_host,
                server_port,
                log_tx.clone(),
                config,
                &client,
            )
            .await;

            match result {
                Ok(test_result) => results.push(test_result),
                Err(e) => {
                    failed_tests.push((idx + 1, e.to_string()));
                    let _ = log_tx
                        .send(format!(
                            "Benchmark test {}/{} failed: {}",
                            idx + 1,
                            total_tests,
                            e
                        ))
                        .await;
                }
            }
        }

        let _ = crate::backend::server::kill_server(server_handle).await;
    } else {
        // Full mode: spawn a new server for each parameter combination
        for (idx, combination) in combinations.iter().enumerate() {
            // Check cancellation before each test
            if *cancel_rx.borrow() {
                let elapsed = start_time.elapsed();
                progress_tx
                    .send(BenchTuneStatus::Cancelled {
                        total_tests,
                        successful_tests: results.len(),
                        failed_tests: failed_tests.len(),
                        elapsed,
                    })
                    .await?;
                return Ok(results);
            }

            let progress = (idx as f32 / total_tests as f32) * 100.0;
            progress_tx
                .send(BenchTuneStatus::Running {
                    current: idx + 1,
                    total: total_tests,
                    progress,
                    current_params: combination.clone(),
                })
                .await?;

            let result = run_bench_tune_single_test(
                main_config,
                combination,
                model,
                &settings,
                config.num_iterations,
                config.prompt.clone(),
                log_tx.clone(),
                config,
                &client,
            )
            .await;

            match result {
                Ok(test_result) => results.push(test_result),
                Err(e) => {
                    failed_tests.push((idx + 1, e.to_string()));
                    let _ = log_tx
                        .send(format!(
                            "Benchmark test {}/{} failed: {}",
                            idx + 1,
                            total_tests,
                            e
                        ))
                        .await;
                }
            }
        }
    }

    // Sort results by combined_tps (descending)
    results.sort_by(|a, b| {
        b.metrics
            .combined_tps
            .partial_cmp(&a.metrics.combined_tps)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let elapsed = start_time.elapsed();
    let successful_tests = results.len();
    let failed_count = failed_tests.len();

    // Final progress update - distinguish between full success and partial success
    if failed_count > 0 {
        progress_tx
            .send(BenchTuneStatus::PartiallyCompleted {
                total_tests,
                successful_tests,
                failed_tests: failed_count,
                elapsed,
            })
            .await?;
    } else {
        progress_tx
            .send(BenchTuneStatus::Completed {
                total_tests,
                successful_tests,
                elapsed,
            })
            .await?;
    }

    Ok(results)
}

/// Run inference iterations and accumulate metrics into a BenchTuneResult.
/// Shared by both runtime-only and full benchmark modes.
async fn run_iteration_loop(
    prompt: &str,
    host: &str,
    port: u16,
    params: &BenchTuneParamValue,
    num_iterations: u32,
    config: &BenchTuneConfig,
    client: &reqwest::Client,
    log_tx: mpsc::Sender<String>,
    log_prefix: &str,
) -> Result<BenchTuneResult, Box<dyn std::error::Error + Send + Sync>> {
    let mut total_prompt_tokens = 0u64;
    let mut total_generation_tokens = 0u64;
    let mut total_prompt_time = Duration::ZERO;
    let mut total_generation_time = Duration::ZERO;
    let mut total_time = Duration::ZERO;
    let mut first_token_times = Vec::new();
    let mut outputs = Vec::new();
    let mut per_iteration_metrics = Vec::new();

    let _ = log_tx
        .send(format!(
            "Running {} inference iterations {}...",
            num_iterations, log_prefix
        ))
        .await;

    for i in 0..num_iterations {
        let result = send_inference_request(prompt, host, port, params, config, client).await;

        match result {
            Ok(res) => {
                total_prompt_tokens += res.prompt_tokens;
                total_generation_tokens += res.generation_tokens;
                total_prompt_time += res.prompt_time;
                total_generation_time += res.generation_time;
                total_time += res.total_time;
                first_token_times.push(res.first_token_time);
                outputs.push(res.content.clone());

                let iter_prompt_tps = if res.prompt_time.as_secs_f64() > 0.0 {
                    res.prompt_tokens as f64 / res.prompt_time.as_secs_f64()
                } else {
                    0.0
                };
                let iter_gen_tps = if res.generation_time.as_secs_f64() > 0.0 {
                    res.generation_tokens as f64 / res.generation_time.as_secs_f64()
                } else {
                    0.0
                };
                let iter_latency = if res.generation_tokens > 0 {
                    res.generation_time.as_millis() as f64 / res.generation_tokens as f64
                } else {
                    0.0
                };

                per_iteration_metrics.push(BenchTuneMetrics {
                    prompt_tps: iter_prompt_tps,
                    generation_tps: iter_gen_tps,
                    combined_tps: 0.0,
                    latency_per_token: iter_latency,
                    first_token_time: res.first_token_time as f64,
                });

                if num_iterations > 1 {
                    let _ = log_tx
                        .send(format!(
                            "  Iteration {}/{}: {:.2} gen t/s",
                            i + 1,
                            num_iterations,
                            iter_gen_tps
                        ))
                        .await;
                }

                let _ = log_tx
                    .send(format!(
                        "--- Generated Output (Iter {}) ---\n{}\n----------------------------------",
                        i + 1,
                        res.content
                    ))
                    .await;
            }
            Err(e) => {
                let _ = log_tx
                    .send(format!(
                        "  Iteration {}/{} FAILED: {}",
                        i + 1,
                        num_iterations,
                        e
                    ))
                    .await;
                if i == 0 {
                    return Err(format!("Inference failed: {}", e).into());
                }
            }
        }
    }

    Ok(build_bench_result(
        params.clone(),
        total_prompt_tokens,
        total_generation_tokens,
        total_prompt_time,
        total_generation_time,
        total_time,
        first_token_times,
        outputs,
        per_iteration_metrics,
        None,
    ))
}

/// Run benchmark in runtime-only mode: sends params in /completion request body, no server restarts
async fn run_bench_tune_runtime_only(
    params: &BenchTuneParamValue,
    settings: &ModelSettings,
    num_iterations: u32,
    prompt: String,
    server_host: &str,
    server_port: u16,
    log_tx: mpsc::Sender<String>,
    config: &BenchTuneConfig,
    client: &reqwest::Client,
) -> Result<BenchTuneResult, Box<dyn std::error::Error + Send + Sync>> {
    run_iteration_loop(
        &prompt,
        server_host,
        server_port,
        params,
        num_iterations,
        config,
        client,
        log_tx,
        "(runtime-only mode)",
    )
    .await
    .map(|mut r| {
        r.base_settings = Some(settings.clone());
        r
    })
}

/// Run a single benchmark tuning test with specific parameters
async fn run_bench_tune_single_test(
    main_config: &crate::config::Config,
    params: &BenchTuneParamValue,
    model: &DiscoveredModel,
    base_settings: &ModelSettings,
    num_iterations: u32,
    prompt: String,
    log_tx: mpsc::Sender<String>,
    config: &BenchTuneConfig,
    client: &reqwest::Client,
) -> Result<BenchTuneResult, Box<dyn std::error::Error + Send + Sync>> {
    // Create settings with test parameters
    let mut settings = base_settings.clone();

    // Apply test parameters
    if let Some(temperature) = params.temperature {
        settings.temperature = temperature as f32;
    }
    if let Some(top_p) = params.top_p {
        settings.top_p = top_p as f32;
    }
    if let Some(top_k) = params.top_k {
        settings.top_k = top_k as i32;
    }
    if let Some(repeat_penalty) = params.repeat_penalty {
        settings.repeat_penalty = repeat_penalty as f32;
    }
    if let Some(flash_attn) = params.flash_attn {
        settings.flash_attn = flash_attn;
    }
    if let Some(threads) = params.threads {
        settings.threads = threads;
        settings.threads_batch = threads; // Usually keep them equal for benchmarks
    }
    if let Some(batch_size) = params.batch_size {
        settings.batch_size = batch_size;
        settings.ubatch_size = batch_size;
    }
    if let Some(expert_count) = params.expert_count {
        settings.expert_count = expert_count;
    }

    // Spawn server with test parameters
    let (exit_tx, _exit_rx) = tokio::sync::mpsc::channel(1);
    let (server_handle, _command) = spawn_server(
        main_config,
        Some(model),
        &settings,
        log_tx.clone(),
        None,
        ServerMode::Normal,
        1,
        exit_tx,
    )
    .await?;
    // Wait for server to be ready
    let mut ready = false;
    let host = if server_handle.host == "0.0.0.0" {
        "127.0.0.1"
    } else {
        &server_handle.host
    };

    let _ = log_tx
        .send(format!(
            "Waiting for server on {}:{}...",
            host, server_handle.port
        ))
        .await;

    for i in 0..HEALTH_CHECK_ITERATIONS {
        if crate::backend::server::check_health(host, server_handle.port).await {
            ready = true;
            break;
        }
        if i % HEALTH_CHECK_LOG_INTERVAL == 0 && i > 0 {
            let _ = log_tx
                .send(format!(
                    "  ... still waiting ({:.0}s)...",
                    i as f32 * (HEALTH_CHECK_INTERVAL_MS as f32 / 1000.0)
                ))
                .await;
        }
        tokio::time::sleep(Duration::from_millis(HEALTH_CHECK_INTERVAL_MS)).await;
    }

    if !ready {
        let _ = log_tx
            .send("Error: Server health check timed out".to_string())
            .await;
        let _ = crate::backend::server::kill_server(server_handle).await;
        return Err("Server failed to become healthy".into());
    }

    let result = run_iteration_loop(
        &prompt,
        host,
        server_handle.port,
        params,
        num_iterations,
        config,
        client,
        log_tx,
        "",
    )
    .await;

    let _ = crate::backend::server::kill_server(server_handle).await;
    tokio::time::sleep(Duration::from_secs(1)).await;

    result.map(|mut r| {
        r.base_settings = Some(base_settings.clone());
        r
    })
}

/// Send an inference request and measure response time
async fn send_inference_request(
    prompt: &str,
    host: &str,
    port: u16,
    params: &BenchTuneParamValue,
    config: &BenchTuneConfig,
    client: &reqwest::Client,
) -> Result<InferenceResult, Box<dyn std::error::Error + Send + Sync>> {
    // Build request body with benchmark params
    let mut body = serde_json::json!({
        "prompt": prompt,
        "n_predict": config.n_predict,
        "stream": false
    });

    if let Some(temperature) = params.temperature {
        body["temperature"] = serde_json::json!(temperature);
    }
    if let Some(top_p) = params.top_p {
        body["top_p"] = serde_json::json!(top_p);
    }
    if let Some(top_k) = params.top_k {
        body["top_k"] = serde_json::json!(top_k);
    }
    if let Some(repeat_penalty) = params.repeat_penalty {
        body["repeat_penalty"] = serde_json::json!(repeat_penalty);
    }

    let url = format!("http://{}:{}/completion", host, port);
    let start = Instant::now();
    let resp = client.post(url).json(&body).send().await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_else(|_| "no body".to_string());
        return Err(format!("Server returned error {}: {}", status, body).into());
    }

    let total_time = start.elapsed();
    let json: serde_json::Value = resp.json().await?;

    // Robust timings parsing
    let prompt_tokens = json["tokens_evaluated"]
        .as_u64()
        .or_else(|| json["prompt_n"].as_u64())
        .unwrap_or(0);

    let generation_tokens = json["tokens_predicted"]
        .as_u64()
        .or_else(|| json["predicted_n"].as_u64())
        .unwrap_or(0);

    let timings = &json["timings"];
    let prompt_time_ms = timings["prompt_ms"]
        .as_f64()
        .or_else(|| timings["prompt_eval_ms"].as_f64())
        .unwrap_or(0.0);

    let generation_time_ms = timings["predicted_ms"]
        .as_f64()
        .or_else(|| timings["eval_ms"].as_f64())
        .unwrap_or(0.0);

    Ok(InferenceResult {
        prompt_tokens,
        generation_tokens,
        prompt_time: Duration::from_millis(prompt_time_ms as u64),
        generation_time: Duration::from_millis(generation_time_ms as u64),
        total_time,
        first_token_time: prompt_time_ms as u128,
        content: json["content"].as_str().unwrap_or("").to_string(),
    })
}

/// Save benchmark results to disk in Markdown format
pub async fn save_results(
    results: &[BenchTuneResult],
    output_dir: &PathBuf,
    config: &BenchTuneConfig,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Create output directory if it doesn't exist
    std::fs::create_dir_all(output_dir)?;

    // Generate timestamp for the filename
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let filename = format!("benchmark_{}.md", timestamp);
    let filepath = output_dir.join(filename);

    let mut md = String::new();
    md.push_str("# LLM Benchmark Results\n\n");
    md.push_str(&format!(
        "Generated on: {}\n\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    ));

    md.push_str("| Temp | Top-P | Top-K | RepPen | FA | Threads | Batch | Exp | Spec | Draft | Prompt t/s | Gen t/s | Latency (ms) | First Tok (ms) |\n");
    md.push_str("|------|-------|-------|--------|----|---------|-------|-----|------|-------|------------|---------|--------------|----------------|\n");

    for r in results {
        let temp = r
            .params
            .temperature
            .map(|v| format!("{:.2}", v))
            .unwrap_or_else(|| "-".to_string());
        let top_p = r
            .params
            .top_p
            .map(|v| format!("{:.2}", v))
            .unwrap_or_else(|| "-".to_string());
        let top_k = r
            .params
            .top_k
            .map(|v| v.to_string())
            .unwrap_or_else(|| "-".to_string());
        let rep_pen = r
            .params
            .repeat_penalty
            .map(|v| format!("{:.2}", v))
            .unwrap_or_else(|| "-".to_string());
        let fa = r
            .params
            .flash_attn
            .map(|v| if v { "ON" } else { "OFF" })
            .unwrap_or("-");
        let threads = r
            .params
            .threads
            .map(|v| v.to_string())
            .unwrap_or_else(|| "-".to_string());
        let batch = r
            .params
            .batch_size
            .map(|v| v.to_string())
            .unwrap_or_else(|| "-".to_string());
        let exp = r
            .params
            .expert_count
            .map(|v| v.to_string())
            .unwrap_or_else(|| "-".to_string());

        let spec = r
            .params
            .spec_type
            .as_ref()
            .map(|s| {
                if s.is_empty() {
                    "-".to_string()
                } else {
                    s.clone()
                }
            })
            .unwrap_or_else(|| "-".to_string());
        let draft = r
            .params
            .draft_tokens
            .map(|v| v.to_string())
            .unwrap_or_else(|| "-".to_string());

        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {:.2} | {:.2} | {:.2} | {:.2} |\n",
            temp,
            top_p,
            top_k,
            rep_pen,
            fa,
            threads,
            batch,
            exp,
            spec,
            draft,
            r.metrics.prompt_tps,
            r.metrics.generation_tps,
            r.metrics.latency_per_token,
            r.metrics.first_token_time
        ));
    }

    tokio::fs::write(&filepath, md).await?;

    // Save full results as JSON with outputs
    let json_filename = format!("benchmark_{}.json", timestamp);
    let json_filepath = output_dir.join(&json_filename);
    let json_content = serde_json::to_string_pretty(&results)?;
    tokio::fs::write(&json_filepath, json_content).await?;

    // Also save full results as YAML with outputs
    let yaml_filename = format!("benchmark_{}.yaml", timestamp);
    let yaml_filepath = output_dir.join(&yaml_filename);
    let yaml_content = serde_yaml::to_string(&results)?;
    tokio::fs::write(&yaml_filepath, yaml_content).await?;

    // Generate HTML report
    let html_filename = format!("benchmark_{}.html", timestamp);
    let html_filepath = output_dir.join(&html_filename);
    let html_content = generate_html_report(results, config);
    tokio::fs::write(&html_filepath, html_content).await?;

    Ok(())
}

fn generate_html_report(results: &[BenchTuneResult], config: &BenchTuneConfig) -> String {
    use chrono::Local;

    let total_tests = results.len();
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    // Extract model metadata from first result's base_settings
    let model_info = results.first().and_then(|r| {
        r.base_settings.as_ref().map(|s| {
            let model_name = if config.model_path.file_name().is_some() {
                config
                    .model_path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_string()
            } else {
                config.model_path.display().to_string()
            };
            let file_size_mb = results
                .first()
                .and_then(|r| {
                    r.base_settings.as_ref().map(|_s| {
                        // We don't have file_size in settings, use a placeholder
                        0u64
                    })
                })
                .unwrap_or(0);
            (model_name, file_size_mb, s.clone())
        })
    });

    // Resolve benchmark params against base settings (fill in None with base values)
    struct ResolvedParams {
        temperature: f64,
        top_p: f64,
        top_k: i64,
        repeat_penalty: f64,
        flash_attn: bool,
        threads: u32,
        batch_size: u32,
        expert_count: i32,
        spec_type: String,
        draft_tokens: u32,
    }

    fn resolve_params(
        params: &BenchTuneParamValue,
        base: &crate::models::ModelSettings,
    ) -> ResolvedParams {
        ResolvedParams {
            temperature: params.temperature.unwrap_or(base.temperature as f64),
            top_p: params.top_p.unwrap_or(base.top_p as f64),
            top_k: params.top_k.unwrap_or(base.top_k as i64),
            repeat_penalty: params.repeat_penalty.unwrap_or(base.repeat_penalty as f64),
            flash_attn: params.flash_attn.unwrap_or(base.flash_attn),
            threads: params.threads.unwrap_or(base.threads),
            batch_size: params.batch_size.unwrap_or(base.batch_size),
            expert_count: params.expert_count.unwrap_or(base.expert_count),
            spec_type: params
                .spec_type
                .clone()
                .unwrap_or_else(|| base.spec_type.clone()),
            draft_tokens: params.draft_tokens.unwrap_or(base.draft_tokens),
        }
    }

    // Statistics helpers
    fn mean(vals: &[f64]) -> f64 {
        if vals.is_empty() {
            return 0.0;
        }
        vals.iter().sum::<f64>() / vals.len() as f64
    }
    fn median(vals: &mut [f64]) -> f64 {
        if vals.is_empty() {
            return 0.0;
        }
        vals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mid = vals.len() / 2;
        if vals.len().is_multiple_of(2) {
            (vals[mid - 1] + vals[mid]) / 2.0
        } else {
            vals[mid]
        }
    }
    fn std_dev(vals: &[f64], avg: f64) -> f64 {
        if vals.len() <= 1 {
            return 0.0;
        }
        let variance =
            vals.iter().map(|v| (v - avg).powi(2)).sum::<f64>() / (vals.len() - 1) as f64;
        variance.sqrt()
    }
    fn min_val(vals: &[f64]) -> f64 {
        vals.iter().cloned().fold(f64::INFINITY, f64::min)
    }
    fn max_val(vals: &[f64]) -> f64 {
        vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    }

    let gen_tps: Vec<f64> = results.iter().map(|r| r.metrics.generation_tps).collect();
    let mut prompt_tps: Vec<f64> = results.iter().map(|r| r.metrics.prompt_tps).collect();
    let latency: Vec<f64> = results
        .iter()
        .map(|r| r.metrics.latency_per_token)
        .collect();
    let mut first_token: Vec<f64> = results.iter().map(|r| r.metrics.first_token_time).collect();

    let mut gen_tps_sorted = gen_tps.clone();
    let mut latency_sorted = latency.clone();

    let avg_gen_tps = mean(&gen_tps);
    let avg_prompt_tps = mean(&prompt_tps);
    let avg_latency = mean(&latency);
    let avg_first_token = mean(&first_token);
    let _avg_combined_tps = mean(
        &results
            .iter()
            .map(|r| r.metrics.combined_tps)
            .collect::<Vec<f64>>(),
    );

    let gen_std = std_dev(&gen_tps, avg_gen_tps);
    let prompt_std = std_dev(&prompt_tps, avg_prompt_tps);
    let lat_std = std_dev(&latency, avg_latency);
    let ft_std = std_dev(&first_token, avg_first_token);

    let best_idx = results
        .iter()
        .enumerate()
        .max_by(|a, b| {
            a.1.metrics
                .generation_tps
                .partial_cmp(&b.1.metrics.generation_tps)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(i, _)| i);
    let best_gen_tps = if !gen_tps.is_empty() {
        max_val(&gen_tps)
    } else {
        0.0
    };
    let best_prompt_tps = if !prompt_tps.is_empty() {
        max_val(&prompt_tps)
    } else {
        0.0
    };
    let best_latency = if !latency.is_empty() {
        min_val(&latency)
    } else {
        0.0
    };
    let best_first_token = if !first_token.is_empty() {
        min_val(&first_token)
    } else {
        0.0
    };
    let min_gen_tps = min_val(&gen_tps);
    let min_prompt_tps = min_val(&prompt_tps);
    let min_latency = min_val(&latency);
    let min_first_token = min_val(&first_token);

    // Per-parameter impact analysis
    let param_names = [("temperature", "Temperature"),
        ("top_p", "Top-P"),
        ("top_k", "Top-K"),
        ("repeat_penalty", "Repeat Penalty"),
        ("flash_attn", "Flash Attention"),
        ("threads", "Threads"),
        ("batch_size", "Batch Size"),
        ("expert_count", "Experts")];

    let impact_data: Vec<(String, String, f64)> = param_names
        .iter()
        .filter_map(|(key, label)| {
            let values: Vec<f64> = results
                .iter()
                .filter_map(|r| {
                    let base = r.base_settings.as_ref()?;
                    let rp = resolve_params(&r.params, base);
                    Some(match *key {
                        "temperature" => rp.temperature,
                        "top_p" => rp.top_p,
                        "top_k" => rp.top_k as f64,
                        "repeat_penalty" => rp.repeat_penalty,
                        "flash_attn" => {
                            if rp.flash_attn {
                                1.0
                            } else {
                                0.0
                            }
                        }
                        "threads" => rp.threads as f64,
                        "batch_size" => rp.batch_size as f64,
                        "expert_count" => rp.expert_count as f64,
                        _ => return None,
                    })
                })
                .collect();

            // Group by parameter value and compute mean gen_tps per group
            let mut groups: std::collections::HashMap<String, Vec<f64>> =
                std::collections::HashMap::new();
            for (r, v) in results.iter().zip(values.iter()) {
                let key_str = if *key == "flash_attn" {
                    if *v > 0.5 {
                        "ON".to_string()
                    } else {
                        "OFF".to_string()
                    }
                } else {
                    format!("{:.2}", v)
                };
                groups
                    .entry(key_str)
                    .or_default()
                    .push(r.metrics.generation_tps);
            }

            if groups.len() <= 1 {
                return None;
            } // Parameter doesn't vary

            let group_means: Vec<f64> = groups.values().map(|vals| mean(vals)).collect();
            let spread = max_val(&group_means) - min_val(&group_means);
            Some((label.to_string(), format!("{:.1}", spread), spread))
        })
        .collect();

    // Sort by impact (spread) descending
    let mut impact_sorted = impact_data.clone();
    impact_sorted.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

    // Consistency indicator (coefficient of variation from per-iteration metrics)
    let consistency_data: Vec<f64> = results
        .iter()
        .map(|r| {
            if r.per_iteration_metrics.len() <= 1 {
                return 1.0; // No consistency data = neutral
            }
            let iter_gen_tps: Vec<f64> = r
                .per_iteration_metrics
                .iter()
                .map(|m| m.generation_tps)
                .collect();
            let iter_mean = mean(&iter_gen_tps);
            if iter_mean == 0.0 {
                return 1.0;
            }
            let iter_std = std_dev(&iter_gen_tps, iter_mean);
            let cv = iter_std / iter_mean; // Coefficient of variation
            // Map CV to 0-1 score (lower CV = more consistent = higher score)
            // CV of 0% = 1.0 (perfect), CV of 20%+ = 0.0 (poor)
            (1.0 - (cv * 5.0)).clamp(0.0, 1.0)
        })
        .collect();

    // Top N for charts
    let top_n = std::cmp::min(20, total_tests);
    let top_indices: Vec<(usize, usize)> = (0..total_tests)
        .map(|i| (i, results[i].metrics.generation_tps))
        .enumerate()
        .take(top_n)
        .map(|(rank, (idx, _))| (rank + 1, idx))
        .collect();

    let top_labels: Vec<String> = top_indices
        .iter()
        .map(|(_rank, idx)| {
            let base = results[*idx].base_settings.as_ref().unwrap();
            let rp = resolve_params(&results[*idx].params, base);
            format!("T={:.2} TP={:.2}", rp.temperature, rp.top_p)
        })
        .collect();
    let top_gen_tps: Vec<f64> = top_indices
        .iter()
        .map(|(_, idx)| results[*idx].metrics.generation_tps)
        .collect();

    // Scatter data with labels
    let scatter_gen_tps: Vec<f64> = results.iter().map(|r| r.metrics.generation_tps).collect();
    let scatter_latency: Vec<f64> = results
        .iter()
        .map(|r| r.metrics.latency_per_token)
        .collect();
    let scatter_first_token: Vec<f64> =
        results.iter().map(|r| r.metrics.first_token_time).collect();

    let param_headers: Vec<String> = vec![
        "Temp".to_string(),
        "Top-P".to_string(),
        "Top-K".to_string(),
        "RepPen".to_string(),
        "FA".to_string(),
        "Threads".to_string(),
        "Batch".to_string(),
        "Exp".to_string(),
        "Spec".to_string(),
        "Draft".to_string(),
    ];
    let param_vals: Vec<Vec<String>> = results
        .iter()
        .map(|r| {
            let base = r.base_settings.as_ref().unwrap();
            let rp = resolve_params(&r.params, base);
            vec![
                format!("{:.2}", rp.temperature),
                format!("{:.2}", rp.top_p),
                rp.top_k.to_string(),
                format!("{:.2}", rp.repeat_penalty),
                if rp.flash_attn {
                    "ON".to_string()
                } else {
                    "OFF".to_string()
                },
                rp.threads.to_string(),
                rp.batch_size.to_string(),
                rp.expert_count.to_string(),
                if rp.spec_type.is_empty() {
                    "-".to_string()
                } else {
                    rp.spec_type.clone()
                },
                rp.draft_tokens.to_string(),
            ]
        })
        .collect();

    // Build metrics JSON with consistency data
    let metrics_data: Vec<serde_json::Value> = results
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let base = r.base_settings.as_ref().unwrap();
            let rp = resolve_params(&r.params, base);
            serde_json::json!({
                "idx": i,
                "temp": rp.temperature,
                "top_p": rp.top_p,
                "top_k": rp.top_k,
                "repeat_penalty": rp.repeat_penalty,
                "flash_attn": rp.flash_attn,
                "threads": rp.threads,
                "batch_size": rp.batch_size,
                "expert_count": rp.expert_count,
                "spec_type": rp.spec_type,
                "draft_tokens": rp.draft_tokens,
                "prompt_tps": r.metrics.prompt_tps,
                "generation_tps": r.metrics.generation_tps,
                "combined_tps": r.metrics.combined_tps,
                "latency_per_token": r.metrics.latency_per_token,
                "first_token_time": r.metrics.first_token_time,
                "consistency": consistency_data[i],
                "outputs": r.outputs,
                "per_iteration_metrics": r.per_iteration_metrics.iter().map(|m| {
                    serde_json::json!({
                        "prompt_tps": m.prompt_tps,
                        "generation_tps": m.generation_tps,
                        "combined_tps": m.combined_tps,
                        "latency_per_token": m.latency_per_token,
                        "first_token_time": m.first_token_time,
                    })
                }).collect::<Vec<_>>(),
            })
        })
        .collect();

    // Scatter data with labels for tooltips
    let scatter_data_json = serde_json::to_string(
        &scatter_gen_tps
            .iter()
            .zip(scatter_latency.iter())
            .zip(scatter_first_token.iter())
            .map(|((g, l), f)| {
                let mut s = String::from("{x:");
                s.push_str(&format!("{:.2}", g));
                s.push_str(",y:");
                s.push_str(&format!("{:.2}", l));
                s.push_str(",ft:");
                s.push_str(&format!("{:.2}", f));
                s.push('}');
                s
            })
            .collect::<Vec<_>>(),
    )
    .unwrap();
    let scatter_data2_json = serde_json::to_string(
        &scatter_gen_tps
            .iter()
            .zip(scatter_first_token.iter())
            .map(|(g, f)| {
                let mut s = String::from("{x:");
                s.push_str(&format!("{:.2}", g));
                s.push_str(",y:");
                s.push_str(&format!("{:.2}", f));
                s.push_str(",lat:");
                s.push_str(&format!("{:.2}", min_val(&latency)));
                s.push('}');
                s
            })
            .collect::<Vec<_>>(),
    )
    .unwrap();

    // Model metadata JSON
    let model_meta_json = model_info.as_ref().map(|(name, _size, settings)| {
        serde_json::json!({
            "model_name": name,
            "context_length": settings.context_length,
            "threads": settings.threads,
            "temperature": settings.temperature,
            "top_p": settings.top_p,
            "top_k": settings.top_k,
            "repeat_penalty": settings.repeat_penalty,
            "flash_attn": settings.flash_attn,
            "kv_cache_offload": settings.kv_cache_offload,
            "mlock": settings.mlock,
            "system_prompt": settings.system_prompt,
        })
    });

    // Impact analysis JSON
    let _impact_json = serde_json::to_string(&impact_sorted).unwrap();

    // Column definitions for visibility toggle
    let column_defs_json = serde_json::to_string(&vec![
        ("col-rank", "#", true),
        ("col-temp", "Temp", true),
        ("col-top-p", "Top-P", true),
        ("col-top-k", "Top-K", true),
        ("col-rep-pen", "RepPen", true),
        ("col-fa", "FA", true),
        ("col-threads", "Threads", true),
        ("col-batch", "Batch", true),
        ("col-exp", "Exp", true),
        ("col-spec", "Spec", true),
        ("col-draft", "Draft", true),
        ("col-gen-tps", "Gen t/s", true),
        ("col-prompt-tps", "Prompt t/s", true),
        ("col-latency", "Latency", true),
        ("col-first-token", "First Tok", true),
        ("col-combined", "Combined", true),
        ("col-consistency", "Consistency", true),
    ])
    .unwrap();

    // CSV data
    let csv_header = "Rank,Temp,Top-P,Top-K,RepPen,FA,Threads,Batch,Exp,Spec,Draft,Gen t/s,Prompt t/s,Latency (ms),First Tok (ms),Combined,Consistency";
    let csv_rows: Vec<String> = (0..total_tests)
        .map(|i| {
            let d = &metrics_data[i];
            let rank = i + 1;
            let spec = d
                .get("spec_type")
                .map(|v| v.as_str().unwrap_or("-"))
                .unwrap_or("-")
                .to_string();
            let draft = d
                .get("draft_tokens")
                .map(|v| v.as_u64().unwrap_or(0).to_string())
                .unwrap_or("-".to_string());
            format!(
                "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{:.1}",
                rank,
                d["temp"].as_f64().unwrap_or(0.0),
                d["top_p"].as_f64().unwrap_or(0.0),
                d["top_k"].as_i64().unwrap_or(0),
                d["repeat_penalty"].as_f64().unwrap_or(0.0),
                if d["flash_attn"].as_bool().unwrap_or(false) {
                    "ON"
                } else {
                    "OFF"
                },
                d["threads"].as_u64().unwrap_or(0),
                d["batch_size"].as_u64().unwrap_or(0),
                d["expert_count"].as_i64().unwrap_or(0),
                spec,
                draft,
                d["generation_tps"].as_f64().unwrap_or(0.0),
                d["prompt_tps"].as_f64().unwrap_or(0.0),
                d["latency_per_token"].as_f64().unwrap_or(0.0),
                d["first_token_time"].as_f64().unwrap_or(0.0),
                d["combined_tps"].as_f64().unwrap_or(0.0),
                d["consistency"].as_f64().unwrap_or(1.0)
            )
        })
        .collect();
    let csv_content = format!("{}\n{}", csv_header, csv_rows.join("\n"));
    let csv_b64 = base64_encode(&csv_content);

    let metrics_json = serde_json::to_string(&metrics_data).unwrap();
    let param_headers_json = serde_json::to_string(&param_headers).unwrap();
    let param_vals_json = serde_json::to_string(&param_vals).unwrap();
    let top_labels_json = serde_json::to_string(&top_labels).unwrap();
    let top_gen_tps_json = serde_json::to_string(&top_gen_tps).unwrap();

    // Build model metadata HTML
    let model_meta_html = model_info
        .as_ref()
        .map(|(name, _size, s)| {
            format!(
                r#"
<div class="meta-section">
<h2>Model &amp; Configuration</h2>
<div class="meta-grid">
<div class="meta-item"><div class="ml">Model</div><div class="mv">{}</div></div>
<div class="meta-item"><div class="ml">Context</div><div class="mv">{}</div></div>
<div class="meta-item"><div class="ml">Threads</div><div class="mv">{}</div></div>
<div class="meta-item"><div class="ml">Flash Attention</div><div class="mv">{}</div></div>
<div class="meta-item"><div class="ml">KV Cache Offload</div><div class="mv">{}</div></div>
<div class="meta-item"><div class="ml">MLOCK</div><div class="mv">{}</div></div>
<div class="meta-item"><div class="ml">Prompt</div><div class="mv meta-prompt">{}</div></div>
</div>
</div>"#,
                escape_html(name),
                s.context_length,
                s.threads,
                if s.flash_attn { "ON" } else { "OFF" },
                if s.kv_cache_offload { "ON" } else { "OFF" },
                if s.mlock { "ON" } else { "OFF" },
                escape_html(&s.system_prompt.chars().take(100).collect::<String>())
            )
        })
        .unwrap_or_default();

    // Build winner section HTML
    let winner_html = best_idx.and_then(|idx| {
        let r = &results[idx];
        let base = r.base_settings.as_ref()?;
        let rp = resolve_params(&r.params, base);
        let m = &r.metrics;
        Some(format!(r#"
<div class="winner-section">
<div class="winner-icon">&#127942;</div>
<div class="winner-content">
<div class="winner-title">Best Configuration</div>
<div class="winner-metrics">
<div class="winner-metric"><span class="wm-label">Gen t/s</span><span class="wm-value" style="color:#3fb950;font-size:1.8em;">{:.2}</span></div>
<div class="winner-metric"><span class="wm-label">Prompt t/s</span><span class="wm-value">{:.2}</span></div>
<div class="winner-metric"><span class="wm-label">Latency</span><span class="wm-value">{:.2}ms</span></div>
<div class="winner-metric"><span class="wm-label">First Token</span><span class="wm-value">{:.0}ms</span></div>
</div>
<div class="winner-params">Temp: {:.2} &middot; Top-P: {:.2} &middot; Top-K: {} &middot; RepPen: {:.2} &middot; FA: {} &middot; Threads: {} &middot; Batch: {} &middot; Exp: {} &middot; Spec: {} &middot; Draft: {}</div>
</div>
</div>"#,
                m.generation_tps, m.prompt_tps, m.latency_per_token, m.first_token_time,
                rp.temperature, rp.top_p, rp.top_k, rp.repeat_penalty,
                if rp.flash_attn { "ON" } else { "OFF" }, rp.threads,
                rp.batch_size, rp.expert_count,
                if rp.spec_type.is_empty() { "Off".to_string() } else { rp.spec_type.clone() }, rp.draft_tokens
            ))
    }).unwrap_or_default();

    // Build impact analysis HTML
    let impact_html = if !impact_sorted.is_empty() {
        let max_impact = impact_sorted[0].2;
        let rows: String = impact_sorted
            .iter()
            .map(|(label, spread, value)| {
                let bar_width = if max_impact > 0.0 {
                    (value / max_impact * 100.0) as i32
                } else {
                    0
                };
                let bar_color = if *value > max_impact * 0.7 {
                    "#f85149"
                } else if *value > max_impact * 0.4 {
                    "#d29922"
                } else {
                    "#3fb950"
                };
                format!(
                    r#"<div class="impact-row">
<div class="impact-label">{}</div>
<div class="impact-bar-bg"><div class="impact-bar-fill" style="width:{}%;background:{}"></div></div>
<div class="impact-value">{}</div>
</div>"#,
                    label, bar_width, bar_color, spread
                )
            })
            .collect();
        format!(
            r#"
<div class="impact-section">
<h2>Parameter Impact Analysis</h2>
<p class="impact-desc">Larger spread in generation throughput between parameter values indicates greater impact on performance.</p>
{}
</div>"#,
            rows
        )
    } else {
        r#"<div class="impact-section"><h2>Parameter Impact Analysis</h2><p class="impact-desc">All parameters were held constant — no impact data available.</p></div>"#.to_string()
    };

    // Empty state
    let empty_html = if total_tests == 0 {
        r#"<div class="empty-state">
<div class="empty-icon">&#128202;</div>
<div class="empty-title">No Results</div>
<div class="empty-text">Run a benchmark tuning test to generate results here.</div>
</div>"#
    } else {
        ""
    };

    let html = include_str!("benchmark_report.html");

    // Replace placeholders
    html.replace("__TIMESTAMP__", &timestamp)
        .replace("__TOTAL_TESTS__", &total_tests.to_string())
        .replace("__EMPTY_STATE__", empty_html)
        .replace("__MODEL_META__", &model_meta_html)
        .replace("__WINNER__", &winner_html)
        .replace("__AVG_GEN_TPS__", &format!("{:.1}", avg_gen_tps))
        .replace(
            "__MED_GEN_TPS__",
            &format!("{:.1}", median(&mut gen_tps_sorted)),
        )
        .replace("__GEN_STD__", &format!("{:.1}", gen_std))
        .replace("__MIN_GEN__", &format!("{:.1}", min_gen_tps))
        .replace("__MAX_GEN__", &format!("{:.1}", best_gen_tps))
        .replace("__AVG_PROMPT_TPS__", &format!("{:.1}", avg_prompt_tps))
        .replace(
            "__MED_PROMPT_TPS__",
            &format!("{:.1}", median(&mut prompt_tps)),
        )
        .replace("__PROMPT_STD__", &format!("{:.1}", prompt_std))
        .replace("__MIN_PROMPT__", &format!("{:.1}", min_prompt_tps))
        .replace("__MAX_PROMPT__", &format!("{:.1}", best_prompt_tps))
        .replace("__AVG_LATENCY__", &format!("{:.1}ms", avg_latency))
        .replace(
            "__MED_LATENCY__",
            &format!("{:.1}ms", median(&mut latency_sorted)),
        )
        .replace("__LAT_STD__", &format!("{:.1}", lat_std))
        .replace("__MIN_LAT__", &format!("{:.1}", min_latency))
        .replace("__MAX_LAT__", &format!("{:.1}", best_latency))
        .replace("__AVG_FT__", &format!("{:.0}ms", avg_first_token))
        .replace("__MED_FT__", &format!("{:.0}ms", median(&mut first_token)))
        .replace("__FT_STD__", &format!("{:.0}", ft_std))
        .replace("__MIN_FT__", &format!("{:.0}ms", min_first_token))
        .replace("__MAX_FT__", &format!("{:.0}ms", best_first_token))
        .replace("__BEST_GEN__", &format!("{:.1}", best_gen_tps))
        .replace("__TOP_N__", &top_n.to_string())
        .replace("__IMPACT_HTML__", &impact_html)
        .replace("__METRICS_JSON__", &metrics_json)
        .replace("__PARAM_HEADERS_JSON__", &param_headers_json)
        .replace("__PARAM_VALS_JSON__", &param_vals_json)
        .replace("__TOP_LABELS_JSON__", &top_labels_json)
        .replace("__TOP_GEN_TPS_JSON__", &top_gen_tps_json)
        .replace("__SCATTER_DATA_JSON__", &scatter_data_json)
        .replace("__SCATTER_DATA2_JSON__", &scatter_data2_json)
        .replace("__COLUMN_DEFS_JSON__", &column_defs_json)
        .replace("__CSV_B64__", &csv_b64)
        .replace(
            "__MODEL_META_JSON__",
            &serde_json::to_string(&model_meta_json).unwrap(),
        )
}

/// Escape HTML special characters
fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Base64 encode a string (no external dependency - simple encoding)
fn base64_encode(input: &str) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = input.as_bytes();
    let mut result = String::new();
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}

/// Result from a single inference request
struct InferenceResult {
    prompt_tokens: u64,
    generation_tokens: u64,
    prompt_time: Duration,
    generation_time: Duration,
    total_time: Duration,
    first_token_time: u128, // milliseconds
    content: String,
}
