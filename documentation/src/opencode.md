# opencode

This document covers how to use [opencode](https://github.com/opencode-alliance/opencode) with llm-manager's API endpoint.

## Prerequisites

Configure the API endpoint and auth key in llm-manager (see [Configuration](config.md)):

1. Open the **Server Settings** panel (F2)
2. Navigate to **API Endpoint** and press `Enter`
3. Configure:
   - **Enabled** — toggle on
   - **Port** — default `49222`, configurable via `api_endpoint_port` in config.yaml
   - **API Key** — the auth key for Bearer token authentication
4. Press `Enter` to save

## Connecting with opencode

### Note: API Endpoint vs Direct Server

You can connect opencode directly to llama.cpp's default port `8080`:

```bash
export OPENAI_API_BASE="http://localhost:8080/v1"
```

However, using llm-manager's API endpoint (port `49222`) provides additional features:
- **Web Search** — automatic SearXNG integration when messages contain research keywords
- **API proxy** — CORS, SSE streaming optimization, request interception
- **Auth key** — Bearer token authentication
- **Metrics** — Prometheus metrics at `/metrics`

For full feature access, use the API endpoint below.

### Auth JSON approach

Set the `OPENAI_API_KEY` and `OPENAI_API_BASE` environment variables:

```bash
export OPENAI_API_KEY="my-secret-key"          # api_endpoint_key value
export OPENAI_API_BASE="http://localhost:49222/v1"  # or https:// if TLS enabled
opencode auth login
```

opencode stores the API key in `~/.local/share/opencode/auth.json`.

### Env var + custom provider approach

Store the token in an environment variable:

```bash
export OPENCODE_BEARER_TOKEN="my-secret-key"
```

Use it in opencode configuration. opencode supports `{env:VAR_NAME}` syntax for referencing environment variables in config:

```json
{
  "llm-manager": {
    "type": "api",
    "key": "{env:OPENCODE_BEARER_TOKEN}"
  }
}
```

### Bypassing TLS certificate verification

For self-signed or untrusted TLS certificates, set `NODE_TLS_REJECT_UNAUTHORIZED=0` before running opencode:

```bash
NODE_TLS_REJECT_UNAUTHORIZED=0 opencode
```

### Custom CA Certificates (More Secure)

If you want to properly trust your self-signed certificate instead of disabling verification:

```bash
export SSL_CERT_FILE=/path/to/your/certificate.crt
opencode
```

## TLS / HTTPS

When `server_tls_enabled: true` in config.yaml, the API endpoint uses HTTPS:

- Change `OPENAI_API_BASE` to `https://localhost:49222/v1`
- Change curl URL to `https://localhost:49222/v1/chat/completions`

The TLS certificate is shared with the WebSocket dashboard (see [WebSocket Dashboard](dashboard.md)).
