# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.6.1] - 2026-06-18

### Added

- **Web Search documentation** — complete SearXNG setup guide with podman and docker-compose examples in `documentation/src/web-search.md`
- **SearXNG config screenshot** — `searxng_config.png` added to documentation
- **Web Search entry in SUMMARY.md** — web-search.md now included in mdbook build
- **Web search injection log** — logs "Web search: results injected (N chars)" when SearXNG data is injected into prompt

### Fixed

- **GRANIAN_PORT env var** — documented requirement to match external port (default 8080 in image)
- **Volume mount** — removed `-v ~/.searxng:/etc/searxng/lib/searx:Z` that overwrites Python package dir
- **Settings file extension** — standardized on `settings.yml` (not `.yaml`)
- **base_url uncommented** — enabled in all documentation examples

## [1.6.0] - 2026-06-18

### Added

- **UNSAVED indicator** — shows in status bar and LLM settings panel when settings are dirty
- **Ctrl+S in Server Settings** — save LLM model parameters from Server Settings panel
- **Save hint in Server Settings** — status bar displays save shortcut when Server Settings is focused
- **Global settings dirty detection** — `is_global_settings_same_as_config()` compares host, port, backend, threads, API endpoint, server mode, llama.cpp versions, and more against config defaults

### Changed

- **LLM panel border** — red (`Rgb(255,130,130)`) when settings are dirty (double border if focused, single if not); green when clean
- **Status bar UNSAVED text** — bold, light red, displayed between mode indicator and server status when settings are dirty
- **Progress bars** — replaced manual `█░` characters with ratatui `Gauge` widget

### Fixed

- **GGUF filename explanation readability** — improved layout and formatting
- **Params column width** — minimum 12 chars so `(MoE)` label always visible
- **Scrollbar alignment** — LLM settings scrollbar now aligned with server settings scrollbar position
- **Dashboard picker** — added `": "` separator in Enabled line
- **Active model panel** — same width as model info panel, brighter borders
- **Onboarding** — updated step 3 Server Settings description and keys, fixed `llama-manager` typo
- **Backend picker** — noted Vulkan works on NVIDIA

### Style

- **Focused panel borders** — changed from `Thick` to `Double` border type
- **LIGHT_GREEN extension** — applied to Models, Log, and Active Model panel borders
- **MoE label** — removed parentheses from MoE label in models list
- **Selection highlight** — changed from GREEN to YELLOW background for clear visibility
- **Panel titles** — always YELLOW for readability on unfocused panels
- **Dialog selection** — BLUE background for better readability
- **Disabled settings text** — GRAY with `Modifier::DIM` for better visual hierarchy
- **Color hierarchy** — added `MID_GRAY`, fixed `STATUS_PAUSED` and `SORT_LABEL` accessibility
- **Active model header** — prompt section colored yellow
- **Server status** — `[ N/A ]` uses GREEN to match active server styling
- **Dialog and panel contrast** — improved visual contrast throughout
- **Font size** — increased size font

### Documentation

- Added screenshots and documentation updates from 1.5.x releases

## [1.5.1] - 2026-06-17

### Added

- **Prompt metrics in GNOME extension** — `prompt_tokens` (eval token count) and `prompt_progress` (progress bar) added to Performance metric group
- **`ratio_pct` metric type** — new progress bar type using raw percentage values (0-100)
- **Color thresholds** — `prompt_progress` bar: red (0-50%), yellow (50-80%), green (80-100%)
- **Pango markup support** — `setMarkup()` on `LlmPanelItem` for colored metric display in top bar
- **5s timeout for web search** — `fetch_wikipedia_content` and `fetch_other_content` now have `std::time::Duration::from_secs(5)` timeout with `(timeout 5s)` error suffix

### Changed

- **Metrics config extracted** — `WS_METRICS` and `METRIC_GROUPS` moved to `config/metrics.js`, `METRIC_FORMATTERS` object added
- **`buildWsUrl` preserves query params** — non-auth query parameters no longer dropped
- **`prompt_progress` color fix** — white at 0%, red at 1%+
- **State badge removed** — `state` removed from default `selected-metrics` schema, `.llm-state-badge` CSS removed
- **`truncateModelName` simplified** — no longer preserves extension suffix
- **GNOME extension docs updated** — metrics count 10→12, new metrics documented in reference table

### Fixed

- **WebSocket callback guard** — `_destroyed` flag prevents use-after-destroy crashes
- **RAM/CPU metric key collision** — fixed `key` field name collision in metric handlers

### Documentation

- **6 new screenshots** — `backend_picker.png`, `host_picker.png`, `command_line.png`, `searxng.png`, `speculative_decoding.png`, `yarn_rope_parameters.png`
- **Quick Start section** — added to `server-settings.md` with host and backend picker screenshots
- **Speculative decoding, Yarn RoPE docs** — screenshots added to `llm-settings.md` and `usage.md`
- **Panel toggle docs** — `disabling_panels.png` screenshot added with panel toggle info

## [1.5.0] - 2026-06-17

### Added

- **TLS indicator in status bar** — shows `TLS:On` when API endpoint TLS is enabled
- **Dashboard URL modal (Ctrl+U)** — displays all server URLs in "Server Settings Summary" modal:
  - API URL with port
  - Metrics URL
  - Dashboard URL with port and auth key
  - opencode base URL (`/v1` endpoint)
  - TLS status indicator
  - Copies all URLs to clipboard on Enter
- **TLS fields in pickers** — `tls_enabled`, `tls_cert`, `tls_key` added to Dashboard and API Endpoint pickers
- **German locale** (`de.json`) — full translation for German language support
- **Config fields** — `language`, `onboarding_complete`, `server_tls_enabled`, `server_tls_cert`, `server_tls_key`, `api_endpoint_key`, `web_search_*`
- **Web Search module** (`backend::web_search.rs`) — SearXNG integration with search results parsing
- **TLS module helpers** — `try_load_tls()`, `validate_tls_path()`, cert validation on load/generation

### Changed

- **Renamed TLS config fields** — `ws_server_tls_*` → `server_tls_*` (shared across WebSocket and API proxy)
- **Removed `ws_server_auth_key`** — replaced by shared `api_endpoint_key`
- **Expanded i18n** — all user-facing strings go through translation system
- **Documentation restructured** — inlined all linked content into `server-settings.md` and `llm-settings.md`
- **Architecture doc updated** — reflects current 85+ file codebase, all 21 GlobalMode variants, new structs
- **API reference updated** — all public types, enums, and module functions documented
- **README simplified** — reduced from 64 to 42 lines, links to full docs

### Fixed

- **Dashboard/ TLS color consistency** — enabled values use WHITE, disabled use DARK_GRAY to match other value lines
- **Label/value color split** — TLS and Dashboard label (YELLOW) and value (WHITE/GRAY) now separate spans
- **i18n French** — fixed API key label to "Clé d'authentification : "

### Documentation

- Added `server_summary.png` screenshot to Dashboard URL modal docs
- Added `info_model.png` to GGUF filename explanation
- Added `server_settings.png` to getting-started
- Added opencode integration docs with TLS and auth key examples
- Restructured SUMMARY.md into Server Settings and LLM Settings groups
- Reduced SUMMARY.md to 10 sections, merged cache docs, added index pages
- Removed obsolete screenshots from getting-started
- Updated GNOME extension docs (removed state metric)

## [1.4.1] - 2026-06-17

### Fixed

- GNOME extension: fixed real-time metrics display
- Dashboard: fixed WebSocket connection issues

[1.6.1]: https://github.com/aginies/llmtui/compare/v1.6.0...v1.6.1
[1.6.0]: https://github.com/aginies/llmtui/compare/v1.5.1...v1.6.0
[1.5.1]: https://github.com/aginies/llmtui/compare/v1.5.0...v1.5.1
[1.5.0]: https://github.com/aginies/llmtui/compare/v1.4.1...v1.5.0
[1.4.1]: https://github.com/aginies/llmtui/compare/v1.4.0...v1.4.1
