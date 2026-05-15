use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use tracing::info;

use crate::backend::server;
use crate::config::Config;
use crate::models::{DiscoveredModel, ModelSettings};

/// Resolve llama-server binary path:
/// 1. If absolute path in config, use it directly
/// 2. If in PATH, use which
/// 3. Check common locations
fn resolve_llama_server(llama_server_path: &PathBuf) -> Result<PathBuf> {
    let mut found_paths = Vec::new();

    // 1. Absolute path
    if llama_server_path.is_absolute() && llama_server_path.exists() {
        return Ok(llama_server_path.clone());
    }

    // 2. Search PATH
    if let Ok(output) = std::process::Command::new("which")
        .arg(llama_server_path)
        .output()
    {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Ok(PathBuf::from(path));
            }
        }
    }

    // 3. Common locations
    for candidate in &[
        "/usr/bin/llama-server",
        "/usr/local/bin/llama-server",
        "/opt/homebrew/bin/llama-server",
        "/snap/bin/llama-server",
    ] {
        found_paths.push(candidate);
        if PathBuf::from(candidate).exists() {
            return Ok(PathBuf::from(candidate));
        }
    }

    Err(anyhow!(
        "llama-server not found.\n\n  Config has: '{}'\n  This binary does not exist on your system.\n\n  Options:\n  1. Set the full path in config.yaml (e.g. /usr/bin/llama-server)\n  2. Ensure llama-server is in your PATH\n  3. Check that llama.cpp is installed and compiled\n\n  Searched locations: {}",
        llama_server_path.display(),
        found_paths.iter().map(|s| s.to_string()).collect::<Vec<_>>().join(", ")
    ))
}

/// Serve a model using the llama-server binary, applying all settings from config.yaml.
///
/// This is a standalone CLI command (llm-manager serve) that:
/// 1. Loads config (same config.yaml as the TUI)
/// 2. Resolves the model path
/// 3. Fetches settings from config overrides, profiles, and defaults
/// 4. Builds and spawns the llama-server command
/// 5. Streams output to stdout/stderr until killed
///
/// Usage:
///   llm-manager serve --model /path/to/model.gguf [--profile qwen] [--config /path/to/config.yaml]
pub async fn serve_model(model_path: &str, profile_name: Option<&str>, config_path: Option<&str>) -> Result<()> {
    // Load config from explicit path or default location
    let config = match config_path {
        Some(p) => {
            let path = PathBuf::from(p);
            Config::load_from(path).map_err(|e| anyhow::anyhow!("Failed to load config: {}", e))?
        }
        None => Config::load().map_err(|e| anyhow::anyhow!("Failed to load config: {}", e))?,
    };

    // Resolve model path
    let model_path = PathBuf::from(model_path);

    // Check for broken symlinks first
    if let Ok(metadata) = model_path.symlink_metadata() {
        if metadata.file_type().is_symlink() {
            if !model_path.exists() {
                let target = std::fs::read_link(&model_path).unwrap_or_default();
                let msg = format!(
                    "Model file is a broken symlink: {}\n  Symlink points to: {}\n  The target does not exist. Fix the symlink or use the actual file.",
                    model_path.display(),
                    target.display()
                );
                return Err(anyhow::Error::msg(msg));
            }
        }
    }

    if !model_path.exists() {
        // Check if parent directory exists
        if let Some(parent) = model_path.parent() {
            if !parent.exists() {
                let msg = format!(
                    "Model file not found: {}\n  Parent directory does not exist: {}",
                    model_path.display(),
                    parent.display()
                );
                return Err(anyhow::Error::msg(msg));
            }
        }
        let msg = format!("Model file not found: {}", model_path.display());
        return Err(anyhow::Error::msg(msg));
    }

    if !model_path.extension().map(|e| e == "gguf").unwrap_or(false) {
        let msg = format!("Model file must be a .gguf file: {}", model_path.display());
        return Err(anyhow::Error::msg(msg));
    }

    let name = model_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let display_name = model_path
        .strip_prefix(&config.models_dir)
        .ok()
        .and_then(|p| p.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| name.clone());

    let model = DiscoveredModel {
        path: model_path.clone(),
        name: name.clone(),
        file_size: std::fs::metadata(&model_path).map(|m| m.len()).unwrap_or(0),
        display_name: display_name.clone(),
    };

    // Build settings: start with defaults, apply model override, then profile override
    let mut settings = ModelSettings::from_config(&config);

    // Apply model-specific override
    if let Some(override_settings) = config.model_overrides.get(&name) {
        override_settings.apply(&mut settings);
    }

    // Apply profile override if specified
    if let Some(profile) = config.profiles.iter().find(|p| profile_name.map(|n| p.name == n).unwrap_or(false)) {
        settings = profile.apply(settings);
        info!("Applied profile: {}", profile.name);
    }

    info!("Serving model: {}", model.display_name);
    info!("Settings: {} threads, {} layers, {} context", settings.threads, settings.gpu_layers, settings.context_length);

    // Resolve llama-server binary path
    let binary = resolve_llama_server(&config.llama_server)?;
    info!("Using llama-server: {}", binary.display());

    // Build the server command
    let (mut cmd, cmd_display) = server::build_server_cmd(&binary, Some(&model), &settings, &config);

    // Spawn the process
    info!("Command: {}", cmd_display);
    let mut child = cmd
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .context(format!("Failed to spawn llama-server.\n\n  Command that was attempted:\n    {}\n\n  Check that the binary exists and is executable.", cmd_display))?;

    info!("llama-server started (pid={})", child.id().unwrap_or(0));
    info!("Press Ctrl+C to stop the server");

    // Wait for the process to exit
    let status = child
        .wait()
        .await
        .context("Failed to wait for llama-server")?;

    if status.success() {
        info!("llama-server exited normally");
    } else {
        info!("llama-server exited with status: {}", status);
    }

    Ok(())
}
