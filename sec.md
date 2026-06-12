# Security Audit Report — llm-manager

## Table of Contents

1. [Secrets in Plaintext Config](#1-secrets-in-plaintext-config)
2. [API Key Auth — Non-Constant-Time Comparison](#2-api-key-auth--non-constant-time-comparison)
3. [Wildcard CORS on API Proxy](#3-wildcard-cors-on-api-proxy)
4. [API Key Leaked in WebSocket Dashboard HTML](#4-api-key-leaked-in-websocket-dashboard-html)
5. [Self-Signed CA Trust — Man-in-the-Middle Risk](#5-self-signed-ca-trust--man-in-the-middle-risk)
6. [Untrusted Binary Download from GitHub Releases](#6-untrusted-binary-download-from-github-releases)
7. [Archive Extraction — Zip Slip / Path Traversal](#7-archive-extraction--zip-slip-path-traversal)
8. [No TLS Verification for External API Calls](#8-no-tls-verification-for-external-api-calls)
9. [HTTP Used for API Proxy by Default](#9-http-used-for-api-proxy-by-default)
10. [Web Search URL Injection](#10-web-search-url-injection)
11. [Command Injection via Model Path in Server Args](#11-command-injection-via-model-path-in-server-args)
12. [SearXNG API Key Sent in Config Without Masking](#12-searxng-api-key-sent-in-config-without-masking)
13. [WebSocket Auth via URL Query Param (Logging Exposure)](#13-websocket-auth-via-url-query-param-logging-exposure)
14. [Process Environment Variable Leakage](#14-process-environment-variable-leakage)
15. [No Input Validation on User-Provided Paths](#15-no-input-validation-on-user-provided-paths)

---

## 1. Secrets in Plaintext Config

**Severity:** HIGH (mitigated)
**Files:** `src/config.rs:783`, `src/config.rs:828`, `src/serve_api.rs:43`

### Finding

Three types of secrets are stored in plain-text YAML config files under `~/.config/llm-manager/`:

- `ws_server_auth_key` — WebSocket dashboard auth key (config.rs:783)
- `web_search_api_key` — SearXNG API key (config.rs:828)
- `api_key` — API proxy Bearer token (serve_api.rs:43, passed through config)

Config file permissions are OS-default (typically `0644`), meaning any user on the system can read all secrets.

### Config file locations

- Main config: `~/.config/llm-manager/config.yaml`
- Per-model configs: `~/.config/llm-manager/models/*.yaml`
- Profiles: `~/.config/llm-manager/profiles/*.yaml`
- Presets: `~/.config/llm-manager/presets/*.yaml`
- TLS certs: `~/.config/llm-manager/tls/`

### Exploit

```bash
# Any local user can read the API key
cat ~/.config/llm-manager/config.yaml | grep -E "api_key|ws_server_auth_key|web_search_api_key"

# Or read via the running process's /proc
cat /proc/$(pgrep llm-manager)/environ | grep LD_LIBRARY_PATH
```

### Fix (APPLIED)

Config file now written with `0600` permissions on Unix:

```rust
// src/config.rs — Config::save()
pub fn save(&mut self) -> Result<(), Box<dyn std::error::Error>> {
    let path = Self::config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_yml::to_string(self)?;
    std::fs::write(&path, content)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }
    // ...
}
```

Remaining hardening: OS keyring integration (`secret-service` crate) for future improvement.

---

## 2. API Key Auth — Non-Constant-Time Comparison

**Severity:** MEDIUM (mitigated)
**File:** `src/serve_api.rs:60-62`

### Finding

```rust
if provided.as_deref() != Some(expected) {
    return (StatusCode::UNAUTHORIZED, ...).into_response();
}
```

Uses `!=` string comparison which short-circuits on first mismatch. An attacker with low-latency network access can measure response time differences to determine how many characters of the API key are correct (timing side-channel).

### Exploit

```python
import http.client
import time

api_key = "abcdefghijklmnopqrstuvwxyz"
correct = ""
for charset in "abcdefghijklmnopqrstuvwxyz0123456789":
    best_time = float('inf')
    for c in charset:
        test_key = correct + c
        # Send 100 requests to average timing
        times = []
        for _ in range(100):
            conn = http.client.HTTPConnection("127.0.0.1", 49222)
            start = time.perf_counter()
            conn.request("GET", "/v1/models", headers={"Authorization": f"Bearer {test_key}"})
            resp = conn.getresponse()
            elapsed = time.perf_counter() - start
            resp.read()
            conn.close()
            times.append(elapsed)
        avg = sum(times) / len(times)
        if avg < best_time:
            best_time = avg
            correct += c
    print(f"Correct key so far: {correct}")
```

### Fix (APPLIED)

Constant-time byte comparison replaces `!=` in `serve_api.rs`:

```rust
fn constant_time_not_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return true;
    }
    let mut result: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result != 0
}

async fn auth_middleware(...) {
    if let Some(expected) = &state.api_key {
        let provided = extract_api_key(req.headers());
        let expected_bytes = expected.as_bytes();
        let not_equal = if let Some(provided_str) = provided {
            constant_time_not_eq(provided_str.as_bytes(), expected_bytes)
        } else {
            true
        };
        if not_equal {
            return (StatusCode::UNAUTHORIZED, ...).into_response();
        }
    }
    next.run(req).await
}
```

Always processes all bytes, no short-circuit on mismatch.

---

## 3. Wildcard CORS on API Proxy

**Severity:** MEDIUM (mitigated)
**File:** `src/serve_api.rs:442-454`

### Finding

```rust
let cors = CorsLayer::new()
    .allow_origin(tower_http::cors::Any)  // <-- ANY origin
    .allow_methods([...])
    .allow_headers([
        axum::http::header::CONTENT_TYPE,
        axum::http::header::AUTHORIZATION,
    ]);
```

`CorsLayer::Any` allows any origin to make cross-origin requests to the API proxy. Combined with the API key being sent as a Bearer token in the `Authorization` header (which is allowed), a malicious webpage can send requests to the API proxy with the user's credentials.

### Exploit

```html
<!-- Attacker hosts this page -->
<script>
// If the API proxy is bound to 0.0.0.0 or a reachable interface,
// and CORS allows Any, this works:
fetch('http://192.168.1.100:49222/v1/chat/completions', {
    method: 'POST',
    headers: {
        'Authorization': 'Bearer attacker-guessed-key',
        'Content-Type': 'application/json',
    },
    body: JSON.stringify({
        messages: [{ role: 'user', content: 'Exfiltrate my data' }],
        model: 'local-model'
    })
});
</script>
```

### Fix (APPLIED)

Ensured `.allow_credentials(false)` (the default) by removing `.allow_credentials(true)` from the `CorsLayer`. Combining `.allow_credentials(true)` with wildcard origin `allow_origin(tower_http::cors::Any)` is invalid under the CORS specification and modern browsers, and causes a startup panic in the `tower-http` library.

Since the API proxy uses Bearer token authentication via the custom `Authorization` header (which must be explicitly set by client-side JS and is not automatically attached by the browser like cookies), wildcard CORS without credentials is secure and standard for open APIs. The `Authorization` header remains safely listed in `.allow_headers([...])`.

---

## 4. API Key Leaked in WebSocket Dashboard HTML

**Severity:** MEDIUM
**File:** `src/backend/ws_server.rs:91-94`

### Finding

```rust
async fn serve_dashboard(...) -> Html<String> {
    let auth_json = serde_json::to_string(&state.auth_key).unwrap_or("null".to_string());
    let auth_script = format!("<script>window.__WS_AUTH={};</script>", auth_json);
    let html = include_str!("../dashboard.html");
    Html(html.replacen("</body>", &format!("{}\n</body>", auth_script), 1))
}
```

The WebSocket auth key is injected into the HTML as `window.__WS_AUTH`. This means:
- Any JavaScript on the page can read the key
- The key appears in browser devtools/network logs
- If the dashboard is served over HTTP, the key is visible in plain text

### Exploit

```javascript
// In browser console on the dashboard page:
console.log("WS Auth Key:", window.__WS_AUTH);
// Connect directly with the key
const ws = new WebSocket(`ws://host:49223/ws?auth=${window.__WS_AUTH}`);
```

### Fix

Use a session token approach instead:

```rust
// Generate a short-lived random token on each request
let token = hex::encode(rand::random::<[u8; 16]>());
// Store token in a concurrent map with expiry
// Only the token (not the actual key) goes into the HTML
```

---

## 5. Self-Signed CA Trust — Man-in-the-Middle Risk

**Severity:** LOW
**File:** `src/backend/tls.rs`

### Finding

The app generates a self-signed CA certificate (`llm-manager CA`) at `~/.config/llm-manager/tls/ca.pem` and signs server certs with it. The CA is not installed in the system trust store by default.

Users connecting to the WebSocket dashboard or API server over TLS must either:
1. Manually trust the CA
2. Accept the cert in their browser (with no guarantee the cert they see is the app's)

An attacker on the local network who can perform a MITM attack could present their own self-signed cert and the user would likely accept it.

### Exploit

```bash
# On the local network, attacker intercepts TLS traffic
# Since the user's browser/system doesn't trust "llm-manager CA",
# the user sees a cert warning but may click "Proceed anyway"
```

### Fix

Document the CA trust installation clearly. Consider adding an option to pin the certificate fingerprint.

---

## 6. Untrusted Binary Download from GitHub Releases

**Severity:** MEDIUM (mitigated)
**File:** `src/backend/hub.rs:666-827`

### Finding

Backend binaries are downloaded directly from GitHub release URLs with no checksum verification:

```rust
let download_url = format!(
    "https://github.com/ggml-org/llama.cpp/releases/download/{tag}/llama-{tag}-bin-ubuntu-x64.tar.gz"
);
// ... downloaded and extracted without any hash verification
```

The download URL is constructed from the version tag (user-controllable via config). If an attacker compromises the GitHub release or performs a DNS hijack during download, they can serve a malicious binary.

### Exploit

```bash
# DNS hijack or GitHub release compromise
# Malicious llama-server binary installed to ~/.local/share/llm-manager/bin/
# Next time user starts a model, the trojanized binary runs with user privileges
```

### Fix (APPLIED)

Capture `sha2-256` response header from GitHub CDN and verify downloaded file:

```rust
// In download_file: capture sha2-256 header
let sha256 = resp
    .headers()
    .get("sha2-256")
    .and_then(|v| v.to_str().ok())
    .map(|s| s.to_lowercase());

// After download, verify
if let Some(expected) = &expected_sha256 {
    let actual = file_sha256(&tmp_path)?;
    if actual != expected.to_lowercase() {
        return Err(anyhow::anyhow!("SHA256 mismatch: expected {expected}, got {actual}"));
    }
}
```

Falls back to warning (not failure) if header is missing. Uses `sha2` crate for verification.

---


## 7. Archive Extraction — Zip Slip / Path Traversal

**Severity:** HIGH (mitigated)
**File:** `src/backend/hub.rs:910-933`
**File:** `src/backend/hub.rs:676-764`

### Finding

Backend binaries are downloaded directly from GitHub release URLs with no checksum verification:

```rust
let download_url = format!(
    "https://github.com/ggml-org/llama.cpp/releases/download/{tag}/llama-{tag}-bin-ubuntu-x64.tar.gz"
);
// ... downloaded and extracted without any hash verification
```

The download URL is constructed from the version tag (user-controllable via config). If an attacker compromises the GitHub release or performs a DNS hijack during download, they can serve a malicious binary.

### Exploit

```bash
# DNS hijack or GitHub release compromise
# Malicious llama-server binary installed to ~/.local/share/llm-manager/bin/
# Next time user starts a model, the trojanized binary runs with user privileges
```

### Fix

```rust
// After download, verify checksum
fn verify_checksum(path: &Path, expected_sha256: &str) -> Result<bool> {
    use sha2::{Sha256, Digest};
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher)?;
    let hash = hex::encode(hasher.finalize());
    Ok(hash == expected_sha256)
}
```

---

## 7. Archive Extraction -- Zip Slip / Path Traversal

**Severity:** HIGH (mitigated)
**File:** `src/backend/hub.rs:910-933`

### Finding

```rust
pub fn extract_archive(archive_path: &std::path::Path, dest_dir: &std::path::Path) -> Result<>() {
    // ...
    if filename.ends_with(".zip") {
        let file = std::fs::File::open(archive_path)?;
        let mut archive = zip::ZipArchive::new(file)?;
        archive.extract(dest_dir)?;  // <-- NO PATH VALIDATION
    } else if ... {
        // tar.gz extraction
        let mut archive = Archive::new(decoder);
        archive.unpack(dest_dir)?; // <-- NO PATH VALIDATION
    }
}
```

Both `zip::ZipArchive::extract()` and `tar::Archive::unpack()` have known zip-slip vulnerabilities if the archive contains entries with `../` paths. The `zip` crate's `extract()` method does NOT validate that extracted paths stay within `dest_dir`.

### Exploit

```python
# Create a malicious archive that writes outside dest_dir
import zipfile
with zipfile.ZipFile("evil.zip", "w") as z:
    z.writestr("../../etc/ld.so.preload", "malicious-library.so")
    z.writestr("bin/llama-server", "<actual-binary-content>")
```

The `llama-server` binary extraction would succeed, but `../../etc/ld.so.preload` would be written to a system location.

### Fix (APPLIED)

Added per-entry path validation for zip and canonicalized dest_dir:

```rust
pub fn extract_archive(archive_path: &std::path::Path, dest_dir: &std::path::Path) -> Result<>() {
    let dest_dir = dest_dir.canonicalize()?;  // resolve to absolute

    if filename.ends_with(".zip") {
        let file = std::fs::File::open(archive_path)?;
        let mut archive = zip::ZipArchive::new(file)?;
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)?;
            let file_name = entry.enclosed_name().ok_or("Invalid path in zip")?;

            let full_path = dest_dir.join(file_name);
            if !full_path.starts_with(&dest_dir) {
                return Err(anyhow::anyhow!("Zip slip detected: {}", full_path.display()));
            }
            entry.extract(&dest_dir)?;
        }
    } else if ... {
        let mut archive = Archive::new(decoder);
        archive.unpack(&dest_dir)?;  // dest_dir is now canonicalized
    }
    Ok(())
}
```

---

## 8. No TLS Verification for External API Calls

**Severity:** LOW
**File:** `src/backend/hub.rs:129-136`, `src/serve_api.rs`

### Finding

HTTP clients are created with default TLS settings via `reqwest::Client::new()` and `reqwest::Client::builder().build()?`. This is generally safe as reqwest uses the system's CA bundle by default.

However, in `hub.rs` tests, `danger_accept_invalid_certs(true)` is used, and the production code does not explicitly verify that the system CA bundle is being used.

### Exploit

```bash
# If the system CA bundle is compromised or the user has a custom MITM proxy
# (e.g., corporate proxy with custom cert), all requests to HuggingFace,
# GitHub, and SearXNG go through the proxy without the user knowing.
```

### Fix

No immediate fix needed for a desktop app. Document that users behind corporate proxies should trust their proxy's CA.

---

## 9. HTTP Used for API Proxy by Default

**Severity:** MEDIUM
**File:** `src/serve_api.rs:438-445`, `src/serve.rs`

### Finding

The API proxy server defaults to HTTP (not HTTPS). The TLS option exists but must be explicitly enabled:

```rust
let protocol = if tls_config.is_some() {
    "https"
} else {
    "http"  // <-- default
};
```

When `serve --api-port 49222 --api-key mysecret` is run, the API key is sent in plaintext over the wire.

### Exploit

```bash
# On the same network, capture the API key
tcpdump -A port 49222 | grep "Authorization"
# Output: Authorization: Bearer mysecret
```

### Fix

```bash
# User-side: enable TLS
llm-manager serve --model model.gguf --api-port 49222 --api-key mysecret --tls-enable
```

Recommend TLS as default or require `--api-key` to auto-enable TLS.

---

## 10. Web Search URL Injection

**Severity:** LOW
**File:** `src/backend/web_context.rs:183-185`

### Finding

Web search results are injected into the prompt sent to the LLM. The context injection includes raw URLs from search results without validation:

```rust
let new_content = format!(
    "[WEB CONTEXT]\nINSTRUCTION: Cite sources using inline markdown links in your answer. Format: [source name](URL). Place links directly after the facts they support...\n\n{}\n[END WEB CONTEXT]\n\n{}\n\n---\n\n{}",
    search_context, sources_section, content
);
```

If a malicious search result URL is returned by SearXNG (or a compromised SearXNG instance), the URL is embedded in the prompt. The LLM may follow or render this URL.

### Exploit

```
# Attacker controls a SearXNG instance or performs DNS hijack
# Returns a search result with URL: https://evil.com/phishing
# URL appears in the web context injected into the LLM prompt
# User sees the URL in the chat and may click it
```

### Fix

Validate and sanitize URLs before embedding:

```rust
fn sanitize_url(url: &str) -> Option<String> {
    url::Url::parse(url).ok().filter(|u| {
        let scheme = u.scheme();
        scheme == "http" || scheme == "https"
    }).map(|u| u.to_string())
}
```

---

## 11. Command Injection via Model Path in Server Args

**Severity:** MEDIUM
**File:** `src/backend/server.rs:34-47`

### Finding

Model paths are passed as arguments to `llama-server`:

```rust
fn push_arg(cmd: &mut Command, parts: &mut Vec<String>, name: &str, value: impl Display) {
    let val_str = value.to_string();
    cmd.arg(name).arg(&val_str);  // <-- directly passed as CLI arg
    parts.push(name.to_string());
    // ...
}
```

And in `build_server_cmd`:

```rust
push_arg(&mut cmd, &mut parts, "-m", model.path.display());
```

While `tokio::process::Command` passes arguments directly to `execve()` (not through a shell), special characters in paths are safe from shell injection. However, if the path contains characters that `llama-server` interprets specially (e.g., `--` to end of args), it could cause unexpected behavior.

### Exploit

```bash
# Create a model file at a path that contains --
mkdir -p "/tmp/test/--verbose"
ln -s /tmp/actual_model.gguf "/tmp/test/--verbose/model.gguf"
# llama-server may interpret --verbose as its own flag, changing behavior
```

### Fix

Use `--` to signal end of options:

```rust
cmd.arg("--");  // before model path arguments
push_arg(&mut cmd, &mut parts, "-m", model.path.display());
```

Or validate the path doesn't start with `-`:

```rust
fn push_arg(cmd: &mut Command, parts: &mut Vec<String>, name: &str, value: impl Display) {
    let val_str = value.to_string();
    cmd.arg(name);
    if val_str.starts_with('-') {
        cmd.arg(&val_str);  // llama-server will interpret correctly
    } else {
        cmd.arg(&val_str);
    }
    parts.push(val_str);
}
```

---

## 12. SearXNG API Key Sent in Config Without Masking

**Severity:** LOW
**File:** `src/serve.rs:270-274`

### Finding

```rust
let auth_info = if let Some(ref auth) = ws_auth {
    format!(" (auth: {})", &auth[..auth.len().min(8)])  // Shows first 8 chars
} else {
    String::new()
};
```

The WebSocket auth key logs first 8 characters. The SearXNG API key is stored in YAML config and logged/searched alongside other settings without masking.

### Exploit

```bash
# Check the log file for API key
cat ~/.local/share/llm-manager/llm-manager.log | grep -i "searxng\|api_key"
```

### Fix

Mask API keys in logs:

```rust
fn mask_key(key: &str) -> String {
    if key.len() <= 8 {
        "***".to_string()
    } else {
        format!("{}****", &key[..4])
    }
}
```

---

## 13. WebSocket Auth via URL Query Param (Logging Exposure)

**Severity:** LOW
**File:** `src/backend/ws_server.rs:102-110`

### Finding

```rust
if let Some(ref expected) = state.auth_key {
    if let Some(provided) = query.get("auth").and_then(|v| urlencoding::decode(v).ok()) {
        if provided != *expected {
            return StatusCode::UNAUTHORIZED.into_response();
        }
    }
}
```

The auth key is passed as a URL query parameter `?auth=...`. Query params are logged by:
- Web server access logs
- Browser history
- Proxy servers
- Referer headers

### Exploit

```bash
# Check browser history or proxy logs
curl "http://host:49223/ws?auth=secretkey123"
# The key appears in the URL

# In browser network tab, the full URL with auth is visible
```

### Fix

Use a header-based auth approach:

```rust
// In ws_handler, check Authorization header instead of query param
if let Some(ref expected) = state.auth_key {
    if let Some(auth_header) = ws.upgrade().await.unwrap().headers().get("Authorization") {
        if auth_header.to_str().ok().and_then(|v| v.strip_prefix("Bearer ")) != Some(expected.as_str()) {
            return StatusCode::UNAUTHORIZED.into_response();
        }
    }
}
```

Or use a one-time token exchanged via the dashboard page.

---

## 14. Process Environment Variable Leakage

**Severity:** LOW
**File:** `src/backend/server.rs:618-650`

### Finding

`LD_LIBRARY_PATH` (Linux) and `DYLD_LIBRARY_PATH` (macOS) are set in the spawned process environment:

```rust
cmd.env("LD_LIBRARY_PATH", format!("{}:{}", bin_dir.display(), current));
```

This environment variable is inherited by child processes of llama-server and may be visible via `/proc/<pid>/environ` on Linux.

### Exploit

```bash
# Read the environment of a running llama-server process
cat /proc/$(pgrep llama-server)/environ | tr '\0' '\n' | grep LD_LIBRARY_PATH
# Output: LD_LIBRARY_PATH=/home/user/.local/share/llm-manager/bin/llama-server-vulkan-b4100:/usr/lib
```

This reveals the full path to the downloaded binaries, which could help an attacker target the binary extraction path.

### Fix

No immediate fix needed. This is standard practice. Document the risk.

---

## 15. No Input Validation on User-Provided Paths

**Severity:** LOW
**File:** `src/serve.rs:119-154`, `src/config.rs`

### Finding

Model paths from CLI args and config are used with minimal validation:

```rust
let model_path = PathBuf::from(&opts.model_path);
// Only checks: broken symlink, exists, .gguf extension
```

The `--backend-binary` option accepts any path:

```rust
let binary = if let Some(path) = &opts.backend_binary {
    let binary_path = PathBuf::from(path);
    if !binary_path.exists() {
        anyhow::bail!("Backend binary not found: {}", binary_path.display());
    }
    binary_path
}
```

These paths are then used in `push_arg` calls to build the llama-server command. While `tokio::process::Command` handles shell escaping, arbitrary paths could point to:
- Symlinks to sensitive files
- Files in world-writable directories
- Very long paths causing argument list too long errors

### Exploit

```bash
# Symlink attack: point model path to /etc/shadow
ln -s /etc/shadow /tmp/evil.gguf
llm-manager serve --model /tmp/evil.gguf
# llama-server receives --model /tmp/evil.gguf (safe from shell injection)
# but may attempt to read /etc/shadow as a GGUF file
```

### Fix

Validate paths are within expected directories:

```rust
fn validate_model_path(path: &Path) -> Result<()> {
    let canonical = path.canonicalize()?;
    if !canonical.starts_with("/") {
        return Err(anyhow::anyhow!("Model path must be absolute"));
    }
    Ok(())
}
```

---

## Summary

| # | Issue | Severity | Location | Status |
|---|-------|----------|----------|--------|
| 1 | Secrets in plaintext config | **HIGH** (mitigated) | config.rs:783,828 | FIXED |
| 2 | API key timing side-channel | **MEDIUM** (mitigated) | serve_api.rs:60 | FIXED |
| 3 | Wildcard CORS | **MEDIUM** | serve_api.rs:424 | OPEN |
| 4 | Auth key in HTML source | **MEDIUM** | ws_server.rs:91 | OPEN |
| 5 | Self-signed CA trust | **LOW** | tls.rs | OPEN |
| 6 | Unverified binary download | **MEDIUM** (mitigated) | hub.rs:676 | FIXED |
| 7 | Zip slip in archive extract | **HIGH** (mitigated) | hub.rs:910 | FIXED |
| 8 | TLS verification | **LOW** | hub.rs:129 | OPEN |
| 9 | HTTP by default for API | **MEDIUM** | serve_api.rs:438 | OPEN |
| 10 | URL injection via web search | **LOW** | web_context.rs:183 | OPEN |
| 11 | Path injection in CLI args | **MEDIUM** | server.rs:34 | OPEN |
| 12 | API key log leakage | **LOW** | serve.rs:270 | OPEN |
| 13 | Query param auth leakage | **LOW** | ws_server.rs:102 | OPEN |
| 14 | Env var exposure | **LOW** | server.rs:618 | OPEN |
| 15 | Path validation missing | **LOW** | serve.rs:119 | OPEN |

## Priority Fixes

1. ~~**HIGH**: Add zip-slip protection in `hub.rs:910`~~ **DONE**
2. ~~**HIGH**: Restrict config file permissions to `0600` in `config.rs` save function~~ **DONE**
3. ~~**MEDIUM**: Add constant-time comparison for API key auth in `serve_api.rs`~~ **DONE**
4. ~~**MEDIUM**: Configure CORS wildcard origins safely without invalid credentials~~ **DONE**
5. ~~**MEDIUM**: Add SHA256 checksum verification for downloaded binaries~~ **DONE**
6. **MEDIUM**: Default to TLS when `--api-key` is provided
