use crate::models::chat_message;
use crate::models::ChatMessage;

/// Send a chat message to the server and get a response.
/// Returns the full response text (non-streaming).
#[allow(dead_code)]
pub async fn chat(
    port: u16,
    messages: &[ChatMessage],
    settings: &crate::models::ModelSettings,
) -> Result<String, String> {
    let url = format!("http://127.0.0.1:{}/v1/chat/completions", port);

    let api_messages: Vec<chat_message::ApiMessage> =
        messages.iter().map(|m| m.to_api_message()).collect();

    let body = chat_message::ChatRequest {
        messages: api_messages,
        temperature: settings.temperature,
        top_k: settings.top_k,
        top_p: settings.top_p,
        repeat_penalty: settings.repeat_penalty,
        max_tokens: settings.max_tokens,
        stream: false,
        seed: settings.seed,
    };

    let resp = reqwest::Client::new()
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Server error {}: {}", status, text));
    }

    let chat_resp: chat_message::ChatResponse = resp
        .json()
        .await
        .map_err(|e| format!("Invalid response: {}", e))?;

    chat_resp
        .choices
        .first()
        .map(|c| c.message.content.clone())
        .ok_or_else(|| "No response from model".to_string())
}
