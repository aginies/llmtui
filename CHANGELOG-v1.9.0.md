# v1.9.0 Changelog

## Features
- **LLaMA Server Options Picker** — configure port, threads, mode, and log level from the UI
- **Log level config** — new `log_level` setting for llama-server output verbosity (default: trace)
- **Accurate download progress bar** — uses llama.cpp's "loading model: X/Y" for real progress
- **Expert field highlighting** — colored expert fields + chat template docs
- **GGUF decode failure message** — improved error display and color layout

## Fixes
- **Dashboard auth** — replaced script injection with safe meta tag
- **WebSocket auth (M2)** — migrated from URL query param to subprotocol
- **CORS (M1)** — dynamic origin validation
- **SSRF (M4)** — scheme validation + constant-time WebSocket auth comparison
- **L1-L8 security hardening** — 8 LOW severity fixes
- **Backend picker** — stays open after delete confirmation
- **Server spawn failure** — properly clears server_handle
- **Download control keys** — restored in search/files modes
- **API endpoint save** — fixed
- **Packaging** — inlined metrics.js into extension.js and prefs.js
- **Missing i18n key** — added `panel.server.api_endpoint`
- **Log level flag** — `-lv` now accepts numeric values (0-4)

## Refactoring
- **RPC worker edit state** — moved from settings/edit to picker namespace
- **Metrics model name** — simplified router-mode logic, increased throttle interval
- **Overlays rendering** — DRY extracted helpers, consistent formatting
- **Picker navigation** — unified j/k vim keys, `wrap_field_picker` helper
- **LLaMA Server panel** — renamed from "LLaMA Server Options" across all locales
- **ratatui** — bumped 0.29 → 0.30.2

## i18n
- Added `dialog.llama_server.log_level_warning` to all locales (de, en, fr, it)
