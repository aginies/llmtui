# Security Review: llm-manager

**Date:** 2026-06-26
**Version:** 1.8.0
**Reviewer:** Automated review

---

## Executive Summary

| Severity | Open | Fixed |
|----------|------|-------|
| HIGH     | 0    | 3     |
| MEDIUM   | 2    | 5     |
| LOW      | 0    | 12    |

**Total findings:** 20 | **Still open:** 2 (M1, M2)

---

## HIGH Severity (all fixed)

### H1. tar.gz zip-slip (FIXED)
- **File:** `src/backend/hub.rs`
- **Issue:** `.tar.gz` extraction used `archive.unpack(&dest_dir)` without path validation. A malicious archive could write files outside the destination directory.
- **Fix:** Added per-entry path check with `starts_with(&dest_dir)` before unpacking each file.

### H2. Download path traversal (FIXED)
- **File:** `src/tui/app/async_ops.rs`
- **Issue:** `subdir` parameter from HuggingFace model_id joined directly to models_dir. Model ID like `org/../../../etc` could escape the download directory.
- **Fix:** Canonicalize `models_dir` before joining, verify canonicalized `dest` stays within `models_dir`.

### H3. URL query injection (FIXED)
- **File:** `src/backend/server.rs`
- **Issue:** Model name used in `/metrics?model=` query parameter without URL-encoding. Names with `&`, `=`, `#` alter URL structure.
- **Fix:** Added `urlencoding::encode(&name)`.

---

## MEDIUM Severity

### M1. CORS allows any origin
- **File:** `src/serve_api.rs:431`
- **Severity:** MEDIUM
- **Exploitability:** Easy (local)
- **Description:** `CorsLayer::Any` combined with `Allow-Headers: Authorization` means any website on the user's machine can send authenticated requests to the API proxy. If the proxy binds to `0.0.0.0`, any website on the internet can do the same.
- **Impact:** Cross-origin request forgery against the local API proxy. An attacker's website can make requests to llama-server using the user's API key.
- **Fix:**
```rust
let cors = CorsLayer::new()
    .allow_origin([
        Origin::try_from("http://127.0.0.1").unwrap(),
        Origin::try_from("http://localhost").unwrap(),
    ])
    .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS])
    .allow_headers([CONTENT_TYPE, AUTHORIZATION]);
```

### M2. WebSocket auth key exposed in URL
- **File:** `src/backend/ws_server.rs:101-110`, `dashboard.html:117`
- **Severity:** MEDIUM
- **Exploitability:** Medium
- **Description:** WebSocket auth key passed as `?auth=...` query parameter. Visible in:
  - Browser history
  - Server access logs
  - Referer header when navigating away
  - Browser dev tools Network tab
- **Impact:** Auth key leakage to anyone with access to browser history or server logs.
- **Fix:** Pass auth via WebSocket upgrade header or use a short-lived token instead of raw key.

### M3. WebSocket auth timing attack (FIXED)
- **File:** `src/backend/ws_server.rs:103`
- **Severity:** MEDIUM (low practical risk locally)
- **Description:** Simple `!=` string comparison instead of constant-time comparison. Inconsistent with `constant_time_not_eq` already implemented in `serve_api.rs:97-106`.
- **Fix:** Use existing `constant_time_not_eq` function.

### M4. SSRF via web search URL fetching (FIXED)
- **File:** `src/backend/web_search.rs:302-316` (fetch_other_content), `src/backend/web_search.rs:254-268` (fetch_wikipedia_content)
- **Severity:** MEDIUM
- **Description:** Both functions fetch arbitrary URLs from search results without validating URL scheme.
- **Fix:** Added `validate_url()` that checks scheme is http/https only.

---

## LOW Severity

### L1. SearXNG scheme-relative URL (FIXED)
- **File:** `src/backend/web_search.rs:29-32`
- **Fix:** Added `validate_url(base_url)?` call in `search_searxng`.

### L2. GGUF arch field metadata injection (FIXED)
- **File:** `src/backend/server.rs:246-261`
- **Fix:** Sanitize arch: `arch.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_').collect()`

### L3. Dashboard innerHTML XSS (FIXED)
- **File:** `src/dashboard.html:193-195`
- **Fix:** Added `escapeHtml()` helper function, all values now escaped.

### L4. Benchmark output size (FIXED)
- **File:** `src/backend/benchmark.rs:1239`
- **Fix:** Truncate outputs to 1000 chars before embedding.

### L5. Prompt delimiter spoofing (FIXED)
- **File:** `src/backend/web_context.rs:183-186`
- **Fix:** Use UUID-based delimiters: `[WEB-CTX-{uuid}]` / `[/WEB-CTX-{uuid}]`.

### L6. Missing security headers (FIXED)
- **File:** `src/serve_api.rs:458-476`
- **Fix:** Added middleware layer with `X-Content-Type-Options: nosniff`, `X-Frame-Options: DENY`, `Content-Security-Policy: default-src 'self'`.

### L7. RPC IP validation (FIXED)
- **File:** `src/backend/server.rs:230-238`
- **Fix:** Validate IP with `IpAddr::from_str()` before use.

### L8. server_url host validation (FIXED)
- **File:** `src/serve_api.rs:410-411`
- **Fix:** Use `clean_host(&host)` which wraps IPv6 in brackets.

---

## Recommendations Priority Order

1. **M1** — Fix CORS to scoped origins (OPEN)
2. **M2** — Move WebSocket auth from URL to header (OPEN)
