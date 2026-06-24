# Systemd Deployment

Run llm-manager as a background service using systemd. This requires a `.env` file for configuration and a `.service` unit file.

## Environment File

Create a `.env` file with your settings. All values are passed as environment variables to the serve command:

```bash
# Path to the model file (.gguf)
LLM_MODEL=/path/to/your/model.gguf

# Path to a per-model override YAML file (optional)
LLM_MODEL_CONFIG=/path/to/model-config.yaml

# Path to main config.yaml
LLM_CONFIG=~/.config/llm-manager/config.yaml

# Settings profile (optional, e.g. qwen, llama, mistral)
# LLM_PROFILE=qwen

# API proxy port (optional, default: 49222)
LLM_API_PORT=49222

# Path to a custom llama-server binary (optional, defaults to auto-resolved)
# LLM_BACKEND_BINARY=/opt/rocm/bin/llama-server

# Host to bind the llama-server to (optional, defaults to config.yaml)
# LLM_HOST=0.0.0.0

# Log file path (optional, defaults to stdout)
# LLM_LOG_FILE=/var/log/llm-manager/model.log

# Enable WebSocket dashboard server
DASHBOARD_ENABLED=true

# WebSocket dashboard port (optional, default: 49223)
# LLM_WS_PORT=49223

# API key for Bearer token authentication (optional)
# LLM_API_KEY=secret

# TLS for WebSocket dashboard and API server (optional)
# LLM_TLS_ENABLE=true
# LLM_TLS_CERT=/path/to/cert.pem
# LLM_TLS_KEY=/path/to/key.pem
```

### Environment Variables

| Variable | Required | Description |
|----------|----------|-------------|
| `LLM_MODEL` | Yes | Path to the GGUF model file |
| `LLM_MODEL_CONFIG` | No | Path to per-model override YAML (auto-detected from model path if not set) |
| `LLM_CONFIG` | No | Path to `config.yaml` (falls back to `~/.config/llm-manager/config.yaml`) |
| `LLM_PROFILE` | No | Settings profile name (e.g., `qwen`, `llama`) |
| `LLM_API_PORT` | No | API proxy port (default: 49222) |
| `LLM_BACKEND_BINARY` | No | Custom llama-server binary path |
| `LLM_HOST` | No | Bind address (default: from config) |
| `LLM_LOG_FILE` | No | Log file path (default: stdout) |
| `DASHBOARD_ENABLED` | No | Enable WebSocket dashboard (`true`/`false`) |
| `LLM_WS_PORT` | No | Dashboard port (default: 49223) |
| `LLM_API_KEY` | No | Bearer token for API/Dashboard auth |
| `LLM_TLS_ENABLE` | No | Enable TLS (`true`/`false`) |
| `LLM_TLS_CERT` | No | TLS certificate path |
| `LLM_TLS_KEY` | No | TLS private key path |

Variables not set or commented out are omitted from the command line. Variables with values use the `${VAR:+--flag "${VAR}"}` bash syntax — only added if the variable is set and non-empty.

## Service File

Install the service unit file in `~/.config/systemd/user/` (user scope) or `/etc/systemd/system/` (system scope):

```ini
[Unit]
Description=LLM Server
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=aginies
Group=aginies
EnvironmentFile=/home/aginies/llmtui/llm-manager.env
ExecStart=/bin/bash -l -c '\
exec /home/aginies/llmtui/target/release/llm-manager serve \
  --model "${LLM_MODEL}" \
  ${LLM_PROFILE:+--profile "${LLM_PROFILE}"} \
  ${LLM_API_PORT:+--api-port "${LLM_API_PORT}"} \
  ${LLM_CONFIG:+--config "${LLM_CONFIG}"} \
  ${LLM_MODEL_CONFIG:+--model-config "${LLM_MODEL_CONFIG}"} \
  ${LLM_BACKEND_BINARY:+--backend-binary "${LLM_BACKEND_BINARY}"} \
  ${LLM_HOST:+--host "${LLM_HOST}"} \
  ${LLM_LOG_FILE:+--log-file "${LLM_LOG_FILE}"} \
  ${DASHBOARD_ENABLED:+--ws-enable} \
  ${LLM_WS_PORT:+--ws-port "${LLM_WS_PORT}"} \
  ${LLM_API_KEY:+--api-key "${LLM_API_KEY}"} \
  ${LLM_TLS_ENABLE:+--tls-enable} \
  ${LLM_TLS_CERT:+--tls-cert "${LLM_TLS_CERT}"} \
  ${LLM_TLS_KEY:+--tls-key "${LLM_TLS_KEY}"}'
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
```

### User vs System Scope

**User scope** (recommended for single-user setups):

```bash
cp llm-manager.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable llm-manager
systemctl --user start llm-manager
```

**System scope** (for multi-user or root-managed services):

```bash
sudo cp llm-manager.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable llm-manager
sudo systemctl start llm-manager
```

Update `User=` and `Group=` in the service file to match your user, and update `EnvironmentFile=` and `ExecStart` paths as needed.

## Management Commands

```bash
# Start the service
systemctl --user start llm-manager

# Stop the service
systemctl --user stop llm-manager

# Restart the service
systemctl --user restart llm-manager

# Enable auto-start on boot
systemctl --user enable llm-manager

# Check status
systemctl --user status llm-manager

# View logs
journalctl --user -u llm-manager -f

# Reload config after .env changes
systemctl --user reload-or-restart llm-manager
```

## Per-Model Config Auto-Detection

When `LLM_MODEL_CONFIG` is not set, llm-manager auto-detects the per-model config file from the model path. The key is derived from the model's display name:

1. Strip the first models directory prefix from the model path
2. Replace path separators (`/`) with double underscores (`__`)
3. Append `.yaml` to the key

For example, model path `/home/aginies/.local/share/llm-manager/models/unsloth/Qwen3.6-35B-A3B-MTP-GGUF/Qwen3.6-35B-A3B-UD-Q4_K_XL.gguf` with models dir `/home/aginies/.local/share/llm-manager/models` produces key `unsloth__Qwen3.6-35B-A3B-MTP-GGUF__Qwen3.6-35B-A3B-UD-Q4_K_XL.gguf`, and the config file is looked up at `~/.config/llm-manager/models/unsloth__Qwen3.6-35B-A3B-MTP-GGUF__Qwen3.6-35B-A3B-UD-Q4_K_XL.gguf.yaml`.

## Override Priority

Settings are applied in this order (later overrides earlier):

1. Rust hardcoded defaults
2. `config.yaml` `default` section
3. `config.yaml` per-model overrides (`models` section)
4. `config.yaml` profile
5. Per-model config YAML file (`LLM_MODEL_CONFIG` or auto-detected)

## Example: Full Production Configuration

```bash
# llm-manager.env
LLM_MODEL=/home/aginies/.local/share/llm-manager/models/unsloth/Qwen3.6-35B-A3B-MTP-GGUF/Qwen3.6-35B-A3B-UD-Q4_K_XL.gguf
LLM_MODEL_CONFIG=/home/aginies/.config/llm-manager/models/unsloth__Qwen3.6-35B-A3B-MTP-GGUF__Qwen3.6-35B-A3B-UD-Q4_K_XL.gguf.yaml
LLM_CONFIG=/home/aginies/.config/llm-manager/config.yaml
LLM_API_PORT=49222
DASHBOARD_ENABLED=true
LLM_WS_PORT=49223
LLM_API_KEY=secret
LLM_TLS_ENABLE=true
LLM_TLS_CERT=/home/aginies/.config/llm-manager/tls/ryzen9-linux.crt
LLM_TLS_KEY=/home/aginies/.config/llm-manager/tls/ryzen9-linux.key
LLM_LOG_FILE=/var/log/llm-manager/model.log
```

This enables the API proxy, WebSocket dashboard with authentication, TLS encryption, and log file output — all suitable for a production server.
