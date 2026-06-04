## Issues Found

### 1. ModelSettings has 75 fields in a single struct

**File:** `src/models.rs` — `ModelSettings`

**Problem:** Enormous struct. Adding a field requires updating 12 locations (documented in a comment). Violates single responsibility and makes the struct unwieldy.

**Proposed:** Decompose into nested structs or use a config-map pattern. Group related fields: `LoadingConfig`, `GpuConfig`, `SamplingConfig`, `ServerConfig`. Use `#[serde(flatten)]` to keep YAML compatibility.

---

### 5. Hardcoded backend version tags

**File:** `src/backend/hub.rs`

**Problem:** Default tags (b4100 for llama.cpp, b9279 for CUDA, b1273 for ROCm Lemonade) are hardcoded. These become stale.

**Proposed:** Fetch latest tag from GitHub API at startup, cache it, and allow config override. Or at minimum, log a warning when a hardcoded tag is older than 30 days.

---

### 6. Config resolution uses 3 macros for field sync

**File:** `src/config.rs`

**Problem:** `apply_scalar!`, `apply_clone!`, `apply_option!` macros handle field synchronization between `ModelSettings` and `ModelOverride`. Adding a field means updating all 3 macros plus the field count test.

**Proposed:** Derive a `FieldDescriptor` trait that lists all field names/types. Use reflection-like iteration to apply overrides. Or use `serde_json::Value` as an intermediate representation.

---

### 13. Download pause only works between chunks

**File:** `src/backend/hub.rs`

**Problem:** Download pause uses `AtomicU8` but the download loop only checks between chunks. A download in progress cannot be paused mid-chunk.

**Proposed:** Document this limitation. Consider using `tokio::sync::watch` for finer-grained control, or at minimum show a "pausing..." indicator.

---

### 15. Missing test coverage for critical paths

**Problem:**
- No integration tests for server spawning
- No tests for backend binary resolution (network-dependent)
- No tests for HuggingFace search (network-dependent)
- No tests for the main event loop

**Proposed:** Add mock-based tests for `build_server_cmd()` with expected output verification. Consider using `wiremock` (already a dev-dependency) for API endpoint tests.

---
