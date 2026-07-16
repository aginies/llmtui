# Changelog

## [1.10.0] - 2026-07-16

### Added
- **GPU device names in Main GPU setting** — show full GPU model names on Linux using lspci (`7a44226`)
- **Toast warning for insufficient disk space** — displays warning when download target has low free space (`f90594d`)

### Fixed
- **llama.cpp backend symlink extraction** — preserve symlinks during tar extraction and library copy, fixing `file too short` errors with versioned `.so` libraries (`8efe10f`)
- **Backend picker scroll** — correct visible entries calculation and add scroll to show all available backends (`3faefd7`, `3ce4fba`)
- **API endpoint not disabled** on transient port bind after server exit (`41f1946`)
- **Config file** — 5 bug fixes in config loading/parsing (`8286a52`)
- **top_k clamping** to 0 in WsMetrics conversion (`ccb5af1`)

### Changed
- **Log level** default changed to `trace`, added `warning` level (`67718d2`)
- **Refactored** RPC worker edit state to picker, simplified metrics model name (`67718d2`)

### Server
- **WebUI toggle** added to LLaMA server options picker, persisted to config (`2c8b7e6`, `12122c8`, `797a2fc`)
