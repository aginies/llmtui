//! Library target for llm-manager.
//!
//! Re-exports the public API so integration tests in `tests/` can import from `llm_manager`.
//! This enables testing without modifying the binary source files.

// Re-declare modules so they're available as `pub mod` in the library target.
// These mirror the private modules in main.rs.
pub mod backend;
pub mod config;
pub mod models;
pub mod serve;
pub mod serve_api;
pub mod tui;

// Re-export key types for convenience in tests.
pub use config::{Config, DefaultParams, ModelOverride, LogLevel, LogEntry, Profile, SystemPromptPreset, builtin_profiles, builtin_system_prompt_presets};
pub use models::*;
pub use tui::app::{App, ActivePanel, ModelsMode, GlobalMode, ConfirmationKind, LoadingPhase};
