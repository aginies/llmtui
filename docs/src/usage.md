# Usage

## Model Management

### Listing Models

The Models panel shows all `.gguf` files found in your models directory (recursively). The display name is the relative path from the models directory.

### Loading and Unloading

- `l` or `Enter` — Load selected model
- `u` — Unload model from server
- `Ctrl+D` — Delete model (with confirmation)

When a model is loaded, its state changes to **Loaded** showing the port and PID. You can load multiple models when using Router mode.

### Deleting Models

Pressing `Ctrl+D` prompts for confirmation before deleting a model file from disk and its settings override from the config.

## Search

Search mode lets you browse and download GGUF models from HuggingFace:

| Key | Action |
|-----|--------|
| `/` | Enter search mode |
| `Enter` | Execute search |
| `Esc` | Exit search |
| `l` | View available GGUF files for a result |
| `S` | Cycle sort order |
| `B` | Go back one page |
| `Down` (at bottom) | Load more results |
| `R` | Fetch and view README |

### GGUF File Browser

When viewing GGUF files for a model:

| Key | Action |
|-----|--------|
| `j` / `k` | Navigate files |
| `Enter` | Download selected file |
| `Esc` | Go back to search results |
| `c` | Cancel download |

Downloads are tracked in the Download panel, which shows progress and status for all active downloads.

## Loading Models

When you load a model, the application:

1. Resolves the llama-server binary for the selected backend (CPU/Vulkan/ROCm)
2. Spawns the server with the current settings
3. Loads the model via the server's `/models/load` API
4. Polls the server's `/metrics` endpoint and log output for status
5. Displays a progress bar showing loading phases

### Loading Phases

The progress bar tracks:

- **Server starting** (8%) — llama.cpp binary is launched
- **Loading model** (7%) — weights file is being read
- **Loading metadata** (7%) — GGUF metadata is parsed
- **Loading tensors** (70%) — tensors are loaded and offloaded to GPU
- **Server listening** (8%) — HTTP server is ready
- **Complete** — model is ready for inference

During tensor loading, the progress bar shows offloaded layers (e.g., `16/32`) parsed from llama.cpp's log output.

## Settings

### Server Settings

| Setting | Default | Description |
|---------|---------|-------------|
| **Host** | 127.0.0.1 | Bind address for the llama.cpp server. Use `0.0.0.0` to accept connections from other machines. |
| **Port** | 8080 | Port for the llama.cpp server. |
| **Backend** | vulkan | Acceleration backend: `cpu` (CPU-only), `vulkan` (NVIDIA/AMD/Intel GPU), `rocm` (AMD GPU). |
| **Threads** | (physical cores) | CPU threads for generation. Set to your physical core count for best performance. |
| **Threads Batch** | 8 | CPU threads for batch processing (prompt evaluation). |
| **Mode** | Normal | Server mode: `Normal` loads a single model, `Router` supports multiple models in memory. |
| **Timeout** | 600 | Server timeout in seconds before auto-shutdown. |
| **Max Models** | 4 | Maximum concurrent models in Router mode. |
| **API Endpoint** | false | Enable the API proxy server (see Serve Mode). |
| **API Port** | 49222 | Port for the API proxy server. |

### LLM Settings

The LLM Settings panel has 22 fields organized into 6 groups. Arrow keys adjust values; `+`/`-` for coarse changes, `Left`/`Right` for fine. Toggle fields (Flash Attention, Unified KV, Keep in memory) respond to `e` or `Ctrl+E`.

#### Loading

| Field | Default | Description |
|-------|---------|-------------|
| **Context** | 32096 | Context window size in tokens. Must be a power of two. Larger values consume more VRAM and RAM. Models often have a maximum context length (e.g., 32K, 128K). |
| **Prompt** | General | System prompt preset that defines the model's initial behavior. Presets include General, Coder, Thinker, Mathematician, and any user-defined prompts. |
| **Keep in memory** | false | Locks model weights in RAM (`-mlock`) to prevent the OS from swapping them out. Useful when repeatedly loading/unloading models. Increases RAM usage. |

#### GPU Offload

| Field | Default | Description |
|-------|---------|-------------|
| **GPU Layers** | Auto | Number of model layers offloaded to GPU memory. `Auto` lets llama.cpp decide based on available VRAM. `Specific` sets an exact number. `All` offloads every layer (`-ngl 999`). |
| **Flash Attention** | true | Enables Flash Attention 2 for faster inference with lower memory usage. Requires GPU support. Can improve throughput by 20-40%. |
| **KV Cache Offload** | true | Offloads the KV cache to RAM when GPU memory is full. Trade-off: more VRAM available for model weights at the cost of slower cache access. |
| **Cache Type K** | F16 | Data type for the key cache. Options: F32 (most accurate, most memory), F16 (default), BF16 (better than F16 for some models), Q8_0, Q5_0, Q5_1, Q4_0, Q4_1, Iq4_NL. |
| **Cache Type V** | F16 | Data type for the value cache. Same options as Cache Type K. Using lower precision reduces VRAM but may affect quality. |
| **Active Experts** | 1 | For Mixture-of-Experts (MoE) models, the number of experts activated per token. Higher values improve quality but increase compute. |

#### Evaluation

| Field | Default | Description |
|-------|---------|-------------|
| **Eval Batch** | 512 | Logical maximum batch size for evaluation. Larger batches improve throughput but increase memory usage. Set to the model's native context length for single-sequence inference. |
| **Unified KV** | true | Shares KV cache across sequences, reducing memory usage when running multiple prompts. Can cause cache eviction conflicts. |
| **Max Concurrent Pred** | 1 | Maximum number of concurrent predictions. Useful in Router mode for parallel inference. |

#### Sampling

| Field | Default | Description |
|-------|---------|-------------|
| **Seed** | -1 | Random seed for reproducible outputs. `-1` means random each time. Set to a fixed value for debugging or reproducibility. |
| **Temperature** | 0.8 | Controls randomness in sampling. Higher values (1.0-2.0) produce more creative/divergent outputs. Lower values (0.0-0.5) produce more deterministic/crisp outputs. |
| **Top-k** | 40 | Limits sampling to the k most likely next tokens. `0` disables. Smaller values make outputs more focused. Typical: 20-50. |
| **Top-p** | 0.95 | Nucleus sampling: limits to tokens whose cumulative probability reaches p. `1.0` disables. Lower values (0.8-0.95) reduce randomness. |
| **Min P** | 0.0 | Minimum probability threshold for sampling. Tokens with probability below this fraction of the highest-probability token are excluded. Useful for controlling extreme outputs. |
| **Max Tokens** | 0 | Maximum tokens to generate per response. `0` means no limit (until EOS token). |

#### Repetition Control

| Field | Default | Description |
|-------|---------|-------------|
| **Repetition Penalty** | 1.1 | Penalizes tokens that have already appeared. Values > 1.0 reduce repetition. Typical: 1.1-1.2. |
| **Rep. Last N** | 64 | Number of recent tokens to consider for repetition penalty. `-1` uses the full context. |
| **Presence Penalty** | 0.0 | Adds penalty to tokens that have appeared at least once. Encourages the model to discuss new topics. Range: -2.0 to 2.0. |
| **Frequency Penalty** | 0.0 | Adds penalty proportional to how often a token has appeared. Stronger than presence penalty. Range: -2.0 to 2.0. |

> **Note:** These parameters are stored in the config and adjustable in the UI, but currently not passed to the llama.cpp server. They will be activated once the app supports them.

#### Changing Values

Use `Left`/`Right` to adjust numeric fields by 1, or `Up`/`Down` for larger steps. Toggle fields respond to `e` or `Ctrl+E`. Dirty (changed) fields are highlighted in yellow.

### Saving Settings

- `Ctrl+S` — Save settings for the selected model
- `Ctrl+R` — Reset to defaults
- `e` / `Ctrl+E` — Toggle enabled/disabled (for Flash Attention, Unified KV, Keep in memory)

Dirty (changed) fields are highlighted in yellow.

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `j` / `k` | Navigate up/down |
| `Enter` | Load model / Execute search / Expand log |
| `Esc` | Back / Exit search / Collapse log |
| `Tab` | Switch panels |
| `t` | Switch settings tab |
| `/` | Search models |
| `l` | Load model |
| `u` | Unload model |
| `Ctrl+D` | Delete model |
| `Ctrl+H` | Panel-specific help |
| `Ctrl+K` | CmdLine overlay |
| `Ctrl+Alt+K` | Kill llama-server |
| `Ctrl+S` | Save settings |
| `Ctrl+R` | Reset settings |
| `Ctrl+E` | Toggle enabled/disabled |
| `g` / `G` | Jump to top/bottom of log |

## CmdLine Overlay

Press `Ctrl+K` to view the full command line that would be executed to start the llama.cpp server. This shows the binary path, model path, and all parameters.

Press `e` in the overlay to export the command to `/tmp/test_llamaserver.sh`.

## Server Status

The status bar shows the current server status at the top:

- **Running:** `● 9090 Normal` (green dot with port and mode)
- **Stopped:** `○ Server` (gray)

Press `Ctrl+Alt+K` to kill the running llama-server. When stopped, all loaded models are reset to **Available** state.

## Profiles

Profiles are named presets of LLM settings. Built-in profiles include Qwen, Gemma, Llama, Mistral, and Phi.

- `p` — Apply a profile to current settings
- `Ctrl+S` — Save current settings as a new profile (in the Profiles panel)
- `Ctrl+D` — Delete a user-defined profile
