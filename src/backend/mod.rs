pub const USER_AGENT: &str = concat!("llm-manager/", env!("CARGO_PKG_VERSION"));

pub mod benchmark;
pub mod hardware;
pub mod hub;
pub mod server;
pub mod server_logs;
pub mod tls;
pub mod web_context;
pub mod web_search;
pub mod ws_server;
