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
| `DiscoveredModel` | `models` | A discovered `.gguf` file with path, name, file_size, and display name |
| `ModelSettings` | `models` | All settings for loading a model via llama.cpp server (70+ fields) |
| `ModelState` | `models` | State of a model: `Available`, `Loading`, `Benchmarking`, `Loaded`, or `Failed` |
| `SearchResult` | `models` | A model found via HuggingFace search |
| `DownloadState` | `models` | Download progress tracking with cancellation support |
| `GgufMetadata` | `models` | Parsed GGUF metadata (layers, hidden size, context, etc.) |
| `ServerMetrics` | `models` | Metrics from the llama.cpp server (TPS, VRAM, CPU, context, latency, prompt progress) |
| `WsMetrics` | `models` | WebSocket-friendly metrics snapshot (serializable, includes settings, command display, timestamp) |
| `LogEntry` | `config` | A single log entry with timestamp, level, and message |
| `GPUBuffer` | `models` | GPU device buffer reported during model loading (device, buffer_size_mib) |
| `LoadProgress` | `models` | Progress during model loading (layers_total, layers_loaded, tensors_total, tensors_loaded, buffers) |

### Enums

| Type | Module | Description |
|------|--------|-------------|
| `Backend` | `models` | Acceleration backend: `Cpu`, `Vulkan`, `Rocm`, `RocmLemonade`, `Cuda`, `CpuArm64`, `CpuWindows`, `VulkanWindows`, `CudaWindows12_4`, `CudaWindows13_1`, `HipWindows`, `CpuMacosArm64`, `CpuMacosX64` |
| `ServerMode` | `models` | Server operating mode: `Normal` (single model), `Router` (multiple, *Work In Progress*), `Bench` (GPU benchmarking), or `BenchTune` (parameter auto-tuning) |
| `GpuLayersMode` | `models` | GPU offloading: `Auto`, `Specific(n)`, or `All` |
| `SearchSort` | `models` | Search result sort order: `Relevance`, `Downloads`, `Likes`, `Trending`, `CreatedAt` |
| `ListSort` | `models` | Local model list sort order: `Name`, `Size`, `Modified` |
| `CacheType` | `models` | Main KV cache data type: `F16`, `BF16`, `Fq8_0`, `Fq4_1` |
| `CacheQuantType` | `models` | KV cache data type for quantization (F32, F16, BF16, Q8_0, Q5_0, Q5_1, Q4_0, Q4_1, Iq4Nl) |
| `CacheTypeK` / `CacheTypeV` | `models` | Type aliases for `CacheQuantType` (used for keys and values) |
| `SplitMode` | `models` | Multi-GPU split mode: `None`, `Layer`, `Row`, `Tensor` |
| `NumMode` | `models` | NUMA optimization: `None`, `Distribute`, `Isolate`, `Numactl` |
| `RopeScaling` | `models` | RoPE frequency scaling: `None`, `Linear`, `Yarn` |
| `Mirostat` | `models` | Mirostat version: `Off`, `V1`, `Mirostat2` |
| `ActivePanel` | `app` | Focused panel: `Models`, `Log`, `ServerSettings`, `LlmSettings`, `Profiles`, `SystemPromptPresets`, `SearchReadme`, `ActiveModel`, `ModelInfo`, `Downloads` |
| `ConfirmationKind` | `app` | Confirmation dialog type: `Exit`, `Reset`, `Delete`, `Unload`, `DeleteBackend` |
| `LoadingPhase` | `app` | Phase of model loading: `ServerStarting`, `LoadingModel`, `LoadingMeta`, `LoadingTensors`, `ServerListening`, `Complete` |
| `LoadProgress` | `models` | Load progress with `layers_total`, `layers_loaded`, `tensors_loaded` |
| `Samplers` | `models` | Semicolon-separated sampler order string |
| `BenchTuneMode` | `benchmark` | Benchmark mode: `RuntimeOnly` or `Full` (default: `Full`) |
| `BenchTuneStatus` | `benchmark` | Status: `Running`, `Completed`, `PartiallyCompleted`, `Cancelled`, or `Error` |
| `WebSearchCheckStatus` | `app` | Web search status: `Checking`, `Ok`, `Error(String)` |

## Main Modules

### `backend::hub`

HuggingFace API integration.

```rust
/// Search models on HuggingFace.
pub async fn search_models(query: &str, limit: u32, offset: u32) -> Result<(Vec<SearchResult>, usize, Vec<String>)> // third element: raw model IDs for post-filtering

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
    progress_tx: Option<tokio::sync::broadcast::Sender<crate::models::DownloadState>>,
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

/// Request to spawn a llama.cpp server process.
pub struct SpawnServerRequest<'a> {
    pub config: &'a Config,
    pub model: Option<&'a DiscoveredModel>,
    pub settings: &'a ModelSettings,
    pub log_tx: mpsc::Sender<String>,
    pub progress_tx: Option<tokio::sync::broadcast::Sender<DownloadState>>,
    pub server_mode: ServerMode,
    pub router_max_models: u32,
    pub exit_tx: mpsc::Sender<()>,
}

/// Spawn a llama.cpp server process.
pub async fn spawn_server(request: SpawnServerRequest) -> Result<(ServerHandle, String), String>

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
    pub models_dirs: Vec<PathBuf>,
    pub llama_server: PathBuf,
    pub default: DefaultParams,
    pub model_overrides: ModelConfigStore,
    pub profiles: ProfileStore,
    pub system_prompt_presets: PresetStore,
    pub rpc_workers: Vec<RpcWorker>,
    pub search_limit: u32,
    pub active_panel: ActivePanel,
    pub left_pct: u16,
    pub language: String,        // UI language (en, fr, it, de)
    pub onboarding_complete: bool,
}

/// Default parameters for new models.
pub struct DefaultParams {
    pub context_length: u32,
    pub threads: u32,
    pub threads_batch: u32,
    pub batch_size: u32,
    pub ubatch_size: u32,
    pub parallel: u32,
    pub max_concurrent_predictions: Option<u32>,
    pub temperature: f32,
    pub top_k: i32,
    pub top_p: f32,
    pub min_p: f32,
    pub typical_p: f32,
    pub seed: i32,
    pub repeat_penalty: f32,
    pub repeat_last_n: i32,
    pub presence_penalty: f32,
    pub frequency_penalty: f32,
    pub dry_multiplier: f32,
    pub dry_base: f32,
    pub dry_allowed_length: i32,
    pub dry_penalty_last_n: i32,
    pub rope_scaling: RopeScaling,
    pub rope_scale: f32,
    pub rope_freq_base: f32,
    pub rope_freq_scale: f32,
    pub rope_yarn_enabled: bool,
    pub host: String,
    pub port: u16,
    pub timeout: u32,
    pub cache_prompt: bool,
    pub cache_reuse: u32,
    pub webui: bool,
    pub ws_server_enabled: bool,
    pub ws_server_port: u16,
    pub server_tls_enabled: bool,
    pub server_tls_cert: Option<String>,
    pub server_tls_key: Option<String>,
    pub router_max_models: u32,
    pub server_mode: ServerMode,
    pub max_tokens: Option<u32>,
    pub cache_type: CacheType,
    pub backend: Backend,
    pub platform: Option<String>,
    pub llama_cpp_version_cpu: Option<String>,
    pub llama_cpp_version_vulkan: Option<String>,
    pub llama_cpp_version_rocm: Option<String>,
    pub llama_cpp_version_rocm_lemonade: Option<String>,
    pub llama_cpp_version_cuda: Option<String>,
    pub api_endpoint_enabled: bool,
    pub api_endpoint_port: u16,
    pub web_search_engine: String,
    pub web_search_engine_url: String,
    pub web_search_enabled: bool,
    pub web_search_api_key: Option<String>,
    pub api_endpoint_key: Option<String>,
    pub spec_type: String,
    pub draft_tokens: u32,
    pub tags: Vec<String>,
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

impl Profile {
    pub fn apply(&self, base: ModelSettings) -> ModelSettings
}

/// A named system prompt preset.
pub struct SystemPromptPreset {
    pub name: String,
    pub description: String,
    pub content: String,
}

/// Per-model settings override (optional fields).
pub struct ModelOverride {
    // Loading
    pub context_length: Option<u32>,
    pub batch_size: Option<u32>,
    pub ubatch_size: Option<u32>,
    pub cache_type_k: Option<CacheTypeK>,
    pub cache_type_v: Option<CacheTypeV>,
    pub keep: Option<i32>,
    pub swa_full: Option<bool>,
    pub mlock: Option<bool>,
    pub mmap: Option<bool>,
    pub numa: Option<NumMode>,
    pub uniform_cache: Option<bool>,
    pub system_prompt: Option<String>,
    pub system_prompt_preset_name: Option<String>,
    pub max_concurrent_predictions: Option<u32>,
    pub threads: Option<u32>,
    pub threads_batch: Option<u32>,
    pub parallel: Option<u32>,
    // GPU
    pub gpu_layers: Option<i32>,
    pub split_mode: Option<SplitMode>,
    pub tensor_split: Option<String>,
    pub main_gpu: Option<i32>,
    pub fit: Option<bool>,
    pub lora: Option<PathBuf>,
    pub lora_scaled: Option<(PathBuf, f32)>,
    pub rpc: Option<String>,
    pub embedding: Option<bool>,
    pub kv_cache_offload: Option<bool>,
    pub flash_attn: Option<bool>,
    pub jinja: Option<bool>,
    pub auto_chat_template: Option<bool>,
    pub chat_template: Option<String>,
    pub chat_template_kwargs: Option<String>,
    pub expert_count: Option<i32>,
    pub gpu_layers_mode: Option<GpuLayersMode>,
    // Sampling
    pub seed: Option<i32>,
    pub temperature: Option<f32>,
    pub top_k: Option<i32>,
    pub top_p: Option<f32>,
    pub min_p: Option<f32>,
    pub typical_p: Option<f32>,
    pub mirostat: Option<Mirostat>,
    pub mirostat_lr: Option<f32>,
    pub mirostat_ent: Option<f32>,
    pub ignore_eos: Option<bool>,
    pub samplers: Option<Samplers>,
    // Repetition
    pub repeat_penalty: Option<f32>,
    pub repeat_last_n: Option<i32>,
    pub presence_penalty: Option<f32>,
    pub frequency_penalty: Option<f32>,
    pub dry_multiplier: Option<f32>,
    pub dry_base: Option<f32>,
    pub dry_allowed_length: Option<i32>,
    pub dry_penalty_last_n: Option<i32>,
    // RoPE
    pub rope_scaling: Option<RopeScaling>,
    pub rope_scale: Option<f32>,
    pub rope_freq_base: Option<f32>,
    pub rope_freq_scale: Option<f32>,
    pub rope_yarn_enabled: Option<bool>,
    // Server
    pub cache_prompt: Option<bool>,
    pub cache_reuse: Option<u32>,
    pub webui: Option<bool>,
    // Other
    pub max_tokens: Option<u32>,
    pub cache_type: Option<CacheType>,
    pub llama_cpp_version_cpu: Option<String>,
    pub llama_cpp_version_vulkan: Option<String>,
    pub llama_cpp_version_rocm: Option<String>,
    pub llama_cpp_version_rocm_lemonade: Option<String>,
    pub llama_cpp_version_cuda: Option<String>,
    pub spec_type: Option<String>,
    pub draft_tokens: Option<u32>,
    pub tags: Option<Vec<String>>,
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
    tls_config: Option<axum_server::tls_rustls::RustlsConfig>,
    host: String,
) -> Result<JoinHandle<()>>
```

### `backend::benchmark`

Benchmark tuning system.

```rust
/// Configuration for a benchmark run.
pub struct BenchTuneConfig {
    pub model_path: PathBuf,
    pub num_iterations: u32,
    pub prompt: String,
    pub params_to_test: Vec<BenchTuneParam>,
    pub test_duration: Duration,
    pub bench_mode: BenchTuneMode,
    pub n_predict: u32,
    pub chat_template_kwargs: Option<String>,
    pub test_timeout: Duration,
}

/// A tunable parameter for benchmarking.
pub struct BenchTuneParam {
    pub name: String,
    pub min: f64,
    pub max: f64,
    pub step: f64,
    pub enabled: bool,
    pub variants: Vec<String>,
}

/// Actual parameter values for a benchmark run.
pub struct BenchTuneParamValue {
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub top_k: Option<i64>,
    pub repeat_penalty: Option<f64>,
    pub context_length: Option<u32>,
    pub batch_size: Option<u32>,
    pub flash_attn: Option<bool>,
    pub threads: Option<u32>,
    pub expert_count: Option<i32>,
    pub spec_type: Option<String>,
    pub draft_tokens: Option<u32>,
}

/// Results from a benchmark run.
pub struct BenchTuneResult {
    pub params: BenchTuneParamValue,
    pub metrics: BenchTuneMetrics,
    pub outputs: Vec<String>,
    pub per_iteration_metrics: Vec<BenchTuneMetrics>,
    pub base_settings: Option<ModelSettings>,
    pub server_command: Option<String>,
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

### `backend::tls`

TLS certificate management.

```rust
/// Load TLS config from PEM certificate and key files.
pub fn load_tls_config(cert_path: &Path, key_path: &Path) -> Result<RustlsConfig>

/// Generate a self-signed CA (certificate + key).
pub fn generate_ca() -> Result<(String, String)>

/// Sign a server certificate with the CA.
pub fn generate_server_cert(ca_cert: &str, ca_key: &str) -> Result<(String, String)>

/// Ensure TLS certs exist, auto-generating if missing.
pub fn ensure_tls_certs() -> Result<(PathBuf, PathBuf)>

/// Validate a TLS certificate/key path.
pub fn validate_tls_path(path: &Path) -> Result<()>

/// Try to load TLS config from paths, returning None if paths are empty.
pub fn try_load_tls(cert_path: &str, key_path: &str) -> Result<Option<RustlsConfig>>
```

### `backend::web_search`

Web search integration with SearXNG.

```rust
/// Search using the configured web search engine.
pub async fn search_web(query: &str, engine_url: &str, api_key: Option<&str>) -> Result<WebSearchResults>

/// Parse SearXNG JSON response into search results.
pub fn parse_searxng_response(json: &str) -> Result<WebSearchResults>
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

/// Format host for display ("" or "127.0.0.1" -> "localhost").
pub fn format_host(host: &str) -> String
```

## ServerMetrics

Metrics collected from the llama.cpp server:

```rust
pub struct ServerMetrics {
    pub loaded: bool,
    pub tps: f64,
    pub prompt_tps: f64,
    pub cpu_usage: f64,
    pub gpu_mem_used: u64,
    pub gpu_mem_total: u64,
    pub ram_used: u64,
    pub ctx_used: u32,
    pub ctx_max: u32,
    pub total_vram_used: u64,           // Sum across all loaded models
    pub decoded_tokens: u64,            // Tokens from print_timing logs
    pub gen_tps: f64,                   // Generation TPS from log parsing
    pub latency_per_token_ms: f64,      // Estimated latency per token
    pub prompt_latency_ms: f64,         // Prompt processing latency
    pub prompt_tokens: u64,             // Tokens in prompt being evaluated
    pub prompt_progress: f64,           // Progress of prompt evaluation (0.0-1.0)
    pub prompt_elapsed_ms: f64,         // Elapsed prompt evaluation time
    pub prompt_tps_eval: f64,           // Prompt evaluation throughput
}
```

## Configuration

Configuration is stored in `~/.config/llm-manager/config.yaml` and loaded via `Config::load()`. The config file structure:

```yaml
models_dirs:
  - ~/.local/share/llm-manager/models
llama_server: llama-server
default:
  context_length: 131072
  threads: <physical cores>
  # ... more default parameters
  ws_server_enabled: false
  ws_server_port: 49223
  server_tls_enabled: true
  api_endpoint_enabled: false
  api_endpoint_port: 49222
  web_search_enabled: false
  web_search_engine: searxng
  web_search_engine_url: ""
  spec_type: "draft-mtp"
  draft_tokens: 0
  tags: []
model_overrides:
  # Per-model configs stored as individual YAML files in ~/.config/llm-manager/models/
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
language: en
onboarding_complete: true
search_limit: 50
active_panel: Models
left_pct: 55
```

Built-in profiles are merged on load, so adding new ones in code automatically appears in the UI.
