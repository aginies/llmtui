## Issues Found

### 1. ModelSettings has 75 fields in a single struct

**File:** `src/models.rs` — `ModelSettings`

**Problem:** Enormous struct. Adding a field requires updating 12 locations (documented in a comment). Violates single responsibility and makes the struct unwieldy.

**Proposed:** Decompose into nested structs or use a config-map pattern. Group related fields: `LoadingConfig`, `GpuConfig`, `SamplingConfig`, `ServerConfig`. Use `#[serde(flatten)]` to keep YAML compatibility.

---

### 2. key.rs is 2459 lines of nested match statements

**File:** `src/tui/event/key.rs`

**Problem:** The key handler is one massive function with deeply nested mode checks. Adding a new overlay mode requires editing this file.

**Proposed:** Extract each overlay handler into its own function (already partially done with `handle_prompt_picker_key`, `handle_bench_tune_setup_key`, etc.). Route overlay dispatch through a lookup table or trait-based dispatcher.

---

### 4. Log-based parsing is fragile

**File:** `src/tui/app/state.rs:39-192`

**Problem:** `detect_loading_phases()` and `parse_loading_details()` parse llama.cpp stdout using string matching (`upper.contains("LOAD_TENSORS:")`). Any llama.cpp output change breaks detection.

**Proposed:** Use JSON logging from llama.cpp if available, or implement a more robust parser with regex patterns. Add integration tests that validate against actual llama.cpp output samples.

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

### 7. No rate limiting on API proxy

**File:** `src/serve_api.rs`

**Problem:** The OpenAI-compatible API proxy has no rate limiting, no request size validation beyond 10MB, and no authentication (unless `--api-key` is explicitly provided).

**Proposed:** Add configurable rate limiting (token bucket). Add request validation middleware. Require `--api-key` by default with an `--no-auth` flag for opt-out.

---

### 9. Magic numbers scattered

**Locations:**
- `SCROLL_TICK_MS = 870` (app.rs:214)
- Health check: 120 iterations x 500ms = 60s (benchmark.rs)
- Log trim at 500 entries (state.rs:336)
- Phase weights hardcoded as `[(LoadingPhase, f32); 5]` (state.rs:229)

**Proposed:** Extract to named constants or a config struct. Document the reasoning behind each value.

---

### 13. Download pause only works between chunks

**File:** `src/backend/hub.rs`

**Problem:** Download pause uses `AtomicU8` but the download loop only checks between chunks. A download in progress cannot be paused mid-chunk.

**Proposed:** Document this limitation. Consider using `tokio::sync::watch` for finer-grained control, or at minimum show a "pausing..." indicator.

---

### 14. Router API tries many endpoint variants

**File:** `src/backend/server.rs`

**Problem:** `load_model()` tries multiple endpoint variants (`/models/load`, `/v1/models/load`), multiple model identification variants (full name, stripped, filename, absolute path), and both `model` and `alias` JSON fields. This is defensive but fragile.

**Proposed:** Use llama.cpp's documented API version detection (`/props` endpoint) to determine which endpoints are available, rather than trying all variants.

---

### 15. Missing test coverage for critical paths

**Problem:**
- No integration tests for server spawning
- No tests for backend binary resolution (network-dependent)
- No tests for HuggingFace search (network-dependent)
- No tests for the main event loop

**Proposed:** Add mock-based tests for `build_server_cmd()` with expected output verification. Consider using `wiremock` (already a dev-dependency) for API endpoint tests.

---
