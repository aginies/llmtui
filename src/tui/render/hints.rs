use super::App;
use super::{Color, Span, Style};
use crate::tui::app::{ActivePanel, ModelsMode};

const HINT_NAV: &str = "j/k nav";
const HINT_PANELS: &str = "⇥ Panels";
const HINT_ABOUT: &str = "Shift+a About";
const HINT_SEP: &str = "  ";

pub fn render_hints(app: &App) -> Vec<Span<'static>> {
    let y = Style::default().fg(Color::Yellow);
    let c = Style::default().fg(Color::Cyan);
    let r = Style::default().fg(Color::Red);

    // Backend picker has its own hint rendering
    if matches!(app.ui.global_mode, crate::tui::app::GlobalMode::BackendPicker { .. }) {
        return vec![
            Span::styled("Del", r),
            Span::raw(HINT_SEP),
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
                Span::raw(HINT_SEP),
                Span::styled("↵ Files", y),
                Span::raw(HINT_SEP),
                Span::styled("-> Readme", y),
                Span::raw(HINT_SEP),
                Span::styled("sort:", c),
                Span::styled(
                    sort_by.label(),
                    Style::default().fg(Color::Magenta),
                ),
            ];
            if *loading {
                parts.push(Span::raw(HINT_SEP));
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
                Span::raw(HINT_SEP),
                Span::styled("⎋ Back", c),
            ]
        }
        ModelsMode::List => {
            if app.ui.active_panel == ActivePanel::LlmSettings {
                let mut parts = vec![
                    Span::styled(HINT_NAV, c),
                    Span::raw(HINT_SEP),
                    Span::styled("^S Save", y),
                    Span::raw(HINT_SEP),
                    Span::styled("^R Reset", y),
                    Span::raw(HINT_SEP),
                    Span::styled("^E Toggle", y),
                    Span::raw(HINT_SEP),
                ];
                if app.is_settings_dirty() {
                    parts.push(Span::raw(HINT_SEP));
                    parts.push(Span::styled("*unsaved*", r));
                    parts.push(Span::raw(HINT_SEP));
                }
                parts.push(Span::styled("^X eXpert", y));
                parts.push(Span::raw(HINT_SEP));
                parts.push(Span::styled("^P Profiles", y));
                parts.push(Span::raw(HINT_SEP));
                parts.push(Span::styled(HINT_PANELS, c));
                parts.push(Span::raw(HINT_SEP));
                parts.push(Span::styled(HINT_ABOUT, c));
                parts
            } else {
                
                match app.ui.active_panel {
                    ActivePanel::Models => {
                        vec![
                            Span::styled(HINT_PANELS, c),
                            Span::raw(HINT_SEP),
                            Span::styled("f Filter", y),
                            Span::raw(HINT_SEP),
                            Span::styled("l/load, u/unload", y),
                            Span::raw(HINT_SEP),
                            Span::styled("^D Del", y),
                            Span::raw(HINT_SEP),
                            Span::styled("^H Help", c),
                            Span::raw(HINT_SEP),
                            Span::styled(HINT_ABOUT, c),
                        ]
                    }
                    ActivePanel::Log => {
                        if app.log.log_expanded {
                            vec![
                                Span::styled("j/k Scroll", c),
                                Span::raw(HINT_SEP),
                                Span::styled("f/follow", c),
                                Span::raw(HINT_SEP),
                                Span::styled("⎋ Collapse", c),
                                Span::raw(HINT_SEP),
                                Span::styled("g/G top/bottom", c),
                            ]
                        } else {
                            vec![
                                Span::styled("f/follow", c),
                                Span::raw(HINT_SEP),
                                Span::styled("⎋ Collapse", c),
                                Span::raw(HINT_SEP),
                                Span::styled("g/G top/bottom", c),
                                Span::raw(HINT_SEP),
                                Span::styled(HINT_PANELS, c),
                            ]
                        }
                    }
                    ActivePanel::ServerSettings => {
                        vec![
                            Span::styled(HINT_NAV, c),
                            Span::raw(HINT_SEP),
                            Span::styled("↵ Toggle", y),
                            Span::raw(HINT_SEP),
                            Span::styled(HINT_PANELS, c),
                            Span::raw(HINT_SEP),
                            Span::styled(HINT_ABOUT, c),
                        ]
                    }
                    ActivePanel::Profiles => {
                        vec![
                            Span::styled(HINT_NAV, c),
                            Span::raw(HINT_SEP),
                            Span::styled("↵ Apply", y),
                            Span::raw(HINT_SEP),
                            Span::styled("s Save", c),
                            Span::raw(HINT_SEP),
                            Span::styled("⎋ Done", c),
                            Span::raw(HINT_SEP),
                            Span::styled(HINT_PANELS, c),
                            Span::raw(HINT_SEP),
                            Span::styled(HINT_ABOUT, c),
                        ]
                    }
                    ActivePanel::SystemPromptPresets => {
                        vec![
                            Span::styled(HINT_NAV, c),
                            Span::raw(HINT_SEP),
                            Span::styled("↵ Apply", y),
                            Span::raw(HINT_SEP),
                            Span::styled("e Edit", c),
                            Span::raw(HINT_SEP),
                            Span::styled("n New", c),
                            Span::raw(HINT_SEP),
                            Span::styled("⎋ Done", c),
                            Span::raw(HINT_SEP),
                            Span::styled(HINT_PANELS, c),
                            Span::raw(HINT_SEP),
                            Span::styled(HINT_ABOUT, c),
                        ]
                    }
                    ActivePanel::SearchReadme => {
                        vec![
                            Span::styled(HINT_NAV, c),
                            Span::raw(HINT_SEP),
                            Span::styled("⎋ Collapse", c),
                            Span::raw(HINT_SEP),
                            Span::styled(HINT_PANELS, c),
                            Span::raw(HINT_SEP),
                            Span::styled(HINT_ABOUT, c),
                        ]
                    }
                    ActivePanel::Downloads => {
                        vec![
                            Span::styled(HINT_NAV, c),
                            Span::raw(HINT_SEP),
                            Span::styled("p Pause", y),
                            Span::raw(HINT_SEP),
                            Span::styled("Alt+C Cancel", y),
                            Span::raw(HINT_SEP),
                            Span::styled(HINT_PANELS, c),
                        ]
                    }
                    _ => {
                        vec![
                            Span::styled(HINT_PANELS, c),
                            Span::raw(HINT_SEP),
                            Span::styled("f Filter", y),
                            Span::raw(HINT_SEP),
                            Span::styled("^H Help", c),
                            Span::raw(HINT_SEP),
                            Span::styled(HINT_ABOUT, c),
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
                    Span::raw(HINT_SEP),
                    Span::styled(HINT_PANELS, c),
                ]
            } else if !app.bench_tune.bench_tune_results.is_empty() {
                vec![
                    Span::styled("↵ View output", y),
                    Span::raw(HINT_SEP),
                    Span::styled("⎋ Stop", r),
                    Span::raw(HINT_SEP),
                    Span::styled(HINT_PANELS, c),
                ]
            } else {
                vec![
                    Span::styled("⎋ Stop", r),
                    Span::raw(HINT_SEP),
                    Span::styled(HINT_PANELS, c),
                ]
            }
        }
    }
}
