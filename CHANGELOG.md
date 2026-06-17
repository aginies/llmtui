# Changelog

## [1.4.1] - 2026-06-17

### Features
- **GNOME Extension Metrics Grouping** — Group top bar metrics into Model, Performance, and Resources categories with labeled headers and separators
- **Preferences** — Bold frame labels for Selected Metrics and About sections
- **CSS** — Add `.llm-group-header` style for metric group labels

### Bug Fixes
- **GNOME Extension** — Remove "State" metric from top bar display
- **GNOME Extension** — Update "Reconnect Interval" label to "Seconds between updates"
- **GNOME Extension** — Fix bar styling (height, border-radius) for VRAM/RAM meters
- **GNOME Extension** — Delete `gschemas.compiled` from repo
- **GNOME Extension** — Update test.js metric count to 9
- **Dashboard** — Remove VRAM usage bar from active model frame title
- **Dashboard** — Remove progress bar from context display, show text-only `used/total (pct%)`
- **Active Panel** — Simplify context line rendering, remove unnecessary bar chars

## [1.4.0] - 2026-06-16

### Features
- **GNOME Shell Extension** — Add `llm-manager@aginies` GNOME extension for top bar metrics
  - Live metrics display (TPS, context, CPU, RAM, VRAM)
  - Configurable metrics selection, update interval, panel position
  - WebSocket-based real-time updates
  - Preferences panel with all settings
- **Build script** — Add `gnome-ext` command to build.sh for extension install + schema compilation
- **API Endpoint Picker** — New picker dialog with TLS sharing between API proxy and WebSocket dashboard
- **Prompt Eval Progress** — Track and display prompt evaluation progress
- **Context Display** — Shorten context display to K units, rename Context → Ctx
- **Token Formatting** — Add token formatting in GNOME extension, increase icon size to 24px
- **About Dialog** — Add author line from CARGO_PKG_AUTHORS

### Bug Fixes
- **Security** — Verify downloaded binaries with SHA256 from GitHub CDN
- **Security** — Add zip slip protection in archive extraction
- **Security** — Add allow_credentials to CORS layer for API proxy
- **Security** — Restrict config file permissions to 0600, add constant-time API key comparison
- **Model Loading** — Detect "failed to initialize" as loading error, narrow error detection patterns
- **TLS** — Atomic cert write to prevent DER parse errors, share TLS config between API and WebSocket servers
- **Download** — Reset DownloadStatus to Downloading when resuming from paused state
- **UI** — Fix confirmation dialog height (buttons no longer cut off)
- **UI** — Improve top bar metrics display with labels, fix RAM/CPU selection
- **UI** — Allow ESC to exit Files/Search mode when README panel is open
- **Dashboard** — Show only LLM filename, not full path
- **GNOME extension** — Remove prompt_tokens metric not provided by llama.cpp
- **CPU** — Improve CPU usage with throttled ticks and deferred model metrics
- **Active Model** — Show model filename only, fix model_filename usage
- **Tests** — Fix flaky test_ctrl_l_cycles_language by removing global i18n state assertions
- **Archive** — Fix zip extraction for zip 2.x, ensure dest_dir exists
- **RPC/Web Search** — Restore RPC Workers and Web Search rows in Server Settings table
- **Spec Decoding** — Remove spec decoding args from llama-bench command
- **Status Bar** — Show [ N/A ] instead of ○ when server not running, improve contrast

### Refactoring
- **Centralized Colors** — Add centralized color constants module, refactor all TUI modules to use them (40+ files)
- **i18n** — Remove unused status.server key from all locale files

### Documentation
- **GNOME Extension** — Add documentation with install, config, and metrics reference
- **README** — Update for API endpoint picker, fix missing/inaccurate docs
- **Build Script** — Document gnome-ext command

### Style
- Unify theme colors for display uniformity, resolve P1 color inconsistencies
