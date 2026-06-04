use std::pin::Pin;
use std::future::Future;

use crossterm::event::KeyEvent;

use crate::tui::app::{App, GlobalMode};

use super::OverlayHandler;

pub struct BenchTuneSetupHandler;

impl OverlayHandler for BenchTuneSetupHandler {
    fn name(&self) -> &'static str {
        "BenchTuneSetup"
    }

    fn can_handle(&self, mode: &GlobalMode) -> bool {
        matches!(mode, GlobalMode::BenchTuneSetup { .. })
    }

    fn handle<'a>(
        &'a self,
        app: &'a mut App,
        key: KeyEvent,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            super::super::key::handle_bench_tune_setup_key(app, key).await;
        })
    }
}
