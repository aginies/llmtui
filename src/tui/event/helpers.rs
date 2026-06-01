use crate::tui::app::{App, ConfirmationKind};

pub struct TextEditor<'a> {
    pub buffer: &'a mut String,
    pub cursor: &'a mut usize,
}

impl TextEditor<'_> {
    pub fn insert_char(&mut self, c: char) {
        let byte_pos = self.buffer
            .char_indices()
            .nth(*self.cursor)
            .map(|(i, _)| i)
            .unwrap_or(self.buffer.len());
        self.buffer.insert_str(byte_pos, &c.to_string());
        *self.cursor += 1;
    }

    pub fn insert_newline(&mut self) {
        let byte_pos = self.buffer
            .char_indices()
            .nth(*self.cursor)
            .map(|(i, _)| i)
            .unwrap_or(self.buffer.len());
        self.buffer.insert_str(byte_pos, "\n");
        *self.cursor += 1;
    }

    pub fn backspace(&mut self) {
        if *self.cursor == 0 {
            return;
        }
        let char_pos = *self.cursor - 1;
        let byte_pos = self.buffer
            .char_indices()
            .nth(char_pos)
            .map(|(i, _)| i)
            .unwrap_or(self.buffer.len());
        let char_len = self.buffer[byte_pos..]
            .chars()
            .next()
            .map(|c| c.len_utf8())
            .unwrap_or(1);
        self.buffer.drain(byte_pos..byte_pos + char_len);
        *self.cursor -= 1;
    }

    #[allow(dead_code)]
    pub fn delete(&mut self) {
        if *self.cursor >= self.buffer.chars().count() {
            return;
        }
        let byte_pos = self.buffer
            .char_indices()
            .nth(*self.cursor)
            .map(|(i, _)| i)
            .unwrap_or(self.buffer.len());
        self.buffer.remove(byte_pos);
    }

    pub fn move_left(&mut self) {
        if *self.cursor > 0 {
            *self.cursor -= 1;
        }
    }

    pub fn move_right(&mut self) {
        let max = self.buffer.chars().count();
        if *self.cursor < max {
            *self.cursor += 1;
        }
    }

    pub fn home(&mut self) {
        *self.cursor = 0;
    }

    pub fn end(&mut self) {
        *self.cursor = self.buffer.chars().count();
    }
}

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
                app.add_log(
                    format!(
                        "Deleting model {} (config moved to unused)...",
                        display_name
                    ),
                    crate::config::LogLevel::Info,
                );
            }
        }
        ConfirmationKind::Unload => {
            if let Some((name, _)) = &app.pending.pending_api_unload {
                app.add_log(
                    format!("Unloading {} via API...", name),
                    crate::config::LogLevel::Info,
                );
            }
        }
        ConfirmationKind::DeleteBackend => {
            // Handled in main.rs loop by looking at pending_backend_deletion
            // and confirming global_mode transitioned back to Normal
        }
    }
}

pub fn mark_settings_dirty(app: &mut App, recalc_vram: bool) {
    app.settings_state.settings_render_cache = None;
    if recalc_vram {
        app.update_vram_estimate();
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
        || app.config.default.llama_cpp_version_rocm_lemonade
            != app.settings.llama_cpp_version_rocm_lemonade
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
    app.config.default.llama_cpp_version_rocm_lemonade =
        app.settings.llama_cpp_version_rocm_lemonade.clone();
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
