use std::future::Future;
use std::pin::Pin;

use crossterm::event::{KeyCode, KeyEvent};

use super::super::helpers::{picker_nav_down, picker_nav_up};
use crate::tui::app::{App, GlobalMode};

use super::OverlayHandler;

pub struct GpuDevicePickerHandler;

/// Apply the picker's selection to `app.settings.devices` and close the picker.
///
/// Entry 0 is the special "(default) — all devices" option and is never
/// stored; an empty `devices` list means "use all detected devices".
fn confirm(app: &mut App) {
    if let GlobalMode::GpuDevicePicker {
        checked, entries, ..
    } = &app.ui.global_mode
    {
        let mut devices = Vec::new();
        for (i, (entry, is_checked)) in entries.iter().zip(checked.iter()).enumerate() {
            if i == 0 {
                continue;
            }
            if *is_checked {
                devices.push(entry.0.clone());
            }
        }
        app.settings.devices = devices;
    }
    app.ui.global_mode = GlobalMode::Normal;
    app.picker.gpu_device_picker_scroll_offset = 0;
}

impl OverlayHandler for GpuDevicePickerHandler {
    fn can_handle(&self, mode: &GlobalMode) -> bool {
        matches!(mode, GlobalMode::GpuDevicePicker { .. })
    }

    fn handle<'a>(
        &'a self,
        app: &'a mut App,
        key: KeyEvent,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            if !matches!(app.ui.global_mode, GlobalMode::GpuDevicePicker { .. }) {
                return;
            }
            match key.code {
                KeyCode::Esc => {
                    app.ui.global_mode = GlobalMode::Normal;
                    app.picker.gpu_device_picker_scroll_offset = 0;
                }
                KeyCode::Enter => {
                    confirm(app);
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if let GlobalMode::GpuDevicePicker { selected, .. } = &mut app.ui.global_mode {
                        picker_nav_up(selected);
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if let GlobalMode::GpuDevicePicker {
                        selected, entries, ..
                    } = &mut app.ui.global_mode
                    {
                        picker_nav_down(selected, entries.len());
                    }
                }
                KeyCode::PageUp => {
                    app.picker.gpu_device_picker_scroll_offset =
                        app.picker.gpu_device_picker_scroll_offset.saturating_sub(5);
                }
                KeyCode::PageDown => {
                    app.picker.gpu_device_picker_scroll_offset =
                        app.picker.gpu_device_picker_scroll_offset.saturating_add(5);
                }
                // Space toggles the item under the cursor. Toggling the default
                // entry (index 0) clears the whole selection.
                KeyCode::Char(' ') => {
                    let idx =
                        if let GlobalMode::GpuDevicePicker { selected, .. } = &app.ui.global_mode {
                            *selected
                        } else {
                            return;
                        };
                    if let GlobalMode::GpuDevicePicker { checked, .. } = &mut app.ui.global_mode {
                        if idx == 0 {
                            for c in checked.iter_mut() {
                                *c = false;
                            }
                        } else if let Some(c) = checked.get_mut(idx) {
                            *c = !*c;
                        }
                    }
                }
                _ => {}
            }
        })
    }
}
