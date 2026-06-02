# Code Review Report — llm-manager

**Date:** 2026-06-02  
**Scope:** `src/` directory and all subdirectories

---

## Summary

| Severity | Count |
|----------|-------|
| Critical | 3 |
| High | 9 |
| Medium | 12 |
| Low | 15 |
| Cosmetic | 8 |

---

## CRITICAL

### C1. SSE streaming responses bypass CORS headers

**File:** `src/serve_api.rs:113-121`

SSE streaming responses are constructed as a raw `axum::response::Response` that bypasses the `CorsLayer`. The response headers come directly from the backend, meaning `Access-Control-Allow-Origin` and other CORS headers are NOT injected. Browser-based clients using streaming endpoints will receive CORS errors.

```rust
// Line 113-121: SSE response bypasses CORS layer
if is_sse {
    let mut response = axum::response::Response::new(Body::from_stream(
        resp.bytes_stream().map(|result| {
            result.map_err(std::io::Error::other)
        }),
    ));
    *response.status_mut() = status;
    *response.headers_mut() = headers;  // <-- backend headers only, no CORS
    response
}
```

**Fix:** Manually inject CORS headers into the SSE response, or restructure the router so the `CorsLayer` wraps the streaming handler.

---

### C2. `/health` and `/metrics` endpoints are unauthenticated

**File:** `src/serve_api.rs:258-276`

The router structure places `/health` and `/metrics` on the top-level `Router`, outside the auth middleware and CORS layer. Anyone can probe server health and read backend metrics without credentials.

```rust
let app = Router::new()
    .route("/health", get(health))       // <-- no auth
    .route("/metrics", get(proxy_streaming)) // <-- no auth
    .merge(Router::new()
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .layer(axum::middleware::from_fn_with_state(state.clone(), auth_middleware))
        // ... protected routes
    );
```

**Fix:** Move these routes inside the auth-protected sub-router, or add explicit authentication logic to these handlers.

---

### C3. Unrestricted CORS (`allow_origin(Any)`)

**File:** `src/serve_api.rs:230-242`

```rust
let cors = CorsLayer::new()
    .allow_origin(tower_http::cors::Any)  // <-- any origin allowed
    .allow_methods([GET, POST, PUT, DELETE, OPTIONS])
    .allow_headers([CONTENT_TYPE, AUTHORIZATION]);
```

Any website can make cross-origin requests to the API proxy. Combined with Bearer token authentication, a malicious page could exploit a user's browser if the API key is embedded client-side.

**Fix:** Restrict to specific trusted origins or make the allowed origins configurable.

---

## HIGH

### H1. Panic: `CString::new(...).unwrap()` in `get_free_space_bytes`

**File:** `src/backend/hub.rs:13`

```rust
let c_path = std::ffi::CString::new(path_str.as_ref()).unwrap();
```

Panics if the path contains an interior null byte. While unlikely, a maliciously crafted path could trigger this.

**Fix:** Use `unwrap_or_else` with a proper error return, or return `Result<u64, ...>`.

---

### H2. Panic: `binary.parent().unwrap()` in serve mode

**File:** `src/serve.rs:313`

```rust
let bin_dir = binary.parent().unwrap();
```

If `--backend-binary` is a bare filename (e.g., `llama-server`), `parent()` returns `None` and `unwrap()` panics.

**Fix:** Use `context()` or `ok_or_else()` to return a proper error message.

---

### H3. `physical_cores()` silently returns `1` on non-Linux

**File:** `src/config.rs:23-46`

```rust
pub fn physical_cores() -> u32 {
    let content = match std::fs::read_to_string("/proc/cpuinfo") {
        Ok(c) => c,
        Err(_) => return 1,  // <-- silent fallback to 1 thread
    };
```

On macOS, Windows, or WSL without `/proc/cpuinfo`, the default thread count is `1`, severely degrading performance. No log message indicates this fallback.

**Fix:** Use `std::thread::available_parallelism()` as a cross-platform fallback, or use the `sysinfo` crate.

---

### H4. Proxy request has no timeout

**File:** `src/serve_api.rs:97-100`

The `reqwest::Client` is built with only `pool_max_idle_per_host(20)`. No `.timeout()` is set. A slow or unresponsive llama-server backend will hold proxy connections open indefinitely, eventually exhausting the connection pool.

**Fix:** Add `.timeout(Duration::from_secs(300))` to the client builder.

---

### H5. All YAML store operations silently discard errors

**File:** `src/config/store.rs:14-64`

Every file operation in the store silently ignores errors:

```rust
// Line 16: read_dir error silently ignored
Err(_) => return map,

// Line 45: remove silently ignored
let _ = std::fs::remove_file(&unused_path);

// Line 50: directory creation silently ignored
let _ = std::fs::create_dir_all(parent);

// Lines 51-52: serialization + write silently ignored
if let Ok(content) = serde_yaml::to_string(item) {
    let _ = std::fs::write(&path, content);
}
```

If the disk is full, permissions are wrong, or the filesystem is read-only, configuration changes are silently lost with no indication to the user.

**Fix:** Return `Result` types and propagate errors, or at minimum log warnings.

---

### H6. `dirs::config_dir().unwrap_or_default()` creates paths under `/`

**Files:** `src/config/model_config.rs:11-14`, `src/config/profiles.rs:12-14`, `src/config/presets.rs:12-14`, `src/config.rs:944-947`

When `dirs::config_dir()` returns `None`, `unwrap_or_default()` gives `PathBuf::new()` (empty path). Subsequent `.join("llm-manager")` produces a relative path `llm-manager/...` which resolves relative to the CWD. If CWD is `/`, this writes to the root filesystem.

**Fix:** Use a hardcoded fallback like `/etc/llm-manager/` or `~/.config/llm-manager/`.

---

### H7. Auth key partially logged (8 characters)

**File:** `src/serve.rs:253-257`

```rust
let auth_info = if let Some(ref auth) = ws_auth {
    format!(" (auth: {})", &auth[..auth.len().min(8)])
} else {
    String::new()
};
```

The first 8 characters of the WebSocket auth key are logged at `info!` level.

**Fix:** Redact the auth key entirely or show only a hash.

---

### H8. `Config::save()` persists built-in profiles to disk as user files

**File:** `src/config.rs:1167-1173`

`self.profiles.all()` returns both built-in AND user profiles. Every `Config::save()` call writes all built-in profiles (Qwen, Gemma, Llama, etc.) to disk as user-owned YAML files. Users cannot distinguish built-in from user-created profiles.

**Fix:** Only save user-created profiles, or filter out built-in names during save.

---

### H9. Unsafe `libc::statvfs` call without validation

**File:** `src/backend/hub.rs:15-25`

```rust
let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
let result = unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) };
```

Uses `std::mem::zeroed()` instead of `MaybeUninit`. While `statvfs` is a C struct that should be zeroed, `MaybeUninit` is the correct Rust pattern.

**Fix:** Use `MaybeUninit::<libc::statvfs>::uninit().assume_init()` or `std::mem::zeroed()` with a safety comment.

---

## MEDIUM

### M1. Unsupported HTTP methods silently converted to GET

**File:** `src/serve_api.rs:81-87`

```rust
let mut request_builder = match method {
    axum::http::Method::GET => state.client.get(&url),
    axum::http::Method::POST => state.client.post(&url),
    axum::http::Method::PUT => state.client.put(&url),
    axum::http::Method::DELETE => state.client.delete(&url),
    _ => state.client.get(&url),  // <-- PATCH, HEAD, OPTIONS become GET
};
```

**Fix:** Return `501 Not Implemented` or `405 Method Not Allowed` for unsupported methods.

---

### M2. Proxy drops all request headers except Content-Type

**File:** `src/serve_api.rs:89-95`

Only `Content-Type` is forwarded. Headers like `Accept`, `X-Request-Id`, and custom headers are dropped, potentially breaking clients that depend on header passthrough.

**Fix:** Iterate over request headers and forward them (excluding hop-by-hop headers like `Host`, `Connection`, `Authorization`).

---

### M3. `body_bytes.clone()` — unnecessary memory allocation

**File:** `src/serve_api.rs:79`

```rust
let body_stream = stream::iter(vec![Ok::<_, reqwest::Error>(body_bytes.clone())]);
```

The `body_bytes` are cloned even though the original is not used afterward.

**Fix:** Move `body_bytes` instead of cloning: `stream::iter(vec![Ok::<_, reqwest::Error>(body_bytes)])`.

---

### M4. `child.kill()` called on already-exited process

**File:** `src/serve.rs:467`

After `child.wait()` completes, the child has already exited. Calling `kill()` is a no-op but misleading.

**Fix:** Remove the dead code.

---

### M5. `ws_server_handle.abort()` called twice

**File:** `src/serve.rs:458-459, 476-478`

`abort()` is called once in the Ctrl+C branch and once after the main loop. The second call is a no-op.

**Fix:** Use a single cleanup path.

---

### M6. `O(n*m)` complexity in profile/preset `all()` methods

**Files:** `src/config/profiles.rs:69-78`, `src/config/presets.rs:78-87`

For each user profile, the entire builtin list is scanned linearly.

**Fix:** Cache builtin names in a `HashSet<String>` for O(1) lookups.

---

### M7. `normalize_config` writes to disk on every config load

**File:** `src/config.rs:1071-1098`

Every app start writes built-in profiles and presets to disk if they don't exist. This causes issues with read-only filesystems.

**Fix:** Defer writes until the first explicit save, or log a warning when writes fail.

---

### M8. `ModelOverride::from_settings` always wraps scalars in `Some`

**File:** `src/config.rs:299-382`

Every scalar field is wrapped in `Some(...)` regardless of whether it's a default value. Override YAML files will always contain all fields, making them verbose.

**Fix:** Only wrap in `Some` when the value differs from the default.

---

### M9. `gpu_layers_adjust` has unreachable pattern for `(-1, All)`

**File:** `src/tui/settings.rs:160`

```rust
(-1, GpuLayersMode::All) => GpuLayersMode::All,  // <-- never changes value
```

Decrementing from `All` keeps it at `All`. Should probably cycle to the max layer count or to `Specific(N)`.

---

### M10. `repeat_penalty` apply_edit clamp range inconsistency

**File:** `src/tui/settings.rs:846-852`

The `adjust` function clamps to `1.0-2.0`, but `apply_edit` clamps to `0.0-2.0`. Users can enter `0.0` via direct edit but not via arrow keys.

**Fix:** Make the ranges consistent.

---

### M11. `presence_penalty` and `frequency_penalty` apply_edit clamp inconsistency

**File:** `src/tui/settings.rs:892-900, 924-932`

The `adjust` function clamps to `-2.0..2.0`, but `apply_edit` clamps to `0.0..1.0`. Direct editing is more restrictive than arrow key adjustment.

**Fix:** Make the ranges consistent.

---

### M12. `save_yaml` deletes file before writing — crash loses data

**File:** `src/config/store.rs:44-53`

```rust
let _ = std::fs::remove_file(&unused_path);  // delete first
// ...
if let Ok(content) = serde_yaml::to_string(item) {
    let _ = std::fs::write(&path, content);  // write second
}
```

If the process crashes between delete and write, the file is lost.

**Fix:** Write to active first, then remove from unused.

---

## LOW

### L1. Only `.yaml` extension recognized, `.yml` silently ignored

**File:** `src/config/store.rs:23`

```rust
if path.extension().map(|e| e == "yaml").unwrap_or(false) {
```

**Fix:** Also accept `.yml`: `e == "yaml" || e == "yml"`.

---

### L2. `load_all_from_dir` silently skips unreadable entries

**File:** `src/config/store.rs:18-21`

```rust
let entry = match entry {
    Ok(e) => e,
    Err(_) => continue,  // <-- silently skipped
};
```

**Fix:** Log a warning for each skipped entry.

---

### L3. Wildcard re-export in `lib.rs`

**File:** `src/lib.rs:20`

```rust
pub use models::*;
```

**Fix:** Use explicit re-exports for the public API surface.

---

### L4. Duplicate `builtin_profiles()` / `builtin_system_prompt_presets()` allocations

**Files:** `src/config/profiles.rs:54,70`, `src/config/presets.rs:48,63,79`

Each call allocates a new `Vec`. In `ProfileStore::all()`, the builtin vec is cloned, then iterated again.

**Fix:** Make the builtin vectors `static` or cache them.

---

### L5. `move_to_unused` doesn't handle cross-filesystem rename

**File:** `src/config/store.rs:62`

`std::fs::rename` fails across filesystem boundaries. Error is silently swallowed.

**Fix:** Fall back to copy-then-delete on `EXDEV` error.

---

### L6. `Config::save` writes all profiles/presets every time

**File:** `src/config.rs:1167-1173`

Every save writes every profile and preset, even if only one changed.

**Fix:** Track dirty state per item.

---

### L7. Unused `Serialize`/`Deserialize` derives on store structs

**Files:** `src/config/model_config.rs:33`, `src/config/profiles.rs:27`, `src/config/presets.rs:27`

Store structs derive `Serialize` and `Deserialize` but are marked `#[serde(default, skip)]` in `Config`.

**Fix:** Remove unused derives.

---

### L8. `Mutex` lock on `loaded_model_names` uses `unwrap_or_else(|e| e.into_inner())`

**File:** `src/main.rs:300`

```rust
let names = app.server.loaded_model_names.lock()
    .unwrap_or_else(|e| e.into_inner());
```

If the Mutex is poisoned, `into_inner()` returns the data, but this masks the underlying panic. The poisoning indicates a bug in the code that holds the lock.

**Fix:** Handle the poisoned state explicitly or use `lock().expect()` with a descriptive message.

---

### L9. `ensure_download_channel` unwrap on `None`

**File:** `src/tui/app/async_ops.rs:1379`

```rust
self.download.download_tx.as_ref().unwrap().clone()
```

If `download_tx` is `None`, this panics. The function name suggests it should ensure the channel exists, but the `unwrap()` is at the end.

**Fix:** Add a guard at the beginning that initializes the channel if `None`.

---

### L10. `ws_server_handle.take().unwrap()` can panic

**File:** `src/tui/app/async_ops.rs:1406`

```rust
let handle = self.ws_server_handle.take().unwrap();
```

If `ws_server_handle` is `None`, this panics.

**Fix:** Use `if let Some(handle) = self.ws_server_handle.take()`.

---

### L11. `upper.chars().nth(i).unwrap()` in tabbed panel

**File:** `src/tui/panel/tabbed.rs:545`

```rust
let ch = upper.chars().nth(i).unwrap();
```

If `i >= upper.len()`, this panics.

**Fix:** Use `unwrap_or(' ')` or guard the index.

---

### L12. `bench_tune_progress.as_ref().unwrap()` in hints

**File:** `src/tui/render/hints.rs:194`

```rust
app.bench_tune.bench_tune_progress.as_ref().unwrap(),
```

Panics if `bench_tune_progress` is `None`. The caller should verify the state before rendering.

**Fix:** Add a guard check before this unwrap.

---

### L13. `current_idx.unwrap()` in key handling

**File:** `src/tui/event/key.rs:1745`

```rust
current_idx.is_some() && current_idx.unwrap() >= previous_results.len();
```

While guarded by `is_some()`, the `unwrap()` is redundant.

**Fix:** Use `if let Some(idx) = current_idx && idx >= previous_results.len()`.

---

### L14. `last_spinner_time.unwrap()` in loading state

**File:** `src/tui/app/state.rs:387`

```rust
|| self.loading.last_spinner_time.unwrap().elapsed() > spinner_interval
```

The short-circuit from the previous condition should guarantee `Some`, but this is fragile if the condition changes.

**Fix:** Use `if let Some(t) = self.loading.last_spinner_time { t.elapsed() > spinner_interval } else { false }`.

---

### L15. `SearchResult` has duplicate `model_id` and `model_name` fields

**File:** `src/models.rs:51-52`

```rust
pub model_id: String,
pub model_name: String,
```

These are always set to the same value (see `hub.rs:77`: `let model_name = model_id.clone()`).

**Fix:** Remove `model_name` and use `model_id` everywhere, or document the semantic difference.

---

## COSMETIC

### Co1. Inconsistent `#[derive(Default)]` placement

**File:** `src/models.rs:272-274`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Hash)]
#[derive(Default)]  // <-- separate derive line
pub enum GpuLayersMode {
```

**Fix:** Combine into a single `#[derive(..., Default)]` attribute.

---

### Co2. Blank line before `DownloadStatus` enum

**File:** `src/models.rs:280-282`

Extra blank line between `GpuLayersMode` and `DownloadStatus`.

---

### Co3. `ServerMode::Router` display includes "XP!" marker

**File:** `src/models.rs:659`

```rust
ServerMode::Router => write!(f, "Router (XP!)"),
```

"XP!" suggests experimental status. Should be documented or removed if the feature is stable.

---

### Co4. Inconsistent comment style in `ModelOverride::apply`

**File:** `src/config.rs:440-454`

Block comment accounting for field mapping uses a different style than the rest of the codebase.

---

### Co5. `from_u8` mapping order doesn't match enum order

**File:** `src/models.rs:347-358`

The `from_u8` method maps `4 → Q5_1`, `5 → Q5_0`, `6 → Q4_1`, `7 → Q4_0`, `8 → Iq4Nl`, which doesn't match the enum variant declaration order.

**Fix:** Document the mapping or align with enum order.

---

### Co6. `Sanitizer` function name vs purpose

**File:** `src/config.rs:1224-1254`

`sanitize_log` is a good name, but the function does three things: truncation, control character stripping, and tab replacement. Consider splitting or documenting the multi-purpose behavior.

---

### Co7. Hardcoded `550.0` magic number in VRAM estimation

**File:** `src/models.rs:1366`

```rust
let total_mib = model_vram + kv_mib + activation_mib + fixed_overhead + 550.0;
```

The `550.0` constant (driver overhead?) has no comment.

**Fix:** Define as a named constant: `const DRIVER_OVERHEAD_MIB: f64 = 550.0;`

---

### Co8. `SearchResult` struct fields could use doc comments

**File:** `src/models.rs:49-73`

Most fields lack doc comments, unlike `ModelSettings` which has extensive documentation.

**Fix:** Add doc comments to `SearchResult` fields.

---

## RECOMMENDED PRIORITY ACTIONS

1. **Fix the router structure** in `serve_api.rs` — move `/health` and `/metrics` inside the auth-protected sub-router (C2)
2. **Restrict CORS** to specific trusted origins (C3)
3. **Add CORS headers to SSE responses** (C1)
4. **Add request timeouts** to the `reqwest::Client` (H4)
5. **Replace `unwrap()` on serve.rs:313** with proper error handling (H2)
6. **Add error propagation** to `store.rs` (H5)
7. **Replace `physical_cores()` fallback** with `std::thread::available_parallelism()` (H3)
8. **Fix the `save_yaml` crash-unsafe ordering** (M12)
9. **Consistent clamp ranges** for penalty settings (M10, M11)
10. **Redact auth key** from log output (H7)
