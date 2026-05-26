use std::path::PathBuf;
use std::time::{Duration, Instant};

use tokio::sync::{mpsc, watch};

use crate::backend::server::spawn_server;
use crate::models::{BenchTuneConfig, BenchTuneMetrics, BenchTuneParamValue, BenchTuneResult, BenchTuneStatus, BenchTuneMode, ModelSettings, ServerMode, DiscoveredModel};

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
        let _ = log_tx.send(format!(
            "WARNING: Benchmark will run {} combinations. This may take a long time.",
            total_tests
        )).await;
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
        ).await?;

        let host = if server_handle.host == "0.0.0.0" { "127.0.0.1" } else { &server_handle.host };

        // Wait for server to be ready
        for i in 0..HEALTH_CHECK_ITERATIONS {
            if *cancel_rx.borrow() {
                let _ = crate::backend::server::kill_server(server_handle).await;
                let elapsed = start_time.elapsed();
                progress_tx.send(BenchTuneStatus::Cancelled {
                    total_tests,
                    successful_tests: results.len(),
                    failed_tests: failed_tests.len(),
                    elapsed,
                }).await?;
                return Ok(results);
            }
            if crate::backend::server::check_health(host, server_handle.port).await {
                break;
            }
            if i % HEALTH_CHECK_LOG_INTERVAL == 0 && i > 0 {
                let _ = log_tx.send(format!("  ... still waiting ({:.0}s)...", i as f32 * (HEALTH_CHECK_INTERVAL_MS as f32 / 1000.0))).await;
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
                progress_tx.send(BenchTuneStatus::Cancelled {
                    total_tests,
                    successful_tests: results.len(),
                    failed_tests: failed_tests.len(),
                    elapsed,
                }).await?;
                return Ok(results);
            }

            let progress = (idx as f32 / total_tests as f32) * 100.0;
            progress_tx.send(BenchTuneStatus::Running {
                current: idx + 1,
                total: total_tests,
                progress,
                current_params: combination.clone(),
            }).await?;

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
            ).await;

            match result {
                Ok(test_result) => results.push(test_result),
                Err(e) => {
                    failed_tests.push((idx + 1, e.to_string()));
                    let _ = log_tx.send(format!("Benchmark test {}/{} failed: {}", idx + 1, total_tests, e)).await;
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
                progress_tx.send(BenchTuneStatus::Cancelled {
                    total_tests,
                    successful_tests: results.len(),
                    failed_tests: failed_tests.len(),
                    elapsed,
                }).await?;
                return Ok(results);
            }

            let progress = (idx as f32 / total_tests as f32) * 100.0;
            progress_tx.send(BenchTuneStatus::Running {
                current: idx + 1,
                total: total_tests,
                progress,
                current_params: combination.clone(),
            }).await?;

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
            ).await;

            match result {
                Ok(test_result) => results.push(test_result),
                Err(e) => {
                    failed_tests.push((idx + 1, e.to_string()));
                    let _ = log_tx.send(format!("Benchmark test {}/{} failed: {}", idx + 1, total_tests, e)).await;
                }
            }
        }
    }

    // Sort results by combined_tps (descending)
    results.sort_by(|a, b| {
        b.metrics.combined_tps.partial_cmp(&a.metrics.combined_tps)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let elapsed = start_time.elapsed();
    let successful_tests = results.len();
    let failed_count = failed_tests.len();

    // Final progress update - distinguish between full success and partial success
    if failed_count > 0 {
        progress_tx.send(BenchTuneStatus::PartiallyCompleted {
            total_tests,
            successful_tests,
            failed_tests: failed_count,
            elapsed,
        }).await?;
    } else {
        progress_tx.send(BenchTuneStatus::Completed {
            total_tests,
            successful_tests,
            elapsed,
        }).await?;
    }

    Ok(results)
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
    let mut total_prompt_tokens = 0u64;
    let mut total_generation_tokens = 0u64;
    let mut total_prompt_time = Duration::ZERO;
    let mut total_generation_time = Duration::ZERO;
    let mut total_time = Duration::ZERO;
    let mut first_token_times = Vec::new();
    let mut outputs = Vec::new();
    let mut per_iteration_metrics = Vec::new();

    let _ = log_tx.send(format!("Running {} inference iterations (runtime-only mode)...", num_iterations)).await;

    for i in 0..num_iterations {
        let result = send_inference_request(&prompt, server_host, server_port, params, config, client).await;

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
                    let _ = log_tx.send(format!("  Iteration {}/{}: {:.2} gen t/s", i + 1, num_iterations, iter_gen_tps)).await;
                }

                let _ = log_tx.send(format!("--- Generated Output (Iter {}) ---\n{}\n----------------------------------", i + 1, res.content)).await;
            }
            Err(e) => {
                let _ = log_tx.send(format!("  Iteration {}/{} FAILED: {}", i + 1, num_iterations, e)).await;
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
        Some(settings.clone()),
    ))
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
    ).await?;    
    // Wait for server to be ready
    let mut ready = false;
    let host = if server_handle.host == "0.0.0.0" { "127.0.0.1" } else { &server_handle.host };
    
    let _ = log_tx.send(format!("Waiting for server on {}:{}...", host, server_handle.port)).await;
    
    for i in 0..HEALTH_CHECK_ITERATIONS {
        if crate::backend::server::check_health(host, server_handle.port).await {
            ready = true;
            break;
        }
        if i % HEALTH_CHECK_LOG_INTERVAL == 0 && i > 0 {
            let _ = log_tx.send(format!("  ... still waiting ({:.0}s)...", i as f32 * (HEALTH_CHECK_INTERVAL_MS as f32 / 1000.0))).await;
        }
        tokio::time::sleep(Duration::from_millis(HEALTH_CHECK_INTERVAL_MS)).await;
    }
    
    if !ready {
        let _ = log_tx.send("Error: Server health check timed out".to_string()).await;
        let _ = crate::backend::server::kill_server(server_handle).await;
        return Err("Server failed to become healthy".into());
    }
    
    // Run inference tests
    let mut total_prompt_tokens = 0u64;
    let mut total_generation_tokens = 0u64;
    let mut total_prompt_time = Duration::ZERO;
    let mut total_generation_time = Duration::ZERO;
    let mut total_time = Duration::ZERO;
    let mut first_token_times = Vec::new();
    let mut outputs = Vec::new();
    let mut per_iteration_metrics = Vec::new();
    
    let _ = log_tx.send(format!("Running {} inference iterations...", num_iterations)).await;
    
    for i in 0..num_iterations {
        // Send prompt and measure response
        let result = send_inference_request(&prompt, host, server_handle.port, params, config, client).await;
        
        match result {
            Ok(res) => {
                total_prompt_tokens += res.prompt_tokens;
                total_generation_tokens += res.generation_tokens;
                total_prompt_time += res.prompt_time;
                total_generation_time += res.generation_time;
                total_time += res.total_time;
                first_token_times.push(res.first_token_time);
                let output_text = res.content.clone();
                outputs.push(res.content);
                
                // Collect per-iteration metrics
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
                let iter_first_token = res.first_token_time as f64;
                
                per_iteration_metrics.push(BenchTuneMetrics {
                    prompt_tps: iter_prompt_tps,
                    generation_tps: iter_gen_tps,
                    combined_tps: 0.0,
                    latency_per_token: iter_latency,
                    first_token_time: iter_first_token,
                });
                
                if num_iterations > 1 {
                    let _ = log_tx.send(format!("  Iteration {}/{}: {:.2} gen t/s", i + 1, num_iterations, 
                        if res.generation_time.as_secs_f64() > 0.0 { res.generation_tokens as f64 / res.generation_time.as_secs_f64() } else { 0.0 }
                    )).await;
                }
                
                // Display the generated content in the log
                let _ = log_tx.send(format!("--- Generated Output (Iter {}) ---\n{}\n----------------------------------", i + 1, output_text)).await;
            }
            Err(e) => {
                let _ = log_tx.send(format!("  Iteration {}/{} FAILED: {}", i + 1, num_iterations, e)).await;
                // If the first iteration fails completely, we might want to abort this combination
                if i == 0 {
                    let _ = crate::backend::server::kill_server(server_handle).await;
                    return Err(format!("Inference failed: {}", e).into());
                }
            }
        }
    }
    
    // Clean up server
    let _ = crate::backend::server::kill_server(server_handle).await;
    tokio::time::sleep(Duration::from_secs(1)).await;
    
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
        Some(base_settings.clone()),
    ))
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
    let resp = client.post(url)
        .json(&body)
        .send()
        .await?;
    
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_else(|_| "no body".to_string());
        return Err(format!("Server returned error {}: {}", status, body).into());
    }
    
    let total_time = start.elapsed();
    let json: serde_json::Value = resp.json().await?;
    
    // Robust timings parsing
    let prompt_tokens = json["tokens_evaluated"].as_u64()
        .or_else(|| json["prompt_n"].as_u64())
        .unwrap_or(0);
        
    let generation_tokens = json["tokens_predicted"].as_u64()
        .or_else(|| json["predicted_n"].as_u64())
        .unwrap_or(0);
    
    let timings = &json["timings"];
    let prompt_time_ms = timings["prompt_ms"].as_f64()
        .or_else(|| timings["prompt_eval_ms"].as_f64())
        .unwrap_or(0.0);
        
    let generation_time_ms = timings["predicted_ms"].as_f64()
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
pub async fn save_results(results: &[BenchTuneResult], output_dir: &PathBuf, config: &BenchTuneConfig) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Create output directory if it doesn't exist
    std::fs::create_dir_all(output_dir)?;

    // Generate timestamp for the filename
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let filename = format!("benchmark_{}.md", timestamp);
    let filepath = output_dir.join(filename);

    let mut md = String::new();
    md.push_str("# LLM Benchmark Results\n\n");
    md.push_str(&format!("Generated on: {}\n\n", chrono::Local::now().format("%Y-%m-%d %H:%M:%S")));

    md.push_str("| Temp | Top-P | Top-K | RepPen | FA | Threads | Batch | Exp | Prompt t/s | Gen t/s | Latency (ms) | First Tok (ms) |\n");
    md.push_str("|------|-------|-------|--------|----|---------|-------|-----|------------|---------|--------------|----------------|\n");

    for r in results {
        let temp = r.params.temperature.map(|v| format!("{:.2}", v)).unwrap_or_else(|| "-".to_string());
        let top_p = r.params.top_p.map(|v| format!("{:.2}", v)).unwrap_or_else(|| "-".to_string());
        let top_k = r.params.top_k.map(|v| v.to_string()).unwrap_or_else(|| "-".to_string());
        let rep_pen = r.params.repeat_penalty.map(|v| format!("{:.2}", v)).unwrap_or_else(|| "-".to_string());
        let fa = r.params.flash_attn.map(|v| if v { "ON" } else { "OFF" }).unwrap_or("-");
        let threads = r.params.threads.map(|v| v.to_string()).unwrap_or_else(|| "-".to_string());
        let batch = r.params.batch_size.map(|v| v.to_string()).unwrap_or_else(|| "-".to_string());
        let exp = r.params.expert_count.map(|v| v.to_string()).unwrap_or_else(|| "-".to_string());

        md.push_str(&format!("| {} | {} | {} | {} | {} | {} | {} | {} | {:.2} | {:.2} | {:.2} | {:.2} |\n",
            temp, top_p, top_k, rep_pen, fa, threads, batch, exp,
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
    let html_content = generate_html_report(&results, &config);
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
                config.model_path.file_name().unwrap().to_string_lossy().to_string()
            } else {
                config.model_path.display().to_string()
            };
            let file_size_mb = results.first().map(|r| {
       r.base_settings.as_ref().map(|_s| {
                    // We don't have file_size in settings, use a placeholder
                    0u64
                })
            }).flatten().unwrap_or(0);
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
    }

    fn resolve_params(params: &BenchTuneParamValue, base: &crate::models::ModelSettings) -> ResolvedParams {
        ResolvedParams {
            temperature: params.temperature.unwrap_or(base.temperature as f64),
            top_p: params.top_p.unwrap_or(base.top_p as f64),
            top_k: params.top_k.unwrap_or(base.top_k as i64),
            repeat_penalty: params.repeat_penalty.unwrap_or(base.repeat_penalty as f64),
            flash_attn: params.flash_attn.unwrap_or(base.flash_attn),
            threads: params.threads.unwrap_or(base.threads),
            batch_size: params.batch_size.unwrap_or(base.batch_size),
            expert_count: params.expert_count.unwrap_or(base.expert_count),
        }
    }

    // Statistics helpers
    fn mean(vals: &[f64]) -> f64 {
        if vals.is_empty() { return 0.0; }
        vals.iter().sum::<f64>() / vals.len() as f64
    }
    fn median(vals: &mut [f64]) -> f64 {
        if vals.is_empty() { return 0.0; }
        vals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mid = vals.len() / 2;
        if vals.len() % 2 == 0 {
            (vals[mid - 1] + vals[mid]) / 2.0
        } else {
            vals[mid]
        }
    }
    fn std_dev(vals: &[f64], avg: f64) -> f64 {
        if vals.len() <= 1 { return 0.0; }
        let variance = vals.iter().map(|v| (v - avg).powi(2)).sum::<f64>() / (vals.len() - 1) as f64;
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
    let latency: Vec<f64> = results.iter().map(|r| r.metrics.latency_per_token).collect();
    let mut first_token: Vec<f64> = results.iter().map(|r| r.metrics.first_token_time).collect();

    let mut gen_tps_sorted = gen_tps.clone();
    let mut latency_sorted = latency.clone();

    let avg_gen_tps = mean(&gen_tps);
    let avg_prompt_tps = mean(&prompt_tps);
    let avg_latency = mean(&latency);
    let avg_first_token = mean(&first_token);
    let _avg_combined_tps = mean(&results.iter().map(|r| r.metrics.combined_tps).collect::<Vec<f64>>());

    let gen_std = std_dev(&gen_tps, avg_gen_tps);
    let prompt_std = std_dev(&prompt_tps, avg_prompt_tps);
    let lat_std = std_dev(&latency, avg_latency);
    let ft_std = std_dev(&first_token, avg_first_token);

    let best_idx = results.iter().enumerate()
        .max_by(|a, b| a.1.metrics.generation_tps.partial_cmp(&b.1.metrics.generation_tps).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(i, _)| i);
    let best_gen_tps = if !gen_tps.is_empty() { max_val(&gen_tps) } else { 0.0 };
    let best_prompt_tps = if !prompt_tps.is_empty() { max_val(&prompt_tps) } else { 0.0 };
    let best_latency = if !latency.is_empty() { min_val(&latency) } else { 0.0 };
    let best_first_token = if !first_token.is_empty() { min_val(&first_token) } else { 0.0 };
    let min_gen_tps = min_val(&gen_tps);
    let min_prompt_tps = min_val(&prompt_tps);
    let min_latency = min_val(&latency);
    let min_first_token = min_val(&first_token);

    // Per-parameter impact analysis
    let param_names = vec![
        ("temperature", "Temperature"),
        ("top_p", "Top-P"),
        ("top_k", "Top-K"),
        ("repeat_penalty", "Repeat Penalty"),
        ("flash_attn", "Flash Attention"),
        ("threads", "Threads"),
        ("batch_size", "Batch Size"),
        ("expert_count", "Experts"),
    ];

    let impact_data: Vec<(String, String, f64)> = param_names.iter().filter_map(|(key, label)| {
        let values: Vec<f64> = results.iter().filter_map(|r| {
            let base = r.base_settings.as_ref()?;
            let rp = resolve_params(&r.params, base);
            Some(match *key {
                "temperature" => rp.temperature,
                "top_p" => rp.top_p,
                "top_k" => rp.top_k as f64,
                "repeat_penalty" => rp.repeat_penalty,
                "flash_attn" => if rp.flash_attn { 1.0 } else { 0.0 },
                "threads" => rp.threads as f64,
                "batch_size" => rp.batch_size as f64,
                "expert_count" => rp.expert_count as f64,
                _ => return None,
            })
        }).collect();

      // Group by parameter value and compute mean gen_tps per group
        let mut groups: std::collections::HashMap<String, Vec<f64>> = std::collections::HashMap::new();
        for (r, v) in results.iter().zip(values.iter()) {
            let key_str = if *key == "flash_attn" {
                if *v > 0.5 { "ON".to_string() } else { "OFF".to_string() }
            } else {
                format!("{:.2}", v)
            };
            groups.entry(key_str).or_default().push(r.metrics.generation_tps);
        }

        if groups.len() <= 1 { return None; } // Parameter doesn't vary

        let group_means: Vec<f64> = groups.values().map(|vals| mean(vals)).collect();
        let spread = max_val(&group_means) - min_val(&group_means);
        Some((label.to_string(), format!("{:.1}", spread), spread))
    }).collect();

    // Sort by impact (spread) descending
    let mut impact_sorted = impact_data.clone();
    impact_sorted.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

    // Consistency indicator (coefficient of variation from per-iteration metrics)
    let consistency_data: Vec<f64> = results.iter().map(|r| {
        if r.per_iteration_metrics.len() <= 1 {
            return 1.0; // No consistency data = neutral
        }
        let iter_gen_tps: Vec<f64> = r.per_iteration_metrics.iter().map(|m| m.generation_tps).collect();
        let iter_mean = mean(&iter_gen_tps);
        if iter_mean == 0.0 { return 1.0; }
        let iter_std = std_dev(&iter_gen_tps, iter_mean);
        let cv = iter_std / iter_mean; // Coefficient of variation
        // Map CV to 0-1 score (lower CV = more consistent = higher score)
        // CV of 0% = 1.0 (perfect), CV of 20%+ = 0.0 (poor)
        (1.0 - (cv * 5.0)).clamp(0.0, 1.0)
    }).collect();

    // Top N for charts
    let top_n = std::cmp::min(20, total_tests);
    let top_indices: Vec<(usize, usize)> = (0..total_tests)
        .map(|i| (i, results[i].metrics.generation_tps))
        .enumerate()
        .take(top_n)
        .map(|(rank, (idx, _))| (rank + 1, idx))
        .collect();

    let top_labels: Vec<String> = top_indices.iter().map(|(_rank, idx)| {
        let base = results[*idx].base_settings.as_ref().unwrap();
        let rp = resolve_params(&results[*idx].params, base);
        format!("T={:.2} TP={:.2}", rp.temperature, rp.top_p)
    }).collect();
    let top_gen_tps: Vec<f64> = top_indices.iter().map(|(_, idx)| results[*idx].metrics.generation_tps).collect();

    // Scatter data with labels
    let scatter_gen_tps: Vec<f64> = results.iter().map(|r| r.metrics.generation_tps).collect();
    let scatter_latency: Vec<f64> = results.iter().map(|r| r.metrics.latency_per_token).collect();
    let scatter_first_token: Vec<f64> = results.iter().map(|r| r.metrics.first_token_time).collect();

    let param_headers: Vec<String> = vec!["Temp".to_string(), "Top-P".to_string(), "Top-K".to_string(), "RepPen".to_string(), "FA".to_string(), "Threads".to_string(), "Batch".to_string(), "Exp".to_string()];
    let param_vals: Vec<Vec<String>> = results.iter().map(|r| {
        let base = r.base_settings.as_ref().unwrap();
        let rp = resolve_params(&r.params, base);
        vec![
            format!("{:.2}", rp.temperature),
            format!("{:.2}", rp.top_p),
            rp.top_k.to_string(),
            format!("{:.2}", rp.repeat_penalty),
            if rp.flash_attn { "ON".to_string() } else { "OFF".to_string() },
            rp.threads.to_string(),
            rp.batch_size.to_string(),
            rp.expert_count.to_string(),
        ]
    }).collect();

    // Build metrics JSON with consistency data
    let metrics_data: Vec<serde_json::Value> = results.iter().enumerate().map(|(i, r)| {
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
    }).collect();

    // Scatter data with labels for tooltips
    let scatter_data_json = serde_json::to_string(&scatter_gen_tps.iter().zip(scatter_latency.iter()).zip(scatter_first_token.iter()).map(|((g, l), f)| {
        let mut s = String::from("{x:");
        s.push_str(&format!("{:.2}", g));
        s.push_str(",y:");
        s.push_str(&format!("{:.2}", l));
        s.push_str(",ft:");
        s.push_str(&format!("{:.2}", f));
        s.push('}');
        s
    }).collect::<Vec<_>>()).unwrap();
    let scatter_data2_json = serde_json::to_string(&scatter_gen_tps.iter().zip(scatter_first_token.iter()).map(|(g, f)| {
        let mut s = String::from("{x:");
        s.push_str(&format!("{:.2}", g));
        s.push_str(",y:");
        s.push_str(&format!("{:.2}", f));
        s.push_str(",lat:");
        s.push_str(&format!("{:.2}", min_val(&latency)));
        s.push('}');
        s
    }).collect::<Vec<_>>()).unwrap();

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
        ("col-gen-tps", "Gen t/s", true),
        ("col-prompt-tps", "Prompt t/s", true),
        ("col-latency", "Latency", true),
        ("col-first-token", "First Tok", true),
        ("col-combined", "Combined", true),
        ("col-consistency", "Consistency", true),
    ]).unwrap();

    // CSV data
    let csv_header = "Rank,Temp,Top-P,Top-K,RepPen,FA,Threads,Batch,Exp,Gen t/s,Prompt t/s,Latency (ms),First Tok (ms),Combined,Consistency";
    let csv_rows: Vec<String> = (0..total_tests).map(|i| {
        let d = &metrics_data[i];
        let rank = i + 1;
        format!("{},{},{},{},{},{},{},{},{},{},{},{},{},{},{:.1}",
            rank,
            d["temp"].as_f64().unwrap_or(0.0),
            d["top_p"].as_f64().unwrap_or(0.0),
            d["top_k"].as_i64().unwrap_or(0),
            d["repeat_penalty"].as_f64().unwrap_or(0.0),
            if d["flash_attn"].as_bool().unwrap_or(false) { "ON" } else { "OFF" },
            d["threads"].as_u64().unwrap_or(0),
            d["batch_size"].as_u64().unwrap_or(0),
            d["expert_count"].as_i64().unwrap_or(0),
            d["generation_tps"].as_f64().unwrap_or(0.0),
            d["prompt_tps"].as_f64().unwrap_or(0.0),
            d["latency_per_token"].as_f64().unwrap_or(0.0),
            d["first_token_time"].as_f64().unwrap_or(0.0),
            d["combined_tps"].as_f64().unwrap_or(0.0),
            d["consistency"].as_f64().unwrap_or(1.0)
        )
    }).collect();
    let csv_content = format!("{}\n{}", csv_header, csv_rows.join("\n"));
    let csv_b64 = base64_encode(&csv_content);

    let metrics_json = serde_json::to_string(&metrics_data).unwrap();
    let param_headers_json = serde_json::to_string(&param_headers).unwrap();
    let param_vals_json = serde_json::to_string(&param_vals).unwrap();
    let top_labels_json = serde_json::to_string(&top_labels).unwrap();
    let top_gen_tps_json = serde_json::to_string(&top_gen_tps).unwrap();

    // Build model metadata HTML
    let model_meta_html = model_info.as_ref().map(|(name, _size, s)| {
         format!(r#"
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
    }).unwrap_or_default();

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
<div class="winner-params">Temp: {:.2} &middot; Top-P: {:.2} &middot; Top-K: {} &middot; RepPen: {:.2} &middot; FA: {} &middot; Threads: {} &middot; Batch: {} &middot; Exp: {}</div>
</div>
</div>"#,
                m.generation_tps, m.prompt_tps, m.latency_per_token, m.first_token_time,
                rp.temperature, rp.top_p, rp.top_k, rp.repeat_penalty,
                if rp.flash_attn { "ON" } else { "OFF" }, rp.threads,
                rp.batch_size, rp.expert_count
            ))
    }).unwrap_or_default();

    // Build impact analysis HTML
    let impact_html = if !impact_sorted.is_empty() {
        let max_impact = impact_sorted[0].2;
        let rows: String = impact_sorted.iter().map(|(label, spread, value)| {
            let bar_width = if max_impact > 0.0 { (value / max_impact * 100.0) as i32 } else { 0 };
            let bar_color = if *value > max_impact * 0.7 { "#f85149" } else if *value > max_impact * 0.4 { "#d29922" } else { "#3fb950" };
            format!(r#"<div class="impact-row">
<div class="impact-label">{}</div>
<div class="impact-bar-bg"><div class="impact-bar-fill" style="width:{}%;background:{}"></div></div>
<div class="impact-value">{}</div>
</div>"#, label, bar_width, bar_color, spread)
        }).collect();
        format!(r#"
<div class="impact-section">
<h2>Parameter Impact Analysis</h2>
<p class="impact-desc">Larger spread in generation throughput between parameter values indicates greater impact on performance.</p>
{}
</div>"#, rows)
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

    let html = r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>LLM Benchmark Report</title>
<script src="https://cdn.jsdelivr.net/npm/chart.js@4.4.7/dist/chart.umd.min.js"></script>
<style>
* { margin: 0; padding: 0; box-sizing: border-box; }
body { font-family: 'Segoe UI', system-ui, -apple-system, sans-serif; background: #0d1117; color: #c9d1d9; line-height: 1.6; }
.container { max-width: 1400px; margin: 0 auto; padding: 20px; }
h1 { text-align: center; padding: 20px 0; color: #58a6ff; font-size: 2em; border-bottom: 1px solid #21262d; margin-bottom: 20px; }
h2 { color: #58a6ff; font-size: 1.3em; margin: 20px 0 10px; padding-bottom: 5px; border-bottom: 1px solid #21262d; }
p { margin: 5px 0; }

/* Empty state */
.empty-state { text-align: center; padding: 60px 20px; color: #8b949e; }
.empty-icon { font-size: 4em; margin-bottom: 15px; }
.empty-title { font-size: 1.5em; color: #c9d1d9; margin-bottom: 10px; }
.empty-text { font-size: 1em; }

/* Summary cards */
.summary-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(180px, 1fr)); gap: 12px; margin: 20px 0; }
.summary-card { background: #161b22; border: 1px solid #30363d; border-radius: 8px; padding: 15px; text-align: center; }
.summary-card .value { font-size: 1.6em; font-weight: bold; color: #58a6ff; }
.summary-card .label { font-size: 0.8em; color: #8b949e; margin-top: 4px; }
.summary-card .stats { font-size: 0.7em; color: #6e7681; margin-top: 3px; }
.best-card .value { color: #3fb950; }

/* Winner section */
.winner-section { background: linear-gradient(135deg, #1a2332 0%, #161b22 100%); border: 2px solid #3fb950; border-radius: 12px; padding: 25px; margin: 20px 0; display: flex; align-items: center; gap: 20px; }
.winner-icon { font-size: 3em; flex-shrink: 0; }
.winner-title { font-size: 1.2em; color: #3fb950; font-weight: bold; margin-bottom: 8px; }
.winner-metrics { display: flex; gap: 25px; flex-wrap: wrap; margin: 10px 0; }
.winner-metric { display: flex; flex-direction: column; }
.wm-label { font-size: 0.7em; color: #8b949e; text-transform: uppercase; }
.wm-value { font-size: 1.1em; color: #c9d1d9; font-weight: 600; }
.winner-params { font-size: 0.85em; color: #8b949e; margin-top: 8px; }

/* Meta section */
.meta-section { margin: 20px 0; }
.meta-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: 10px; }
.meta-item { background: #161b22; border: 1px solid #30363d; border-radius: 6px; padding: 10px 14px; }
.ml { font-size: 0.7em; color: #8b949e; text-transform: uppercase; }
.mv { font-size: 1em; color: #c9d1d9; margin-top: 2px; word-break: break-word; }
.meta-prompt { font-size: 0.85em; color: #8b949e; max-height: 3em; overflow: hidden; }

/* Impact analysis */
.impact-section { margin: 20px 0; }
.impact-desc { font-size: 0.85em; color: #8b949e; margin-bottom: 12px; }
.impact-row { display: flex; align-items: center; gap: 12px; margin: 8px 0; }
.impact-label { width: 140px; font-size: 0.9em; color: #c9d1d9; flex-shrink: 0; }
.impact-bar-bg { flex: 1; background: #21262d; border-radius: 4px; height: 20px; overflow: hidden; }
.impact-bar-fill { height: 100%; border-radius: 4px; transition: width 0.5s; }
.impact-value { width: 80px; text-align: right; font-size: 0.85em; color: #8b949e; flex-shrink: 0; }

/* Charts */
.charts-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 20px; margin: 20px 0; }
.chart-container { background: #161b22; border: 1px solid #30363d; border-radius: 8px; padding: 15px; }
.chart-container canvas { max-height: 350px; }

/* Table */
table { width: 100%; border-collapse: collapse; background: #161b22; border-radius: 8px; overflow: hidden; margin: 15px 0; font-size: 0.85em; }
th { background: #21262d; color: #58a6ff; padding: 10px 8px; text-align: center; cursor: pointer; user-select: none; white-space: nowrap; position: relative; }
th:hover { background: #30363d; }
th .col-toggle { position: absolute; top: 2px; right: 2px; font-size: 0.6em; opacity: 0.5; cursor: pointer; }
td { padding: 8px; text-align: center; border-top: 1px solid #21262d; }
tr:hover { background: #1c2128; }
tr.expanded { background: #1c2128; }
.detail-row { display: none; background: #0d1117; }
.detail-row.visible { display: table-row; }
.detail-cell { padding: 15px; }
.detail-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(150px, 1fr)); gap: 10px; margin: 10px 0; }
.detail-item { background: #161b22; border: 1px solid #30363d; border-radius: 6px; padding: 8px 12px; }
.detail-item .dl { font-size: 0.75em; color: #8b949e; text-transform: uppercase; }
.detail-item .dv { font-size: 1.1em; color: #c9d1d9; }
.output-text { background: #0d1117; border: 1px solid #30363d; border-radius: 6px; padding: 10px; margin: 10px 0; max-height: 200px; overflow-y: auto; font-family: 'Cascadia Code', 'Fira Code', monospace; font-size: 0.85em; color: #8b949e; white-space: pre-wrap; word-break: break-word; }
.iteration-table { width: 100%; border-collapse: collapse; margin: 10px 0; font-size: 0.85em; }
.iteration-table th { background: #161b22; color: #8b949e; padding: 5px 8px; }
.iteration-table td { padding: 5px 8px; border-top: none; }

/* Consistency stars */
.consistency-stars { letter-spacing: 1px; }
.star-full { color: #3fb950; }
.star-half { color: #d29922; }
.star-empty { color: #484f58; }

/* Filter bar */
.filter-bar { display: flex; gap: 10px; margin: 15px 0; flex-wrap: wrap; align-items: center; }
.filter-bar input, .filter-bar select, .filter-bar button { background: #161b22; border: 1px solid #30363d; color: #c9d1d9; padding: 6px 12px; border-radius: 6px; font-size: 0.9em; }
.filter-bar input:focus, .filter-bar select:focus { outline: none; border-color: #58a6ff; }
.filter-bar button:hover { border-color: #58a6ff; cursor: pointer; }
.filter-bar button.primary { background: #238636; border-color: #238636; }
.filter-bar button.primary:hover { background: #2ea043; }

/* Column visibility */
.col-vis-bar { display: flex; gap: 8px; margin: 10px 0; flex-wrap: wrap; padding: 10px; background: #161b22; border: 1px solid #30363d; border-radius: 6px; }
.col-vis-btn { padding: 3px 10px; border-radius: 12px; font-size: 0.75em; cursor: pointer; border: 1px solid #30363d; background: #21262d; color: #c9d1d9; transition: all 0.2s; }
.col-vis-btn.active { background: #58a6ff; border-color: #58a6ff; color: #0d1117; }
.col-vis-btn:hover { border-color: #58a6ff; }

/* Expand hint */
.expand-hint { color: #8b949e; font-size: 0.8em; }

/* Responsive */
@media (max-width: 900px) { .charts-grid { grid-template-columns: 1fr; } }
@media (max-width: 600px) { .summary-grid { grid-template-columns: repeat(2, 1fr); } .winner-section { flex-direction: column; text-align: center; } .winner-metrics { justify-content: center; } }

/* Print styles */
@media print {
    body { background: #fff; color: #000; }
    .container { max-width: 100%; padding: 10px; }
    h1, h2 { color: #000; border-bottom-color: #ccc; }
    .summary-card, .meta-item, .chart-container, .detail-item { background: #f8f8f8; border-color: #ccc; }
    .summary-card .value, .detail-item .dv { color: #000; }
    .winner-section { border-color: #999; background: #f0f0f0; }
    .winner-title, .winner-metric .wm-value { color: #000; }
    .filter-bar, .col-vis-bar, .no-print { display: none !important; }
    .detail-row { display: table-row !important; }
    .chart-container { page-break-inside: avoid; }
    table { font-size: 0.75em; }
    th { background: #e0e0e0 !important; color: #000 !important; }
    td { color: #000; }
    .impact-bar-bg { border: 1px solid #999; }
    .impact-bar-fill { background: #666 !important; }
}
</style>
</head>
<body>
<div class="container">
<h1>LLM Benchmark Report</h1>
<p style="text-align:center;color:#8b949e;margin-bottom:15px;">Generated: __TIMESTAMP__ &middot; __TOTAL_TESTS__ tests completed</p>

__EMPTY_STATE__
__MODEL_META__
__WINNER__

<div class="summary-grid">
<div class="summary-card">
<div class="value">__AVG_GEN_TPS__</div>
<div class="label">Avg Gen t/s</div>
<div class="stats">Std: __GEN_STD__ &middot; Range: [__MIN_GEN__, __MAX_GEN__]</div>
</div>
<div class="summary-card">
<div class="value">__MED_GEN_TPS__</div>
<div class="label">Median Gen t/s</div>
</div>
<div class="summary-card">
<div class="value">__AVG_PROMPT_TPS__</div>
<div class="label">Avg Prompt t/s</div>
<div class="stats">Std: __PROMPT_STD__ &middot; Range: [__MIN_PROMPT__, __MAX_PROMPT__]</div>
</div>
<div class="summary-card">
<div class="value">__MED_PROMPT_TPS__</div>
<div class="label">Median Prompt t/s</div>
</div>
<div class="summary-card">
<div class="value">__AVG_LATENCY__</div>
<div class="label">Avg Latency/token</div>
<div class="stats">Std: __LAT_STD__ &middot; Range: [__MIN_LAT__, __MAX_LAT__]</div>
</div>
<div class="summary-card">
<div class="value">__MED_LATENCY__</div>
<div class="label">Median Latency</div>
</div>
<div class="summary-card">
<div class="value">__AVG_FT__</div>
<div class="label">Avg First Token</div>
<div class="stats">Std: __FT_STD__ &middot; Range: [__MIN_FT__, __MAX_FT__]</div>
</div>
<div class="summary-card best-card">
<div class="value">__BEST_GEN__</div>
<div class="label">Best Gen t/s</div>
</div>
</div>

__IMPACT_HTML__

<h2>Performance Comparison (Top __TOP_N__)</h2>
<div class="charts-grid">
<div class="chart-container">
<canvas id="barChart"></canvas>
</div>
<div class="chart-container">
<canvas id="scatterChart"></canvas>
</div>
</div>

<h2>Latency vs Throughput</h2>
<div class="chart-container" style="margin:15px 0;">
<canvas id="scatter2Chart"></canvas>
</div>

<h2>All Results <span class="expand-hint">(click row to expand details)</span></h2>
<div class="col-vis-bar no-print" id="colVisBar"></div>
<div class="filter-bar no-print">
<input type="text" id="filterInput" placeholder="Filter parameters..." oninput="filterTable()">
<select id="sortSelect" onchange="sortTable(parseInt(this.value))">
<option value="0">Sort: Gen t/s (desc)</option>
<option value="1">Sort: Prompt t/s (desc)</option>
<option value="2">Sort: Latency (asc)</option>
<option value="3">Sort: First Token (asc)</option>
</select>
<button onclick="exportCSV()" class="primary">&#128190; Export CSV</button>
</div>
<div style="overflow-x:auto;">
<table id="resultsTable">
<thead>
<tr id="tableHeaderRow"></tr>
</thead>
<tbody id="resultsBody"></tbody>
</table>
</div>
</div>

<script>
const DATA = __METRICS_JSON__;
const PARAM_HEADERS = __PARAM_HEADERS_JSON__;
const PARAM_VALS = __PARAM_VALS_JSON__;
const COLUMN_DEFS = __COLUMN_DEFS_JSON__;
const CSV_B64 = '__CSV_B64__';
const MODEL_META = __MODEL_META_JSON__;
const currentSort = { col: 0, asc: false };
let displayOrder = DATA.map((_, i) => i);
let colVisibility = {};

// Initialize column visibility from definitions
COLUMN_DEFS.forEach(c => { colVisibility[c[0]] = c[2]; });

function formatNum(n, d=2) { return typeof n === 'number' ? n.toFixed(d) : '-'; }

function getMetricColor(val, min, max, invert) {
    if (max === min) return '#c9d1d9';
    const ratio = invert ? (max - val) / (max - min) : (val - min) / (max - min);
    const r = Math.round(248 * (1 - ratio) + 63 * ratio);
    const g = Math.round(81 * ratio + 185 * (1 - ratio));
    const b = Math.round(73 * ratio + 80 * (1 - ratio));
    return `rgb(${r},${g},${b})`;
}

function consistencyStars(score) {
    const full = Math.floor(score * 5);
    const half = (score * 5 - full) >= 0.5 ? 1 : 0;
    const empty = 5 - full - half;
    return '<span class="star-full">' + '\u2605'.repeat(full) + '</span>' +
           (half ? '<span class="star-half">\u2606</span>' : '') +
           '<span class="star-empty">' + '\u2606'.repeat(empty) + '</span>';
}

function escapeHtml(s) {
    return String(s).replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;');
}

// Render column visibility toggles
function renderColVis() {
    const bar = document.getElementById('colVisBar');
    bar.innerHTML = '<span style="font-size:0.8em;color:#8b949e;margin-right:5px;">Columns:</span>' +
        COLUMN_DEFS.map(c =>
            `<span class="col-vis-btn ${colVisibility[c[0]] ? 'active' : ''}" data-col="${c[0]}" onclick="toggleCol('${c[0]}')">${c[1]}</span>`
        ).join('') +
        '<span class="col-vis-btn" style="margin-left:auto;" onclick="resetCols()">Reset</span>';
}

function toggleCol(colId) {
    colVisibility[colId] = !colVisibility[colId];
    renderColVis();
    renderTable();
}

function resetCols() {
    COLUMN_DEFS.forEach(c => colVisibility[c[0]] = c[2]);
    renderColVis();
    renderTable();
}

// Render table header with column toggles
function renderHeader() {
    const row = document.getElementById('tableHeaderRow');
    row.innerHTML = COLUMN_DEFS.map((c, i) =>
        colVisibility[c[0]] ? `<th onclick="sortTable(${i})" style="display:table-cell;">${c[1]}</th>` : ''
    ).join('');
}

function renderTable() {
    const tbody = document.getElementById('resultsBody');
    tbody.innerHTML = '';
    const genMin = Math.min(...DATA.map(d => d.generation_tps));
    const genMax = Math.max(...DATA.map(d => d.generation_tps));
    const latMin = Math.min(...DATA.map(d => d.latency_per_token));
    const latMax = Math.max(...DATA.map(d => d.latency_per_token));
    const ftMin = Math.min(...DATA.map(d => d.first_token_time));
    const ftMax = Math.max(...DATA.map(d => d.first_token_time));

    displayOrder.forEach((idx, rank) => {
        const d = DATA[idx];
        const tr = document.createElement('tr');
        tr.dataset.idx = idx;
        tr.onclick = (e) => { if(e.target.tagName !== 'BUTTON') toggleDetail(idx); };

        const genColor = getMetricColor(d.generation_tps, genMin, genMax, false);
        const latColor = getMetricColor(d.latency_per_token, latMin, latMax, true);
        const ftColor = getMetricColor(d.first_token_time, ftMin, ftMax, true);

        const cells = [
            `<td>${rank + 1}</td>`,
            `<td class="col-temp">${PARAM_VALS[idx][0]}</td>`,
            `<td class="col-top-p">${PARAM_VALS[idx][1]}</td>`,
            `<td class="col-top-k">${PARAM_VALS[idx][2]}</td>`,
            `<td class="col-rep-pen">${PARAM_VALS[idx][3]}</td>`,
            `<td class="col-fa">${PARAM_VALS[idx][4]}</td>`,
            `<td class="col-threads">${PARAM_VALS[idx][5]}</td>`,
            `<td class="col-batch">${PARAM_VALS[idx][6]}</td>`,
            `<td class="col-exp">${PARAM_VALS[idx][7]}</td>`,
            `<td class="col-gen-tps" style="color:${genColor};font-weight:bold;">${formatNum(d.generation_tps)}</td>`,
            `<td class="col-prompt-tps">${formatNum(d.prompt_tps)}</td>`,
            `<td class="col-latency" style="color:${latColor}">${formatNum(d.latency_per_token)}</td>`,
            `<td class="col-first-token" style="color:${ftColor}">${formatNum(d.first_token_time)}</td>`,
            `<td class="col-combined">${formatNum(d.combined_tps)}</td>`,
            `<td class="col-consistency"><span class="consistency-stars">${consistencyStars(d.consistency)}</span></td>`,
        ];

        tr.innerHTML = cells.join('');
        tbody.appendChild(tr);

        const detailTr = document.createElement('tr');
        detailTr.className = 'detail-row';
        detailTr.id = 'detail-' + idx;
        const outputsHtml = d.outputs.map((o, oi) => `<div style="margin:8px 0;"><strong>Iteration ${oi + 1}:</strong><div class="output-text">${escapeHtml(o)}</div></div>`).join('');
        detailTr.innerHTML = `<td colspan="15" class="detail-cell">
            <div class="detail-grid">
                <div class="detail-item"><div class="dl">Gen t/s</div><div class="dv" style="color:#3fb950">${formatNum(d.generation_tps)}</div></div>
                <div class="detail-item"><div class="dl">Prompt t/s</div><div class="dv">${formatNum(d.prompt_tps)}</div></div>
                <div class="detail-item"><div class="dl">Latency/token</div><div class="dv">${formatNum(d.latency_per_token)}ms</div></div>
                <div class="detail-item"><div class="dl">First Token</div><div class="dv">${formatNum(d.first_token_time)}ms</div></div>
                <div class="detail-item"><div class="dl">Combined t/s</div><div class="dv">${formatNum(d.combined_tps)}</div></div>
                <div class="detail-item"><div class="dl">Consistency</div><div class="dv"><span class="consistency-stars">${consistencyStars(d.consistency)}</span></div></div>
            </div>
            <h3 style="color:#58a6ff;margin:10px 0 5px;font-size:1em;">Per-Iteration Metrics</h3>
            <table class="iteration-table"><thead><tr><th>Iter</th><th>Gen t/s</th><th>Latency (ms)</th><th>First Tok (ms)</th></tr></thead><tbody>
            ${d.per_iteration_metrics.map((m, mi) => `<tr><td>${mi + 1}</td><td>${formatNum(m.generation_tps)}</td><td>${formatNum(m.latency_per_token)}</td><td>${formatNum(m.first_token_time)}</td></tr>`).join('')}
            </tbody></table>
            ${outputsHtml}
        </td>`;
        tbody.appendChild(detailTr);
    });

    // Apply column visibility
    COLUMN_DEFS.forEach(c => {
        const cols = document.querySelectorAll('.' + c[0].replace('col-', 'col-'));
        // Use a simpler approach: set display on each column class
    });

    // Apply column visibility using CSS classes
    COLUMN_DEFS.forEach(c => {
        const cells = document.querySelectorAll('.' + c[0]);
        cells.forEach(cell => {
            cell.style.display = colVisibility[c[0]] ? '' : 'none';
        });
    });
}

function toggleDetail(idx) {
    const row = document.getElementById('detail-' + idx);
    row.classList.toggle('visible');
    row.previousElementSibling?.classList.toggle('expanded');
}

function filterTable() {
    const q = document.getElementById('filterInput').value.toLowerCase();
    const rows = document.querySelectorAll('#resultsBody tr:not(.detail-row)');
    rows.forEach(row => {
        const text = row.textContent.toLowerCase();
        row.style.display = text.includes(q) ? '' : 'none';
    });
}

function sortTable(col) {
    if (currentSort.col === col) { currentSort.asc = !currentSort.asc; }
    else { currentSort.col = col; currentSort.asc = col <= 1; }
    const keys = [null, 'generation_tps', 'prompt_tps', 'latency_per_token', 'first_token_time'];
    const key = keys[col];
    if (!key) return;
    displayOrder.sort((a, b) => {
        const va = DATA[a][key], vb = DATA[b][key];
        return currentSort.asc ? va - vb : vb - va;
    });
    renderTable();
}

function exportCSV() {
    const link = document.createElement('a');
    link.href = 'data:text/csv;base64,' + CSV_B64;
    link.download = 'benchmark_results.csv';
    link.click();
}

// Charts with trend lines
const barCtx = document.getElementById('barChart').getContext('2d');
new Chart(barCtx, {
    type: 'bar',
    data: {
       labels: __TOP_LABELS_JSON__,
        datasets: [{
            label: 'Generation Throughput (tokens/s)',
            data: __TOP_GEN_TPS_JSON__,
            backgroundColor: 'rgba(88, 166, 255, 0.6)',
            borderColor: 'rgba(88, 166, 255, 1)',
            borderWidth: 1,
        }]
    },
    options: {
        responsive: true,
        maintainAspectRatio: false,
        plugins: {
            legend: { display: false },
            title: { display: true, text: 'Top __TOP_N__ Configs by Gen t/s', color: '#c9d1d9' },
            tooltip: {
                backgroundColor: '#161b22',
                titleColor: '#58a6ff',
                bodyColor: '#c9d1d9',
                borderColor: '#30363d',
                borderWidth: 1,
                padding: 10,
                displayColors: false,
            }
        },
        scales: {
            y: { beginAtZero: true, grid: { color: '#21262d' }, ticks: { color: '#8b949e' } },
            x: { grid: { display: false }, ticks: { color: '#8b949e', maxRotation: 45, font: { size: 10 } } }
        }
    }
});

const scatterCtx = document.getElementById('scatterChart').getContext('2d');
new Chart(scatterCtx, {
    type: 'scatter',
    data: {
        datasets: [{
            label: 'Config',
            data: __SCATTER_DATA_JSON__,
            backgroundColor: 'rgba(88, 166, 255, 0.6)',
            borderColor: 'rgba(88, 166, 255, 1)',
            pointRadius: 6,
            pointHoverRadius: 8,
        }]
    },
    options: {
        responsive: true,
        maintainAspectRatio: false,
        plugins: {
            legend: { display: false },
            title: { display: true, text: 'Throughput vs Latency (lower-right = better)', color: '#c9d1d9' },
            tooltip: {
                backgroundColor: '#161b22',
                titleColor: '#58a6ff',
                bodyColor: '#c9d1d9',
                borderColor: '#30363d',
                borderWidth: 1,
                padding: 10,
                callbacks: {
                    label: function(ctx) {
                        return `Gen: ${ctx.parsed.x.toFixed(2)} t/s, Lat: ${ctx.parsed.y.toFixed(2)} ms/token`;
                    }
                }
            }
        },
        scales: {
            x: { title: { display: true, text: 'Gen t/s', color: '#8b949e' }, grid: { color: '#21262d' }, ticks: { color: '#8b949e' } },
            y: { title: { display: true, text: 'Latency (ms/token)', color: '#8b949e' }, grid: { color: '#21262d' }, ticks: { color: '#8b949e' }, reverse: true }
        }
    }
});

const scatter2Ctx = document.getElementById('scatter2Chart').getContext('2d');
new Chart(scatter2Ctx, {
    type: 'scatter',
    data: {
        datasets: [{
            label: 'Config',
            data: __SCATTER_DATA2_JSON__,
            backgroundColor: 'rgba(63, 185, 80, 0.6)',
            borderColor: 'rgba(63, 185, 80, 1)',
            pointRadius: 6,
            pointHoverRadius: 8,
        }]
    },
    options: {
        responsive: true,
        maintainAspectRatio: false,
        plugins: {
            legend: { display: false },
            title: { display: true, text: 'Throughput vs First Token Latency', color: '#c9d1d9' },
            tooltip: {
                backgroundColor: '#161b22',
                titleColor: '#58a6ff',
                bodyColor: '#c9d1d9',
                borderColor: '#30363d',
                borderWidth: 1,
                padding: 10,
                callbacks: {
                    label: function(ctx) {
                        return `Gen: ${ctx.parsed.x.toFixed(2)} t/s, First Tok: ${ctx.parsed.y.toFixed(2)} ms`;
                    }
                }
            }
        },
        scales: {
            x: { title: { display: true, text: 'Gen t/s', color: '#8b949e' }, grid: { color: '#21262d' }, ticks: { color: '#8b949e' } },
            y: { title: { display: true, text: 'First Token (ms)', color: '#8b949e' }, grid: { color: '#21262d' }, ticks: { color: '#8b949e' }, reverse: true }
        }
    }
});

// Initialize
renderColVis();
renderHeader();
renderTable();
</script>
</body>
</html>
"#;

    // Replace placeholders
    html.replace("__TIMESTAMP__", &timestamp)
        .replace("__TOTAL_TESTS__", &total_tests.to_string())
        .replace("__EMPTY_STATE__", empty_html)
        .replace("__MODEL_META__", &model_meta_html)
        .replace("__WINNER__", &winner_html)
        .replace("__AVG_GEN_TPS__", &format!("{:.1}", avg_gen_tps))
        .replace("__MED_GEN_TPS__", &format!("{:.1}", median(&mut gen_tps_sorted)))
        .replace("__GEN_STD__", &format!("{:.1}", gen_std))
        .replace("__MIN_GEN__", &format!("{:.1}", min_gen_tps))
        .replace("__MAX_GEN__", &format!("{:.1}", best_gen_tps))
        .replace("__AVG_PROMPT_TPS__", &format!("{:.1}", avg_prompt_tps))
        .replace("__MED_PROMPT_TPS__", &format!("{:.1}", median(&mut prompt_tps)))
        .replace("__PROMPT_STD__", &format!("{:.1}", prompt_std))
        .replace("__MIN_PROMPT__", &format!("{:.1}", min_prompt_tps))
        .replace("__MAX_PROMPT__", &format!("{:.1}", best_prompt_tps))
        .replace("__AVG_LATENCY__", &format!("{:.1}ms", avg_latency))
        .replace("__MED_LATENCY__", &format!("{:.1}ms", median(&mut latency_sorted)))
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
        .replace("__MODEL_META_JSON__", &serde_json::to_string(&model_meta_json).unwrap())
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
