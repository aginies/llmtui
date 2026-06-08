use regex::Regex;
use std::sync::LazyLock;

fn compile(pattern: &str) -> Regex {
    Regex::new(pattern).expect("invalid regex pattern")
}

// ── Phase detection patterns ──────────────────────────────────────

/// Matches llama.cpp server startup lines: "llama server", "ggml version", etc.
pub static LLAMA_START: LazyLock<Regex> = LazyLock::new(|| {
    compile(r"(?i)(llama.*server|ggml\s+version)")
});

/// Matches model loader start: "llama_model_loader" (old format)
/// or "load_model: loading model" (new format)
pub static LOADING_MODEL: LazyLock<Regex> = LazyLock::new(|| {
    compile(r"(?i)(llama_model_loader|load_model:\s*loading\s*model)")
});

/// Matches metadata loaded: "loaded X meta data" (old format)
/// or "Loaded meta data with N key-value pairs" (new format)
/// or "fitting params to device memory" (new format)
pub static LOADED_META: LazyLock<Regex> = LazyLock::new(|| {
    compile(r"(?i)(loaded\s+\d+\s+meta data|Loaded meta data with|fitting params to device memory)")
});

/// Matches tensor loading start: "load tensors:" or "loading model tensors"
pub static LOAD_TENSORS: LazyLock<Regex> = LazyLock::new(|| {
    compile(r"(?i)(load\s*tensors:|load_tensors:|loading model tensors)")
});

/// Matches "server listening on" or "http server listening"
pub static SERVER_LISTENING: LazyLock<Regex> = LazyLock::new(|| {
    compile(r"(?i)(server listening|http server listening|load_model:\s*initializing\s+slots)")
});

// ── Detail parsing patterns ───────────────────────────────────────

/// Matches "loading tensor 12 of 345" or "loading tensor 12 out of 345"
pub static LOADING_TENSOR: LazyLock<Regex> =
    LazyLock::new(|| compile(r"(?i)loading tensor\s+(\d+)\s+(?:of|out of)\s+(\d+)"));

/// Matches "offloading 32 repeating layers to GPU"
pub static OFFLOADING_LAYERS: LazyLock<Regex> =
    LazyLock::new(|| compile(r"(?i)offloading\s+(\d+)\s+repeating layers"));

/// Matches "offloaded X/Y layers" or "offloaded X out of Y layers"
pub static OFFLOADED_LAYERS: LazyLock<Regex> = LazyLock::new(|| {
    compile(r"(?i)offloaded\s+(\d+)\s*(?:out\s+of|/)\s*(\d+)\s*layers")
});

/// Matches "Vulkan0 model buffer size = 12345.67 MiB" or "CPU model buffer size = 12345.67 MiB"
pub static MODEL_BUFFER_SIZE: LazyLock<Regex> = LazyLock::new(|| {
    compile(r"(?i)((?:vulkan\d|roc\d|cuda|cpu|metal)?(?:\s*))model buffer size\s*=\s*([\d.]+)\s*MiB")
});

/// Matches "kv buffer size = 1234.56 MiB"
pub static KV_BUFFER_SIZE: LazyLock<Regex> = LazyLock::new(|| {
    compile(r"(?i)kv buffer size\s*=\s*([\d.]+)\s*MiB")
});

// ── Error detection ───────────────────────────────────────────────

/// Detects whether a log line indicates a loading error.
pub fn is_loading_error(msg: &str) -> bool {
    let lower = msg.to_lowercase();
    lower.contains("out of memory")
        || lower.contains("out_of_memory")
        || lower.contains("out_of_device_memory")
        || lower.contains("outofmemory")
        || lower.contains("outofdevicememory")
        || lower.contains("error")
        || lower.contains("failed to load")
        || lower.contains("exception")
        || lower.contains("vk::systemerror")
}

/// Detects whether the error is specifically an OOM.
pub fn is_oom_error(msg: &str) -> bool {
    let lower = msg.to_lowercase();
    lower.contains("out of memory")
        || lower.contains("out_of_memory")
        || lower.contains("out_of_device_memory")
        || lower.contains("outofmemory")
        || lower.contains("outofdevicememory")
}
