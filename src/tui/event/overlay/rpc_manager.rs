use std::pin::Pin;
use std::future::Future;

use crossterm::event::KeyEvent;

use crate::tui::app::{App, GlobalMode};

use super::OverlayHandler;

pub struct RpcManagerHandler;

impl OverlayHandler for RpcManagerHandler {
    fn name(&self) -> &'static str {
        "RpcManager"
    }

    fn can_handle(&self, mode: &GlobalMode) -> bool {
        matches!(mode, GlobalMode::RpcManager)
    }

    fn handle<'a>(
        &'a self,
        app: &'a mut App,
        key: KeyEvent,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            super::super::rpc_workers::handle_rpc_workers_key(app, key);
        })
    }
}
