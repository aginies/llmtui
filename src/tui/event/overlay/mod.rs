use std::future::Future;
use std::pin::Pin;

use crossterm::event::KeyEvent;

use crate::tui::app::{App, GlobalMode};

mod about;
mod api_endpoint_picker;
mod backend_picker;
mod bench_tune_setup;
mod chat_template_file_picker;
mod chat_template_picker;
mod cmd_line;
mod confirmation;
mod dashboard_picker;
mod dashboard_url;
mod directory_picker;
mod gguf_naming;
mod host_picker;
mod max_concurrent_picker;
mod onboarding;
mod profile_picker;
mod prompt_picker;
mod rpc_manager;
mod search_input;
mod spec_type_picker;
mod web_search_picker;
mod yarn_rope_settings;

pub use about::AboutHandler;
pub use api_endpoint_picker::ApiEndpointPickerHandler;
pub use backend_picker::BackendPickerHandler;
pub use bench_tune_setup::BenchTuneSetupHandler;
pub use chat_template_file_picker::ChatTemplateFilePickerHandler;
pub use chat_template_picker::ChatTemplatePickerHandler;
pub use cmd_line::CmdLineHandler;
pub use confirmation::ConfirmationHandler;
pub use dashboard_picker::DashboardPickerHandler;
pub use dashboard_url::DashboardUrlHandler;
pub use gguf_naming::GgufNamingHandler;
pub use host_picker::HostPickerHandler;
pub use max_concurrent_picker::MaxConcurrentPickerHandler;
pub use onboarding::OnboardingHandler;
pub use profile_picker::ProfilePickerHandler;
pub use prompt_picker::PromptPickerHandler;
pub use rpc_manager::RpcManagerHandler;
pub use search_input::SearchInputHandler;
pub use spec_type_picker::SpecTypePickerHandler;
pub use web_search_picker::WebSearchPickerHandler;
pub use yarn_rope_settings::YarnRoPESettingsHandler;

pub trait OverlayHandler {
    fn can_handle(&self, mode: &GlobalMode) -> bool;
    fn handle<'a>(
        &'a self,
        app: &'a mut App,
        key: KeyEvent,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>;
}

pub struct OverlayRegistry {
    handlers: Vec<Box<dyn OverlayHandler + Send + Sync>>,
}

impl OverlayRegistry {
    pub fn new() -> Self {
        let mut handlers: Vec<Box<dyn OverlayHandler + Send + Sync>> = Vec::new();

        handlers.push(Box::new(OnboardingHandler));
        handlers.push(Box::new(CmdLineHandler));
        handlers.push(Box::new(AboutHandler));
        handlers.push(Box::new(SearchInputHandler));
        handlers.push(Box::new(DashboardPickerHandler));
        handlers.push(Box::new(ApiEndpointPickerHandler));
        handlers.push(Box::new(SpecTypePickerHandler));
        handlers.push(Box::new(ChatTemplatePickerHandler));
        handlers.push(Box::new(ChatTemplateFilePickerHandler));
        handlers.push(Box::new(YarnRoPESettingsHandler));
        handlers.push(Box::new(DashboardUrlHandler));
        handlers.push(Box::new(HostPickerHandler));
        handlers.push(Box::new(ProfilePickerHandler));
        handlers.push(Box::new(PromptPickerHandler));
        handlers.push(Box::new(BenchTuneSetupHandler));
        handlers.push(Box::new(BackendPickerHandler));
        handlers.push(Box::new(MaxConcurrentPickerHandler));
        handlers.push(Box::new(ConfirmationHandler));
        handlers.push(Box::new(RpcManagerHandler));
        handlers.push(Box::new(GgufNamingHandler));
        handlers.push(Box::new(WebSearchPickerHandler));

        Self { handlers }
    }

    pub async fn dispatch(&self, app: &mut App, key: KeyEvent) -> bool {
        for handler in &self.handlers {
            if handler.can_handle(&app.ui.global_mode) {
                handler.handle(app, key).await;
                return true;
            }
        }
        false
    }
}
