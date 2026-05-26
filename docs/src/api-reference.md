# API Reference

The full Rust API reference is available at [docs.rs/llm-manager](https://docs.rs/llm-manager/).

Generate it locally with:

```bash
cargo doc --open
```

## Public Types

### Core Types

| Type | Module | Description |
|------|--------|-------------|
| `DiscoveredModel` | `models` | A discovered `.gguf` file with path, name, size, and display name |
| `ModelSettings` | `models` | All settings for loading a model via llama.cpp server (70+ fields) |
| `ModelState` | `models` | State of a model: `Available`, `Loading`, `Loaded`, or `Failed` |
| `SearchResult` | `models` | A model found via HuggingFace search |
| `DownloadState` | `models` | Download progress tracking with cancellation support |
| `GgufMetadata` | `models` | Parsed GGUF metadata (layers, hidden size, context, etc.) |
| `ServerMetrics` | `models` | Metrics from the llama.cpp server (TPS, VRAM, CPU, context) |
| `WsMetrics` | `models` | WebSocket-friendly metrics snapshot (serializable, includes settings and command display) |
| `LogEntry` | `config` | A single log entry with timestamp, level, and message |

### Enums

| Type | Module | Description |
|------|--------|-------------|
| `Backend` | `models` | Acceleration backend: `Cpu`, `Vulkan`, `Rocm`, `RocmLemonade`, or `Cuda` |
| `ServerMode` | `models` | Server operating mode: `Normal` (single model), `Router` (multiple), `Bench` (GPU benchmarking), or `BenchTune` (parameter auto-tuning) |
| `GpuLayersMode` | `models` | GPU offloading: `Auto`, `Specific(n)`, or `All` |
| `SearchSort` | `models` | Search result sort order: `Relevance`, `Downloads`, `Likes`, `Trending`, `Created` |
| `CacheType` | `models` | Main KV cache data type: `F16`, `BF16`, `Fq8_0`, `Fq4_1` |
| `CacheQuantType` | `models` | KV cache data type for quantization (F32, F16, BF16, Q8_0, Q5_0, Q5_1, Q4_0, Q4_1, Iq4Nl) |
| `CacheTypeK` / `CacheTypeV` | `models` | Type aliases for `CacheQuantType` (used for keys and values) |
| `SplitMode` | `models` | Multi-GPU split mode: `None`, `Layer`, `Row`, `Tensor` |
| `NumMode` | `models` | NUMA optimization: `None`, `Distribute`, `Isolate`, `Numactl` |
| `RopeScaling` | `models` | RoPE frequency scaling: `None`, `Linear`, `Yarn` |
| `Mirostat` | `models` | Mirostat version: `Off`, `Mirostat`, `Mirostat2` |
| `ReasoningMode` | `models` | Reasoning format: `Default` (DeepSeek/OpenAI style) or `Gemma` (Gemma style) |
| `ServerMode` | `models` | Server operating mode: `Normal`, `Router`, `Bench`, or `BenchTune` |
| `LoadingPhase` | `app` | Phase of model loading (used internally by the TUI) |
| `LoadProgress` | `models` | Load progress with `layers_total`, `layers_loaded`, `tensors_loaded` |
| `Samplers` | `models` | Semicolon-separated sampler order string |
| `BenchTuneMode` | `benchmark` | Benchmark mode: `RuntimeOnly` or `Full` |
| `BenchTuneStatus` | `benchmark` | Status: `Running`, `Completed`, or `Error` |

## Main Modules

### `backend::hub`

HuggingFace API integration.

```rust
/// Search models on HuggingFace.
pub async fn search_models(query: &str, limit: u32, offset: u32) -> Result<(Vec<SearchResult>, usize)>

/// List all GGUF files for a model.
pub async fn list_gguf_files(model_id: &str) -> Result<Vec<(String, u64, String)>>

/// Fetch the README for a model from HuggingFace.
pub async fn fetch_readme(model_id: &str) -> Result<String>

/// Download a file with progress tracking.
pub async fn download_file(
    model_id: &str,
    filename: &str,
    url: &str,
    dest: &Path,
    progress: &mut DownloadState,
    download_state: Arc<AtomicU8>,
    tx: broadcast::Sender<DownloadState>,
) -> Result<()>

/// Get available free disk space in bytes for a given path.
pub fn get_free_space_bytes(path: &Path) -> u64

/// Resolve the llama-server binary path for a given backend.
/// Downloads the binary from GitHub releases if not already cached.
pub async fn resolve_backend_binary(
    backend: Backend,
    tag: Option<&str>,
    log_tx: Option<mpsc::Sender<String>>,
    download_tx: Option<broadcast::Sender<DownloadProgressUpdate>>,
) -> Result<PathBuf>
```

### `backend::server`

llama.cpp server process management.

```rust
/// Manages a single llama.cpp server process.
pub struct ServerHandle {
    pub port: u16,
    pub host: String,
    pub pid: u32,
    pub kill_tx: mpsc::Sender<()>,
}

/// Build the full llama-server command line from settings.
pub fn build_server_cmd(
    binary: &Path,
    model: Option<&DiscoveredModel>,
    settings: &ModelSettings,
    config: &Config,
    server_mode: ServerMode,
    router_max_models: u32,
) -> (Command, String)

/// Spawn a llama.cpp server process.
pub async fn spawn_server(
    config: &Config,
    model: Option<&DiscoveredModel>,
    settings: &ModelSettings,
    log_tx: mpsc::Sender<String>,
) -> Result<(ServerHandle, String), String>

/// Check if the server is healthy and responsive.
pub async fn check_health(host: &str, port: u16) -> bool

/// Kill a running server.
pub async fn kill_server(handle: ServerHandle) -> Result<(), String>

/// Poll metrics from the server.
pub async fn get_metrics(
    host: &str,
    port: u16,
    model_name: Option<&str>,
    pid: Option<u32>,
) -> Result<ServerMetrics, String>

/// Load a model via the llama-server Router API.
pub async fn load_model(host: &str, port: u16, model_id: &str, model_path: Option<&str>) -> Result<(), String>

/// List all models and their status from the llama-server Router API.
pub async fn list_models(host: &str, port: u16) -> Result<Vec<(String, String, Option<String>)>, String>

/// Unload a model via the llama-server Router API.
pub async fn unload_model(host: &str, port: u16, model_id: &str, model_path: Option<&str>) -> Result<(), String>
```

### `config`

Configuration loading and saving.

```rust
/// Global configuration.
pub struct Config {
    pub models_dir: PathBuf,
    pub llama_server: PathBuf,
    pub default: DefaultParams,
    pub model_overrides: HashMap<String, ModelOverride>,
    pub profiles: Vec<Profile>,
    pub system_prompt_presets: Vec<SystemPromptPreset>,
    pub rpc_workers: Vec<RpcWorker>,
}

/// A remote RPC worker for distributed inference.
pub struct RpcWorker {
    pub selected: bool,
    pub name: String,
    pub ip: String,
    pub port: u16,
}

/// A named profile of settings.
pub struct Profile {
    pub name: String,
    pub description: String,
    pub settings: ModelOverride,
}

/// A named system prompt preset.
pub struct SystemPromptPreset {
    pub name: String,
    pub description: String,
    pub content: String,
}

/// Per-model settings override (optional fields).
pub struct ModelOverride {
    pub context_length: Option<u32>,
    pub threads: Option<u32>,
    pub temperature: Option<f32>,
    // ... 50+ optional fields
}

/// Built-in profiles with sensible defaults.
pub fn builtin_profiles() -> Vec<Profile>

/// Built-in system prompt presets.
pub fn builtin_system_prompt_presets() -> Vec<SystemPromptPreset>
```

### `backend::ws_server`

WebSocket dashboard server.

```rust
pub struct WsAppState {
    pub metrics_rx: Arc<broadcast::Receiver<WsMetrics>>,
    pub auth_key: Option<String>,
}

pub async fn start_ws_server(
    port: u16,
    metrics_rx: Arc<broadcast::Receiver<WsMetrics>>,
    auth_key: Option<String>,
) -> JoinHandle<()>

pub fn stop_ws_server(handle: JoinHandle<()>)
```

### `backend::benchmark`

Benchmark tuning system.

```rust
/// Configuration for a benchmark run.
pub struct BenchTuneConfig {
    pub model_path: PathBuf,
    pub iterations: usize,
    pub prompt: String,
    pub params: Vec<BenchTuneParam>,
    pub duration: Duration,
    pub mode: BenchTuneMode,
    pub n_predict: usize,
    pub chat_template_kwargs: serde_json::Value,
}

/// A tunable parameter for benchmarking.
pub struct BenchTuneParam {
    pub name: String,
    pub min: f64,
    pub max: f64,
    pub step: f64,
    pub enabled: bool,
}

/// Actual parameter values for a benchmark run.
pub struct BenchTuneParamValue {
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub top_k: Option<u32>,
    pub repeat_penalty: Option<f64>,
    pub context_length: Option<u32>,
    pub batch_size: Option<u32>,
    pub threads: Option<u32>,
    pub flash_attn: Option<bool>,
    pub expert_count: Option<i32>,
}

/// Results from a benchmark run.
pub struct BenchTuneResult {
    pub params: BenchTuneParamValue,
    pub metrics: BenchTuneMetrics,
    pub outputs: Vec<String>,
    pub per_iteration_metrics: Vec<BenchTuneMetrics>,
    pub base_settings: BenchTuneParamValue,
}

/// Metrics from a benchmark run.
pub struct BenchTuneMetrics {
    pub prompt_tps: f64,
    pub generation_tps: f64,
    pub combined_tps: f64,
    pub latency_per_token: f64,
    pub first_token_time: f64,
}
```

### `models`

Domain types and utilities.

```rust
/// Estimate VRAM usage (in MiB) for a model with the given settings.
pub fn estimate_vram_mib(
    model_mib: u64,
    settings: &ModelSettings,
    total_layers: u32,
    hidden_size_opt: Option<u32>,
    n_head_opt: Option<u32>,
    n_kv_head_opt: Option<u32>,
    gpu_mem_total_mib: u64,
) -> u64

/// Format bytes as MB or GB.
pub fn format_mib(mib: u64) -> String
```

## Configuration

Configuration is stored in `~/.config/llm-manager/config.yaml` and loaded via `Config::load()`. The config file structure:

```yaml
models_dir: ~/.local/share/llm-manager/models
llama_server: llama-server
default:
  context_length: 32096
  threads: <physical cores>
  # ... more default parameters
model_overrides:
  model.gguf:
    temperature: 0.7
    gpu_layers: 32
profiles:
  - name: Qwen
    description: Optimized for Qwen models
    settings:
      temperature: 0.6
      top_k: 20
rpc_workers:
  - name: Remote-GPU-1
    ip: 192.168.1.50
    port: 50052
    selected: true
system_prompt_presets:
  - name: General
    description: General-purpose assistant
    content: "You are a helpful assistant."
```

Built-in profiles are merged on load, so adding new ones in code automatically appears in the UI.
