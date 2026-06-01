use super::App;
use super::{Color, Modifier, Span, Style, Line};
use ratatui::layout::Rect;
use crate::tui::app::{ActivePanel, GlobalMode, ModelsMode};
use crate::tui::render::hints::render_hints;

pub fn render_status_bar<'a>(app: &'a App, panel_area: Rect) -> Line<'a> {
    let mut parts = Vec::new();

    let mode_name = match &app.models_mode {
        ModelsMode::List => "List".to_string(),
        ModelsMode::Search { results, .. } => format!("Search({} results)", results.len()),
        ModelsMode::Files { files, .. } => format!("Files({} files)", files.len()),
        ModelsMode::BenchTune => "BenchTune".to_string(),
    };
    parts.push(Span::styled(format!("[Mode: {}] ", mode_name), Style::default().fg(Color::DarkGray)));

    if app.settings_state.expert_mode {
        parts.push(Span::styled("[EXPERT] ", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)));
    }

    if let Some(handle) = &app.server.server_handle {
        let label = if app.server_mode == crate::models::ServerMode::Bench {
            "BENCHMARKING".to_string()
        } else {
            format!("{} {}", handle.port, app.server_mode)
        };
        parts.push(Span::styled(format!("● {}", label), Style::default().fg(Color::Green)));
    } else if app.server_mode == crate::models::ServerMode::BenchTune {
        if let Some(progress) = &app.bench_tune.bench_tune_progress {
            match progress {
                crate::models::BenchTuneProgress::Running { current, total, progress, current_params: _ } => {
                    let progress_str = format!("BENCH TUNE {}/{} ({:.0}%)", current, total, progress);
                    parts.push(Span::styled(format!("● {}", progress_str), Style::default().fg(Color::Yellow)));
                }
                crate::models::BenchTuneProgress::Completed { total_tests, successful_tests, elapsed } => {
                    let elapsed_str = format!("{}s", elapsed.as_secs());
                    let progress_str = format!("BENCH TUNE COMPLETED ({}/{}) in {}", total_tests, successful_tests, elapsed_str);
                    parts.push(Span::styled(format!("● {}", progress_str), Style::default().fg(Color::Green)));
                }
                crate::models::BenchTuneProgress::PartiallyCompleted { total_tests, successful_tests, failed_tests, elapsed } => {
                    let elapsed_str = format!("{}s", elapsed.as_secs());
                    let progress_str = format!("BENCH TUNE PARTIALLY COMPLETED ({}/{}, {} failed) in {}", total_tests, successful_tests, failed_tests, elapsed_str);
                    parts.push(Span::styled(format!("● {}", progress_str), Style::default().fg(Color::Yellow)));
                }
                crate::models::BenchTuneProgress::Cancelled { total_tests, successful_tests, failed_tests, elapsed } => {
                    let elapsed_str = format!("{}s", elapsed.as_secs());
                    let progress_str = format!("BENCH TUNE CANCELLED ({}/{}, {} failed) in {}", total_tests, successful_tests, failed_tests, elapsed_str);
                    parts.push(Span::styled(format!("● {}", progress_str), Style::default().fg(Color::Yellow)));
                }
                crate::models::BenchTuneProgress::Error { error } => {
                    parts.push(Span::styled(format!("● BENCH TUNE ERROR: {}", error), Style::default().fg(Color::Red)));
                }
            }
        } else {
            parts.push(Span::styled("● BENCH TUNE READY", Style::default().fg(Color::Yellow)));
        }
    } else {
        parts.push(Span::styled("○ Server", Style::default().fg(Color::DarkGray)));
    }

    if matches!(app.ui.global_mode, GlobalMode::HostPicker { .. }) {
        parts.push(Span::raw("  "));
        parts.push(Span::styled("[HOST PICKER]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
    }
    if matches!(app.ui.global_mode, GlobalMode::RpcManager) {
        parts.push(Span::raw("  "));
        parts.push(Span::styled("[RPC MANAGER]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
    }
    if matches!(app.ui.global_mode, GlobalMode::About) {
        parts.push(Span::raw("  "));
        parts.push(Span::styled("[ABOUT]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
    }
    if let GlobalMode::BenchTuneSetup { editing_prompt, .. } = &app.ui.global_mode {
        parts.push(Span::raw("  "));
        if *editing_prompt {
            parts.push(Span::styled("[EDITING PROMPT]", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));
        } else {
            parts.push(Span::styled("[BENCH SETUP]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
        }
    }

    match &app.models_mode {
        ModelsMode::Search { query: _, sort_by, .. } => {
            parts.push(Span::raw("  "));
            if app.ui.active_panel == ActivePanel::Models {
                parts.push(Span::styled("SEARCH", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
            } else {
                let panel_label = match app.ui.active_panel {
                    ActivePanel::Log => "LOG",
                    ActivePanel::ServerSettings => "SERVER",
                    ActivePanel::LlmSettings => "LLM",
                    ActivePanel::Profiles => "PROFILES",
                    ActivePanel::SystemPromptPresets => "PROMPTS",
                    ActivePanel::SearchReadme => "README",
                    _ => "SEARCH",
                };
                parts.push(Span::styled(panel_label, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
            }
            parts.push(Span::raw(" "));
            parts.push(Span::styled(sort_by.label(), Style::default().fg(Color::Magenta)));
        }
        ModelsMode::Files { model_id, .. } => {
            parts.push(Span::raw("  "));
            if app.ui.active_panel == ActivePanel::Models {
                parts.push(Span::styled("FILES", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
            } else {
                let panel_label = match app.ui.active_panel {
                    ActivePanel::Log => "LOG",
                    ActivePanel::ServerSettings => "SERVER",
                    ActivePanel::LlmSettings => "LLM",
                    ActivePanel::Profiles => "PROFILES",
                    ActivePanel::SystemPromptPresets => "PROMPTS",
                    ActivePanel::SearchReadme => "README",
                    _ => "FILES",
                };
                parts.push(Span::styled(panel_label, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
            }
            parts.push(Span::raw(" "));
            parts.push(Span::styled(model_id, Style::default().fg(Color::Cyan)));
        }
        ModelsMode::List => {
            parts.push(Span::raw("  "));
            let panel_label = match app.ui.active_panel {
                ActivePanel::Models => "MODELS",
                ActivePanel::Log => "LOG",
                ActivePanel::ServerSettings => "SERVER",
                ActivePanel::LlmSettings => "LLM",
                ActivePanel::Profiles => "PROFILES",
                ActivePanel::SystemPromptPresets => "PROMPTS",
                ActivePanel::SearchReadme => "README",
                ActivePanel::Downloads => "DOWNLOADS",
                _ => "APP",
            };
            parts.push(Span::styled(panel_label, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
        }
        ModelsMode::BenchTune => {
            parts.push(Span::raw("  "));
            parts.push(Span::styled("BENCHTUNE", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
        }
    }

 

    let left_width: usize = parts.iter().map(|s| s.width()).sum();
    let remaining = panel_area.width.saturating_sub(left_width as u16) as usize;

    let hints = render_hints(app);
    let hints_width: usize = hints.iter().map(|s| s.width()).sum();
    let padding = remaining.saturating_sub(hints_width).max(0);

    if padding > 0 {
        parts.push(Span::raw(" ".repeat(padding)));
    }
    parts.extend(hints);

    Line::from(parts)
}
