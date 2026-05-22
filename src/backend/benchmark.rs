use std::path::PathBuf;
use std::time::{Duration, Instant};

use tokio::sync::mpsc;

use crate::backend::server::spawn_server;
use crate::models::{BenchTuneConfig, BenchTuneMetrics, BenchTuneParamValue, BenchTuneResult, BenchTuneStatus, ModelSettings, ServerMode};

/// Run a benchmark tuning test with multiple parameter combinations
pub async fn run_bench_tune(
    config: &BenchTuneConfig,
    model_path: &PathBuf,
    settings: &ModelSettings,
    progress_tx: mpsc::Sender<BenchTuneStatus>,
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
            model_path,
            settings,
            config.num_iterations,
            config.prompt.clone(),
        ).await;
        
        match result {
            Ok(test_result) => {
                results.push(test_result);
            }
            Err(e) => {
                // Log error but continue with other tests
                eprintln!("Benchmark tune test {} failed: {}", idx + 1, e);
            }
        }
    }
    
    // Sort results by tokens_per_sec (descending)
    results.sort_by(|a, b| {
        b.metrics.tokens_per_sec.partial_cmp(&a.metrics.tokens_per_sec)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    
    let elapsed = start_time.elapsed();
    println!("Benchmark tuning completed in {:.2}s", elapsed.as_secs_f64());
    
    Ok(results)
}

/// Run a single benchmark tuning test with specific parameters
async fn run_bench_tune_single_test(
    params: &BenchTuneParamValue,
    _model_path: &PathBuf,
    base_settings: &ModelSettings,
    num_iterations: u32,
    prompt: String,
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
    
// Spawn server with test parameters
    let config = crate::config::Config::default();
    let (log_tx, _) = tokio::sync::mpsc::channel(100);
    let (server_handle, _command) = spawn_server(
        &config,
        None,
        &settings,
        log_tx,
        None,
        ServerMode::Normal,
        1,
    ).await?;
    
    // Wait for server to be ready
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    // Run inference tests
    let mut total_tokens = 0u64;
    let mut total_time = Duration::ZERO;
    let mut first_token_times = Vec::new();
    
    for _ in 0..num_iterations {
        // Send prompt and measure response
        let result = send_inference_request(&prompt).await?;
        
        total_tokens += result.tokens_generated;
        total_time += result.total_time;
        first_token_times.push(result.first_token_time);
    }
    
    // Calculate metrics
    let avg_tokens_per_sec = if total_time.as_millis() > 0 {
        (total_tokens as f64) / (total_time.as_secs_f64())
    } else {
        0.0
    };
    
    let avg_latency_per_token = if total_tokens > 0 {
        total_time.as_millis() as f64 / (total_tokens as f64)
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
            tokens_per_sec: avg_tokens_per_sec,
            latency_per_token: avg_latency_per_token,
            first_token_time: avg_first_token_time,
        },
    })
}

/// Send an inference request and measure response time
async fn send_inference_request(_prompt: &str) -> Result<InferenceResult, Box<dyn std::error::Error + Send + Sync>> {
    // This will be implemented to interact with the running server
    // For now, return a placeholder
    let start = Instant::now();
    let first_token_time = 50u128; // Placeholder
    
    // Simulate inference
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    let total_time = start.elapsed();
    
    Ok(InferenceResult {
        tokens_generated: 100,
        total_time,
        first_token_time,
    })
}

/// Save benchmark results to disk
pub async fn save_results(results: &[BenchTuneResult], output_dir: &PathBuf) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Create output directory if it doesn't exist
    std::fs::create_dir_all(output_dir)?;
    
    // Generate timestamp for the filename
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let filename = format!("benchmark_{}.json", timestamp);
    let filepath = output_dir.join(filename);
    
    // Save results to JSON file
    let json = serde_json::to_string_pretty(results)?;
    std::fs::write(&filepath, json)?;
    
    println!("Benchmark results saved to: {}", filepath.display());
    Ok(())
}

/// Result from a single inference request
struct InferenceResult {
    tokens_generated: u64,
    total_time: Duration,
    first_token_time: u128, // milliseconds
}
