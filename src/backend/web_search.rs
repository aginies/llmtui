use anyhow::{Context, Result};
use tracing::info;

pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

pub fn needs_search(message: &str) -> bool {
    message
        .rfind("$web")
        .map(|i| {
            let before_ok = i == 0 || !message[..i].chars().last().unwrap().is_alphanumeric();
            let after_pos = i + "$web".len();
            let after_ok = after_pos >= message.len()
                || !message[after_pos..]
                    .chars()
                    .next()
                    .unwrap()
                    .is_alphanumeric();
            before_ok && after_ok
        })
        .unwrap_or(false)
}

const MAX_FETCH_URLS: usize = 5;

/// Extract bare http/https URLs from a message.
/// Strips trailing punctuation, dedupes, caps at MAX_FETCH_URLS.
pub fn extract_urls(message: &str) -> Vec<String> {
    let re =
        regex::Regex::new(r##"https?://[^\s<>\[\]()\"']+"##).expect("static regex should compile");
    let mut seen = std::collections::HashSet::new();
    let mut urls = Vec::new();
    for cap in re.find_iter(message) {
        let mut url = cap.as_str().to_string();
        while url.ends_with('.')
            || url.ends_with(',')
            || url.ends_with(';')
            || url.ends_with('!')
            || url.ends_with('?')
            || url.ends_with(')')
            || url.ends_with(']')
            || url.ends_with('}')
            || url.ends_with('"')
            || url.ends_with('\'')
            || url.ends_with(':')
        {
            url.pop();
        }
        if url.is_empty() {
            continue;
        }
        if seen.insert(url.clone()) {
            urls.push(url);
            if urls.len() >= MAX_FETCH_URLS {
                break;
            }
        }
    }
    urls
}

pub async fn search_web(
    query: &str,
    max_results: usize,
    engine: &str,
    engine_url: &str,
    api_key: &str,
) -> Result<Vec<SearchResult>> {
    if engine == "searxng" && !engine_url.is_empty() {
        search_searxng(engine_url, query, max_results, api_key).await
    } else {
        Ok(Vec::new())
    }
}

async fn search_searxng(
    base_url: &str,
    query: &str,
    max_results: usize,
    api_key: &str,
) -> Result<Vec<SearchResult>> {
    validate_url(base_url)?;
    let client = reqwest::Client::new();
    let url = format!(
        "{}/search?q={}&format=json",
        base_url.trim_end_matches('/'),
        urlencoding::encode(query)
    );

    info!("Web search: SearXNG query: {}", url);

    let mut request = client
        .get(&url)
        .header(
            "User-Agent",
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        );

    if !api_key.is_empty() {
        request = request.header("Authorization", format!("Bearer {}", api_key));
    }

    let response = request
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("SearXNG search request failed: {}", e))?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|e| anyhow::anyhow!("SearXNG response read failed: {}", e))?;

    if !body.starts_with('{') && !body.starts_with('[') {
        info!(
            "Web search: SearXNG returned non-JSON (status {}, body len {}): {}",
            status,
            body.len(),
            &body[..body.len().min(300)]
        );
    }

    let json: serde_json::Value = body
        .parse()
        .map_err(|e| anyhow::anyhow!("SearXNG response parse failed (status {}): {}", status, e))?;

    let results_array = json
        .get("results")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let search_results: Vec<SearchResult> = results_array
        .into_iter()
        .filter_map(|r| {
            let title = r
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let url = r
                .get("url")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let snippet = r
                .get("content")
                .or_else(|| r.get("snippet"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            if url.is_empty() || title.is_empty() {
                None
            } else {
                Some(SearchResult {
                    title,
                    url,
                    snippet,
                })
            }
        })
        .take(max_results)
        .collect();

    info!(
        "Web search: SearXNG returned {} results",
        search_results.len()
    );
    Ok(search_results)
}

/// Check if a SearXNG instance is reachable and returns valid JSON.
pub async fn check_health(engine_url: &str, api_key: &str) -> Result<(), String> {
    let client = reqwest::Client::new();
    let url = format!(
        "{}/search?q=test&format=json",
        engine_url.trim_end_matches('/')
    );

    let mut request = client
        .get(&url)
        .header(
            "User-Agent",
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        )
        .timeout(std::time::Duration::from_secs(10));

    if !api_key.is_empty() {
        request = request.header("Authorization", format!("Bearer {}", api_key));
    }

    let response = request
        .send()
        .await
        .map_err(|e| format!("Connection failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "HTTP {}: {}",
            response.status(),
            response.status().canonical_reason().unwrap_or("Unknown")
        ));
    }

    let body = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?;

    body.parse::<serde_json::Value>()
        .map(|_| ())
        .map_err(|e| format!("Invalid JSON response: {}", e))
}

pub fn is_wikipedia(url: &str) -> bool {
    url.contains("wikipedia.org")
}

fn extract_source_url(content: &str) -> Option<String> {
    content.split('\n').next().and_then(|line| {
        let line = line.trim_start_matches('#').trim();
        if let Some(stripped) = line.strip_prefix("[")
            && let Some(pos) = stripped.rfind("](")
        {
            // The URL ends at the first closing paren; anything after it
            // (trailing text on the line) is not part of the URL.
            let rest = &stripped[pos + 2..];
            let url = rest.split(')').next().unwrap_or(rest);
            let url = url.trim();
            if url.is_empty() {
                return None;
            }
            return Some(url.to_string());
        }
        None
    })
}

pub fn truncate_content(text: &str, max_chars: usize) -> String {
    if text.len() <= max_chars {
        text.to_string()
    } else {
        let truncated: String = text.chars().take(max_chars).collect();
        format!("{}...", truncated)
    }
}

fn clean_html_text(html: &str) -> String {
    use scraper::Html;
    let document = Html::parse_document(html);
    let root = document.root_element();
    let text: String = root.text().collect();
    collapse_whitespace(&text)
}

fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

pub async fn gather_search_context(
    query: &str,
    engine: &str,
    engine_url: &str,
    api_key: &str,
) -> Result<(String, Vec<String>)> {
    info!("Web search: gathering context for '{}'", query);

    let search_results = search_web(query, 10, engine, engine_url, api_key).await?;
    if search_results.is_empty() {
        return Ok((String::new(), Vec::new()));
    }

    let results_summary: String = search_results
        .iter()
        .enumerate()
        .map(|(i, r)| format!("{}. **{}** - {}\n   {}", i + 1, r.title, r.url, r.snippet))
        .collect::<Vec<_>>()
        .join("\n\n");

    let wikipedia_url: Option<String> = search_results
        .iter()
        .find(|r| is_wikipedia(&r.url))
        .map(|r| r.url.clone());

    let other_urls: Vec<String> = search_results
        .iter()
        .filter(|r| !is_wikipedia(&r.url))
        .take(5)
        .map(|r| r.url.clone())
        .collect();

    let mut tasks = Vec::new();

    if let Some(url) = wikipedia_url {
        tasks.push(tokio::spawn(
            async move { fetch_wikipedia_content(&url).await },
        ));
    }

    for url in other_urls {
        tasks.push(tokio::spawn(async move { fetch_other_content(&url).await }));
    }

    let mut context_parts = Vec::new();
    let mut sources = Vec::new();
    let mut failed_count = 0u32;
    let mut success_count = 0u32;

    let results = futures_util::future::join_all(tasks).await;
    for result in results {
        match result {
            Ok(Ok(content)) => {
                if content.is_empty() {
                    continue;
                }
                success_count += 1;
                if let Some(url) = extract_source_url(&content) {
                    sources.push(url);
                }
                context_parts.push(content);
            }
            Ok(Err(e)) => {
                info!("Web search: page fetch failed: {}", e);
                failed_count += 1;
            }
            Err(e) => {
                info!("Web search: task failed: {}", e);
                failed_count += 1;
            }
        }
    }

    info!(
        "Web search: fetch complete - {} succeeded, {} failed",
        success_count, failed_count
    );
    if context_parts.is_empty() {
        info!("Web search: no context gathered (all fetches failed or returned empty)");
        return Ok((String::new(), Vec::new()));
    }

    let context = format!(
        "## Search Results\n{}\n\n---\n\n## Web Context\n{}",
        results_summary,
        context_parts.join("\n\n---\n\n")
    );
    Ok((context, sources))
}

fn validate_url(url: &str) -> Result<()> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        anyhow::bail!("URL scheme not allowed: {}", url);
    }
    Ok(())
}

/// Check if an IP address is in a blocked range (private, loopback, link-local,
/// unspecified, or cloud metadata). Used to prevent SSRF.
fn is_blocked_ip(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => {
            // 127.0.0.0/8 loopback
            v4.is_loopback()
                // 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16 private
                || v4.is_private()
                // 169.254.0.0/16 link-local (cloud metadata)
                || v4.is_link_local()
                // 0.0.0.0/8 unspecified
                || v4.is_unspecified()
                // 100.64.0.0/10 carrier-grade NAT
                || (v4.octets()[0] == 100 && v4.octets()[1] >= 64 && v4.octets()[1] <= 127)
                // 192.0.0.0/24 IETF reserved
                || (v4.octets()[0] == 192 && v4.octets()[1] == 0)
                // 198.18.0.0/15 benchmarking
                || (v4.octets()[0] == 198 && (v4.octets()[1] == 18 || v4.octets()[1] == 19))
                // 240.0.0.0/4 reserved
                || (v4.octets()[0] & 0xF0) == 0xF0
        }
        std::net::IpAddr::V6(v6) => {
            // ::1 loopback
            v6.is_loopback()
                // :: unspecified
                || v6.is_unspecified()
                // fe80::/10 link-local
                || (v6.segments()[0] & 0xFFC0) == 0xFE80
                // fc00::/7 unique local
                || (v6.segments()[0] & 0xFE00) == 0xFC00
                // ::ffff:0:0/96 IPv4-mapped — check embedded IPv4
                || (v6.to_ipv4_mapped().map(|v4| is_blocked_ip(v4.into())).unwrap_or(false))
        }
    }
}

/// Validate a URL for SSRF: scheme check + hostname resolution + IP block check.
/// Resolves the hostname and rejects if any resolved IP is in a blocked range.
async fn validate_url_ssrf(url: &str) -> Result<()> {
    validate_url(url)?;
    let parsed =
        reqwest::Url::parse(url).map_err(|e| anyhow::anyhow!("Invalid URL '{}': {}", url, e))?;
    let host = parsed
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("URL has no host: {}", url))?;

    // If host is already an IP literal, check directly.
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        if is_blocked_ip(ip) {
            anyhow::bail!("Blocked IP address (SSRF protection): {}", host);
        }
        return Ok(());
    }

    // Strip port if present for lookup.
    let lookup_host = host.to_string();
    let port = parsed.port_or_known_default().unwrap_or(80);
    let addr = format!("{}:{}", lookup_host, port);

    let addrs = tokio::net::lookup_host(&addr)
        .await
        .map_err(|e| anyhow::anyhow!("DNS resolution failed for '{}': {}", host, e))?;

    for resolved in addrs {
        if is_blocked_ip(resolved.ip()) {
            anyhow::bail!(
                "Blocked IP address (SSRF protection): {} resolves to {}",
                host,
                resolved.ip()
            );
        }
    }
    Ok(())
}

/// Fetch a set of URLs concurrently, extract main content from each.
/// Returns (combined_context, sources). Failed/blocked URLs are skipped and counted.
pub async fn fetch_urls_context(urls: &[String]) -> (String, Vec<String>, u32, u32) {
    if urls.is_empty() {
        return (String::new(), Vec::new(), 0, 0);
    }
    info!("Web fetch: fetching {} URL(s)", urls.len());

    let tasks: Vec<_> = urls
        .iter()
        .map(|url| {
            let url = url.clone();
            tokio::spawn(async move { fetch_other_content(&url).await })
        })
        .collect();

    let mut context_parts = Vec::new();
    let mut sources = Vec::new();
    let mut failed_count = 0u32;
    let mut success_count = 0u32;

    let results = futures_util::future::join_all(tasks).await;
    for result in results {
        match result {
            Ok(Ok(content)) => {
                if content.is_empty() {
                    continue;
                }
                success_count += 1;
                if let Some(url) = extract_source_url(&content) {
                    sources.push(url);
                }
                context_parts.push(content);
            }
            Ok(Err(e)) => {
                info!("Web fetch: page fetch failed: {}", e);
                failed_count += 1;
            }
            Err(e) => {
                info!("Web fetch: task failed: {}", e);
                failed_count += 1;
            }
        }
    }

    info!(
        "Web fetch: complete - {} succeeded, {} failed",
        success_count, failed_count
    );

    let context = if context_parts.is_empty() {
        String::new()
    } else {
        format!("## Fetched Pages\n{}", context_parts.join("\n\n---\n\n"))
    };

    (context, sources, success_count, failed_count)
}

async fn fetch_wikipedia_content(url: &str) -> Result<String> {
    use scraper::{Html, Selector};
    info!("Web search: fetching Wikipedia: {}", url);
    validate_url(url)?;

    let client = reqwest::Client::new();
    let html = client
        .get(url)
        .header(
            "User-Agent",
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        )
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .context(format!("Failed to fetch Wikipedia page (timeout 5s): {}", url))?
        .text()
        .await
        .context(format!("Failed to read Wikipedia content: {}", url))?;

    let document = Html::parse_document(&html);

    let title_selector =
        Selector::parse("#firstHeading").map_err(|e| anyhow::anyhow!("Selector error: {}", e))?;
    let content_selector = Selector::parse("#mw-content-text")
        .map_err(|e| anyhow::anyhow!("Selector error: {}", e))?;

    let title = document
        .select(&title_selector)
        .next()
        .and_then(|n| n.text().next())
        .unwrap_or("Unknown")
        .trim()
        .to_string();

    let content_text = if let Some(content_div) = document.select(&content_selector).next() {
        let text = content_div.text().collect::<Vec<_>>().join("\n");
        let text = collapse_whitespace(&text);
        truncate_content(&text, 2000)
    } else {
        clean_html_text(&html)
    };

    Ok(format!("## [{}]({})\n\n{}", title, url, content_text))
}

async fn fetch_other_content(url: &str) -> Result<String> {
    use scraper::{Html, Selector};
    info!("Web search: fetching page: {}", url);
    validate_url_ssrf(url).await?;

    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(5))
        .build()?;
    let response = client
        .get(url)
        .header(
            "User-Agent",
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        )
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .context(format!("Failed to fetch page (timeout 10s): {}", url))?;

    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if content_type.contains("pdf") || url.ends_with(".pdf") {
        return Err(anyhow::anyhow!("Skipping PDF URL: {}", url));
    }

    let max_bytes: u64 = 5 * 1024 * 1024;
    let content_length = response
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());
    if let Some(cl) = content_length
        && cl > max_bytes
    {
        return Err(anyhow::anyhow!(
            "Page too large ({} bytes, max {}): {}",
            cl,
            max_bytes,
            url
        ));
    }

    let bytes = response
        .bytes()
        .await
        .context(format!("Failed to read page content: {}", url))?;

    if bytes.len() > max_bytes as usize {
        return Err(anyhow::anyhow!(
            "Page too large ({} bytes, max {}): {}",
            bytes.len(),
            max_bytes,
            url
        ));
    }

    let html = String::from_utf8_lossy(&bytes).to_string();

    if html.len() < 500 {
        return Err(anyhow::anyhow!(
            "Page too short ({} bytes), likely blocked or empty: {}",
            html.len(),
            url
        ));
    }

    let lower_html = html.to_lowercase();
    if lower_html.contains("attention required")
        || lower_html.contains("access denied")
        || lower_html.contains("enable javascript")
        || lower_html.contains("challenge-error")
        || lower_html.contains("reference #")
    {
        return Err(anyhow::anyhow!(
            "Page blocked by Cloudflare or security filter: {}",
            url
        ));
    }

    let document = Html::parse_document(&html);

    let title_selector =
        Selector::parse("title").map_err(|e| anyhow::anyhow!("Selector error: {}", e))?;

    let title = document
        .select(&title_selector)
        .next()
        .and_then(|n| n.text().next())
        .map(|s| s.to_string())
        .unwrap_or_else(|| url.to_string())
        .trim()
        .to_string();

    let text = extract_main_content(&document, url);
    let text = truncate_content(&text, 3000);

    info!(
        "Web search: fetched {} from {} ({} chars)",
        title,
        url,
        text.len()
    );

    Ok(format!("## [{}]({})\n\n{}", title, url, text))
}

fn extract_main_content(document: &scraper::Html, url: &str) -> String {
    use scraper::Selector;

    // Try GitHub issue/pr content first
    if url.contains("github.com") {
        if let Ok(selector) = Selector::parse(".markdown-body") {
            let text: String = document
                .select(&selector)
                .flat_map(|el| el.text())
                .collect();
            let collapsed = collapse_whitespace(&text);
            if !collapsed.is_empty() {
                return collapsed;
            }
        }
        if let Ok(selector) = Selector::parse(".timeline-comment") {
            let text: String = document
                .select(&selector)
                .flat_map(|el| el.text())
                .collect();
            let collapsed = collapse_whitespace(&text);
            if !collapsed.is_empty() {
                return collapsed;
            }
        }
    }

    // Try common article/content selectors
    let selectors = [
        ".post-content",
        ".article-content",
        ".entry-content",
        ".content",
        "#content",
        "article",
        ".post",
        ".article",
    ];
    for sel in &selectors {
        if let Ok(selector) = Selector::parse(sel) {
            let text: String = document
                .select(&selector)
                .flat_map(|el| el.text())
                .collect();
            let collapsed = collapse_whitespace(&text);
            if collapsed.len() > 200 {
                return collapsed;
            }
        }
    }

    // Fallback: extract text from specific content-bearing elements only
    let content_selectors = [
        "p",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "li",
        "td",
        "th",
        "blockquote",
    ];
    let mut text_parts = Vec::new();
    for sel in &content_selectors {
        if let Ok(selector) = Selector::parse(sel) {
            let text: String = document
                .select(&selector)
                .filter_map(|el| el.text().next())
                .collect::<Vec<_>>()
                .join(" ");
            if !text.trim().is_empty() {
                text_parts.push(text);
            }
        }
    }

    collapse_whitespace(&text_parts.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_source_url_plain_link() {
        assert_eq!(
            extract_source_url("[Source](https://example.com/article)"),
            Some("https://example.com/article".to_string())
        );
    }

    #[test]
    fn extract_source_url_trailing_text_stripped() {
        assert_eq!(
            extract_source_url("[Source](https://example.com/article) some trailing text"),
            Some("https://example.com/article".to_string())
        );
    }

    #[test]
    fn extract_source_url_heading_prefix() {
        assert_eq!(
            extract_source_url("## [Docs](https://example.com/docs)"),
            Some("https://example.com/docs".to_string())
        );
    }

    #[test]
    fn extract_source_url_no_link() {
        assert_eq!(extract_source_url("plain text without a link"), None);
    }

    #[test]
    fn extract_source_url_empty_url() {
        assert_eq!(extract_source_url("[Source]()"), None);
    }

    #[test]
    fn needs_search_standalone_at_start() {
        assert!(needs_search("$web find info"));
    }

    #[test]
    fn needs_search_standalone_in_middle() {
        assert!(needs_search("hello $web world"));
    }

    #[test]
    fn needs_search_standalone_at_end() {
        assert!(needs_search("search $web"));
    }

    #[test]
    fn needs_search_not_in_webui() {
        assert!(!needs_search("using llama.cpp webui"));
    }

    #[test]
    fn needs_search_not_in_compound() {
        assert!(!needs_search("sounds like $webui do not work"));
    }

    #[test]
    fn needs_search_standalone_after_webui() {
        assert!(needs_search("sounds like $web do not work"));
    }

    #[test]
    fn needs_search_with_punctuation() {
        assert!(!needs_search("check$thisweb"));
        assert!(needs_search("$web."));
        assert!(needs_search("$web,"));
        assert!(needs_search("ask $web?"));
    }

    #[test]
    fn extract_urls_single() {
        let urls = extract_urls("see https://example.com/page for info");
        assert_eq!(urls, vec!["https://example.com/page".to_string()]);
    }

    #[test]
    fn extract_urls_multiple() {
        let urls =
            extract_urls("a https://one.com/x and https://two.com/y and https://three.com/z");
        assert_eq!(
            urls,
            vec![
                "https://one.com/x".to_string(),
                "https://two.com/y".to_string(),
                "https://three.com/z".to_string()
            ]
        );
    }

    #[test]
    fn extract_urls_strips_trailing_punctuation() {
        let urls = extract_urls("visit https://example.com/page.");
        assert_eq!(urls, vec!["https://example.com/page".to_string()]);
        let urls = extract_urls("see (https://example.com/page) ok");
        assert_eq!(urls, vec!["https://example.com/page".to_string()]);
        let urls = extract_urls("check https://example.com/page, right?");
        assert_eq!(urls, vec!["https://example.com/page".to_string()]);
    }

    #[test]
    fn extract_urls_dedupes() {
        let urls = extract_urls("https://example.com and https://example.com again");
        assert_eq!(urls, vec!["https://example.com".to_string()]);
    }

    #[test]
    fn extract_urls_caps_at_five() {
        let msg =
            "https://a.com https://b.com https://c.com https://d.com https://e.com https://f.com";
        let urls = extract_urls(msg);
        assert_eq!(urls.len(), 5);
        assert_eq!(
            urls,
            vec![
                "https://a.com".to_string(),
                "https://b.com".to_string(),
                "https://c.com".to_string(),
                "https://d.com".to_string(),
                "https://e.com".to_string()
            ]
        );
    }

    #[test]
    fn extract_urls_ignores_non_http() {
        assert!(extract_urls("no links here").is_empty());
        assert!(extract_urls("ftp://files.example.com/x").is_empty());
        assert!(extract_urls("mailto:a@b.com").is_empty());
    }

    #[test]
    fn extract_urls_empty() {
        assert!(extract_urls("").is_empty());
    }

    #[test]
    fn is_blocked_ip_loopback_v4() {
        assert!(is_blocked_ip("127.0.0.1".parse().unwrap()));
        assert!(is_blocked_ip("127.255.0.1".parse().unwrap()));
    }

    #[test]
    fn is_blocked_ip_private_v4() {
        assert!(is_blocked_ip("10.0.0.1".parse().unwrap()));
        assert!(is_blocked_ip("172.16.0.1".parse().unwrap()));
        assert!(is_blocked_ip("192.168.1.1".parse().unwrap()));
    }

    #[test]
    fn is_blocked_ip_link_local_v4() {
        assert!(is_blocked_ip("169.254.169.254".parse().unwrap()));
    }

    #[test]
    fn is_blocked_ip_unspecified() {
        assert!(is_blocked_ip("0.0.0.0".parse().unwrap()));
        assert!(is_blocked_ip("::".parse().unwrap()));
    }

    #[test]
    fn is_blocked_ip_loopback_v6() {
        assert!(is_blocked_ip("::1".parse().unwrap()));
    }

    #[test]
    fn is_blocked_ip_unique_local_v6() {
        assert!(is_blocked_ip("fc00::1".parse().unwrap()));
        assert!(is_blocked_ip("fd12::1".parse().unwrap()));
    }

    #[test]
    fn is_blocked_ip_link_local_v6() {
        assert!(is_blocked_ip("fe80::1".parse().unwrap()));
    }

    #[test]
    fn is_blocked_ip_carrier_grade_nat() {
        assert!(is_blocked_ip("100.64.0.1".parse().unwrap()));
        assert!(is_blocked_ip("100.127.255.255".parse().unwrap()));
    }

    #[test]
    fn is_blocked_ip_ipv4_mapped_v6() {
        assert!(is_blocked_ip("::ffff:127.0.0.1".parse().unwrap()));
        assert!(is_blocked_ip("::ffff:10.0.0.1".parse().unwrap()));
    }

    #[test]
    fn is_not_blocked_ip_public() {
        assert!(!is_blocked_ip("8.8.8.8".parse().unwrap()));
        assert!(!is_blocked_ip("1.1.1.1".parse().unwrap()));
        assert!(!is_blocked_ip("93.184.216.34".parse().unwrap()));
        assert!(!is_blocked_ip("2606:4700:4700::1111".parse().unwrap()));
    }

    #[tokio::test]
    async fn validate_url_ssrf_blocks_loopback() {
        let result = validate_url_ssrf("http://127.0.0.1/admin").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("SSRF"));
    }

    #[tokio::test]
    async fn validate_url_ssrf_blocks_localhost() {
        let result = validate_url_ssrf("http://localhost:8080/").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn validate_url_ssrf_blocks_cloud_metadata() {
        let result = validate_url_ssrf("http://169.254.169.254/latest/meta-data/").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("SSRF"));
    }

    #[tokio::test]
    async fn validate_url_ssrf_blocks_private() {
        let result = validate_url_ssrf("http://192.168.1.1/").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn validate_url_ssrf_blocks_ipv6_loopback() {
        let result = validate_url_ssrf("http://[::1]/").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn validate_url_ssrf_rejects_non_http() {
        let result = validate_url_ssrf("ftp://example.com/").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn validate_url_ssrf_allows_public_ip() {
        let result = validate_url_ssrf("http://8.8.8.8/").await;
        assert!(result.is_ok());
    }
}
