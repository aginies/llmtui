# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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

[1.5.0]: https://github.com/aginies/llmtui/compare/v1.4.1...v1.5.0
[1.4.1]: https://github.com/aginies/llmtui/compare/v1.4.0...v1.4.1
