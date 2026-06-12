use tracing::info;

use crate::backend::web_search;

const WEB_SEARCH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

/// Result of building an injected prompt with web search context.
pub struct InjectedPrompt {
    /// The modified message content with web context prepended.
    pub content: String,
    /// Whether web search was actually performed.
    pub performed: bool,
}

fn log(cb: &std::sync::Mutex<Option<Box<dyn Fn(String) + Send + Sync>>>, msg: String) {
    if let Some(c) = cb.lock().unwrap().as_ref() {
        c(msg);
    }
}

/// Build the full prompt to send to llama-server, including web search context injection.
/// Returns the original request if no web search is needed or the preset doesn't match.
pub async fn build_injected_prompt(
    preset_name: &str,
    messages: &serde_json::Value,
    web_search_enabled: bool,
    web_search_engine: &str,
    web_search_engine_url: &str,
    web_search_api_key: &str,
    log_callback: &std::sync::Mutex<Option<Box<dyn Fn(String) + Send + Sync>>>,
) -> InjectedPrompt {
    log(log_callback, format!("Web search: preset='{}', enabled={}", preset_name, web_search_enabled));
    if !web_search_enabled {
        log(log_callback, "Web search: disabled in config, skipping".into());
        return InjectedPrompt {
            content: String::new(),
            performed: false,
        };
    }

    let messages_array = match messages.get("messages").and_then(|m| m.as_array()) {
        Some(m) => {
            info!("Web search: found {} messages", m.len());
            log(log_callback, format!("Web search: found {} messages", m.len()));
            m
        }
        None => {
            info!("Web search: no messages array in request");
            log(log_callback, "Web search: no messages array in request".into());
            return InjectedPrompt {
                content: String::new(),
                performed: false,
            };
        }
    };

    if messages_array.is_empty() {
        log(log_callback, "Web search: empty messages array".into());
        return InjectedPrompt {
            content: String::new(),
            performed: false,
        };
    }

    let last_msg = messages_array.last().unwrap();
    let user_content = last_msg.get("content");
    let content = match user_content {
        Some(serde_json::Value::String(s)) => {
            info!("Web search: content is String ({} chars)", s.len());
            log(log_callback, format!("Web search: content is String ({} chars)", s.len()));
            s.clone()
        }
        Some(serde_json::Value::Array(parts)) => {
            info!("Web search: content is Array with {} parts", parts.len());
            let text_parts: Vec<&str> = parts
                .iter()
                .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
                .collect();
            if text_parts.is_empty() {
                info!("Web search: no text parts in array");
                log(log_callback, "Web search: no text parts in array".into());
                return InjectedPrompt {
                    content: String::new(),
                    performed: false,
                };
            }
            let joined = text_parts.join(" ");
            info!("Web search: joined content ({} chars)", joined.len());
            log(log_callback, format!("Web search: joined content ({} chars)", joined.len()));
            joined
        }
        _ => {
            info!("Web search: content type is {:?}", user_content.map(|v| {
                match v {
                    serde_json::Value::Null => "null",
                    serde_json::Value::Bool(_) => "bool",
                    serde_json::Value::Number(_) => "number",
                    serde_json::Value::String(_) => "string",
                    serde_json::Value::Array(_) => "array",
                    serde_json::Value::Object(_) => "object",
                }
            }));
            log(log_callback, "Web search: unsupported content type".into());
            return InjectedPrompt {
                content: String::new(),
                performed: false,
            };
        }
    };

    let needs = web_search::needs_search(&content);
    info!("Web search: needs_search={} for '{}'", needs, &content[..content.char_indices().nth(80).map(|(i, _)| i).unwrap_or(content.len())]);
    log(log_callback, format!("Web search: needs_search={} for '{}'", needs, &content[..content.char_indices().nth(80).map(|(i, _)| i).unwrap_or(content.len())]));
    if !needs {
        log(log_callback, "Web search: no search keywords found, skipping".into());
        return InjectedPrompt {
            content: String::new(),
            performed: false,
        };
    }

    info!("Web search: triggering for message: {}", &content[..content.char_indices().nth(100).map(|(i, _)| i).unwrap_or(content.len())]);
    log(log_callback, format!("Web search: triggering for: '{}'", &content[..content.char_indices().nth(100).map(|(i, _)| i).unwrap_or(content.len())]));

    let query = content.to_string();
    let engine = web_search_engine.to_string();
    let engine_url = web_search_engine_url.to_string();
    let api_key = web_search_api_key.to_string();
    log(log_callback, format!("Web search: engine={}, url={}", engine, engine_url));
    let search_handle = tokio::spawn(async move {
        web_search::gather_search_context(&query, &engine, &engine_url, &api_key).await
    });

    let search_result = match tokio::time::timeout(WEB_SEARCH_TIMEOUT, search_handle).await {
        Ok(Ok(Ok((ctx, sources)))) => {
            info!("Web search: gathered context ({} chars)", ctx.len());
            log(log_callback, format!("Web search: gathered {} chars, {} sources", ctx.len(), sources.len()));
            (ctx, sources)
        }
        Ok(Ok(Err(e))) => {
            info!("Web search failed: {}", e);
            log(log_callback, format!("Web search failed: {}", e));
            return InjectedPrompt {
                content: String::new(),
                performed: false,
            };
        }
        Ok(Err(e)) => {
            info!("Web search task panicked: {}", e);
            log(log_callback, format!("Web search task panicked: {}", e));
            return InjectedPrompt {
                content: String::new(),
                performed: false,
            };
        }
        Err(_) => {
            info!("Web search timed out");
            log(log_callback, "Web search timed out".into());
            return InjectedPrompt {
                content: String::new(),
                performed: false,
            };
        }
    };

    let (search_context, sources) = search_result;

    // Build sources list
    let sources_section = if sources.is_empty() {
        String::new()
    } else {
        let sources_list: String = sources
            .iter()
            .enumerate()
            .map(|(i, url)| format!("{}. {}", i + 1, url))
            .collect::<Vec<_>>()
            .join("\n");
        format!("\n\n---\n\n**Sources:**\n{}\n\n**When using information from these sources, display the original URL as a reference.**", sources_list)
    };

    info!("Web search: gathered context ({} chars)", search_context.len());

    let new_content = format!(
        "[WEB CONTEXT]\nINSTRUCTION: Cite sources using inline markdown links in your answer. Format: [source name](URL). Place links directly after the facts they support. If you find PDF link, add them to the list with brief description. Do NOT include claims you cannot verify.\n\n{}\n[END WEB CONTEXT]\n\n{}\n\n---\n\n{}",
        search_context, sources_section, content
    );

    #[allow(unused_variables)]
    if let Some(cb) = log_callback.lock().unwrap().as_ref() {
        // cb(format!("Web search: results injected ({} chars)", search_context.len()));
    }

    InjectedPrompt {
        content: new_content,
        performed: true,
    }
}
