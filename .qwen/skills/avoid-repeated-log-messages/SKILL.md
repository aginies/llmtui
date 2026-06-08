---
name: avoid-repeated-log-messages
description: Fix log messages that repeat on every tick due to being placed before a conditional check
source: auto-skill
extracted_at: '2026-06-08T13:00:00.000Z'
---

## Avoiding Repeated Log Messages in TUI Tick Functions

When a log message appears repeatedly in the TUI log panel (every tick), it's usually because `add_log()` is called **before** a conditional check that determines whether the operation actually succeeded or needs to run again.

### The Pattern

```rust
// BUG: message logged BEFORE the operation — fires every tick
if needs_reload {
    self.add_log(crate::t!("async.tls_generating"), LogLevel::Info);
    match ensure_tls_certs() {
        Ok((cert, key)) => {
            // ... use cert/key
        }
        Err(_) => None,
    }
}
```

The tick function runs every ~1s. If `needs_reload` stays true (e.g., because `ensure_tls_certs()` returns existing cached certs, not new ones), the message repeats every tick.

### The Fix

Move `add_log()` **inside** the `Ok` branch so it only fires when the operation actually produces new output:

```rust
// FIXED: message logged AFTER successful generation
if needs_reload {
    match ensure_tls_certs() {
        Ok((cert, key)) => {
            self.add_log(crate::t!("async.tls_generating"), LogLevel::Info);
            // ... use cert/key
        }
        Err(_) => None,
    }
}
```

### Where to Look

Search for `add_log` calls in tick functions (functions ending in `_tick` or called from the main event loop):

```bash
grep -n "add_log" src/tui/app/async_ops.rs
```

### Common Scenarios

| Function | Message | Condition |
|----------|---------|-----------|
| `tick_ws_server()` | "Auto-generating TLS certificate and key" | Before `ensure_tls_certs()` |
| Any tick function | Status messages before conditional checks | Before `Ok`/`Err` branches |

### Verify the Fix

1. Confirm the message only appears once in the log (not repeated on every tick).
2. Check that the message still appears the first time the operation actually runs.
3. Run `cargo test` to ensure no regressions.
