/// Event types for channel-based pending operations.
///
/// Replaces the old `pending.*` Option fields with an event-driven
/// architecture. Key handlers send events to a channel; the main loop
/// drains and processes them.

use crate::models::Backend;
use crate::models::{DiscoveredModel, ModelSettings};
use std::path::PathBuf;

/// Represents a pending user action that needs processing in the main loop.
pub enum PendingEvent {
    Download {
        model_id: String,
        filename: String,
        url: String,
        file_size: u64,
        subdir: String,
    },
    Deletion {
        path: PathBuf,
    },
    BackendDeletion {
        backend: Backend,
        tag: String,
    },
    Spawn {
        model: Option<DiscoveredModel>,
        settings: ModelSettings,
    },
    /// The ServerHandle is stored in `pending_kill` field (kept as Option
    /// because the main loop needs to take() it for the async kill).
    KillHandle {
        handle: crate::backend::server::ServerHandle,
    },
    Search {
        query: String,
        offset: u32,
    },
}
