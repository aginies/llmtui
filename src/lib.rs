//! Library target for llm-manager.
//!
//! Re-exports the public API so integration tests in `tests/` can import from `llm_manager`.
//! This enables testing without modifying the binary source files.

// Re-declare modules so they're available as `pub mod` in the library target.
pub mod backend;
pub mod config;
pub mod models;
pub mod serve;
pub mod serve_api;
pub mod tui;

// Re-export key types for convenience in tests.
pub use backend::USER_AGENT;
pub use config::{
    Config, DefaultParams, LogEntry, LogLevel, ModelConfigStore, ModelOverride, Profile,
    SystemPromptPreset, builtin_profiles, builtin_system_prompt_presets,
};
pub use models::*;
pub use tui::app::{ActivePanel, App, ConfirmationKind, GlobalMode, LoadingPhase, ModelsMode};
