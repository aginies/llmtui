# API Endpoint

The API Endpoint exposes an OpenAI-compatible API proxy that forwards requests to llama-server.

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
| `/v1/chat/completions` | POST | Chat completions (OpenAI) |
| `/v1/completions` | POST | Completions (OpenAI) |
| `/v1/embeddings` | POST | Embeddings |
| `/v1/models` | GET | List models |
| `/api/status` | GET | Server status |

All other paths are proxied to llama-server (chat completions, embeddings, reranking, tokenization, etc.).

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

CORS is enabled for all origins with GET/POST/PUT/DELETE/OPTIONS methods.

## SSE Streaming

The API proxy supports **SSE (Server-Sent Events) streaming** for chat completions and other streaming endpoints. Set `stream: true` in the request body.
