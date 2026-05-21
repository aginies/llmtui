# Configuration

## Config File

Configuration is stored in `~/.config/llm-manager/config.yaml`. The config is created automatically on first run with sensible defaults.

```yaml
models_dir: ~/.local/share/llm-manager/models
llama_server: llama-server
default:
  context_length: 32096
  threads: <physical cores>
  threads_batch: 8
  batch_size: 512
  temperature: 0.8
  # ... more settings
```

You can specify a custom config path with `--config`:

```bash
cargo run -- --config /path/to/config.yaml
```

## Default Parameters

| Parameter | Default | Description |
|-----------|---------|-------------|
| `context_length` | 32096 | Context window size in tokens |
| `threads` | (physical cores) | CPU threads for generation |
| `threads_batch` | 8 | CPU threads for batch processing |
| `batch_size` | 512 | Logical maximum batch size |
| `ubatch_size` | 512 | Physical maximum batch size |
| `keep` | 0 | Keep N tokens from initial prompt |
| `mlock` | false | Lock model weights in RAM |
| `mmap` | true | Memory-map the model |
| `kv_cache_offload` | true | Offload KV cache to RAM |
| `flash_attn` | true | Enable Flash Attention |
| `temperature` | 0.8 | Sampling temperature |
| `top_k` | 40 | Top-k sampling |
| `top_p` | 0.95 | Top-p sampling |
| `repeat_penalty` | 1.1 | Repetition penalty |
| `backend` | vulkan | Default backend (cpu/vulkan/rocm) |

## Profiles

Profiles are named presets of settings. The built-in profiles are:

| Profile | Description | Key Settings |
|---------|-------------|--------------|
| Qwen | Optimized for Qwen models | temp: 0.6, top-k: 20, context: 32768 |
| Gemma | Optimized for Gemma models | temp: 0.8, min-p: 0.1, typical-p: 0.9 |
| Llama | Optimized for Llama models | temp: 0.7, repeat-penalty: 1.1 |
| Mistral | Optimized for Mistral models | temp: 0.7, top-k: 50 |
| Phi | Optimized for Phi models | temp: 0.7, top-k: 50 |

User-defined profiles are stored in the `profiles` list in the config file. Built-in profiles are merged on load, so adding a new built-in profile will automatically appear.

## System Prompt Presets

System prompt presets define the initial system prompt. Built-in presets:

| Preset | Description |
|--------|-------------|
| General | "You are a helpful assistant." |
| Coder | Expert software developer |
| Thinker | Analytical and thoughtful |
| Mathematician | Expert in mathematics |

User-defined presets are stored in the `system_prompt_presets` list in the config file.

## Backend Binaries

llama-server binaries are stored in `~/.local/share/llm-manager/bin/` with versioned directories:

```
~/.local/share/llm-manager/bin/
├── llama-server-cpu-{version}/llama-server
├── llama-server-vulkan-{version}/llama-server
└── llama-server-rocm-{version}/llama-server
```

Binaries are downloaded from the `ggml-org/llama.cpp` GitHub releases on first use. Switching versions is instant — no re-download.

### Per-backend Version Config

```yaml
llama_cpp_version_cpu: null
llama_cpp_version_vulkan: null
llama_cpp_version_rocm: null
```

Setting to `null` uses the latest release. Specific versions can be set via the version picker in LLM Settings. These selections are automatically persisted to your configuration and remembered across restarts.

### Asset Names

- **CPU:** `llama-{tag}-bin-ubuntu-x64.tar.gz`
- **Vulkan:** `llama-{tag}-bin-ubuntu-vulkan-x64.tar.gz`
- **ROCm:** `llama-{tag}-bin-ubuntu-rocm-7.2-x64.tar.gz`

## Serve Mode

You can start a model directly from the command line without the TUI:

```bash
./build.sh serve --model /path/to/model.gguf
```

### Options

| Option | Description |
|--------|-------------|
| `--model` | Path to the GGUF model file |
| `--profile` | Apply a settings profile (e.g., `qwen`, `llama`) |
| `--config` | Path to config file |
| `--api-port` | Start API proxy on given port |
| `--api-key` | API key for Bearer token authentication |
| `--host` | Bind address for the server |
| `--backend` | Acceleration backend (cpu / vulkan / rocm) |
| `--threads` | CPU threads |
| `--context` | Context length |
| `--gpu-layers` | Number of GPU layers |

### API Proxy

The API proxy forwards requests to the llama.cpp server and provides OpenAI-compatible and Anthropic-compatible endpoints. When `--api-key` is set, all requests require `Authorization: Bearer <key>`.

### API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/v1/chat/completions` | POST | Chat completions (OpenAI) |
| `/v1/completions` | POST | Completions (OpenAI) |
| `/v1/responses` | POST | Responses (Anthropic) |
| `/v1/messages` | POST | Messages (Anthropic) |
| `/v1/messages/count_tokens` | POST | Count tokens (Anthropic) |
| `/v1/embeddings` | POST | Embeddings |
| `/v1/models` | GET | List models |
| `/completion` | POST | Legacy completion |
| `/infill` | POST | Code completion (FIM) |
| `/reranking` | POST | Re-ranking |
| `/tokenize` | POST | Tokenize text |
| `/detokenize` | POST | Detokenize tokens |
| `/apply-template` | POST | Apply chat template |
| `/health` | GET | Health check |
| `/v1/health` | GET | Health check (alias) |
| `/metrics` | GET | Prometheus metrics |
| `/props` | GET/POST | Get/set server properties |
| `/slots` | GET | Slot monitoring |
| `/lora-adapters` | GET/POST | List/load LoRA adapters |
| `/models/load` | POST | Load a model (router mode) |
| `/models/unload` | POST | Unload a model (router mode) |
| `/api/status` | GET | Server status (pid, uptime, loaded models) |

Any endpoint not listed above is automatically proxied to the llama-server instance.

## Model Overrides

Settings can be saved per-model. These overrides are stored in the `model_overrides` map in the config file, keyed by model file name. When a model is loaded, its override settings are merged into the defaults.

## RPC Workers

You can manage a list of remote `llama-rpc-server` nodes for distributed inference. These are stored in the `rpc_workers` list in the config:

```yaml
rpc_workers:
  - selected: true
    name: "Worker 1"
    ip: "192.168.1.10"
    port: 50052
```

Workers can be managed via the **RPC Workers** window in the **Server Settings** panel. Selected workers are combined into the `--rpc` flag when starting the server.
