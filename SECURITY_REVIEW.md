# Security Review: llm-manager

**Date:** 2026-06-26
**Version:** 1.8.0
**Reviewer:** Automated review

---

## Executive Summary

| Severity | Open | Fixed |
|----------|------|-------|
| HIGH     | 0    | 3     |
| MEDIUM   | 4    | 3     |
| LOW      | 10   | 1     |

**Total findings:** 18 | **Still open:** 14

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

### M3. WebSocket auth timing attack
- **File:** `src/backend/ws_server.rs:103`
- **Severity:** MEDIUM (low practical risk locally)
- **Description:** Simple `!=` string comparison instead of constant-time comparison. Inconsistent with `constant_time_not_eq` already implemented in `serve_api.rs:97-106`.
- **Fix:** Use existing `constant_time_not_eq` function.

### M4. SSRF via web search URL fetching
- **File:** `src/backend/web_search.rs:302-316` (fetch_other_content), `src/backend/web_search.rs:254-268` (fetch_wikipedia_content)
- **Severity:** MEDIUM
- **Exploitability:** Medium (requires controlled SearXNG or crafted search results)
- **Description:** Both functions fetch arbitrary URLs from search results without validating:
  - URL scheme (could be `file://`, `gopher://`, etc.)
  - IP ranges (could point to `169.254.169.254` AWS metadata, `127.0.0.1`, `10.0.0.0/8`, etc.)
- **Impact:** Server-side request forgery. Application fetches internal URLs as the user, potentially exposing metadata, internal services, or enabling port scanning.
- **Fix:**
```rust
use url::Url;

fn validate_url(url: &str) -> Result<()> {
    let parsed = Url::parse(url)?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        anyhow::bail!("Only http/https schemes allowed");
    }
    // Optional: block private IP ranges
    Ok(())
}
```

---

## LOW Severity

### L1. SearXNG scheme-relative URL
- **File:** `src/backend/web_search.rs:29-32`
- **Description:** `engine_url` config value not validated. `//evil.com` resolves as protocol-relative URL, defaulting to `http://`.
- **Fix:** Validate URL starts with `http://` or `https://`, or prepend `https://` if missing.

### L2. GGUF arch field metadata injection
- **File:** `src/backend/server.rs:246-261`
- **Description:** `arch` from GGUF metadata used in `--override-kv` argument. Malicious GGUF could contain crafted arch value affecting llama-server parsing.
- **Fix:** Sanitize arch: `arch.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_').collect()`

### L3. Dashboard innerHTML XSS
- **File:** `src/dashboard.html:193-195`
- **Description:** `innerHTML` renders WebSocket metrics values without escaping. A compromised llama-server could send malicious `backend` value like `</div><script>alert(1)</script>`.
- **Fix:** Use `textContent` or escape HTML:
```javascript
function escapeHtml(str) {
    const div = document.createElement('div');
    div.textContent = str;
    return div.innerHTML;
}
```

### L4. Benchmark output size
- **File:** `src/backend/benchmark.rs:1239`
- **Description:** Model outputs embedded in HTML report JSON without truncation. Very long outputs could cause memory issues or slow page rendering.
- **Fix:** Truncate outputs to ~1000 chars before embedding.

### L5. Prompt delimiter spoofing
- **File:** `src/backend/web_context.rs:183-186`
- **Description:** `[WEB CONTEXT]` / `[END WEB CONTEXT]` markers in prompt could be spoofed if search results contain those exact strings.
- **Fix:** Use UUID-based delimiters.

### L6. Missing security headers
- **File:** `src/serve_api.rs:458-476`
- **Description:** API proxy lacks `Content-Security-Policy`, `X-Content-Type-Options`, `X-Frame-Options` headers.
- **Fix:** Add via middleware layer.

### L7. RPC IP validation
- **File:** `src/backend/server.rs:230-238`
- **Description:** User-configured `worker.ip` passed directly to llama-server without validation.
- **Fix:** Validate IP format before use.

### L8. server_url host validation
- **File:** `src/serve_api.rs:410-411`
- **Description:** `host` from config used in `server_url` format string. IPv6 addresses like `::1` produce malformed URL `http://::1:8080`.
- **Fix:** Use `clean_host(&host)` which wraps IPv6 in brackets.

---

## Recommendations Priority Order

1. **M1** — Fix CORS to scoped origins
2. **M4** — Add SSRF protection for web search URLs
3. **M2** — Move WebSocket auth from URL to header
4. **M3** — Use constant-time comparison for WebSocket auth
5. **L3** — Fix innerHTML XSS in dashboard
6. **L1** — Validate SearXNG engine_url scheme
7. **L2** — Sanitize GGUF arch field
8. **L4** — Truncate benchmark outputs
9. **L5** — Use UUID delimiters for prompt context
10. **L6** — Add security headers to API proxy
11. **L7** — Validate RPC worker IPs
12. **L8** — Use clean_host for server_url construction
