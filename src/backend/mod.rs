pub const USER_AGENT: &str = concat!("llm-manager/", env!("CARGO_PKG_VERSION"));

pub mod benchmark;
pub mod hardware;
pub mod hub;
pub mod server;
pub mod tls;
pub mod ws_server;
