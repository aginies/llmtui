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

/// Matches model loader start: "llama_model_loader"
pub static LOADING_MODEL: LazyLock<Regex> = LazyLock::new(|| {
    compile(r"(?i)llama_model_loader")
});

/// Matches metadata loaded: "loaded X meta", "meta data"
pub static LOADED_META: LazyLock<Regex> = LazyLock::new(|| {
    compile(r"(?i)(loaded\s+\d+\s+meta|meta\s+data)")
});

/// Matches tensor loading start: "load tensors:"
pub static LOAD_TENSORS: LazyLock<Regex> = LazyLock::new(|| {
    compile(r"(?i)load tensors:")
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

// ── Error detection patterns ──────────────────────────────────────

pub static ERROR_OOM: LazyLock<Regex> =
    LazyLock::new(|| compile(r"(?i)(out\s+of\s+(device\s+)?memory|outof(devic)?memory|vk\s*outofmemory)"));

pub static ERROR_GENERIC: LazyLock<Regex> = LazyLock::new(|| {
    compile(r"(?i)(error|failed\s+to\s+load|exception|vk::systemerror)")
});

fn normalize(msg: &str) -> String {
    msg.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c.is_ascii_whitespace() { c } else { ' ' })
        .collect()
}

/// Detects whether a log line indicates a loading error.
pub fn is_loading_error(msg: &str) -> bool {
    let n = normalize(msg);
    ERROR_OOM.is_match(&n) || ERROR_GENERIC.is_match(&n)
}

/// Detects whether the error is specifically an OOM.
pub fn is_oom_error(msg: &str) -> bool {
    let n = normalize(msg);
    ERROR_OOM.is_match(&n)
}
