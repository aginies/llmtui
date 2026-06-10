//! Tests for tui/app/sync_ops.rs — model_is_downloaded.
//!
//! Tests cover: name matching with separators, case insensitivity, GGUF extension handling,
//! partial prefix rejection, empty model lists, and repo name suffix handling.

use llm_manager::models::DiscoveredModel;
use llm_manager::tui::app::sync_ops::model_is_downloaded;
use std::path::PathBuf;

fn make_discovered(name: &str) -> DiscoveredModel {
    DiscoveredModel {
        path: PathBuf::from(format!("/models/{}", name)),
        name: name.to_string(),
        file_size: 0,
        display_name: name.to_string(),
        pipeline_tag: None,
        capabilities: vec![],
    }
}

#[test]
fn exact_match() {
    let models = vec![make_discovered("Qwen2.5-7B-Instruct.gguf")];
    assert!(model_is_downloaded(&models, "Qwen/Qwen2.5-7B-Instruct"));
}

#[test]
fn hyphen_separator_match() {
    let models = vec![make_discovered("Qwen2.5-7B-Instruct-Q4_K_M.gguf")];
    assert!(model_is_downloaded(&models, "Qwen/Qwen2.5-7B-Instruct"));
}

#[test]
fn underscore_separator_match() {
    let models = vec![make_discovered("Qwen2.5_7B_Instruct.gguf")];
    assert!(model_is_downloaded(&models, "Qwen/Qwen2.5-7B-Instruct"));
}

#[test]
fn underscore_separator_match_with_quant() {
    let models = vec![make_discovered("Qwen2.5_7B_Instruct-Q4_K_M.gguf")];
    assert!(model_is_downloaded(&models, "Qwen/Qwen2.5-7B-Instruct"));
}

#[test]
fn case_insensitive_match() {
    let models = vec![make_discovered("qwen2.5-7b-instruct-q4_k_m.gguf")];
    assert!(model_is_downloaded(&models, "Qwen/Qwen2.5-7B-Instruct"));
}

#[test]
fn no_match_different_model() {
    let models = vec![make_discovered("Llama-3.1-8B-Instruct-Q4_K_M.gguf")];
    assert!(!model_is_downloaded(&models, "Qwen/Qwen2.5-7B-Instruct"));
}

#[test]
fn no_match_partial_prefix_false_positive() {
    let models = vec![make_discovered("Qwen2.5-Mistral-Q4_K_M.gguf")];
    assert!(!model_is_downloaded(&models, "Qwen/Qwen2.5-7B-Instruct"));
}

#[test]
fn no_match_empty_models() {
    let models: Vec<DiscoveredModel> = vec![];
    assert!(!model_is_downloaded(&models, "Qwen/Qwen2.5-7B-Instruct"));
}

#[test]
fn strip_gguf_extension() {
    let models = vec![make_discovered("Qwen2.5-7B-Instruct-Q4_K_M.GGUF")];
    assert!(model_is_downloaded(&models, "Qwen/Qwen2.5-7B-Instruct"));
}

#[test]
fn no_gguf_extension() {
    let models = vec![make_discovered("Qwen2.5-7B-Instruct-Q4_K_M")];
    assert!(model_is_downloaded(&models, "Qwen/Qwen2.5-7B-Instruct"));
}

#[test]
fn multiple_models() {
    let models = vec![
        make_discovered("Llama-3.1-8B-Instruct-Q4_K_M.gguf"),
        make_discovered("Qwen2.5-7B-Instruct-Q4_K_M.gguf"),
        make_discovered("Mistral-7B-v0.3-Q8_0.gguf"),
    ];
    assert!(model_is_downloaded(&models, "Qwen/Qwen2.5-7B-Instruct"));
    assert!(model_is_downloaded(
        &models,
        "meta-llama/Llama-3.1-8B-Instruct"
    ));
    assert!(model_is_downloaded(&models, "mistralai/Mistral-7B-v0.3"));
    assert!(!model_is_downloaded(&models, "google/gemma-2-9b"));
}

#[test]
fn repo_name_with_extra_suffix() {
    // HF repo: unsloth/Qwen3.6-27B-MTP-GGUF, local file: Qwen3.6-27B-Q3_K_S.gguf
    // The repo name has "MTP-GGUF" suffix not in the local filename
    let models = vec![make_discovered("Qwen3.6-27B-Q3_K_S.gguf")];
    assert!(model_is_downloaded(&models, "unsloth/Qwen3.6-27B-MTP-GGUF"));
}

#[test]
fn repo_name_with_extra_suffix_different_size() {
    // Should NOT match: repo is 27B, local is 7B
    let models = vec![make_discovered("Qwen3.6-7B-Q3_K_S.gguf")];
    assert!(!model_is_downloaded(
        &models,
        "unsloth/Qwen3.6-27B-MTP-GGUF"
    ));
}

#[test]
fn prevent_false_positives() {
    // Should NOT match if authors are different and specified
    let mut local_model_with_author = make_discovered("qwen3.5-4b-instruct-q4_k_m.gguf");
    local_model_with_author.display_name =
        "Qwen/Qwen3.5-4B-Instruct/qwen3.5-4b-instruct-q4_k_m.gguf".to_string();
    let models = vec![local_model_with_author];

    // Different author and base vs Instruct: titan0115/Qwen3.5-4B vs Qwen/Qwen3.5-4B-Instruct
    assert!(!model_is_downloaded(&models, "titan0115/Qwen3.5-4B"));

    // Correct author and same variant should match
    assert!(model_is_downloaded(&models, "Qwen/Qwen3.5-4B-Instruct"));
}

#[test]
fn subdirectory_prefix_match() {
    let mut local_model = make_discovered("Qwen3.5-4B-UD-IQ3_XXS.gguf");
    local_model.display_name = "unsloth/Qwen3.5-4B-MTP-GGUF/Qwen3.5-4B-UD-IQ3_XXS.gguf".to_string();
    let models = vec![local_model];

    // Should match because the file is in the exact directory matching the model_id
    assert!(model_is_downloaded(&models, "unsloth/Qwen3.5-4B-MTP-GGUF"));
}
