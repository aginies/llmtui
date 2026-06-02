use super::App;
use super::{Color, Span, Style};
use crate::tui::app::{ActivePanel, ModelsMode};

pub fn render_hints(app: &App) -> Vec<Span<'static>> {
    let y = Style::default().fg(Color::Yellow);
    let c = Style::default().fg(Color::Cyan);
    let r = Style::default().fg(Color::Red);

    match &app.models_mode {
        ModelsMode::Search {
            sort_by,
            show_readme: _,
            loading,
            ..
        } => {
            let mut parts = vec![
                Span::styled("⎋ exit", c),
                Span::raw("  "),
                Span::styled("↵ files", y),
                Span::raw("  "),
                Span::styled("-> readme", y),
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
                Span::styled("↵ download", y),
                Span::raw("  "),
                Span::styled("⎋ back", c),
            ]
        }
        ModelsMode::List => {
            if app.ui.active_panel == ActivePanel::LlmSettings {
                let mut parts = vec![
                    Span::styled("j/k nav", c),
                    Span::raw("  "),
                    Span::styled("^S save", y),
                    Span::raw("  "),
                    Span::styled("^R reset", y),
                    Span::raw("  "),
                    Span::styled("^E toggle", y),
                    Span::raw("  "),
                ];
                if app.is_settings_dirty() {
                    parts.push(Span::raw("  "));
                    parts.push(Span::styled("*unsaved*", r));
                    parts.push(Span::raw("  "));
                }
                parts.push(Span::styled("^X expert", y));
                parts.push(Span::raw("  "));
                parts.push(Span::styled("^P profiles", y));
                parts.push(Span::raw("  "));
                parts.push(Span::styled("⇥ panels", c));
                parts.push(Span::raw("  "));
                parts.push(Span::styled("Shift+A about", c));
                parts
            } else {
                
                match app.ui.active_panel {
                    ActivePanel::Models => {
                        vec![
                            Span::styled("⇥ panels", c),
                            Span::raw("  "),
                            Span::styled("f filter", y),
                            Span::raw("  "),
                            Span::styled("l/load, u/unload", y),
                            Span::raw("  "),
                            Span::styled("^D del", y),
                            Span::raw("  "),
                            Span::styled("^H help", c),
                            Span::raw("  "),
                            Span::styled("Shift+A about", c),
                        ]
                    }
                    ActivePanel::Log => {
                        if app.log.log_expanded {
                            vec![
                                Span::styled("j/k scroll", c),
                                Span::raw("  "),
                                Span::styled("f/follow", c),
                                Span::raw("  "),
                                Span::styled("⎋ collapse", c),
                                Span::raw("  "),
                                Span::styled("g/G top/bottom", c),
                            ]
                        } else {
                            vec![
                                Span::styled("f/follow", c),
                                Span::raw("  "),
                                Span::styled("⎋ collapse", c),
                                Span::raw("  "),
                                Span::styled("g/G top/bottom", c),
                                Span::raw("  "),
                                Span::styled("⇥ panels", c),
                            ]
                        }
                    }
                    ActivePanel::ServerSettings => {
                        vec![
                            Span::styled("j/k nav", c),
                            Span::raw("  "),
                            Span::styled("↵ toggle", y),
                            Span::raw("  "),
                            Span::styled("⇥ panels", c),
                            Span::raw("  "),
                            Span::styled("Shift+A about", c),
                        ]
                    }
                    ActivePanel::Profiles => {
                        vec![
                            Span::styled("j/k nav", c),
                            Span::raw("  "),
                            Span::styled("↵ apply", y),
                            Span::raw("  "),
                            Span::styled("s save", c),
                            Span::raw("  "),
                            Span::styled("⎋ done", c),
                            Span::raw("  "),
                            Span::styled("⇥ panels", c),
                            Span::raw("  "),
                            Span::styled("Shift+A about", c),
                        ]
                    }
                    ActivePanel::SystemPromptPresets => {
                        vec![
                            Span::styled("j/k nav", c),
                            Span::raw("  "),
                            Span::styled("↵ apply", y),
                            Span::raw("  "),
                            Span::styled("e edit", c),
                            Span::raw("  "),
                            Span::styled("n new", c),
                            Span::raw("  "),
                            Span::styled("⎋ done", c),
                            Span::raw("  "),
                            Span::styled("⇥ panels", c),
                            Span::raw("  "),
                            Span::styled("Shift+A about", c),
                        ]
                    }
                    ActivePanel::SearchReadme => {
                        vec![
                            Span::styled("j/k nav", c),
                            Span::raw("  "),
                            Span::styled("⎋ collapse", c),
                            Span::raw("  "),
                            Span::styled("⇥ panels", c),
                            Span::raw("  "),
                            Span::styled("Shift+A about", c),
                        ]
                    }
                    ActivePanel::Downloads => {
                        vec![
                            Span::styled("j/k nav", c),
                            Span::raw("  "),
                            Span::styled("p pause", y),
                            Span::raw("  "),
                            Span::styled("⌥C cancel", y),
                            Span::raw("  "),
                            Span::styled("⇥ panels", c),
                        ]
                    }
                    _ => {
                        vec![
                            Span::styled("⇥ panels", c),
                            Span::raw("  "),
                            Span::styled("f filter", y),
                            Span::raw("  "),
                            Span::styled("^H help", c),
                            Span::raw("  "),
                            Span::styled("Shift+A about", c),
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
                    Span::styled("⎋ stop", r),
                    Span::raw("  "),
                    Span::styled("⇥ panels", c),
                ]
            } else if !app.bench_tune.bench_tune_results.is_empty() {
                vec![
                    Span::styled("↵ view output", y),
                    Span::raw("  "),
                    Span::styled("⎋ stop", r),
                    Span::raw("  "),
                    Span::styled("⇥ panels", c),
                ]
            } else {
                vec![
                    Span::styled("⎋ stop", r),
                    Span::raw("  "),
                    Span::styled("⇥ panels", c),
                ]
            }
        }
    }
}
