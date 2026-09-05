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
    let config = BenchTuneConfig::new(PathBuf::from("/models/qwen.gguf"), 3, "test prompt".into());
    assert_eq!(config.model_path, PathBuf::from("/models/qwen.gguf"));
    assert_eq!(config.num_iterations, 3);
    assert_eq!(config.prompt, "test prompt");
}

#[test]
fn bench_tune_config_new_default_duration() {
    let config = BenchTuneConfig::new(PathBuf::new(), 1, "prompt".into());
    assert_eq!(config.test_duration, Duration::from_secs(30));
}

#[test]
fn bench_tune_config_new_n_predict() {
    let config = BenchTuneConfig::new(PathBuf::new(), 1, "prompt".into());
    assert_eq!(config.n_predict, 512);
}

#[test]
fn bench_tune_config_new_has_6_params() {
    let config = BenchTuneConfig::new(PathBuf::new(), 1, "prompt".into());
    assert_eq!(config.params_to_test.len(), 6);
}

#[test]
fn bench_tune_config_new_params_all_disabled_by_default() {
    let config = BenchTuneConfig::new(PathBuf::new(), 1, "prompt".into());
    for p in &config.params_to_test {
        assert!(!p.enabled, "param {} should be disabled by default", p.name);
    }
}

// ── Parameter definitions ──────────────────────────────────────

#[test]
fn bench_tune_config_has_flash_attn_param() {
    let config = BenchTuneConfig::new(PathBuf::new(), 1, "prompt".into());
    let fa = config
        .params_to_test
        .iter()
        .find(|p| p.name == "flash_attn")
        .unwrap();
    assert_eq!(fa.min, 0.0);
    assert_eq!(fa.max, 1.0);
    assert_eq!(fa.step, 1.0);
}

#[test]
fn bench_tune_config_has_threads_param() {
    let config = BenchTuneConfig::new(PathBuf::new(), 1, "prompt".into());
    let th = config
        .params_to_test
        .iter()
        .find(|p| p.name == "threads")
        .unwrap();
    assert_eq!(th.min, 4.0);
    assert_eq!(th.max, 16.0);
    assert_eq!(th.step, 4.0);
}

#[test]
fn bench_tune_config_has_batch_size_param() {
    let config = BenchTuneConfig::new(PathBuf::new(), 1, "prompt".into());
    let bs = config
        .params_to_test
        .iter()
        .find(|p| p.name == "batch_size")
        .unwrap();
    assert_eq!(bs.min, 512.0);
    assert_eq!(bs.max, 2048.0);
    assert_eq!(bs.step, 512.0);
}

#[test]
fn bench_tune_config_has_expert_count_param() {
    let config = BenchTuneConfig::new(PathBuf::new(), 1, "prompt".into());
    let ec = config
        .params_to_test
        .iter()
        .find(|p| p.name == "expert_count")
        .unwrap();
    assert_eq!(ec.min, -1.0);
    assert_eq!(ec.max, 4.0);
    assert_eq!(ec.step, 1.0);
}

// ── Combination generation ─────────────────────────────────────

#[test]
fn generate_combinations_all_disabled_returns_one() {
    let config = BenchTuneConfig::new(PathBuf::new(), 1, "prompt".into());
    // All params are disabled by default
    let combos = config.generate_combinations();
    assert_eq!(combos.len(), 1);
}

#[test]
fn generate_combinations_two_enabled_multiply() {
    let mut config = BenchTuneConfig::new(PathBuf::new(), 1, "prompt".into());
    // Enable threads (4 values) and batch_size (4 values) = 16 combinations
    if let Some(p) = config
        .params_to_test
        .iter_mut()
        .find(|p| p.name == "threads")
    {
        p.enabled = true;
    }
    if let Some(p) = config.params_to_test.iter_mut().find(|p| p.name == "batch_size") {
        p.enabled = true;
    }
    let combos = config.generate_combinations();
    assert_eq!(combos.len(), 16);
}

#[test]
fn generate_combinations_flash_attn_binary() {
    let mut config = BenchTuneConfig::new(PathBuf::new(), 1, "prompt".into());
    // Enable only flash_attn (0, 1 = 2 values)
    if let Some(p) = config
        .params_to_test
        .iter_mut()
        .find(|p| p.name == "flash_attn")
    {
        p.enabled = true;
    }
    let combos = config.generate_combinations();
    assert_eq!(combos.len(), 2);
}

#[test]
fn generate_combinations_threads_three_values() {
    let mut config = BenchTuneConfig::new(PathBuf::new(), 1, "prompt".into());
    // Enable only threads (4, 8, 12, 16 = 4 values)
    if let Some(p) = config
        .params_to_test
        .iter_mut()
        .find(|p| p.name == "threads")
    {
        p.enabled = true;
    }
    let combos = config.generate_combinations();
    assert_eq!(combos.len(), 4);
}

#[test]
fn generate_combinations_batch_size_four_values() {
    let mut config = BenchTuneConfig::new(PathBuf::new(), 1, "prompt".into());
    // Enable only batch_size (512, 1024, 1536, 2048 = 4 values)
    if let Some(p) = config
        .params_to_test
        .iter_mut()
        .find(|p| p.name == "batch_size")
    {
        p.enabled = true;
    }
    let combos = config.generate_combinations();
    assert_eq!(combos.len(), 4);
}

#[test]
fn generate_combinations_expert_count_six_values() {
    let mut config = BenchTuneConfig::new(PathBuf::new(), 1, "prompt".into());
    // Enable only expert_count (-1 to 4 step 1 = 6 values: -1, 0, 1, 2, 3, 4)
    if let Some(p) = config
        .params_to_test
        .iter_mut()
        .find(|p| p.name == "expert_count")
    {
        p.enabled = true;
    }
    let combos = config.generate_combinations();
    assert_eq!(combos.len(), 6);
}

// ── Total test count ──────────────────────────────────────────

#[test]
fn get_total_tests_count_matches_generate() {
    let config = BenchTuneConfig::new(PathBuf::new(), 1, "prompt".into());
    let count = config.get_total_tests_count();
    let combos = config.generate_combinations();
    assert_eq!(count, combos.len());
}

#[test]
fn get_total_tests_count_with_enabled_params() {
    let mut config = BenchTuneConfig::new(PathBuf::new(), 1, "prompt".into());
    if let Some(p) = config
        .params_to_test
        .iter_mut()
        .find(|p| p.name == "threads")
    {
        p.enabled = true;
    }
    if let Some(p) = config.params_to_test.iter_mut().find(|p| p.name == "batch_size") {
        p.enabled = true;
    }
    let count = config.get_total_tests_count();
    assert_eq!(count, 16);
}

// ── BenchTuneParamValue defaults ───────────────────────────────

#[test]
fn bench_tune_param_value_default_all_none() {
    let v = BenchTuneParamValue {
        context_length: None,
        batch_size: None,
        flash_attn: None,
        threads: None,
        expert_count: None,
        spec_type: None,
        draft_tokens: None,
    };
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
        context_length: Some(32768),
        batch_size: Some(512),
        flash_attn: Some(true),
        threads: Some(8),
        expert_count: Some(2),
        spec_type: Some("draft-mtp".to_string()),
        draft_tokens: Some(10),
    };
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
        prompt_processing_time: 150.0,
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
            prompt_processing_time: 100.0,
        },
        outputs: vec!["Hello world".into()],
        per_iteration_metrics: vec![],
        base_settings: None,
        server_command: None,
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

// ── BenchTuneConfig with custom settings ───────────────────────

#[test]
fn bench_tune_config_custom_iterations() {
    let config = BenchTuneConfig::new(PathBuf::new(), 10, "custom prompt".into());
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
    let mut config = BenchTuneConfig::new(PathBuf::new(), 1, "prompt".into());
    config.chat_template_kwargs = Some(r#"{"enable_thinking": true}"#.into());
    assert_eq!(
        config.chat_template_kwargs,
        Some(r#"{"enable_thinking": true}"#.into())
    );
}

// ── Combination values correctness ─────────────────────────────

#[test]
fn generate_combinations_threads_values_correct() {
    let mut config = BenchTuneConfig::new(PathBuf::new(), 1, "prompt".into());
    if let Some(p) = config
        .params_to_test
        .iter_mut()
        .find(|p| p.name == "threads")
    {
        p.enabled = true;
    }
    let combos = config.generate_combinations();
    let threads: Vec<u32> = combos.iter().filter_map(|c| c.threads).collect();
    assert_eq!(threads, vec![4, 8, 12, 16]);
}

#[test]
fn generate_combinations_batch_size_values_correct() {
    let mut config = BenchTuneConfig::new(PathBuf::new(), 1, "prompt".into());
    if let Some(p) = config
        .params_to_test
        .iter_mut()
        .find(|p| p.name == "batch_size")
    {
        p.enabled = true;
    }
    let combos = config.generate_combinations();
    let batches: Vec<u32> = combos.iter().filter_map(|c| c.batch_size).collect();
    assert_eq!(batches, vec![512, 1024, 1536, 2048]);
}

#[test]
fn generate_combinations_flash_attn_values_correct() {
    let mut config = BenchTuneConfig::new(PathBuf::new(), 1, "prompt".into());
    if let Some(p) = config
        .params_to_test
        .iter_mut()
        .find(|p| p.name == "flash_attn")
    {
        p.enabled = true;
    }
    let combos = config.generate_combinations();
    let fas: Vec<bool> = combos.iter().filter_map(|c| c.flash_attn).collect();
    assert_eq!(fas, vec![false, true]);
}

#[test]
fn generate_combinations_expert_count_values_correct() {
    let mut config = BenchTuneConfig::new(PathBuf::new(), 1, "prompt".into());
    if let Some(p) = config
        .params_to_test
        .iter_mut()
        .find(|p| p.name == "expert_count")
    {
        p.enabled = true;
    }
    let combos = config.generate_combinations();
    let exps: Vec<i32> = combos.iter().filter_map(|c| c.expert_count).collect();
    assert_eq!(exps, vec![-1, 0, 1, 2, 3, 4]);
}

// ── Complex combination test ───────────────────────────────────

#[test]
fn generate_combinations_multiple_enabled_product() {
    let mut config = BenchTuneConfig::new(PathBuf::new(), 1, "prompt".into());
    // Enable threads (4) * batch_size (4) * flash_attn (2) = 32
    if let Some(p) = config
        .params_to_test
        .iter_mut()
        .find(|p| p.name == "threads")
    {
        p.enabled = true;
    }
    if let Some(p) = config.params_to_test.iter_mut().find(|p| p.name == "batch_size") {
        p.enabled = true;
    }
    if let Some(p) = config
        .params_to_test
        .iter_mut()
        .find(|p| p.name == "flash_attn")
    {
        p.enabled = true;
    }
    let combos = config.generate_combinations();
    assert_eq!(combos.len(), 32);
}

#[test]
fn generate_combinations_all_enabled_large_product() {
    let mut config = BenchTuneConfig::new(PathBuf::new(), 1, "prompt".into());
    // Enable all 6 params:
    // flash_attn: 2, threads: 4, batch_size: 4, expert_count: 6 (-1 to 4 step 1)
    // spec_type: 9, draft_tokens: 9
    // Total: 2 * 4 * 4 * 6 * 9 * 9 = 15552
    for p in &mut config.params_to_test {
        p.enabled = true;
    }
    let combos = config.generate_combinations();
    assert_eq!(combos.len(), 15552);
}

// ── BenchTuneProgress from_status ──────────────────────────────

#[test]
fn bench_tune_progress_from_running_has_values() {
    let status = BenchTuneStatus::Running {
        current: 5,
        total: 10,
        progress: 50.0,
        current_params: BenchTuneParamValue {
            context_length: None,
            batch_size: None,
            flash_attn: None,
            threads: Some(8),
            expert_count: None,
            spec_type: None,
            draft_tokens: None,
        },
    };
    if let Some(BenchTuneProgress::Running {
        current,
        total,
        progress,
        current_params,
    }) = BenchTuneProgress::from_status(&status)
    {
        assert_eq!(current, 5);
        assert_eq!(total, 10);
        assert!((progress - 50.0).abs() < f32::EPSILON);
        assert_eq!(current_params.threads, Some(8));
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
    if let Some(BenchTuneProgress::Completed {
        total_tests,
        successful_tests,
        elapsed,
    }) = BenchTuneProgress::from_status(&status)
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
