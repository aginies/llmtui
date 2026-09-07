pub const USER_AGENT: &str = concat!("llm-manager/", env!("CARGO_PKG_VERSION"));

/// Shared HTTP client for short API calls (search, health checks, GitHub).
/// Reuses the connection pool and TLS context across calls instead of
/// building a new client (and pool) per request.
pub static HTTP_CLIENT: std::sync::LazyLock<reqwest::Client> = std::sync::LazyLock::new(|| {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("failed to build shared HTTP client")
});

/// Shared client for long streaming downloads: connect timeout only, no
/// overall timeout (files stream for minutes).
pub static DOWNLOAD_CLIENT: std::sync::LazyLock<reqwest::Client> = std::sync::LazyLock::new(|| {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("failed to build download HTTP client")
});

pub mod benchmark;
pub mod hardware;
pub mod hub;
pub mod server;
pub mod server_logs;
pub mod tls;
pub mod web_context;
pub mod web_search;
pub mod ws_server;
