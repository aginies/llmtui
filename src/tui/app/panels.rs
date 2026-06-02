use super::types::{ActivePanel, App, ModelsMode};

impl App {
    /// Check if a panel is visible.
    pub fn is_panel_visible(&self, index: u8) -> bool {
        self.ui.panel_visibility & (1 << index) != 0
    }

    /// Toggle visibility of a panel.
    pub fn toggle_panel_visibility(&mut self, index: u8) {
        self.ui.panel_visibility ^= 1 << index;
        // If hiding the log while expanded, collapse it.
        if index == 5 && !self.is_panel_visible(5) {
            self.log.log_expanded = false;
        }
    }

    /// Return a list of all currently visible and focusable panels in logical order.
    pub fn get_visible_panels(&self) -> Vec<ActivePanel> {
        let mut visible = Vec::new();

        // 1. Models (Left Top)
        if self.is_panel_visible(0) {
            visible.push(ActivePanel::Models);
        }

        // 3. Right Panel (README / Settings / Profiles / Presets)
        let is_search = matches!(self.models_mode, ModelsMode::Search { .. });
        let is_files = matches!(self.models_mode, ModelsMode::Files { .. });
        let is_bench_tune = matches!(self.models_mode, ModelsMode::BenchTune);
        let show_readme = match &self.models_mode {
            ModelsMode::Search { show_readme, .. } => *show_readme,
            ModelsMode::Files { .. } => true,
            _ => false,
        };

        if self.ui.active_panel == ActivePanel::Profiles {
            visible.push(ActivePanel::Profiles);
        } else if self.ui.active_panel == ActivePanel::SystemPromptPresets {
            visible.push(ActivePanel::SystemPromptPresets);
        } else if show_readme && (is_search || is_files) {
            visible.push(ActivePanel::SearchReadme);
        } else {
            if self.is_panel_visible(1) && self.server.server_handle.is_none() && !is_bench_tune {
                visible.push(ActivePanel::ServerSettings);
            }
            if self.is_panel_visible(3) {
                visible.push(ActivePanel::LlmSettings);
            }
        }

        // 4. Active Model (Bottom Middle) — read-only, not focusable

        // 5. Log (Bottom)
        if self.is_panel_visible(5) {
            visible.push(ActivePanel::Log);
        }

        // 6. Downloads (Bottom, shown when downloading)
        if self.download.downloading {
            visible.push(ActivePanel::Downloads);
        }

        visible
    }

    pub fn focus_next(&mut self) {
        let visible = self.get_visible_panels();
        if visible.is_empty() {
            return;
        }

        let current_idx = visible
            .iter()
            .position(|&p| p == self.ui.active_panel)
            .unwrap_or(0);
        let next_idx = (current_idx + 1) % visible.len();
        self.ui.active_panel = visible[next_idx];
    }

    pub fn focus_prev(&mut self) {
        let visible = self.get_visible_panels();
        if visible.is_empty() {
            return;
        }

        let current_idx = visible
            .iter()
            .position(|&p| p == self.ui.active_panel)
            .unwrap_or(0);
        let prev_idx = (current_idx + visible.len() - 1) % visible.len();
        self.ui.active_panel = visible[prev_idx];
    }
}
