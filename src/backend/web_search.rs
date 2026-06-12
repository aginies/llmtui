use anyhow::{Context, Result};
use tracing::info;

pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

pub fn needs_search(message: &str) -> bool {
    message.contains("!web")
}

pub async fn search_web(query: &str, max_results: usize, engine: &str, engine_url: &str, api_key: &str) -> Result<Vec<SearchResult>> {
    if engine == "searxng" && !engine_url.is_empty() {
        search_searxng(engine_url, query, max_results, api_key).await
    } else {
        Ok(Vec::new())
    }
}

async fn search_searxng(base_url: &str, query: &str, max_results: usize, api_key: &str) -> Result<Vec<SearchResult>> {
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

    let response = request.send().await.map_err(|e| anyhow::anyhow!("SearXNG search request failed: {}", e))?;

    let status = response.status();
    let body = response.text().await
        .map_err(|e| anyhow::anyhow!("SearXNG response read failed: {}", e))?;

    if !body.starts_with('{') && !body.starts_with('[') {
        info!("Web search: SearXNG returned non-JSON (status {}, body len {}): {}", status, body.len(), &body[..body.len().min(300)]);
    }

    let json: serde_json::Value = body.parse()
        .map_err(|e| anyhow::anyhow!("SearXNG response parse failed (status {}): {}", status, e))?;

    let results_array = json
        .get("results")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let search_results: Vec<SearchResult> = results_array
        .into_iter()
        .filter_map(|r| {
            let title = r.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let url = r.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let snippet = r.get("content")
                .or_else(|| r.get("snippet"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            if url.is_empty() || title.is_empty() {
                None
            } else {
                Some(SearchResult { title, url, snippet })
            }
        })
        .take(max_results)
        .collect();

    info!("Web search: SearXNG returned {} results", search_results.len());
    Ok(search_results)
}

pub fn is_wikipedia(url: &str) -> bool {
    url.contains("wikipedia.org")
}

fn extract_source_url(content: &str) -> Option<String> {
    content.split('\n').next().and_then(|line| {
        let line = line.trim_start_matches("# ");
        if let Some(stripped) = line.strip_prefix("[") {
            if let Some(pos) = stripped.rfind("](") {
                return Some(stripped[pos+2..].to_string());
            }
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


pub async fn gather_search_context(query: &str, engine: &str, engine_url: &str, api_key: &str) -> Result<(String, Vec<String>)> {
    info!("Web search: gathering context for '{}'", query);

    let search_results = search_web(query, 10, engine, engine_url, api_key).await?;
    if search_results.is_empty() {
        return Ok((String::new(), Vec::new()));
    }

    let results_summary: String = search_results
        .iter()
        .enumerate()
        .map(|(i, r)| {
            format!(
                "{}. **{}** - {}\n   {}",
                i + 1,
                r.title,
                r.url,
                r.snippet
            )
        })
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
        tasks.push(tokio::spawn(async move {
            fetch_wikipedia_content(&url).await
        }));
    }

    for url in other_urls {
        tasks.push(tokio::spawn(async move {
            fetch_other_content(&url).await
        }));
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

    info!("Web search: fetch complete - {} succeeded, {} failed", success_count, failed_count);
    if context_parts.is_empty() {
        info!("Web search: no context gathered (all fetches failed or returned empty)");
        return Ok((String::new(), Vec::new()));
    }

    let context = format!("## Search Results\n{}\n\n---\n\n## Web Context\n{}", results_summary, context_parts.join("\n\n---\n\n"));
    Ok((context, sources))
}

async fn fetch_wikipedia_content(url: &str) -> Result<String> {
    use scraper::{Html, Selector};
    info!("Web search: fetching Wikipedia: {}", url);

    let client = reqwest::Client::new();
    let html = client
        .get(url)
        .header(
            "User-Agent",
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        )
        .send()
        .await
        .context(format!("Failed to fetch Wikipedia page: {}", url))?
        .text()
        .await
        .context(format!("Failed to read Wikipedia content: {}", url))?;

    let document = Html::parse_document(&html);

    let title_selector = Selector::parse("#firstHeading").map_err(|e| anyhow::anyhow!("Selector error: {}", e))?;
    let content_selector = Selector::parse("#mw-content-text").map_err(|e| anyhow::anyhow!("Selector error: {}", e))?;

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

    Ok(format!(
        "## [{}]({})\n\n{}",
        title,
        url,
        content_text
    ))
}

async fn fetch_other_content(url: &str) -> Result<String> {
    use scraper::{Html, Selector};
    info!("Web search: fetching page: {}", url);

    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .header(
            "User-Agent",
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        )
        .send()
        .await
        .context(format!("Failed to fetch page: {}", url))?;

    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if content_type.contains("pdf") || url.ends_with(".pdf") {
        return Err(anyhow::anyhow!("Skipping PDF URL: {}", url));
    }

    let html = response
        .text()
        .await
        .context(format!("Failed to read page content: {}", url))?;

    if html.len() < 500 {
        return Err(anyhow::anyhow!("Page too short ({} bytes), likely blocked or empty: {}", html.len(), url));
    }

    let lower_html = html.to_lowercase();
    if lower_html.contains("attention required") || lower_html.contains("access denied") || lower_html.contains("enable javascript") || lower_html.contains("challenge-error") || lower_html.contains("reference #") {
        return Err(anyhow::anyhow!("Page blocked by Cloudflare or security filter: {}", url));
    }

    let document = Html::parse_document(&html);

    let title_selector = Selector::parse("title").map_err(|e| anyhow::anyhow!("Selector error: {}", e))?;

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

    info!("Web search: fetched {} from {} ({} chars)", title, url, text.len());

    Ok(format!(
        "## [{}]({})\n\n{}",
        title,
        url,
        text
    ))
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
    let selectors = [".post-content", ".article-content", ".entry-content",
                     ".content", "#content", "article", ".post", ".article"];
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
    let content_selectors = ["p", "h1", "h2", "h3", "h4", "h5", "h6", "li", "td", "th", "blockquote"];
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
