use super::App;
use super::{Color, Span, Style};
use crate::tui::app::{ActivePanel, ModelsMode};

pub fn render_hints(app: &App) -> Vec<Span<'static>> {
    let y = Style::default().fg(Color::Yellow);
    let c = Style::default().fg(Color::Cyan);
    let r = Style::default().fg(Color::Red);

    // Backend picker has its own hint rendering
    if matches!(app.ui.global_mode, crate::tui::app::GlobalMode::BackendPicker { .. }) {
        return vec![
            Span::styled("Del", r),
            Span::raw("  "),
            Span::styled("⎋ Exit", c),
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
                Span::styled("⎋ Exit", c),
                Span::raw("  "),
                Span::styled("↵ Files", y),
                Span::raw("  "),
                Span::styled("-> Readme", y),
                Span::raw("  "),
                Span::styled("sort:", c),
                Span::styled(
                    sort_by.label(),
                    Style::default().fg(Color::Magenta),
                ),
            ];
            if *loading {
                parts.push(Span::raw("  "));
                parts.push(Span::styled(
                    "[loading]",
                    Style::default().fg(Color::Yellow),
                ));
            }
            parts
        }
        ModelsMode::Files { .. } => {
            vec![
                Span::styled("↵ Download", y),
                Span::raw("  "),
                Span::styled("⎋ Back", c),
            ]
        }
        ModelsMode::List => {
            if app.ui.active_panel == ActivePanel::LlmSettings {
                let mut parts = vec![
                    Span::styled("j/k nav", c),
                    Span::raw("  "),
                    Span::styled("^S Save", y),
                    Span::raw("  "),
                    Span::styled("^R Reset", y),
                    Span::raw("  "),
                    Span::styled("^E Toggle", y),
                    Span::raw("  "),
                ];
                if app.is_settings_dirty() {
                    parts.push(Span::raw("  "));
                    parts.push(Span::styled("*unsaved*", r));
                    parts.push(Span::raw("  "));
                }
                parts.push(Span::styled("^X eXpert", y));
                parts.push(Span::raw("  "));
                parts.push(Span::styled("^P Profiles", y));
                parts.push(Span::raw("  "));
                parts.push(Span::styled("⇥ Panels", c));
                parts.push(Span::raw("  "));
                parts.push(Span::styled("Shift+a About", c));
                parts
            } else {
                
                match app.ui.active_panel {
                    ActivePanel::Models => {
                        vec![
                            Span::styled("⇥ Panels", c),
                            Span::raw("  "),
                            Span::styled("f Filter", y),
                            Span::raw("  "),
                            Span::styled("l/load, u/unload", y),
                            Span::raw("  "),
                            Span::styled("^D Del", y),
                            Span::raw("  "),
                            Span::styled("^H Help", c),
                            Span::raw("  "),
                            Span::styled("Shift+a About", c),
                        ]
                    }
                    ActivePanel::Log => {
                        if app.log.log_expanded {
                            vec![
                                Span::styled("j/k Scroll", c),
                                Span::raw("  "),
                                Span::styled("f/follow", c),
                                Span::raw("  "),
                                Span::styled("⎋ Collapse", c),
                                Span::raw("  "),
                                Span::styled("g/G top/bottom", c),
                            ]
                        } else {
                            vec![
                                Span::styled("f/follow", c),
                                Span::raw("  "),
                                Span::styled("⎋ Collapse", c),
                                Span::raw("  "),
                                Span::styled("g/G top/bottom", c),
                                Span::raw("  "),
                                Span::styled("⇥ Panels", c),
                            ]
                        }
                    }
                    ActivePanel::ServerSettings => {
                        vec![
                            Span::styled("j/k nav", c),
                            Span::raw("  "),
                            Span::styled("↵ Toggle", y),
                            Span::raw("  "),
                            Span::styled("⇥ Panels", c),
                            Span::raw("  "),
                            Span::styled("Shift+a About", c),
                        ]
                    }
                    ActivePanel::Profiles => {
                        vec![
                            Span::styled("j/k nav", c),
                            Span::raw("  "),
                            Span::styled("↵ Apply", y),
                            Span::raw("  "),
                            Span::styled("s Save", c),
                            Span::raw("  "),
                            Span::styled("⎋ Done", c),
                            Span::raw("  "),
                            Span::styled("⇥ Panels", c),
                            Span::raw("  "),
                            Span::styled("Shift+a About", c),
                        ]
                    }
                    ActivePanel::SystemPromptPresets => {
                        vec![
                            Span::styled("j/k nav", c),
                            Span::raw("  "),
                            Span::styled("↵ Apply", y),
                            Span::raw("  "),
                            Span::styled("e Edit", c),
                            Span::raw("  "),
                            Span::styled("n New", c),
                            Span::raw("  "),
                            Span::styled("⎋ Done", c),
                            Span::raw("  "),
                            Span::styled("⇥ Panels", c),
                            Span::raw("  "),
                            Span::styled("Shift+a About", c),
                        ]
                    }
                    ActivePanel::SearchReadme => {
                        vec![
                            Span::styled("j/k nav", c),
                            Span::raw("  "),
                            Span::styled("⎋ Collapse", c),
                            Span::raw("  "),
                            Span::styled("⇥ Panels", c),
                            Span::raw("  "),
                            Span::styled("Shift+a About", c),
                        ]
                    }
                    ActivePanel::Downloads => {
                        vec![
                            Span::styled("j/k nav", c),
                            Span::raw("  "),
                            Span::styled("p Pause", y),
                            Span::raw("  "),
                            Span::styled("Alt+C Cancel", y),
                            Span::raw("  "),
                            Span::styled("⇥ Panels", c),
                        ]
                    }
                    _ => {
                        vec![
                            Span::styled("⇥ Panels", c),
                            Span::raw("  "),
                            Span::styled("f Filter", y),
                            Span::raw("  "),
                            Span::styled("^H Help", c),
                            Span::raw("  "),
                            Span::styled("Shift+a About", c),
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
                    Span::styled("⎋ Stop", r),
                    Span::raw("  "),
                    Span::styled("⇥ Panels", c),
                ]
            } else if !app.bench_tune.bench_tune_results.is_empty() {
                vec![
                    Span::styled("↵ View output", y),
                    Span::raw("  "),
                    Span::styled("⎋ Stop", r),
                    Span::raw("  "),
                    Span::styled("⇥ Panels", c),
                ]
            } else {
                vec![
                    Span::styled("⎋ Stop", r),
                    Span::raw("  "),
                    Span::styled("⇥ Panels", c),
                ]
            }
        }
    }
}
