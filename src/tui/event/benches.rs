use crossterm::event::KeyCode;

use crate::tui::app::App;

pub fn handle_rpc_workers_key(app: &mut App, key: crossterm::event::KeyEvent) {
    let editing = app.picker.editing_rpc_worker.is_some();
    
    if editing {
        match key.code {
            KeyCode::Enter => {
                if !app.settings_state.settings_edit_buffer.is_empty() {
                    // Parse: [Name], IP, Port
                    let parts: Vec<&str> = app.settings_state.settings_edit_buffer.split(',').map(|s: &str| s.trim()).collect();
                    let (name, ip_str, port_str) = match parts.len() {
                        1 => ("".to_string(), parts[0].to_string(), "50052".to_string()),
                        2 => {
                            // Check if second part is a port
                            if parts[1].parse::<u16>().is_ok() {
                                ("".to_string(), parts[0].to_string(), parts[1].to_string())
                            } else {
                                (parts[0].to_string(), parts[1].to_string(), "50052".to_string())
                            }
                        }
                        3 => (parts[0].to_string(), parts[1].to_string(), parts[2].to_string()),
                        _ => ("".to_string(), "".to_string(), "0".to_string()),
                    };

                    // Validate IP
                    let is_valid_ip = ip_str.parse::<std::net::IpAddr>().is_ok();
                    // Validate Port (Unix range 1-65535, though 0 is technically reserved)
                    let port = port_str.parse::<u32>().unwrap_or(0);
                    let is_valid_port = port > 0 && port <= 65535;

                    if !is_valid_ip {
                        app.add_log(format!("Invalid IP address: {}", ip_str), crate::config::LogLevel::Error);
                    } else if !is_valid_port {
                        app.add_log(format!("Invalid port (1-65535): {}", port_str), crate::config::LogLevel::Error);
                    } else {
                        let worker = crate::config::RpcWorker {
                            selected: true,
                            name,
                            ip: ip_str,
                            port: port as u16,
                        };
                        
                        if let Some(idx) = app.picker.editing_rpc_worker {
                            if idx < app.config.rpc_workers.len() {
                                app.config.rpc_workers[idx] = worker;
                            } else {
                                app.config.rpc_workers.push(worker);
                            }
                        }
                        let _ = app.config.save();
                        app.add_log("RPC worker saved.", crate::config::LogLevel::Info);
                    }
                }
                app.picker.editing_rpc_worker = None;
                app.settings_state.settings_edit_buffer.clear();
                app.edit.edit_cursor_pos = 0;
            }
            KeyCode::Esc => {
                app.picker.editing_rpc_worker = None;
                app.settings_state.settings_edit_buffer.clear();
                app.edit.edit_cursor_pos = 0;
            }
            KeyCode::Char(c) => {
                let byte_idx = app.settings_state.settings_edit_buffer.char_indices().nth(app.edit.edit_cursor_pos).map(|(i, _)| i).unwrap_or(app.settings_state.settings_edit_buffer.len());
                app.settings_state.settings_edit_buffer.insert(byte_idx, c);
                app.edit.edit_cursor_pos += 1;
            }
            KeyCode::Backspace => {
                if app.edit.edit_cursor_pos > 0 {
                    app.edit.edit_cursor_pos -= 1;
                    let byte_idx = app.settings_state.settings_edit_buffer.char_indices().nth(app.edit.edit_cursor_pos).map(|(i, _)| i).unwrap_or(0);
                    app.settings_state.settings_edit_buffer.remove(byte_idx);
                }
            }
            KeyCode::Delete => {
                if app.edit.edit_cursor_pos < app.settings_state.settings_edit_buffer.chars().count() {
                    let byte_idx = app.settings_state.settings_edit_buffer.char_indices().nth(app.edit.edit_cursor_pos).map(|(i, _)| i).unwrap_or(app.settings_state.settings_edit_buffer.len());
                    app.settings_state.settings_edit_buffer.remove(byte_idx);
                }
            }
            KeyCode::Left => {
                app.edit.edit_cursor_pos = app.edit.edit_cursor_pos.saturating_sub(1);
            }
            KeyCode::Right => {
                app.edit.edit_cursor_pos = (app.edit.edit_cursor_pos + 1).min(app.settings_state.settings_edit_buffer.chars().count());
            }
            _ => {}
        }
    } else {
        match key.code {
            KeyCode::Esc => {
                app.ui.global_mode = crate::tui::app::GlobalMode::Normal;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                app.picker.rpc_workers_selected_idx = app.picker.rpc_workers_selected_idx.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let total = app.config.rpc_workers.len();
                if total > 0 {
                    app.picker.rpc_workers_selected_idx = (app.picker.rpc_workers_selected_idx + 1).min(total - 1);
                }
            }
            KeyCode::Char(' ') => {
                if let Some(worker) = app.config.rpc_workers.get_mut(app.picker.rpc_workers_selected_idx) {
                    worker.selected = !worker.selected;
                    let _ = app.config.save();
                }
            }
            KeyCode::Char('n') => {
                app.picker.editing_rpc_worker = Some(app.config.rpc_workers.len());
                app.settings_state.settings_edit_buffer.clear();
                app.edit.edit_cursor_pos = 0;
            }
            KeyCode::Char('e') => {
                if let Some(worker) = app.config.rpc_workers.get(app.picker.rpc_workers_selected_idx) {
                    app.picker.editing_rpc_worker = Some(app.picker.rpc_workers_selected_idx);
                    app.settings_state.settings_edit_buffer = format!("{}, {}, {}", worker.name, worker.ip, worker.port);
                    app.edit.edit_cursor_pos = app.settings_state.settings_edit_buffer.len();
                }
            }
            KeyCode::Char('d') => {
                if !app.config.rpc_workers.is_empty() {
                    app.config.rpc_workers.remove(app.picker.rpc_workers_selected_idx);
                    if app.picker.rpc_workers_selected_idx >= app.config.rpc_workers.len() && !app.config.rpc_workers.is_empty() {
                        app.picker.rpc_workers_selected_idx = app.config.rpc_workers.len() - 1;
                    }
                    let _ = app.config.save();
                }
            }
            _ => {}
        }
    }
}
