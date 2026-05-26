pub mod models;
pub mod log;
pub mod settings;
pub mod tags;
pub mod profiles;
pub mod system_prompts;

pub use models::handle_models_key;
pub use log::handle_log_key;
pub use settings::handle_settings_key;
pub use profiles::handle_profiles_key;
pub use system_prompts::handle_system_prompt_presets_key;

mod downloads;
pub use downloads::handle_downloads_key;
