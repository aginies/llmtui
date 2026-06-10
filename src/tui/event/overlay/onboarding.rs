use std::future::Future;
use std::pin::Pin;

use crossterm::event::{KeyCode, KeyEvent};

use crate::tui::app::{App, GlobalMode};

use super::OverlayHandler;

const TOTAL_STEPS: usize = 8;

pub struct OnboardingHandler;

impl OverlayHandler for OnboardingHandler {
    fn can_handle(&self, mode: &GlobalMode) -> bool {
        matches!(mode, GlobalMode::Onboarding { .. })
    }

    fn handle<'a>(
        &'a self,
        app: &'a mut App,
        key: KeyEvent,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            let step = match &app.ui.global_mode {
                GlobalMode::Onboarding { step } => *step,
                _ => return,
            };

            match key.code {
                KeyCode::Enter | KeyCode::Char('n') | KeyCode::Right => {
                    let next = step + 1;
                    if next >= TOTAL_STEPS {
                        // Last step — complete onboarding
                        app.config.onboarding_complete = true;
                        app.config.save().ok();
                        app.ui.global_mode = GlobalMode::Normal;
                    } else {
                        app.ui.global_mode = GlobalMode::Onboarding { step: next };
                    }
                }
                KeyCode::Char('p') | KeyCode::Left => {
                    if step > 0 {
                        app.ui.global_mode = GlobalMode::Onboarding { step: step - 1 };
                    }
                }
                KeyCode::Esc | KeyCode::Char('q') => {
                    // Skip onboarding — mark complete
                    app.config.onboarding_complete = true;
                    app.config.save().ok();
                    app.ui.global_mode = GlobalMode::Normal;
                }
                _ => {}
            }
        })
    }
}
