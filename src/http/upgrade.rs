/// WebSocket upgrade middleware and helpers.
///
/// axum 0.8 handles WebSocket upgrades via `WebSocketUpgrade` extractor,
/// so no additional middleware is needed. This module exists for future
/// extensions like connection rate limiting or authentication.
#[allow(dead_code)]
pub mod helpers {}
