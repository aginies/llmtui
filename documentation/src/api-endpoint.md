# API Endpoint

The API Endpoint exposes an OpenAI-compatible API proxy that forwards requests to llama-server.

## Features

- **Web search injection** — the proxy intercepts `chat/completions` requests, checks for search keywords, performs a SearXNG search, and injects the results directly into the user's last message. No special headers or endpoints needed — any OpenAI-compatible client works.

- **Auth key isolation** — the proxy's `Authorization: Bearer` key is stripped before forwarding to llama.cpp. The backend never sees the proxy API key, preventing credential leakage.

- **Timing-safe authentication** — Bearer token comparison uses constant-time byte comparison (`constant_time_not_eq`) to prevent timing side-channel attacks.

- **Status caching** — the `/models` count is cached with a 5-second TTL, avoiding repeated polling of llama.cpp and reducing backend overhead.

## Enabling

Enable from the **Server Settings** panel (F2):

1. Navigate to **API Endpoint** and press `Enter`
2. Configure:
   - **Enabled** — toggle on
   - **Port** — default `49222`, configurable via `api_endpoint_port` in config.yaml
   - **API Key** — optional Bearer token for authentication
3. Press `Enter` to save

Or in `~/.config/llm-manager/config.yaml`:

```yaml
default:
  api_endpoint_enabled: true
  api_endpoint_port: 49222
  api_endpoint_key: your-secret-key
```

## Serve Mode

Start with the API proxy from the command line:

```bash
./build.sh serve --model model.gguf --api-port 49222
```

With authentication:

```bash
./build.sh serve --model model.gguf --api-port 49222 --api-key secret
```

In serve mode, `--api-key` sets the key for **both** the API proxy and the WebSocket dashboard (if enabled).

## API Endpoints

The proxy handles these endpoints explicitly:

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/health` | GET | Health check |
| `/metrics` | GET | Prometheus metrics |
| `/chat` | GET | Built-in web chat UI (no auth) |
| `/v1/chat/completions` | POST | Chat completions (OpenAI) |
| `/v1/completions` | POST | Completions (OpenAI) |
| `/v1/embeddings` | POST | Embeddings |
| `/v1/models` | GET | List models |
| `/api/status` | GET | Server status |

All other paths are proxied to llama-server (chat completions, embeddings, reranking, tokenization, etc.).

## Web Chat UI

Served at `/chat` on this same port (default `http://localhost:49222/chat`), the built-in web chat UI offers conversation history, Markdown/code/math rendering, and streaming with a stop button. See [Web Chat](web-chat.md).

## File Transfer API

Send files or directories from one llm-manager server to another — useful for
shipping code to a remote model for review without stuffing it into the prompt.
The transfer is a plain authenticated HTTP push of a `tar.gz` archive, so it
works between any two machines that can reach each other's API port.

Enable it in `config.yaml` (disabled by default):

```yaml
default:
  api_endpoint_enabled: true
  api_endpoint_port: 49222
  api_endpoint_key: your-secret-key
  api_transfer_enabled: true
```

### Endpoints

All require the `Authorization: Bearer <api_endpoint_key>` header (when a key is configured):

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/transfer/upload?label=<name>` | POST | Upload a `tar.gz` archive (raw body). Extracted to the transfer dir; returns `{id, path, files: [{path, size}]}`. |
| `/api/transfer/list` | GET | List received transfers with file count and total size. |
| `/api/transfer/{id}/files` | GET | File manifest of one transfer, plus the absolute path on the receiving machine. |
| `/api/agent/run` | POST | Run the review agent on a transfer: `{transfer_id, task, max_rounds?}` → `{answer, rounds, files_read}`. |

### Sending a directory to a peer server

```bash
tar cz -C /path/to/project . | curl -s \
  -H "Authorization: Bearer $API_KEY" \
  --data-binary @- \
  "http://peer-host:49222/api/transfer/upload?label=review"
```

The response contains the extraction path on the receiving machine, which you
can hand to the remote model so it reads the files directly:

```json
{
  "id": "20250115-143000-review",
  "path": "/home/user/.local/share/llm-manager/transfers/20250115-143000-review",
  "files": [{"path": "src/main.rs", "size": 12345}]
}
```

### Review agent

A plain chat model can only see the prompt, so for real code review the
receiving server can run its local model as a small tool-loop agent over the
extracted files. It pages through the transfer itself — no prompt-size wall:

```bash
curl -s -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"transfer_id": "20250115-143000-review", "task": "Review this code for bugs and style issues"}' \
  "http://peer-host:49222/api/agent/run"
```

The model gets three tools scoped to the transfer directory (`LIST [subdir]`,
`READ <path>`, `GREP <regex>`) and a file manifest to start from; it reads
what it needs and replies with `FINAL: <answer>`. Safety limits: 20 rounds by
default (hard cap 50), 200 KiB per read, 1 MiB total tool output per run,
all paths confined to the transfer directory.

**Full flow from a peer server:**

1. `tar cz -C project . | curl ... /api/transfer/upload?label=review` → get the `id`
2. `POST /api/agent/run` with that `id` and the task → get the `answer`

### Notes

- Received files are stored under `~/.local/share/llm-manager/transfers/<timestamp>-<label>/` (override the root with the `LLM_MANAGER_TRANSFER_DIR` environment variable).
- Uploads are streamed to disk with a hard 2 GiB cap; archives containing absolute paths, `..` components, or symlink/hardlink entries are rejected.
- In the TUI, the API proxy restarts automatically when `api_transfer_enabled` is toggled.

## Authentication

When `api_endpoint_key` is configured, clients must include:

```
Authorization: Bearer <key>
```

## TLS / HTTPS

The API proxy shares TLS configuration with the WebSocket dashboard. Enable in config.yaml:

```yaml
default:
  server_tls_enabled: true
  server_tls_cert: /path/to/cert.pem  # optional, auto-generated if omitted
  server_tls_key: /path/to/key.pem     # optional, auto-generated if omitted
```

When TLS is enabled, use `https://localhost:49222` instead of `http://`.

### Auto-generated Certificates

When TLS is enabled without specifying cert/key paths, llm-manager auto-generates a self-signed certificate and CA. Certificates are stored in `~/.config/llm-manager/tls/`:

```
~/.config/llm-manager/tls/
├── ca.pem              # CA certificate
├── ca-key.pem          # CA private key
├── server.pem          # Server certificate
└── server-key.pem      # Server private key
```

To trust the auto-generated CA:

```bash
# Linux (system-wide)
sudo cp ~/.config/llm-manager/tls/ca.pem /usr/local/share/ca-certificates/ && sudo update-ca-certificates

# macOS
sudo security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain ~/.config/llm-manager/tls/ca.pem
```

## CORS

CORS is enabled with dynamic origin validation. Only requests from `localhost`, `127.0.0.1`, or the configured bind host in `config.yaml` are allowed. External websites are blocked.

## SSE Streaming

The API proxy supports **SSE (Server-Sent Events) streaming** for chat completions and other streaming endpoints. Set `stream: true` in the request body.
