use crate::tui::app::pending_events::PendingEvent;
use crate::tui::app::{ActivePanel, App, ConfirmationKind};

pub fn picker_nav_up(selected: &mut usize) {
    *selected = selected.saturating_sub(1);
}
pub fn picker_nav_down(selected: &mut usize, len: usize) {
    *selected = (*selected + 1).min(len.saturating_sub(1));
}

pub fn wrap_field_picker(selected: &mut i32, first: i32, last: i32, up: bool) {
    if up {
        if *selected <= first {
            *selected = last;
        } else {
            *selected -= 1;
        }
    } else {
        if *selected >= last {
            *selected = first;
        } else {
            *selected += 1;
        }
    }
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
                let parts: Vec<&str> = path.splitn(3, ':').collect();
                if let (Some(backend_str), Some(tag)) = (parts.first(), parts.get(1)) {
                    let backend = match crate::models::Backend::from_slug(backend_str) {
                        Some(b) => b,
                        None => {
                            app.add_log(
                                crate::t_fmt!("backend.delete_unknown", backend_str),
                                crate::config::LogLevel::Warning,
                            );
                            return;
                        }
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

    if !common_fields_changed(&app.config.default, &app.settings)
        && app.config.default.server_mode == app.server_mode
        && app.config.default.router_max_models == app.router_max_models
    {
        return;
    }

    sync_config_common_fields(&mut app.config.default, &app.settings);
    app.config.default.server_mode = app.server_mode;
    app.config.default.router_max_models = app.router_max_models;

    if has_model {
        if let Some(model) = app.selected_model() {
            let name = model.display_name.clone();
            app.config.model_overrides.update_fields(
                &name,
                Some(app.settings.webui),
                Some(app.settings.cache_prompt),
            );
        }
        sync_model_cache_fields(&mut app.model_settings_cache, &app.settings);
    } else {
        app.config.default.parallel = app.settings.parallel;
        app.config.default.max_concurrent_predictions = app.settings.max_concurrent_predictions;
        app.config.default.threads = app.settings.threads;
        app.config.default.threads_batch = app.settings.threads_batch;
        app.model_settings_cache = app.settings.clone();
    }

    if let Err(e) = app.config.save() {
        app.add_log(
            format!("Failed to save global settings: {}", e),
            crate::config::LogLevel::Error,
        );
    }
}

fn common_fields_changed(
    source: &crate::config::DefaultParams,
    target: &crate::models::ModelSettings,
) -> bool {
    source.host != target.host
        || source.port != target.port
        || source.backend != target.backend
        || source.api_endpoint_enabled != target.api_endpoint_enabled
        || source.api_endpoint_port != target.api_endpoint_port
        || source.api_endpoint_key != target.api_endpoint_key
        || source.llama_cpp_version_cpu != target.llama_cpp_version_cpu
        || source.llama_cpp_version_vulkan != target.llama_cpp_version_vulkan
        || source.llama_cpp_version_rocm != target.llama_cpp_version_rocm
        || source.llama_cpp_version_rocm_lemonade != target.llama_cpp_version_rocm_lemonade
        || source.llama_cpp_version_cuda != target.llama_cpp_version_cuda
        || source.webui != target.webui
        || source.cache_prompt != target.cache_prompt
}

fn sync_config_common_fields(
    target: &mut crate::config::DefaultParams,
    source: &crate::models::ModelSettings,
) {
    target.host = source.host.clone();
    target.port = source.port;
    target.backend = source.backend;
    target.api_endpoint_enabled = source.api_endpoint_enabled;
    target.api_endpoint_port = source.api_endpoint_port;
    target.api_endpoint_key = source.api_endpoint_key.clone();
    target.llama_cpp_version_cpu = source.llama_cpp_version_cpu.clone();
    target.llama_cpp_version_vulkan = source.llama_cpp_version_vulkan.clone();
    target.llama_cpp_version_rocm = source.llama_cpp_version_rocm.clone();
    target.llama_cpp_version_rocm_lemonade = source.llama_cpp_version_rocm_lemonade.clone();
    target.llama_cpp_version_cuda = source.llama_cpp_version_cuda.clone();
    target.webui = source.webui;
    target.cache_prompt = source.cache_prompt;
}

fn sync_model_cache_fields(
    target: &mut crate::models::ModelSettings,
    source: &crate::models::ModelSettings,
) {
    target.host = source.host.clone();
    target.port = source.port;
    target.backend = source.backend;
    target.api_endpoint_enabled = source.api_endpoint_enabled;
    target.api_endpoint_port = source.api_endpoint_port;
    target.api_endpoint_key = source.api_endpoint_key.clone();
    target.llama_cpp_version_cpu = source.llama_cpp_version_cpu.clone();
    target.llama_cpp_version_vulkan = source.llama_cpp_version_vulkan.clone();
    target.llama_cpp_version_rocm = source.llama_cpp_version_rocm.clone();
    target.llama_cpp_version_rocm_lemonade = source.llama_cpp_version_rocm_lemonade.clone();
    target.llama_cpp_version_cuda = source.llama_cpp_version_cuda.clone();
    target.webui = source.webui;
    target.cache_prompt = source.cache_prompt;
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
