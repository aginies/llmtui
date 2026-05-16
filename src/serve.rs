use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::{Context, Result};
use tokio::select;
use tracing::info;

use crate::backend::server;
use crate::config::Config;
use crate::models::{Backend, DiscoveredModel, ModelSettings};

/// Serve a model using the llama-server binary, applying all settings from config.yaml.
///
/// This is a standalone CLI command (llm-manager serve) that:
/// 1. Loads config (same config.yaml as the TUI)
/// 2. Resolves the model path
/// 3. Fetches settings from config overrides, profiles, and defaults
/// 4. Builds and spawns the llama-server command
/// 5. Optionally starts an API proxy server on a separate port
/// 6. Streams output to stdout/stderr until killed
///
/// Usage:
///   llm-manager serve --model /path/to/model.gguf [--profile qwen] [--config /path/to/config.yaml]
///   llm-manager serve --model model.gguf --api-port 49222 --api-key secret
pub async fn serve_model(
    model_path: &str,
    profile_name: Option<&str>,
    config_path: Option<&str>,
    api_port: Option<u16>,
    api_key: Option<String>,
) -> Result<()> {
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
    if let Ok(metadata) = model_path.symlink_metadata()
        && metadata.file_type().is_symlink()
            && !model_path.exists() {
                let target = std::fs::read_link(&model_path).unwrap_or_default();
                let msg = format!(
                    "Model file is a broken symlink: {}\n  Symlink points to: {}\n  The target does not exist. Fix the symlink or use the actual file.",
                    model_path.display(),
                    target.display()
                );
                return Err(anyhow::Error::msg(msg));
            }

    if !model_path.exists() {
        // Check if parent directory exists
        if let Some(parent) = model_path.parent()
            && !parent.exists() {
                let msg = format!(
                    "Model file not found: {}\n  Parent directory does not exist: {}",
                    model_path.display(),
                    parent.display()
                );
                return Err(anyhow::Error::msg(msg));
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
    let layers_str = match settings.gpu_layers_mode {
        crate::models::GpuLayersMode::Auto => "auto".to_string(),
        crate::models::GpuLayersMode::Specific(n) => n.to_string(),
        crate::models::GpuLayersMode::All => "all".to_string(),
    };
    info!("Settings: {} threads, {} layers, {} context", settings.threads, layers_str, settings.context_length);

    // Resolve the backend binary (downloads if needed)
    let version_param = match settings.backend {
        Backend::Cpu => settings.llama_cpp_version_cpu.as_deref(),
        Backend::Vulkan => settings.llama_cpp_version_vulkan.as_deref(),
        Backend::Rocrm => settings.llama_cpp_version_rocm.as_deref(),
    };
    let binary = match crate::backend::hub::resolve_backend_binary(settings.backend, version_param).await {
        Ok(path) => {
            if !path.exists() {
                anyhow::bail!("llama-server binary not found at: {}", path.display());
            }
            path
        }
        Err(e) => anyhow::bail!("Failed to resolve backend binary: {}", e),
    };
    info!("Using llama-server: {} (backend: {})", binary.display(), settings.backend);

    // Build the server command
    let (mut cmd, cmd_display) = server::build_server_cmd(&binary, Some(&model), &settings, &config, config.default.server_mode.clone(), config.default.router_max_models);

    // Set LD_LIBRARY_PATH so the binary can find its shared libraries
    let bin_dir = binary.parent().unwrap();
    if let Ok(current) = std::env::var("LD_LIBRARY_PATH") {
        cmd.env("LD_LIBRARY_PATH", format!("{}:{}", bin_dir.display(), current));
    } else {
        cmd.env("LD_LIBRARY_PATH", bin_dir);
    }

    // Spawn the process
    info!("Command: {}", cmd_display);
    let mut child = cmd
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .context(format!("Failed to spawn llama-server.\n\n  Command that was attempted:\n    {}\n\n  Check that the binary exists and is executable.", cmd_display))?;

    info!("llama-server started (pid={})", child.id().unwrap_or(0));
    info!("Press Ctrl+C to stop the server");

    let server_pid = child.id().unwrap_or(0);

    // Optionally start the API proxy server
    let api_server_handle = if let Some(port) = api_port {
        let addr: SocketAddr = format!("127.0.0.1:{}", port).parse()?;
        let model_name = model.display_name.clone();
        let server_port = settings.port;
        let api_key_clone = api_key.clone();
        let handle = tokio::spawn(crate::serve_api::start_api_server(
            addr,
            api_key_clone,
            server_port,
            model_name,
            server_pid,
        ));
        info!("API proxy server enabled on http://127.0.0.1:{}", port);
        Some(handle)
    } else {
        None
    };

    // Wait for either llama-server or API server to exit
    let status = select! {
        status = child.wait() => {
            status.context("Failed to wait for llama-server")?
        }
        _ = async {
            if let Some(handle) = api_server_handle {
                let _ = handle.await;
            }
        } => {
            child.wait().await.context("Failed to wait for llama-server")?
        }
    };

    // Kill the other process if both are running
    if !status.success() {
        let _ = child.kill().await;
    }

    if status.success() {
        info!("llama-server exited normally");
    } else {
        info!("llama-server exited with status: {}", status);
    }

    Ok(())
}
