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
pub use web_search_picker::{check_web_search_health, WebSearchPickerHandler};
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
        let handlers: Vec<Box<dyn OverlayHandler + Send + Sync>> = vec![
            Box::new(OnboardingHandler),
            Box::new(CmdLineHandler),
            Box::new(AboutHandler),
            Box::new(SearchInputHandler),
            Box::new(DashboardPickerHandler),
            Box::new(ApiEndpointPickerHandler),
            Box::new(SpecTypePickerHandler),
            Box::new(ChatTemplatePickerHandler),
            Box::new(ChatTemplateFilePickerHandler),
            Box::new(YarnRoPESettingsHandler),
            Box::new(DashboardUrlHandler),
            Box::new(HostPickerHandler),
            Box::new(ProfilePickerHandler),
            Box::new(PromptPickerHandler),
            Box::new(BenchTuneSetupHandler),
            Box::new(BackendPickerHandler),
            Box::new(MaxConcurrentPickerHandler),
            Box::new(ConfirmationHandler),
            Box::new(RpcManagerHandler),
            Box::new(GgufNamingHandler),
            Box::new(WebSearchPickerHandler),
        ];

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
