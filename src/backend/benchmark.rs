use std::path::PathBuf;
use std::time::{Duration, Instant};

use tokio::sync::mpsc;

use crate::backend::server::spawn_server;
use crate::models::{BenchTuneConfig, BenchTuneMetrics, BenchTuneParamValue, BenchTuneResult, BenchTuneStatus, ModelSettings, ServerMode, DiscoveredModel};

/// Run a benchmark tuning test with multiple parameter combinations
pub async fn run_bench_tune(
    config: &BenchTuneConfig,
    model: &DiscoveredModel,
    settings: &ModelSettings,
    progress_tx: mpsc::Sender<BenchTuneStatus>,
    log_tx: mpsc::Sender<String>,
) -> Result<Vec<BenchTuneResult>, Box<dyn std::error::Error + Send + Sync>> {
    let start_time = Instant::now();
    let total_tests = config.get_total_tests_count();
    
    // Generate all parameter combinations
    let combinations = config.generate_combinations();
    
    // Results storage
    let mut results = Vec::new();
    
    // Run each parameter combination
    for (idx, combination) in combinations.iter().enumerate() {
        // Update progress
        let progress = (idx as f32 / total_tests as f32) * 100.0;
        progress_tx.send(BenchTuneStatus::Running {
            current: idx + 1,
            total: total_tests,
            progress,
            current_params: combination.clone(),
        }).await?;
        
        // Run the test
        let result = run_bench_tune_single_test(
            combination,
            model,
            settings,
            config.num_iterations,
            config.prompt.clone(),
            log_tx.clone(),
        ).await;
        
        match result {
            Ok(test_result) => {
                results.push(test_result);
            }
            Err(e) => {
                // Log error but continue with other tests
                let _ = log_tx.send(format!("Benchmark test {}/{} failed: {}", idx + 1, total_tests, e)).await;
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
    
    // Final progress update
    progress_tx.send(BenchTuneStatus::Completed {
        total_tests,
        successful_tests,
        elapsed,
    }).await?;
    
    Ok(results)
}

/// Run a single benchmark tuning test with specific parameters
async fn run_bench_tune_single_test(
    params: &BenchTuneParamValue,
    model: &DiscoveredModel,
    base_settings: &ModelSettings,
    num_iterations: u32,
    prompt: String,
    log_tx: mpsc::Sender<String>,
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
    
// Spawn server with test parameters
    let config = crate::config::Config::default();
    let (server_handle, _command) = spawn_server(
        &config,
        Some(model),
        &settings,
        log_tx.clone(),
        None,
        ServerMode::Normal,
        1,
    ).await?;
    
    // Wait for server to be ready
    let mut ready = false;
    let host = if server_handle.host == "0.0.0.0" { "127.0.0.1" } else { &server_handle.host };
    
    let _ = log_tx.send(format!("Waiting for server on {}:{}...", host, server_handle.port)).await;
    
    // Increased timeout to 60s for large models
    for i in 0..120 {
        if crate::backend::server::check_health(host, server_handle.port).await {
            ready = true;
            break;
        }
        if i % 10 == 0 && i > 0 {
            let _ = log_tx.send(format!("  ... still waiting ({:.0}s)...", i as f32 * 0.5)).await;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    
    if !ready {
        let _ = log_tx.send("Error: Server health check timed out after 60s".to_string()).await;
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
    
    let _ = log_tx.send(format!("Running {} inference iterations...", num_iterations)).await;
    
    for i in 0..num_iterations {
        // Send prompt and measure response
        let result = send_inference_request(&prompt, host, server_handle.port).await;
        
        match result {
            Ok(res) => {
                total_prompt_tokens += res.prompt_tokens;
                total_generation_tokens += res.generation_tokens;
                total_prompt_time += res.prompt_time;
                total_generation_time += res.generation_time;
                total_time += res.total_time;
                first_token_times.push(res.first_token_time);
                
                if num_iterations > 1 {
                    let _ = log_tx.send(format!("  Iteration {}/{}: {:.2} gen t/s", i + 1, num_iterations, 
                        if res.generation_time.as_secs_f64() > 0.0 { res.generation_tokens as f64 / res.generation_time.as_secs_f64() } else { 0.0 }
                    )).await;
                }
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
    
    // Calculate metrics
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
    
    // Clean up server
    let _ = crate::backend::server::kill_server(server_handle).await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    Ok(BenchTuneResult {
        params: params.clone(),
        metrics: BenchTuneMetrics {
            prompt_tps,
            generation_tps,
            combined_tps,
            latency_per_token: avg_latency_per_token,
            first_token_time: avg_first_token_time,
        },
    })
}

/// Send an inference request and measure response time
async fn send_inference_request(prompt: &str, host: &str, port: u16) -> Result<InferenceResult, Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(120)) // High timeout for slow generation
        .build()?;
        
    let url = format!("http://{}:{}/completion", host, port);
    
    let body = serde_json::json!({
        "prompt": prompt,
        "n_predict": 128,
        "stream": false
    });
    
    let start = Instant::now();
    let resp = client.post(&url)
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
    })
}

/// Save benchmark results to disk in Markdown format
pub async fn save_results(results: &[BenchTuneResult], output_dir: &PathBuf) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Create output directory if it doesn't exist
    std::fs::create_dir_all(output_dir)?;

    // Generate timestamp for the filename
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let filename = format!("benchmark_{}.md", timestamp);
    let filepath = output_dir.join(filename);

    let mut md = String::new();
    md.push_str("# LLM Benchmark Results\n\n");
    md.push_str(&format!("Generated on: {}\n\n", chrono::Local::now().format("%Y-%m-%d %H:%M:%S")));

    md.push_str("| Temp | Top-P | Top-K | RepPen | FA | Threads | Batch | Prompt t/s | Gen t/s | Latency (ms) | First Tok (ms) |\n");
    md.push_str("|------|-------|-------|--------|----|---------|-------|------------|---------|--------------|----------------|\n");

    for r in results {
        let temp = r.params.temperature.map(|v| format!("{:.2}", v)).unwrap_or_else(|| "-".to_string());
        let top_p = r.params.top_p.map(|v| format!("{:.2}", v)).unwrap_or_else(|| "-".to_string());
        let top_k = r.params.top_k.map(|v| v.to_string()).unwrap_or_else(|| "-".to_string());
        let rep_pen = r.params.repeat_penalty.map(|v| format!("{:.2}", v)).unwrap_or_else(|| "-".to_string());
        let fa = r.params.flash_attn.map(|v| if v { "ON" } else { "OFF" }).unwrap_or("-");
        let threads = r.params.threads.map(|v| v.to_string()).unwrap_or_else(|| "-".to_string());
        let batch = r.params.batch_size.map(|v| v.to_string()).unwrap_or_else(|| "-".to_string());

        md.push_str(&format!("| {} | {} | {} | {} | {} | {} | {} | {:.2} | {:.2} | {:.2} | {:.2} |\n",
            temp, top_p, top_k, rep_pen, fa, threads, batch,
            r.metrics.prompt_tps,
            r.metrics.generation_tps,
            r.metrics.latency_per_token,
            r.metrics.first_token_time
        ));
    }
    tokio::fs::write(&filepath, md).await?;
    
    Ok(())
}

/// Result from a single inference request
struct InferenceResult {
    prompt_tokens: u64,
    generation_tokens: u64,
    prompt_time: Duration,
    generation_time: Duration,
    total_time: Duration,
    first_token_time: u128, // milliseconds
}
