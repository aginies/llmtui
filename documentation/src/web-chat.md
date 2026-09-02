# Web Chat UI

llm-manager ships a built-in web-based chat interface. It runs in any browser and talks to the same API proxy as external OpenAI-compatible clients, so you can chat with a loaded model without a terminal or a separate chat frontend.

## Access

Open the chat at `/chat` on the API proxy port (default `49222`):

```
http://localhost:49222/chat
```

With TLS enabled, use `https://` instead of `http://`.

The chat UI is served by the API proxy (`src/serve_api.rs`), so it requires the **API Endpoint** to be enabled. Enable it from the **Server Settings** panel (F2 → **API Endpoint**), or in `~/.config/llm-manager/config.yaml`:

```yaml
default:
  api_endpoint_enabled: true
  api_endpoint_port: 49222
  api_endpoint_key: your-secret-key  # optional
```

## Authentication

When an API key is configured (`api_endpoint_key`), the chat page prompts for it on first load. The key is sent as an `Authorization: Bearer` header on every request and persisted in `localStorage` for future visits. Leave the field empty to clear a stored key.

With TLS enabled, the same `https://` URL is used.

## Features

- **Conversations** — the sidebar lists past conversations, grouped by date, sorted by most recently used. Create a new conversation, search the list, or delete one. Selecting a conversation does not reorder the list.
- **Streaming** — responses stream token-by-token. Press **Stop** to abort generation mid-response.
- **Live latency** — generation latency is shown live, preferring the model's `gen_tps`.
- **Markdown** — messages render as Markdown, including tables, links, and blockquotes.
- **Code** — code blocks are syntax-highlighted (highlight.js, Tokyo Night theme).
- **Math** — LaTeX math is rendered with KaTeX and auto-rendered as it streams in.
- **Theme** — toggle between a dark and a light theme.
- **Web context** — when web search is enabled, messages that contain URLs have their page content fetched and injected so the model can reason over linked pages (see [Web Search](web-search.md)).

## How It Works

The chat page is embedded in the binary (`src/chat.html`) and served at `/chat` (no auth, like `/health` and `/metrics`). From there it makes authenticated calls to the API proxy:

| Endpoint | Purpose |
|----------|---------|
| `/v1/chat/completions` | Send messages, receive the streamed response |
| `/api/status` | Poll the loaded model and live metrics |
| `/health` | Health check |

The WebSocket dashboard (if enabled) also connects from this page for real-time TPS/latency, falling back to `/api/status` polling when it is not.

## Using Web Search

If web search is enabled, prefix a message with `$web` to trigger a SearXNG search, or include one or more URLs to have those pages fetched and summarized. Both run concurrently and the gathered context is injected into the prompt before your message. See [Web Search](web-search.md) for details.
