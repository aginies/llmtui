use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::{Context, Result};
use tokio::select;
use tokio::signal;
use tracing::info;

use crate::backend::server;
use crate::backend::tls;
use crate::config::Config;
use crate::models::{DiscoveredModel, WsMetrics};

#[derive(Default)]
pub struct ServeOptions {
    pub model_path: String,
    pub profile_name: Option<String>,
    pub config_path: Option<String>,
    pub api_port: Option<u16>,
    pub api_key: Option<String>,
    pub ws_enable: bool,
    pub ws_port: Option<u16>,
    pub ws_auth: Option<String>,
    pub backend_binary: Option<String>,
    pub host: Option<String>,
    pub tls_enable: bool,
    pub tls_cert: Option<String>,
    pub tls_key: Option<String>,
    pub log_file: Option<String>,
}

async fn start_metrics_polling_task(
    host: String,
    port: u16,
    pid: u32,
    model_name: String,
    settings: crate::models::ModelSettings,
    cmd_display: String,
    tx: tokio::sync::broadcast::Sender<WsMetrics>,
    shutdown_rx: tokio::sync::watch::Receiver<bool>,
) {
    let mut consecutive_failures: u32 = 0;
    let max_failures: u32 = 15;

    loop {
        // Check shutdown first
        if *shutdown_rx.borrow() {
            break;
        }

        let m = match tokio::time::timeout(
            std::time::Duration::from_secs(3),
            server::get_metrics(&host, port, None, Some(pid)),
        )
        .await
        {
            Ok(Ok(metrics)) => {
                consecutive_failures = 0;
                metrics
            }
            Ok(Err(_)) | Err(_) => {
                consecutive_failures += 1;
                if consecutive_failures >= max_failures {
                    tracing::warn!(
                        "Metrics polling aborted after {} consecutive failures (server likely dead)",
                        max_failures
                    );
                    break;
                }
                if consecutive_failures % 5 == 1 {
                    tracing::warn!(
                        "Metrics polling: server unreachable (attempt {}/{})",
                        consecutive_failures,
                        max_failures
                    );
                }
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                continue;
            }
        };

        let state = "loaded";
        let ws_metrics =
            WsMetrics::from_metrics(&m, &model_name, state, &settings, Some(&cmd_display));

        if let Err(e) = tx.send(ws_metrics) {
            tracing::debug!("Failed to send metrics to broadcast channel: {e}");
        }

        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
}

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
pub async fn serve_model(opts: ServeOptions) -> Result<()> {
    // Load config from explicit path or default location
    let config = match opts.config_path.as_deref() {
        Some(p) => {
            let path = PathBuf::from(p);
            Config::load_from(path).map_err(|e| anyhow::anyhow!("Failed to load config: {}", e))?
        }
        None => Config::load().map_err(|e| anyhow::anyhow!("Failed to load config: {}", e))?,
    };

    // Resolve model path
    let model_path = PathBuf::from(&opts.model_path);

    // Check for broken symlinks first
    if let Ok(metadata) = model_path.symlink_metadata()
        && metadata.file_type().is_symlink()
        && !model_path.exists()
    {
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
            && !parent.exists()
        {
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
        .strip_prefix(config.models_dirs.first().unwrap_or(&PathBuf::new()))
        .ok()
        .and_then(|p| p.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| name.clone());

    let model = DiscoveredModel {
        path: model_path.clone(),
        name: name.clone(),
        file_size: std::fs::metadata(&model_path).map(|m| m.len()).unwrap_or(0),
        display_name: display_name.clone(),
        pipeline_tag: None,
        capabilities: vec![],
    };

    // Build settings: start with defaults, apply model override, then profile override
    tracing::info!("Model display_name for config lookup: {}", display_name);
    tracing::info!(
        "Available model config keys: {:?}",
        config.model_overrides.keys()
    );
    let mut settings = config.resolve_settings(Some(&display_name), opts.profile_name.as_deref());

    // Auto-enable MTP if supported by model and not explicitly enabled in config
    if settings.spec_type.is_empty()
        && let Ok(meta) = crate::models::GgufMetadata::from_path(&model_path)
        && meta.arch == "mtp"
    {
        tracing::info!("Auto-enabling MTP (Multi-Token Prediction) for model");
        settings.spec_type = "draft-mtp".to_string();
        if settings.draft_tokens == 0 {
            settings.draft_tokens = meta.draft_tokens;
        }
    }

    // WebSocket settings: CLI flags take precedence, then config.yaml
    let ws_enable = opts.ws_enable || config.default.ws_server_enabled;
    let ws_port = opts.ws_port.unwrap_or(config.default.ws_server_port);
    let ws_auth: Option<String> = opts.ws_auth.or(config.default.ws_server_auth_key.clone());

    // TLS settings: CLI flags take precedence, then config.yaml
    let tls_enable = opts.tls_enable || config.default.ws_server_tls_enabled;
    let tls_cert = opts.tls_cert.or(config.default.ws_server_tls_cert.clone());
    let tls_key = opts.tls_key.or(config.default.ws_server_tls_key.clone());

    let tls_config = if tls_enable || (tls_cert.is_some() && tls_key.is_some()) {
        let (cert_path, key_path) = if let Some(cert) = &tls_cert {
            match &tls_key {
                Some(key) => {
                    tls::validate_tls_path(cert).map_err(|e| anyhow::anyhow!("TLS: {}", e))?;
                    tls::validate_tls_path(key).map_err(|e| anyhow::anyhow!("TLS: {}", e))?;
                    (cert.clone(), key.clone())
                }
                None => {
                    return Err(anyhow::anyhow!(
                        "TLS key is required when TLS certificate is provided"
                    ));
                }
            }
        } else {
            let (cert, key) = tls::ensure_tls_certs().map_err(|e| anyhow::anyhow!("TLS: {}", e))?;
            (
                cert.to_string_lossy().to_string(),
                key.to_string_lossy().to_string(),
            )
        };
        let tls_cfg = tls::load_tls_config(&cert_path, &key_path)
            .await
            .map_err(|e| anyhow::anyhow!("TLS: {}", e))?;
        Some(tls_cfg)
    } else {
        None
    };

    if tls_config.is_some() {
        info!("TLS enabled for WebSocket dashboard and API server");
    }

    // CLI host override
    if let Some(h) = &opts.host {
        settings.host = h.to_string();
    }

    info!("Serving model: {}", model.display_name);
    let layers_str = match settings.gpu_layers_mode {
        crate::models::GpuLayersMode::Auto => "auto".to_string(),
        crate::models::GpuLayersMode::Specific(n) => n.to_string(),
        crate::models::GpuLayersMode::All => "all".to_string(),
    };
    info!(
        "Settings: {} threads, {} layers, {} context",
        settings.threads, layers_str, settings.context_length
    );

    // Trace backend binary selection
    let active_version = settings.get_active_backend_version();
    let version_display = settings.get_active_backend_version_display();
    info!(
        "Backend: {}, version config: {:?} (display: {})",
        settings.backend, active_version, version_display
    );
    if let Some(ref cpu_ver) = settings.llama_cpp_version_cpu {
        info!("  llama_cpp_version_cpu = {}", cpu_ver);
    }
    if let Some(ref cuda_ver) = settings.llama_cpp_version_cuda {
        info!("  llama_cpp_version_cuda = {}", cuda_ver);
    }

    if ws_enable {
        let auth_info = if let Some(ref auth) = ws_auth {
            format!(" (auth: {})", &auth[..auth.len().min(8)])
        } else {
            String::new()
        };
        info!(
            "WebSocket dashboard enabled on port {}{}",
            ws_port, auth_info
        );
    }

    // Resolve the backend binary (downloads if needed)
    let binary = if let Some(path) = &opts.backend_binary {
        let binary_path = PathBuf::from(path);
        if !binary_path.exists() {
            anyhow::bail!("Backend binary not found: {}", binary_path.display());
        }
        info!("Using custom backend binary: {}", binary_path.display());
        binary_path
    } else {
        let version_param = settings.get_active_backend_version().map(|s| s.as_str());
        info!(
            "Resolving backend binary: backend={}, version_param={:?}",
            settings.backend, version_param
        );
        match crate::backend::hub::resolve_backend_binary(
            settings.backend,
            version_param,
            None,
            None,
        )
        .await
        {
            Ok(path) => {
                info!("Resolved binary path: {}", path.display());
                if !path.exists() {
                    anyhow::bail!("llama-server binary not found at: {}", path.display());
                }
                path
            }
            Err(e) => anyhow::bail!("Failed to resolve backend binary: {}", e),
        }
    };
    info!(
        "Using llama-server: {} (backend: {})",
        binary.display(),
        settings.backend
    );

    // Build the server command
    let (mut cmd, cmd_display) = server::build_server_cmd(
        &binary,
        Some(&model),
        &settings,
        &config,
        config.default.server_mode,
        config.default.router_max_models,
    );

    // Set LD_LIBRARY_PATH so the binary can find its shared libraries
    let bin_dir = binary.parent().context(
        "Backend binary path has no parent directory. Use a full path for --backend-binary.",
    )?;
    if let Ok(current) = std::env::var("LD_LIBRARY_PATH") {
        cmd.env(
            "LD_LIBRARY_PATH",
            format!("{}:{}", bin_dir.display(), current),
        );
    } else {
        cmd.env("LD_LIBRARY_PATH", bin_dir);
    }

    // Spawn the process
    info!("Command: {}", cmd_display);

    let (stdout_file, stderr_file) = if let Some(path) = &opts.log_file {
        let path = PathBuf::from(path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .expect("Failed to open log file for llama-server output");
        info!("llama-server output logging to: {}", path.display());
        let stdout = file.try_clone().expect("Failed to clone file handle");
        let stderr = file.try_clone().expect("Failed to clone file handle");
        (std::process::Stdio::from(stdout), std::process::Stdio::from(stderr))
    } else {
        (std::process::Stdio::inherit(), std::process::Stdio::inherit())
    };

    let mut child = cmd
        .stdout(stdout_file)
        .stderr(stderr_file)
        .spawn()
        .context(format!("Failed to spawn llama-server.\n\n  Command that was attempted:\n    {}\n\n  Check that the binary exists and is executable.", cmd_display))?;

    info!("llama-server started (pid={})", child.id().unwrap_or(0));
    info!("Press Ctrl+C to stop the server");

    let server_pid = child.id().unwrap_or(0);

    // Optionally start the API proxy server
    let (api_done_tx, api_done_rx) = tokio::sync::oneshot::channel();
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let mut api_server_handle = if let Some(port) = opts.api_port {
        let host_str = &settings.host;
        let addr: SocketAddr = format!("{}:{}", host_str, port).parse()?;
        let model_name = model.display_name.clone();
        let server_port = settings.port;
        let api_key_clone = opts.api_key.clone();
        let shutdown_rx_for_api = shutdown_rx.clone();
        let host_clone = host_str.clone();
        let tls_for_api = tls_config.clone();
        let handle = tokio::spawn(async move {
            let _ = crate::serve_api::start_api_server(
                addr,
                api_key_clone,
                server_port,
                model_name,
                server_pid,
                shutdown_rx_for_api,
                host_clone,
                tls_for_api,
            )
            .await;
            let _ = api_done_tx.send(());
        });
        let api_protocol = if tls_config.is_some() {
            "https"
        } else {
            "http"
        };
        info!(
            "API proxy started on {api_protocol}://{}:{}",
            host_str, port
        );
        Some((handle, api_done_rx, shutdown_tx))
    } else {
        None
    };

    // Start WebSocket dashboard server
    let ws_server_handle = if ws_enable {
        let (tx, rx) = tokio::sync::broadcast::channel(64);
        let ws_rx = std::sync::Arc::new(rx);
        let host_str = &settings.host;
        let handle = crate::backend::ws_server::start_ws_server(
            ws_port,
            ws_rx,
            ws_auth.clone(),
            tls_config.clone(),
            host_str.clone(),
        )
        .await?;

        let auth_param = if let Some(ref auth) = ws_auth {
            format!("?auth={}", urlencoding::encode(auth))
        } else {
            "".to_string()
        };
        let protocol = if tls_config.is_some() {
            "https"
        } else {
            "http"
        };
        info!(
            "Dashboard enabled: {protocol}://{}:{}/dashboard{}",
            host_str, ws_port, auth_param
        );

        // Start metrics polling task
        let settings_clone = settings.clone();
        let model_name_clone = model.display_name.clone();
        let host_clone = settings.host.clone();
        let server_port_clone = settings.port;
        let pid_clone = server_pid;
        let cmd_display_clone = cmd_display.clone();
        let shutdown_rx_clone = shutdown_rx.clone();
        tokio::spawn(async move {
            start_metrics_polling_task(
                host_clone,
                server_port_clone,
                pid_clone,
                model_name_clone,
                settings_clone,
                cmd_display_clone,
                tx,
                shutdown_rx_clone,
            )
            .await;
        });

        Some(handle)
    } else {
        None
    };

    // Wait for either llama-server, API server, or Ctrl+C
    let status = loop {
        select! {
            exit_result = child.wait() => {
                // llama-server exited — gracefully shut down API server
                if let Some((_, _, tx)) = &mut api_server_handle {
                    let _ = tx.send(true);
                }
                break exit_result.unwrap_or_else(|e| {
                    tracing::error!("Failed to wait for llama-server: {}", e);
                    std::process::Command::new("sh")
                        .arg("-c")
                        .arg("exit 1")
                        .status()
                        .expect("failed to get exit status")
                });
            }
            _ = async {
                let (_, rx, _) = api_server_handle.as_mut().unwrap();
                let _ = rx.await;
            }, if api_server_handle.is_some() => {
                // API server exited — gracefully shut down, then wait for llama-server
                if let Some((_, _, tx)) = &mut api_server_handle {
                    let _ = tx.send(true);
                }
                break child.wait().await.unwrap_or_else(|e| {
                    tracing::error!("Failed to wait for llama-server: {}", e);
                    std::process::Command::new("sh")
                        .arg("-c")
                        .arg("exit 1")
                        .status()
                        .expect("failed to get exit status")
                });
            }
            _ = signal::ctrl_c() => {
                info!("Received SIGINT, shutting down llama-server...");
                let _ = child.kill().await;
                if let Some((_, _, tx)) = &mut api_server_handle {
                    let _ = tx.send(true);
                }
            }
        }
    };

    // Drop the API server handle so the spawned task can finish
    if let Some((handle, _, _)) = api_server_handle {
        let _ = handle.await;
    }

    // Abort the WebSocket dashboard server
    if let Some(handle) = ws_server_handle {
        handle.abort();
    }

    if status.success() {
        info!("llama-server exited normally");
    } else {
        info!("llama-server exited with status: {}", status);
    }

    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("llama-server exited with status: {}", status)
    }
}
