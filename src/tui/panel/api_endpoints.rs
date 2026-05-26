use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

pub fn render_api_endpoints(app: &crate::tui::app::App) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let y = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
    let g = Style::default().fg(Color::Green);
    let c = Style::default().fg(Color::Cyan);
    let d = Style::default().fg(Color::DarkGray);
    let magenta = Style::default().fg(Color::Magenta);

    let enabled = app.settings.api_endpoint_enabled;

    lines.push(Line::from(vec![
        Span::styled("API Endpoints", y),
        Span::raw(" — "),
        Span::styled("Esc / ⌃A to close", d),
    ]));
    lines.push(Line::from(""));

    if enabled {
        let host = app.settings.host.clone();
        let port = app.settings.api_endpoint_port;
        let url = format!("http://{}:{}", host, port);
        lines.push(Line::from(vec![
            Span::styled("Base URL: ", d),
            Span::raw(url).style(c),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Auth: ", d),
            Span::styled("Bearer token required", magenta),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::styled("API proxy: ", d),
            Span::styled("Disabled", g),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Enable in Server Settings (F2) to expose endpoints", d),
        ]));
    }

    lines.push(Line::from(""));

    lines.push(Line::from(vec![
        Span::styled("Explicitly handled endpoints:", y),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(format!(
        "  {:<6} {:<30} {}",
        Span::styled("METHOD", y),
        Span::styled("PATH", y),
        Span::styled("DESCRIPTION", y),
    )));
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled("GET", g),
        Span::raw("  "),
        Span::raw("/health"),
        Span::raw("            Health check"),
    ]));
    lines.push(Line::from(format!(
        "  {}  {:<30} {}",
        Span::styled("GET", g),
        Span::raw("/metrics"),
        Span::raw("Prometheus metrics"),
    )));
    lines.push(Line::from(format!(
        "  {}  {:<30} {}",
        Span::styled("POST", y),
        Span::raw("/v1/chat/completions"),
        Span::raw("Chat completions (OpenAI)"),
    )));
    lines.push(Line::from(format!(
        "  {}  {:<30} {}",
        Span::styled("POST", y),
        Span::raw("/v1/completions"),
        Span::raw("Completions (OpenAI)"),
    )));
    lines.push(Line::from(format!(
        "  {}  {:<30} {}",
        Span::styled("POST", y),
        Span::raw("/v1/embeddings"),
        Span::raw("Embeddings"),
    )));
    lines.push(Line::from(format!(
        "  {}  {:<30} {}",
        Span::styled("GET", g),
        Span::raw("/v1/models"),
        Span::raw("List models"),
    )));
    lines.push(Line::from(format!(
        "  {}  {:<30} {}",
        Span::styled("GET", g),
        Span::raw("/api/status"),
        Span::raw("Server status (pid, uptime, loaded models)"),
    )));
    lines.push(Line::from(""));

    lines.push(Line::from(vec![
        Span::styled("llama.cpp endpoints (proxied via fallback):", y),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(format!(
        "  {:<6} {:<30} {}",
        Span::styled("METHOD", y),
        Span::styled("PATH", y),
        Span::styled("DESCRIPTION", y),
    )));
    lines.push(Line::from(format!(
        "  {}  {:<30} {}",
        Span::styled("POST", y),
        Span::raw("/completion"),
        Span::raw("Legacy completion"),
    )));
    lines.push(Line::from(format!(
        "  {}  {:<30} {}",
        Span::styled("POST", y),
        Span::raw("/infill"),
        Span::raw("Code completion (FIM)"),
    )));
    lines.push(Line::from(format!(
        "  {}  {:<30} {}",
        Span::styled("POST", y),
        Span::raw("/reranking"),
        Span::raw("Re-ranking"),
    )));
    lines.push(Line::from(format!(
        "  {}  {:<30} {}",
        Span::styled("POST", y),
        Span::raw("/tokenize"),
        Span::raw("Tokenize text"),
    )));
    lines.push(Line::from(format!(
        "  {}  {:<30} {}",
        Span::styled("POST", y),
        Span::raw("/detokenize"),
        Span::raw("Detokenize tokens"),
    )));
    lines.push(Line::from(format!(
        "  {}  {:<30} {}",
        Span::styled("POST", y),
        Span::raw("/apply-template"),
        Span::raw("Apply chat template"),
    )));
    lines.push(Line::from(format!(
        "  {}  {:<30} {}",
        Span::styled("GET", g),
        Span::raw("/v1/health"),
        Span::raw("Health check (alias)"),
    )));
    lines.push(Line::from(format!(
        "  {}  {:<30} {}",
        Span::styled("GET/POST", magenta),
        Span::raw("/props"),
        Span::raw("Get/set server properties"),
    )));
    lines.push(Line::from(format!(
        "  {}  {:<30} {}",
        Span::styled("GET", g),
        Span::raw("/slots"),
        Span::raw("Slot monitoring"),
    )));
    lines.push(Line::from(format!(
        "  {}  {:<30} {}",
        Span::styled("GET/POST", magenta),
        Span::raw("/lora-adapters"),
        Span::raw("List/load LoRA adapters"),
    )));
    lines.push(Line::from(format!(
        "  {}  {:<30} {}",
        Span::styled("POST", y),
        Span::raw("/models/load"),
        Span::raw("Load a model (router mode)"),
    )));
    lines.push(Line::from(format!(
        "  {}  {:<30} {}",
        Span::styled("POST", y),
        Span::raw("/models/unload"),
        Span::raw("Unload a model (router mode)"),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("Note: ", d),
        Span::raw("Any other path not listed above is also proxied to llama.cpp."),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("j/k scroll", d),
        Span::raw("  "),
        Span::styled("Esc / ⌃A close", d),
    ]));

    lines
}
