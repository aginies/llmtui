//! Tests for benchmark tuning configuration and parameter generation.
//!
//! Tests cover: BenchTuneConfig creation, parameter combination generation,
//  total test counts, and default values.

use llm_manager::models::*;
use std::path::PathBuf;
use std::time::Duration;

// ── BenchTuneConfig creation ───────────────────────────────────

#[test]
fn bench_tune_config_new_basic() {
    let config = BenchTuneConfig::new(
        PathBuf::from("/models/qwen.gguf"),
        3,
        "test prompt".into(),
    );
    assert_eq!(config.model_path, PathBuf::from("/models/qwen.gguf"));
    assert_eq!(config.num_iterations, 3);
    assert_eq!(config.prompt, "test prompt");
}

#[test]
fn bench_tune_config_new_default_duration() {
    let config = BenchTuneConfig::new(
        PathBuf::new(),
        1,
        "prompt".into(),
    );
    assert_eq!(config.test_duration, Duration::from_secs(30));
}

#[test]
fn bench_tune_config_new_default_mode() {
    let config = BenchTuneConfig::new(
        PathBuf::new(),
        1,
        "prompt".into(),
    );
    assert_eq!(config.bench_mode, BenchTuneMode::Full);
}

#[test]
fn bench_tune_config_new_n_predict() {
    let config = BenchTuneConfig::new(
        PathBuf::new(),
        1,
        "prompt".into(),
    );
    assert_eq!(config.n_predict, 512);
}

#[test]
fn bench_tune_config_new_has_8_params() {
    let config = BenchTuneConfig::new(
        PathBuf::new(),
        1,
        "prompt".into(),
    );
    assert_eq!(config.params_to_test.len(), 8);
}

#[test]
fn bench_tune_config_new_params_all_disabled_by_default() {
    let config = BenchTuneConfig::new(
        PathBuf::new(),
        1,
        "prompt".into(),
    );
    for p in &config.params_to_test {
        assert!(!p.enabled, "param {} should be disabled by default", p.name);
    }
}

// ── Parameter definitions ──────────────────────────────────────

#[test]
fn bench_tune_config_has_temperature_param() {
    let config = BenchTuneConfig::new(
        PathBuf::new(),
        1,
        "prompt".into(),
    );
    let temp = config.params_to_test.iter().find(|p| p.name == "temperature").unwrap();
    assert_eq!(temp.min, 0.4);
    assert_eq!(temp.max, 1.0);
    assert_eq!(temp.step, 0.1);
}

#[test]
fn bench_tune_config_has_top_p_param() {
    let config = BenchTuneConfig::new(
        PathBuf::new(),
        1,
        "prompt".into(),
    );
    let tp = config.params_to_test.iter().find(|p| p.name == "top_p").unwrap();
    assert_eq!(tp.min, 0.8);
    assert_eq!(tp.max, 1.0);
    assert_eq!(tp.step, 0.1);
}

#[test]
fn bench_tune_config_has_top_k_param() {
    let config = BenchTuneConfig::new(
        PathBuf::new(),
        1,
        "prompt".into(),
    );
    let tk = config.params_to_test.iter().find(|p| p.name == "top_k").unwrap();
    assert_eq!(tk.min, 10.0);
    assert_eq!(tk.max, 40.0);
    assert_eq!(tk.step, 5.0);
}

#[test]
fn bench_tune_config_has_repeat_penalty_param() {
    let config = BenchTuneConfig::new(
        PathBuf::new(),
        1,
        "prompt".into(),
    );
    let rp = config.params_to_test.iter().find(|p| p.name == "repeat_penalty").unwrap();
    assert_eq!(rp.min, 1.0);
    assert_eq!(rp.max, 1.2);
    assert_eq!(rp.step, 0.1);
}

#[test]
fn bench_tune_config_has_flash_attn_param() {
    let config = BenchTuneConfig::new(
        PathBuf::new(),
        1,
        "prompt".into(),
    );
    let fa = config.params_to_test.iter().find(|p| p.name == "flash_attn").unwrap();
    assert_eq!(fa.min, 0.0);
    assert_eq!(fa.max, 1.0);
    assert_eq!(fa.step, 1.0);
}

#[test]
fn bench_tune_config_has_threads_param() {
    let config = BenchTuneConfig::new(
        PathBuf::new(),
        1,
        "prompt".into(),
    );
    let th = config.params_to_test.iter().find(|p| p.name == "threads").unwrap();
    assert_eq!(th.min, 4.0);
    assert_eq!(th.max, 16.0);
    assert_eq!(th.step, 4.0);
}

#[test]
fn bench_tune_config_has_batch_size_param() {
    let config = BenchTuneConfig::new(
        PathBuf::new(),
        1,
        "prompt".into(),
    );
    let bs = config.params_to_test.iter().find(|p| p.name == "batch_size").unwrap();
    assert_eq!(bs.min, 512.0);
    assert_eq!(bs.max, 2048.0);
    assert_eq!(bs.step, 512.0);
}

#[test]
fn bench_tune_config_has_expert_count_param() {
    let config = BenchTuneConfig::new(
        PathBuf::new(),
        1,
        "prompt".into(),
    );
    let ec = config.params_to_test.iter().find(|p| p.name == "expert_count").unwrap();
    assert_eq!(ec.min, 1.0);
    assert_eq!(ec.max, 4.0);
    assert_eq!(ec.step, 1.0);
}

// ── Combination generation ─────────────────────────────────────

#[test]
fn generate_combinations_all_disabled_returns_one() {
    let config = BenchTuneConfig::new(
        PathBuf::new(),
        1,
        "prompt".into(),
    );
    // All params are disabled by default
    let combos = config.generate_combinations();
    assert_eq!(combos.len(), 1);
}

#[test]
fn generate_combinations_one_enabled_temperature() {
    let mut config = BenchTuneConfig::new(
        PathBuf::new(),
        1,
        "prompt".into(),
    );
    // Enable only temperature (0.4, 0.5, ..., 1.0 = 7 values)
    if let Some(p) = config.params_to_test.iter_mut().find(|p| p.name == "temperature") {
        p.enabled = true;
    }
    let combos = config.generate_combinations();
    assert_eq!(combos.len(), 7);
}

#[test]
fn generate_combinations_one_enabled_top_p() {
    let mut config = BenchTuneConfig::new(
        PathBuf::new(),
        1,
        "prompt".into(),
    );
    // Enable only top_p (0.8, 0.9, 1.0 = 3 values)
    if let Some(p) = config.params_to_test.iter_mut().find(|p| p.name == "top_p") {
        p.enabled = true;
    }
    let combos = config.generate_combinations();
    assert_eq!(combos.len(), 3);
}

#[test]
fn generate_combinations_one_enabled_top_k() {
    let mut config = BenchTuneConfig::new(
        PathBuf::new(),
        1,
        "prompt".into(),
    );
    // Enable only top_k (10, 15, 20, 25, 30, 35, 40 = 7 values)
    if let Some(p) = config.params_to_test.iter_mut().find(|p| p.name == "top_k") {
        p.enabled = true;
    }
    let combos = config.generate_combinations();
    assert_eq!(combos.len(), 7);
}

#[test]
fn generate_combinations_two_enabled_multiply() {
    let mut config = BenchTuneConfig::new(
        PathBuf::new(),
        1,
        "prompt".into(),
    );
    // Enable temperature (7 values) and top_p (3 values) = 21 combinations
    if let Some(p) = config.params_to_test.iter_mut().find(|p| p.name == "temperature") {
        p.enabled = true;
    }
    if let Some(p) = config.params_to_test.iter_mut().find(|p| p.name == "top_p") {
        p.enabled = true;
    }
    let combos = config.generate_combinations();
    assert_eq!(combos.len(), 21);
}

#[test]
fn generate_combinations_flash_attn_binary() {
    let mut config = BenchTuneConfig::new(
        PathBuf::new(),
        1,
        "prompt".into(),
    );
    // Enable only flash_attn (0, 1 = 2 values)
    if let Some(p) = config.params_to_test.iter_mut().find(|p| p.name == "flash_attn") {
        p.enabled = true;
    }
    let combos = config.generate_combinations();
    assert_eq!(combos.len(), 2);
}

#[test]
fn generate_combinations_threads_three_values() {
    let mut config = BenchTuneConfig::new(
        PathBuf::new(),
        1,
        "prompt".into(),
    );
    // Enable only threads (4, 8, 12, 16 = 4 values)
    if let Some(p) = config.params_to_test.iter_mut().find(|p| p.name == "threads") {
        p.enabled = true;
    }
    let combos = config.generate_combinations();
    assert_eq!(combos.len(), 4);
}

#[test]
fn generate_combinations_batch_size_four_values() {
    let mut config = BenchTuneConfig::new(
        PathBuf::new(),
        1,
        "prompt".into(),
    );
    // Enable only batch_size (512, 1024, 1536, 2048 = 4 values)
    if let Some(p) = config.params_to_test.iter_mut().find(|p| p.name == "batch_size") {
        p.enabled = true;
    }
    let combos = config.generate_combinations();
    assert_eq!(combos.len(), 4);
}

#[test]
fn generate_combinations_expert_count_four_values() {
    let mut config = BenchTuneConfig::new(
        PathBuf::new(),
        1,
        "prompt".into(),
    );
    // Enable only expert_count (1, 2, 3, 4 = 4 values)
    if let Some(p) = config.params_to_test.iter_mut().find(|p| p.name == "expert_count") {
        p.enabled = true;
    }
    let combos = config.generate_combinations();
    assert_eq!(combos.len(), 4);
}

#[test]
fn generate_combinations_repeat_penalty_three_values() {
    let mut config = BenchTuneConfig::new(
        PathBuf::new(),
        1,
        "prompt".into(),
    );
    // Enable only repeat_penalty (1.0, 1.1, 1.2 = 3 values)
    if let Some(p) = config.params_to_test.iter_mut().find(|p| p.name == "repeat_penalty") {
        p.enabled = true;
    }
    let combos = config.generate_combinations();
    assert_eq!(combos.len(), 3);
}

// ── Total test count ──────────────────────────────────────────

#[test]
fn get_total_tests_count_matches_generate() {
    let config = BenchTuneConfig::new(
        PathBuf::new(),
        1,
        "prompt".into(),
    );
    let count = config.get_total_tests_count();
    let combos = config.generate_combinations();
    assert_eq!(count, combos.len());
}

#[test]
fn get_total_tests_count_with_enabled_params() {
    let mut config = BenchTuneConfig::new(
        PathBuf::new(),
        1,
        "prompt".into(),
    );
    if let Some(p) = config.params_to_test.iter_mut().find(|p| p.name == "temperature") {
        p.enabled = true;
    }
    if let Some(p) = config.params_to_test.iter_mut().find(|p| p.name == "top_p") {
        p.enabled = true;
    }
    let count = config.get_total_tests_count();
    assert_eq!(count, 21);
}

// ── BenchTuneParamValue defaults ───────────────────────────────

#[test]
fn bench_tune_param_value_default_all_none() {
    let v = BenchTuneParamValue {
        temperature: None,
        top_p: None,
        top_k: None,
        repeat_penalty: None,
        context_length: None,
        batch_size: None,
        flash_attn: None,
        threads: None,
        expert_count: None,
        spec_type: None,
        draft_tokens: None,
    };
    assert!(v.temperature.is_none());
    assert!(v.top_p.is_none());
    assert!(v.top_k.is_none());
    assert!(v.repeat_penalty.is_none());
    assert!(v.context_length.is_none());
    assert!(v.batch_size.is_none());
    assert!(v.flash_attn.is_none());
    assert!(v.threads.is_none());
    assert!(v.expert_count.is_none());
    assert!(v.spec_type.is_none());
    assert!(v.draft_tokens.is_none());
}

// ── BenchTuneParamValue with values ────────────────────────────

#[test]
fn bench_tune_param_value_with_all_values() {
    let v = BenchTuneParamValue {
        temperature: Some(0.7),
        top_p: Some(0.9),
        top_k: Some(40),
        repeat_penalty: Some(1.1),
        context_length: Some(32768),
        batch_size: Some(512),
        flash_attn: Some(true),
        threads: Some(8),
        expert_count: Some(2),
        spec_type: Some("draft-mtp".to_string()),
        draft_tokens: Some(10),
    };
    assert_eq!(v.temperature, Some(0.7));
    assert_eq!(v.top_p, Some(0.9));
    assert_eq!(v.top_k, Some(40));
    assert_eq!(v.repeat_penalty, Some(1.1));
    assert_eq!(v.context_length, Some(32768));
    assert_eq!(v.batch_size, Some(512));
    assert_eq!(v.flash_attn, Some(true));
    assert_eq!(v.threads, Some(8));
    assert_eq!(v.expert_count, Some(2));
    assert_eq!(v.spec_type, Some("draft-mtp".to_string()));
    assert_eq!(v.draft_tokens, Some(10));
}

// ── BenchTuneMetrics serialization ─────────────────────────────

#[test]
fn bench_tune_metrics_serializable() {
    let m = BenchTuneMetrics {
        prompt_tps: 50.5,
        generation_tps: 25.3,
        combined_tps: 30.0,
        latency_per_token: 39.5,
        first_token_time: 150.0,
    };
    let json = serde_json::to_string(&m).expect("should serialize");
    assert!(json.contains("50.5"));
    assert!(json.contains("25.3"));
    let deserialized: BenchTuneMetrics = serde_json::from_str(&json).expect("should deserialize");
    assert!((deserialized.prompt_tps - 50.5).abs() < f64::EPSILON);
    assert!((deserialized.generation_tps - 25.3).abs() < f64::EPSILON);
}

// ── BenchTuneResult serialization ──────────────────────────────

#[test]
fn bench_tune_result_serializable() {
    let r = BenchTuneResult {
        params: BenchTuneParamValue {
            temperature: Some(0.7),
            top_p: Some(0.9),
            top_k: Some(40),
            repeat_penalty: Some(1.1),
            context_length: None,
            batch_size: None,
            flash_attn: Some(true),
            threads: Some(8),
            expert_count: None,
            spec_type: None,
            draft_tokens: None,
        },
        metrics: BenchTuneMetrics {
            prompt_tps: 50.0,
            generation_tps: 25.0,
            combined_tps: 30.0,
            latency_per_token: 40.0,
            first_token_time: 100.0,
        },
        outputs: vec!["Hello world".into()],
        per_iteration_metrics: vec![],
        base_settings: None,
    };
    let json = serde_json::to_string(&r).expect("should serialize");
    assert!(json.contains("Hello world"));
}

// ── BenchTuneStatus variants ───────────────────────────────────

#[test]
fn bench_tune_status_running_serializable() {
    let status = BenchTuneStatus::Running {
        current: 1,
        total: 10,
        progress: 10.0,
        current_params: BenchTuneParamValue {
            temperature: None,
            top_p: None,
            top_k: None,
            repeat_penalty: None,
            context_length: None,
            batch_size: None,
            flash_attn: None,
            threads: None,
            expert_count: None,
            spec_type: None,
            draft_tokens: None,
        },
    };
    let json = serde_json::to_string(&status).expect("should serialize");
    assert!(json.contains("Running"));
}

#[test]
fn bench_tune_status_completed_serializable() {
    let status = BenchTuneStatus::Completed {
        total_tests: 10,
        successful_tests: 9,
        elapsed: Duration::from_secs(60),
    };
    let json = serde_json::to_string(&status).expect("should serialize");
    assert!(json.contains("Completed"));
}

#[test]
fn bench_tune_status_error_serializable() {
    let status = BenchTuneStatus::Error {
        error: "test error".into(),
    };
    let json = serde_json::to_string(&status).expect("should serialize");
    assert!(json.contains("Error"));
    assert!(json.contains("test error"));
}

// ── BenchTuneMode variants ─────────────────────────────────────

#[test]
fn bench_tune_mode_runtime_only_serializable() {
    let mode = BenchTuneMode::RuntimeOnly;
    let json = serde_json::to_string(&mode).expect("should serialize");
    assert!(json.contains("RuntimeOnly"));
}

#[test]
fn bench_tune_mode_full_serializable() {
    let mode = BenchTuneMode::Full;
    let json = serde_json::to_string(&mode).expect("should serialize");
    assert!(json.contains("Full"));
}

// ── BenchTuneConfig with custom settings ───────────────────────

#[test]
fn bench_tune_config_custom_iterations() {
    let config = BenchTuneConfig::new(
        PathBuf::new(),
        10,
        "custom prompt".into(),
    );
    assert_eq!(config.num_iterations, 10);
}

#[test]
fn bench_tune_config_custom_prompt() {
    let config = BenchTuneConfig::new(
        PathBuf::new(),
        1,
        "A very long custom prompt for benchmarking.".into(),
    );
    assert_eq!(config.prompt, "A very long custom prompt for benchmarking.");
}

#[test]
fn bench_tune_config_custom_kwargs() {
    let mut config = BenchTuneConfig::new(
        PathBuf::new(),
        1,
        "prompt".into(),
    );
    config.chat_template_kwargs = Some(r#"{"enable_thinking": true}"#.into());
    assert_eq!(
        config.chat_template_kwargs,
        Some(r#"{"enable_thinking": true}"#.into())
    );
}

// ── Combination values correctness ─────────────────────────────

#[test]
fn generate_combinations_temperature_values_correct() {
    let mut config = BenchTuneConfig::new(
        PathBuf::new(),
        1,
        "prompt".into(),
    );
    if let Some(p) = config.params_to_test.iter_mut().find(|p| p.name == "temperature") {
        p.enabled = true;
    }
    let combos = config.generate_combinations();
    let temps: Vec<f64> = combos.iter()
        .filter_map(|c| c.temperature)
        .collect();
    assert!((temps[0] - 0.4).abs() < f64::EPSILON);
    assert!((temps[1] - 0.5).abs() < f64::EPSILON);
    assert!((temps[2] - 0.6).abs() < f64::EPSILON);
    assert!((temps[3] - 0.7).abs() < f64::EPSILON);
    assert!((temps[4] - 0.8).abs() < f64::EPSILON);
    assert!((temps[5] - 0.9).abs() < f64::EPSILON);
    assert!((temps[6] - 1.0).abs() < f64::EPSILON);
}

#[test]
fn generate_combinations_top_p_values_correct() {
    let mut config = BenchTuneConfig::new(
        PathBuf::new(),
        1,
        "prompt".into(),
    );
    if let Some(p) = config.params_to_test.iter_mut().find(|p| p.name == "top_p") {
        p.enabled = true;
    }
    let combos = config.generate_combinations();
    let tops: Vec<f64> = combos.iter()
        .filter_map(|c| c.top_p)
        .collect();
    assert_eq!(tops, vec![0.8, 0.9, 1.0]);
}

#[test]
fn generate_combinations_threads_values_correct() {
    let mut config = BenchTuneConfig::new(
        PathBuf::new(),
        1,
        "prompt".into(),
    );
    if let Some(p) = config.params_to_test.iter_mut().find(|p| p.name == "threads") {
        p.enabled = true;
    }
    let combos = config.generate_combinations();
    let threads: Vec<u32> = combos.iter()
        .filter_map(|c| c.threads)
        .collect();
    assert_eq!(threads, vec![4, 8, 12, 16]);
}

#[test]
fn generate_combinations_batch_size_values_correct() {
    let mut config = BenchTuneConfig::new(
        PathBuf::new(),
        1,
        "prompt".into(),
    );
    if let Some(p) = config.params_to_test.iter_mut().find(|p| p.name == "batch_size") {
        p.enabled = true;
    }
    let combos = config.generate_combinations();
    let batches: Vec<u32> = combos.iter()
        .filter_map(|c| c.batch_size)
        .collect();
    assert_eq!(batches, vec![512, 1024, 1536, 2048]);
}

#[test]
fn generate_combinations_flash_attn_values_correct() {
    let mut config = BenchTuneConfig::new(
        PathBuf::new(),
        1,
        "prompt".into(),
    );
    if let Some(p) = config.params_to_test.iter_mut().find(|p| p.name == "flash_attn") {
        p.enabled = true;
    }
    let combos = config.generate_combinations();
    let fas: Vec<bool> = combos.iter()
        .filter_map(|c| c.flash_attn)
        .collect();
    assert_eq!(fas, vec![false, true]);
}

#[test]
fn generate_combinations_expert_count_values_correct() {
    let mut config = BenchTuneConfig::new(
        PathBuf::new(),
        1,
        "prompt".into(),
    );
    if let Some(p) = config.params_to_test.iter_mut().find(|p| p.name == "expert_count") {
        p.enabled = true;
    }
    let combos = config.generate_combinations();
    let exps: Vec<i32> = combos.iter()
        .filter_map(|c| c.expert_count)
        .collect();
    assert_eq!(exps, vec![1, 2, 3, 4]);
}

#[test]
fn generate_combinations_repeat_penalty_values_correct() {
    let mut config = BenchTuneConfig::new(
        PathBuf::new(),
        1,
        "prompt".into(),
    );
    if let Some(p) = config.params_to_test.iter_mut().find(|p| p.name == "repeat_penalty") {
        p.enabled = true;
    }
    let combos = config.generate_combinations();
    let rps: Vec<f64> = combos.iter()
        .filter_map(|c| c.repeat_penalty)
        .collect();
    assert!((rps[0] - 1.0).abs() < f64::EPSILON);
    assert!((rps[1] - 1.1).abs() < f64::EPSILON);
    assert!((rps[2] - 1.2).abs() < f64::EPSILON);
}

// ── Complex combination test ───────────────────────────────────

#[test]
fn generate_combinations_multiple_enabled_product() {
    let mut config = BenchTuneConfig::new(
        PathBuf::new(),
        1,
        "prompt".into(),
    );
    // Enable temperature (7) * top_p (3) = 21
    if let Some(p) = config.params_to_test.iter_mut().find(|p| p.name == "temperature") {
        p.enabled = true;
    }
    if let Some(p) = config.params_to_test.iter_mut().find(|p| p.name == "top_p") {
        p.enabled = true;
    }
    // Enable flash_attn (2) → 21 * 2 = 42
    if let Some(p) = config.params_to_test.iter_mut().find(|p| p.name == "flash_attn") {
        p.enabled = true;
    }
    let combos = config.generate_combinations();
    assert_eq!(combos.len(), 42);
}

#[test]
fn generate_combinations_all_enabled_large_product() {
    let mut config = BenchTuneConfig::new(
        PathBuf::new(),
        1,
        "prompt".into(),
    );
    // Enable all 8 params:
    // temperature: 7, top_p: 3, top_k: 7, repeat_penalty: 3
    // flash_attn: 2, threads: 4, batch_size: 4, expert_count: 4
    // Total: 7 * 3 * 7 * 3 * 2 * 4 * 4 * 4 = 56448
    for p in &mut config.params_to_test {
        p.enabled = true;
    }
    let combos = config.generate_combinations();
    assert_eq!(combos.len(), 56448);
}

// ── BenchTuneProgress from_status ──────────────────────────────

#[test]
fn bench_tune_progress_from_running_has_values() {
    let status = BenchTuneStatus::Running {
        current: 5,
        total: 10,
        progress: 50.0,
        current_params: BenchTuneParamValue {
            temperature: Some(0.7),
            top_p: None,
            top_k: None,
            repeat_penalty: None,
            context_length: None,
            batch_size: None,
            flash_attn: None,
            threads: None,
            expert_count: None,
            spec_type: None,
            draft_tokens: None,
        },
    };
    if let Some(BenchTuneProgress::Running { current, total, progress, current_params }) =
        BenchTuneProgress::from_status(&status)
    {
        assert_eq!(current, 5);
        assert_eq!(total, 10);
        assert!((progress - 50.0).abs() < f32::EPSILON);
        assert_eq!(current_params.temperature, Some(0.7));
    } else {
        panic!("expected Running variant");
    }
}

#[test]
fn bench_tune_progress_from_completed_has_values() {
    let status = BenchTuneStatus::Completed {
        total_tests: 10,
        successful_tests: 9,
        elapsed: Duration::from_secs(120),
    };
    if let Some(BenchTuneProgress::Completed { total_tests, successful_tests, elapsed }) =
        BenchTuneProgress::from_status(&status)
    {
        assert_eq!(total_tests, 10);
        assert_eq!(successful_tests, 9);
        assert_eq!(elapsed, Duration::from_secs(120));
    } else {
        panic!("expected Completed variant");
    }
}

#[test]
fn bench_tune_progress_from_error_has_message() {
    let status = BenchTuneStatus::Error {
        error: "connection refused".into(),
    };
    if let Some(BenchTuneProgress::Error { error }) = BenchTuneProgress::from_status(&status) {
        assert_eq!(error, "connection refused");
    } else {
        panic!("expected Error variant");
    }
}
