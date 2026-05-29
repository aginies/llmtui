use crate::tui::app::{App, ConfirmationKind};

pub async fn execute_confirmation(app: &mut App, kind: ConfirmationKind) {
    match kind {
        ConfirmationKind::Exit => {
            app.running = false;
        }
        ConfirmationKind::Reset => {
            app.reset_to_defaults();
        }
        ConfirmationKind::Delete => {
            if let Some(model) = app.selected_model() {
                let display_name = model.display_name.clone();
                app.add_log(format!("Deleting model {} (config moved to unused)...", display_name), crate::config::LogLevel::Info);
            }
        }
        ConfirmationKind::Unload => {
            if let Some((name, _)) = &app.pending.pending_api_unload {
                app.add_log(format!("Unloading {} via API...", name), crate::config::LogLevel::Info);
            }
        }
        ConfirmationKind::DeleteBackend => {
            // Handled in main.rs loop by looking at pending_backend_deletion
            // and confirming global_mode transitioned back to Normal
        }
    }
}

pub fn sync_global_settings(app: &mut App) {
    let changed = app.config.default.host != app.settings.host
        || app.config.default.port != app.settings.port
        || app.config.default.backend != app.settings.backend
        || app.config.default.parallel != app.settings.parallel
        || app.config.default.max_concurrent_predictions != app.settings.max_concurrent_predictions
        || app.config.default.threads != app.settings.threads
        || app.config.default.threads_batch != app.settings.threads_batch
        || app.config.default.api_endpoint_enabled != app.settings.api_endpoint_enabled
        || app.config.default.api_endpoint_port != app.settings.api_endpoint_port
        || app.config.default.server_mode != app.server_mode
        || app.config.default.router_max_models != app.router_max_models
        || app.config.default.llama_cpp_version_cpu != app.settings.llama_cpp_version_cpu
        || app.config.default.llama_cpp_version_vulkan != app.settings.llama_cpp_version_vulkan
        || app.config.default.llama_cpp_version_rocm != app.settings.llama_cpp_version_rocm
        || app.config.default.llama_cpp_version_rocm_lemonade != app.settings.llama_cpp_version_rocm_lemonade
        || app.config.default.llama_cpp_version_cuda != app.settings.llama_cpp_version_cuda
        || app.config.default.ws_server_enabled != app.settings.ws_server_enabled
        || app.config.default.ws_server_port != app.settings.ws_server_port
        || app.config.default.ws_server_auth_key != app.settings.ws_server_auth_key
        || app.config.default.ws_server_tls_enabled != app.settings.ws_server_tls_enabled
        || app.config.default.ws_server_tls_cert != app.settings.ws_server_tls_cert
        || app.config.default.ws_server_tls_key != app.settings.ws_server_tls_key;
    if !changed {
        return;
    }
    app.config.default.host = app.settings.host.clone();
    app.config.default.port = app.settings.port;
    app.config.default.backend = app.settings.backend;
    app.config.default.parallel = app.settings.parallel;
    app.config.default.max_concurrent_predictions = app.settings.max_concurrent_predictions;
    app.config.default.threads = app.settings.threads;
    app.config.default.threads_batch = app.settings.threads_batch;
    app.config.default.api_endpoint_enabled = app.settings.api_endpoint_enabled;
    app.config.default.api_endpoint_port = app.settings.api_endpoint_port;
    app.config.default.server_mode = app.server_mode.clone();
    app.config.default.router_max_models = app.router_max_models;
    app.config.default.llama_cpp_version_cpu = app.settings.llama_cpp_version_cpu.clone();
    app.config.default.llama_cpp_version_vulkan = app.settings.llama_cpp_version_vulkan.clone();
    app.config.default.llama_cpp_version_rocm = app.settings.llama_cpp_version_rocm.clone();
    app.config.default.llama_cpp_version_rocm_lemonade = app.settings.llama_cpp_version_rocm_lemonade.clone();
    app.config.default.llama_cpp_version_cuda = app.settings.llama_cpp_version_cuda.clone();
    app.config.default.ws_server_enabled = app.settings.ws_server_enabled;
    app.config.default.ws_server_port = app.settings.ws_server_port;
    app.config.default.ws_server_auth_key = app.settings.ws_server_auth_key.clone();
    app.config.default.ws_server_tls_enabled = app.settings.ws_server_tls_enabled;
    app.config.default.ws_server_tls_cert = app.settings.ws_server_tls_cert.clone();
    app.config.default.ws_server_tls_key = app.settings.ws_server_tls_key.clone();

    // Also sync the dedicated ws_server struct in config
    app.config.ws_server.enabled = app.settings.ws_server_enabled;
    app.config.ws_server.port = app.settings.ws_server_port;
    app.config.ws_server.auth_key = app.settings.ws_server_auth_key.clone();
    app.config.ws_server.tls_enabled = app.settings.ws_server_tls_enabled;
    app.config.ws_server.tls_cert = app.settings.ws_server_tls_cert.clone();
    app.config.ws_server.tls_key = app.settings.ws_server_tls_key.clone();

    if let Err(e) = app.config.save() {
        app.add_log(
            format!("Failed to save global settings: {}", e),
            crate::config::LogLevel::Error,
        );
    }
}
