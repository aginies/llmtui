# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.1.2] — 2026-06-03

### Added
- **Persisted UI state** — active panel and left panel width now saved to config and restored on startup
- **Backend picker Delete key** — `Delete` key now works alongside `d` to delete backend versions
- **Backend picker hint rendering** — custom hint bar showing `Suppr` and `⎋ Exit` in the backend picker

### Changed
- **Improved backend version resolution** — `resolve_backend_binary` now compares local installed version against GitHub releases and picks the newer one, using asset-pattern matching per backend

### Fixed
- Test compilation for new `active_panel` and `left_pct` config fields

## [1.1.1] — 2026-05-28

### Fixed
- **Cross-platform builds** — `statvfs` gated behind Linux-only `cfg`; glob pattern for Windows `.exe` binary in release workflow

### Changed
- Updated version across Cargo.toml, README, and User-Agent headers
