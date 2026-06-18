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
- **Docs link in Web Search picker** — `cf4dfba` adds documentation link to Web Search Configuration dialog
- **Auto SearXNG health check** — `04e0cf7` auto-checks SearXNG health when Web Search picker opens
- **Alt+F3 toggle** — `25e4268` add Alt+F3 to toggle LLM Settings visibility
- **VRAM usage bar** — `149237f` add VRAM usage bar to active model panel title
- **README panel always visible** — `2e8a2d4`/`90c8b97`/`82b1cd9` README panel visible in all modes (except BenchTune), restored on ESC
- **Ctx (U) column** — `bbea35e` rename context column header to 'Ctx (U)' to indicate user-configured context from LLM settings

### Changed

- **F-key navigation restructured** — `1d06d54` refactor F-key panel navigation; F2/F3/F6 focus panels, Ctrl+F2/F3/F6 toggle visibility; Ctrl+F10 shows all, F10 hides all
- **Status bar split into two tiers** — `63aca0f` split status bar into mode + hints tiers
- **Sort label moved to status line** — `999e5ea` move sort label from hints to status line for List and Search modes
- **Status column removed** — `52c478b` remove status column, add [LOADED/LOADING/BENCHMARK] name prefixes instead
- **Section headers** — `4c35d3f` replace `--- Title ---` section headers with `━━━ Title ━━━`
- **Panel title colors** — `fd8a939` panel title turns green when focused, yellow when not
- **Unfocused borders** — `16673ac` standardize borders to Rounded for unfocused panels and pickers
- **Scrollbar alignment** — `4c35d3f` fix scroll offset for new section header style

### Fixed

- **Dirty flag on Ctrl+S** — `078d082` clear dirty flag on Ctrl+S by syncing global settings
- **VRAM metrics display** — `88b8585` cache VRAM metrics, extract HOP_BY_HOP constant, fix VRAM not displaying
- **Ctrl+F10/F10 behavior** — `775b07d` swap Ctrl+F10 (show all) / F10 (hide all) behavior
- **Log panel title** — `7e55737` log panel title always shows (F6) [^F6] + make Ctrl+F2/F3 global
- **Sort info in status bar** — `c61c000` move sort info inside Mode block in status bar
- **Context column width** — `45ca666` context column min width 7 chars for 'Ctx (U)' header
- **Context column restored** — `5cb89bb` restore context column to Percentage(11)
- **README restore on ESC** — `9a88c95` always show README in search mode, restore all panels on ESC
- **Documentation discrepancies** — `44f0d43` fix --ws-auth flag, F-key mappings, Bench display name, F3 for LLM Settings

### Documentation

- **Web Search docs** — `3497b83` add SearXNG web search documentation with podman deployment guide
- **Keyboard shortcuts** — `c8aac51` restructure keyboard shortcuts per panel
- **Ctx (U) explanation** — `83c684b` explain Ctx (U) column as user-configured context from LLM settings
- **Status column removal docs** — `b08c4c8` update usage and router-mode docs for status column removal
- **Git tag v1.6.1** — `45c8179` bump version to 1.6.1

### Style

- **Selection highlight** — `29e0a2d` update helpers, profiles, status, tests; add screenshot

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
