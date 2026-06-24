use crate::tui::app::pending_events::PendingEvent;
use crate::tui::app::{ActivePanel, App, ConfirmationKind};

pub fn picker_nav_up(selected: &mut usize) {
    *selected = selected.saturating_sub(1);
}
pub fn picker_nav_down(selected: &mut usize, len: usize) {
    *selected = (*selected + 1).min(len.saturating_sub(1));
}

pub struct TextEditor<'a> {
    pub buffer: &'a mut String,
    pub cursor: &'a mut usize,
}

impl TextEditor<'_> {
    pub fn insert_char(&mut self, c: char) {
        let byte_pos = self
            .buffer
            .char_indices()
            .nth(*self.cursor)
            .map(|(i, _)| i)
            .unwrap_or(self.buffer.len());
        self.buffer.insert(byte_pos, c);
        *self.cursor += 1;
    }

    pub fn insert_newline(&mut self) {
        let byte_pos = self
            .buffer
            .char_indices()
            .nth(*self.cursor)
            .map(|(i, _)| i)
            .unwrap_or(self.buffer.len());
        self.buffer.insert(byte_pos, '\n');
        *self.cursor += 1;
    }

    pub fn backspace(&mut self) {
        if *self.cursor == 0 {
            return;
        }
        let char_pos = *self.cursor - 1;
        let byte_pos = self
            .buffer
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

    pub fn delete(&mut self) {
        if *self.cursor >= self.buffer.chars().count() {
            return;
        }
        let byte_pos = self
            .buffer
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

pub async fn execute_confirmation(
    app: &mut App,
    kind: ConfirmationKind,
    display_name: String,
    detail: Option<String>,
) {
    match kind {
        ConfirmationKind::Exit => {
            app.running = false;
        }
        ConfirmationKind::Reset => {
            app.reset_to_defaults();
        }
        ConfirmationKind::Delete => {
            // Path is in detail field
            if let Some(path) = detail {
                let _ = app
                    .pending_tx
                    .send(PendingEvent::Deletion {
                        path: std::path::PathBuf::from(path),
                    })
                    .await;
            }
            app.add_log(
                format!(
                    "Deleting model {} (config moved to unused)...",
                    display_name
                ),
                crate::config::LogLevel::Info,
            );
        }
        ConfirmationKind::Unload => {
            app.pending.pending_api_unload = Some(display_name);
        }
        ConfirmationKind::DeleteBackend => {
            if let Some(path) = detail {
                // Backend deletion path encoding: "backend:tag"
                if let Some((backend_str, tag)) = path.split_once(':') {
                    let backend = match backend_str {
                        "cpu" => crate::models::Backend::Cpu,
                        "vulkan" => crate::models::Backend::Vulkan,
                        "rocm" => crate::models::Backend::Rocm,
                        "rocm-lemonade" => crate::models::Backend::RocmLemonade,
                        "cuda" => crate::models::Backend::Cuda,
                        _ => return,
                    };
                    let _ = app
                        .pending_tx
                        .send(PendingEvent::BackendDeletion {
                            backend,
                            tag: tag.to_string(),
                        })
                        .await;
                }
            }
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
    let has_model = app.selected_model().is_some();

    let changed = if has_model {
        app.config.default.host != app.settings.host
            || app.config.default.port != app.settings.port
            || app.config.default.backend != app.settings.backend
            || app.config.default.api_endpoint_enabled != app.settings.api_endpoint_enabled
            || app.config.default.api_endpoint_port != app.settings.api_endpoint_port
            || app.config.default.api_endpoint_key != app.settings.api_endpoint_key
            || app.config.default.server_mode != app.server_mode
            || app.config.default.router_max_models != app.router_max_models
            || app.config.default.llama_cpp_version_cpu != app.settings.llama_cpp_version_cpu
            || app.config.default.llama_cpp_version_vulkan != app.settings.llama_cpp_version_vulkan
            || app.config.default.llama_cpp_version_rocm != app.settings.llama_cpp_version_rocm
            || app.config.default.llama_cpp_version_rocm_lemonade
                != app.settings.llama_cpp_version_rocm_lemonade
            || app.config.default.llama_cpp_version_cuda != app.settings.llama_cpp_version_cuda
    } else {
        app.config.default.host != app.settings.host
            || app.config.default.port != app.settings.port
            || app.config.default.backend != app.settings.backend
            || app.config.default.parallel != app.settings.parallel
            || app.config.default.max_concurrent_predictions != app.settings.max_concurrent_predictions
            || app.config.default.threads != app.settings.threads
            || app.config.default.threads_batch != app.settings.threads_batch
            || app.config.default.api_endpoint_enabled != app.settings.api_endpoint_enabled
            || app.config.default.api_endpoint_port != app.settings.api_endpoint_port
            || app.config.default.api_endpoint_key != app.settings.api_endpoint_key
            || app.config.default.server_mode != app.server_mode
            || app.config.default.router_max_models != app.router_max_models
            || app.config.default.llama_cpp_version_cpu != app.settings.llama_cpp_version_cpu
            || app.config.default.llama_cpp_version_vulkan != app.settings.llama_cpp_version_vulkan
            || app.config.default.llama_cpp_version_rocm != app.settings.llama_cpp_version_rocm
            || app.config.default.llama_cpp_version_rocm_lemonade
                != app.settings.llama_cpp_version_rocm_lemonade
            || app.config.default.llama_cpp_version_cuda != app.settings.llama_cpp_version_cuda
    };

    if !changed {
        return;
    }

    if has_model {
        app.config.default.host = app.settings.host.clone();
        app.config.default.port = app.settings.port;
        app.config.default.backend = app.settings.backend;
        app.config.default.api_endpoint_enabled = app.settings.api_endpoint_enabled;
        app.config.default.api_endpoint_port = app.settings.api_endpoint_port;
        app.config.default.api_endpoint_key = app.settings.api_endpoint_key.clone();
        app.config.default.server_mode = app.server_mode;
        app.config.default.router_max_models = app.router_max_models;
        app.config.default.llama_cpp_version_cpu = app.settings.llama_cpp_version_cpu.clone();
        app.config.default.llama_cpp_version_vulkan = app.settings.llama_cpp_version_vulkan.clone();
        app.config.default.llama_cpp_version_rocm = app.settings.llama_cpp_version_rocm.clone();
        app.config.default.llama_cpp_version_rocm_lemonade =
            app.settings.llama_cpp_version_rocm_lemonade.clone();
        app.config.default.llama_cpp_version_cuda = app.settings.llama_cpp_version_cuda.clone();

        // Sync synced global fields to model_settings_cache to keep is_settings_dirty() correct
        app.model_settings_cache.host = app.settings.host.clone();
        app.model_settings_cache.port = app.settings.port;
        app.model_settings_cache.backend = app.settings.backend;
        app.model_settings_cache.api_endpoint_enabled = app.settings.api_endpoint_enabled;
        app.model_settings_cache.api_endpoint_port = app.settings.api_endpoint_port;
        app.model_settings_cache.api_endpoint_key = app.settings.api_endpoint_key.clone();
        app.model_settings_cache.llama_cpp_version_cpu = app.settings.llama_cpp_version_cpu.clone();
        app.model_settings_cache.llama_cpp_version_vulkan = app.settings.llama_cpp_version_vulkan.clone();
        app.model_settings_cache.llama_cpp_version_rocm = app.settings.llama_cpp_version_rocm.clone();
        app.model_settings_cache.llama_cpp_version_rocm_lemonade =
            app.settings.llama_cpp_version_rocm_lemonade.clone();
        app.model_settings_cache.llama_cpp_version_cuda = app.settings.llama_cpp_version_cuda.clone();
    } else {
        app.config.default.host = app.settings.host.clone();
        app.config.default.port = app.settings.port;
        app.config.default.backend = app.settings.backend;
        app.config.default.parallel = app.settings.parallel;
        app.config.default.max_concurrent_predictions = app.settings.max_concurrent_predictions;
        app.config.default.threads = app.settings.threads;
        app.config.default.threads_batch = app.settings.threads_batch;
        app.config.default.api_endpoint_enabled = app.settings.api_endpoint_enabled;
        app.config.default.api_endpoint_port = app.settings.api_endpoint_port;
        app.config.default.api_endpoint_key = app.settings.api_endpoint_key.clone();
        app.config.default.server_mode = app.server_mode;
        app.config.default.router_max_models = app.router_max_models;
        app.config.default.llama_cpp_version_cpu = app.settings.llama_cpp_version_cpu.clone();
        app.config.default.llama_cpp_version_vulkan = app.settings.llama_cpp_version_vulkan.clone();
        app.config.default.llama_cpp_version_rocm = app.settings.llama_cpp_version_rocm.clone();
        app.config.default.llama_cpp_version_rocm_lemonade =
            app.settings.llama_cpp_version_rocm_lemonade.clone();
        app.config.default.llama_cpp_version_cuda = app.settings.llama_cpp_version_cuda.clone();

        // Since no model is selected, sync all fields to model_settings_cache
        app.model_settings_cache = app.settings.clone();
    }

    if let Err(e) = app.config.save() {
        app.add_log(
            format!("Failed to save global settings: {}", e),
            crate::config::LogLevel::Error,
        );
    }
}

pub fn handle_fkey_focus(app: &mut App, panel_idx: u8, target_panel: ActivePanel) {
    if app.has_toasts() {
        app.dismiss_toast();
    }
    app.ui.panel_visibility |= 1 << panel_idx;
    app.ui.active_panel = target_panel;
}

pub fn handle_fkey_toggle(
    app: &mut App,
    panel_idx: u8,
    target_panel: Option<ActivePanel>,
    require_no_server: bool,
) {
    if require_no_server && app.server.server_handle.is_some() {
        return;
    }
    if app.has_toasts() {
        app.dismiss_toast();
    }
    app.toggle_panel_visibility(panel_idx);
    if app.is_panel_visible(panel_idx)
        && let Some(panel) = target_panel
    {
        app.ui.active_panel = panel;
    }
}

pub fn handle_fkey_show_all(app: &mut App) {
    app.ui.panel_visibility = 0b111111;
    app.log.log_expanded = false;
}
