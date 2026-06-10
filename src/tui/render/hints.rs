use super::App;
use super::{Color, Span, Style};
use crate::tui::app::{ActivePanel, ModelsMode};

fn hint_nav() -> &'static str {
    crate::t!("hints.nav")
}
fn hint_panels() -> &'static str {
    crate::t!("hints.panels")
}
fn hint_about() -> &'static str {
    crate::t!("hints.about")
}
const HINT_SEP: &str = "  ";

pub fn render_hints(app: &App) -> Vec<Span<'static>> {
    let y = Style::default().fg(Color::Yellow);
    let c = Style::default().fg(Color::Cyan);
    let r = Style::default().fg(Color::Red);

    // Backend picker has its own hint rendering
    if matches!(
        app.ui.global_mode,
        crate::tui::app::GlobalMode::BackendPicker { .. }
    ) {
        return vec![
            Span::styled(crate::t!("hints.del"), r),
            Span::raw(HINT_SEP),
            Span::styled(crate::t!("hints.exit"), c),
        ];
    }

    match &app.models_mode {
        ModelsMode::Search {
            sort_by,
            show_readme: _,
            loading,
            ..
        } => {
            let mut parts = vec![
                Span::styled(crate::t!("hints.exit"), c),
                Span::raw(HINT_SEP),
                Span::styled(crate::t!("hints.files"), y),
                Span::raw(HINT_SEP),
                Span::styled(crate::t!("hints.readme"), y),
                Span::raw(HINT_SEP),
                Span::styled("sort:", c),
                Span::styled(sort_by.label(), Style::default().fg(Color::Magenta)),
            ];
            if *loading {
                parts.push(Span::raw(HINT_SEP));
                parts.push(Span::styled(
                    crate::t!("hints.loading"),
                    Style::default().fg(Color::Yellow),
                ));
            }
            parts
        }
        ModelsMode::Files { .. } => {
            vec![
                Span::styled(crate::t!("hints.gguf"), c),
                Span::raw(HINT_SEP),
                Span::styled(crate::t!("hints.download"), y),
                Span::raw(HINT_SEP),
                Span::styled(crate::t!("hints.back"), c),
            ]
        }
        ModelsMode::List { sort_by: _ } => {
            if app.ui.active_panel == ActivePanel::LlmSettings {
                let mut parts = vec![
                    Span::styled(hint_nav(), c),
                    Span::raw(HINT_SEP),
                    Span::styled(crate::t!("hints.save"), y),
                    Span::raw(HINT_SEP),
                    Span::styled(crate::t!("hints.reset"), y),
                    Span::raw(HINT_SEP),
                    Span::styled(crate::t!("hints.toggle"), y),
                    Span::raw(HINT_SEP),
                    Span::styled(crate::t!("hints.toggle_field"), y),
                    Span::raw(HINT_SEP),
                ];
                parts.push(Span::styled(crate::t!("hints.expert"), y));
                parts.push(Span::raw(HINT_SEP));
                parts.push(Span::styled(crate::t!("hints.profiles"), y));
                parts.push(Span::raw(HINT_SEP));
                parts.push(Span::styled(hint_panels(), c));
                parts.push(Span::raw(HINT_SEP));
                parts.push(Span::styled(hint_about(), c));
                parts
            } else {
                match app.ui.active_panel {
                    ActivePanel::Models => {
                        let sort_label = match &app.models_mode {
                            ModelsMode::List { sort_by } => sort_by.label(),
                            _ => String::new(),
                        };
                        let parts = vec![
                            Span::styled(crate::t!("hints.gguf"), c),
                            Span::raw(HINT_SEP),
                            Span::styled(hint_panels(), c),
                            Span::raw(HINT_SEP),
                            Span::styled(crate::t!("hints.filter"), y),
                            Span::raw(HINT_SEP),
                            Span::styled(crate::t!("hints.load_unload"), y),
                            Span::raw(HINT_SEP),
                            Span::styled(crate::t!("hints.delete"), y),
                            Span::raw(HINT_SEP),
                            Span::styled("sort:", c),
                            Span::styled(sort_label, Style::default().fg(Color::Magenta)),
                            Span::raw(HINT_SEP),
                            Span::styled(crate::t!("hints.help"), c),
                            Span::raw(HINT_SEP),
                            Span::styled(hint_about(), c),
                        ];
                        parts
                    }
                    ActivePanel::Log => {
                        if app.log.log_expanded {
                            vec![
                                Span::styled(crate::t!("hints.scroll"), c),
                                Span::raw(HINT_SEP),
                                Span::styled(crate::t!("hints.follow"), c),
                                Span::raw(HINT_SEP),
                                Span::styled(crate::t!("hints.collapse"), c),
                                Span::raw(HINT_SEP),
                                Span::styled(crate::t!("hints.top_bottom"), c),
                            ]
                        } else {
                            vec![
                                Span::styled(crate::t!("hints.follow"), c),
                                Span::raw(HINT_SEP),
                                Span::styled(crate::t!("hints.collapse"), c),
                                Span::raw(HINT_SEP),
                                Span::styled(crate::t!("hints.top_bottom"), c),
                                Span::raw(HINT_SEP),
                                Span::styled(hint_panels(), c),
                            ]
                        }
                    }
                    ActivePanel::ServerSettings => {
                        vec![
                            Span::styled(hint_nav(), c),
                            Span::raw(HINT_SEP),
                            Span::styled(crate::t!("hints.toggle"), y),
                            Span::raw(HINT_SEP),
                            Span::styled(hint_panels(), c),
                            Span::raw(HINT_SEP),
                            Span::styled(hint_about(), c),
                        ]
                    }
                    ActivePanel::Profiles => {
                        vec![
                            Span::styled(hint_nav(), c),
                            Span::raw(HINT_SEP),
                            Span::styled(crate::t!("hints.apply"), y),
                            Span::raw(HINT_SEP),
                            Span::styled(crate::t!("hints.save_profile"), c),
                            Span::raw(HINT_SEP),
                            Span::styled(crate::t!("hints.done"), c),
                            Span::raw(HINT_SEP),
                            Span::styled(hint_panels(), c),
                            Span::raw(HINT_SEP),
                            Span::styled(hint_about(), c),
                        ]
                    }
                    ActivePanel::SystemPromptPresets => {
                        vec![
                            Span::styled(hint_nav(), c),
                            Span::raw(HINT_SEP),
                            Span::styled(crate::t!("hints.apply"), y),
                            Span::raw(HINT_SEP),
                            Span::styled(crate::t!("hints.edit"), c),
                            Span::raw(HINT_SEP),
                            Span::styled(crate::t!("hints.new"), c),
                            Span::raw(HINT_SEP),
                            Span::styled(crate::t!("hints.done"), c),
                            Span::raw(HINT_SEP),
                            Span::styled(hint_panels(), c),
                            Span::raw(HINT_SEP),
                            Span::styled(hint_about(), c),
                        ]
                    }
                    ActivePanel::SearchReadme => {
                        vec![
                            Span::styled(hint_nav(), c),
                            Span::raw(HINT_SEP),
                            Span::styled(crate::t!("hints.collapse"), c),
                            Span::raw(HINT_SEP),
                            Span::styled(hint_panels(), c),
                            Span::raw(HINT_SEP),
                            Span::styled(hint_about(), c),
                        ]
                    }
                    ActivePanel::Downloads => {
                        vec![
                            Span::styled(hint_nav(), c),
                            Span::raw(HINT_SEP),
                            Span::styled(crate::t!("hints.pause"), y),
                            Span::raw(HINT_SEP),
                            Span::styled(crate::t!("hints.cancel_download"), y),
                            Span::raw(HINT_SEP),
                            Span::styled(hint_panels(), c),
                        ]
                    }
                    _ => {
                        vec![
                            Span::styled(hint_panels(), c),
                            Span::raw(HINT_SEP),
                            Span::styled(crate::t!("hints.filter"), y),
                            Span::raw(HINT_SEP),
                            Span::styled(crate::t!("hints.help"), c),
                            Span::raw(HINT_SEP),
                            Span::styled(hint_about(), c),
                        ]
                    }
                }
            }
        }
        ModelsMode::BenchTune => {
            if app.bench_tune.bench_tune_progress.is_some()
                && matches!(
                    app.bench_tune.bench_tune_progress.as_ref().unwrap(),
                    crate::models::BenchTuneProgress::Running { .. }
                )
            {
                vec![
                    Span::styled(crate::t!("hints.stop"), r),
                    Span::raw(HINT_SEP),
                    Span::styled(hint_panels(), c),
                ]
            } else if !app.bench_tune.bench_tune_results.is_empty() {
                vec![
                    Span::styled(crate::t!("hints.view_output"), y),
                    Span::raw(HINT_SEP),
                    Span::styled(crate::t!("hints.stop"), r),
                    Span::raw(HINT_SEP),
                    Span::styled(hint_panels(), c),
                ]
            } else {
                vec![
                    Span::styled(crate::t!("hints.stop"), r),
                    Span::raw(HINT_SEP),
                    Span::styled(hint_panels(), c),
                ]
            }
        }
    }
}
