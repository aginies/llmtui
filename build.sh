#!/usr/bin/env bash
# Build script for llm-manager
# Usage: ./build.sh [command]
# Commands: build, run, clean, format, check, release

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

usage() {
    echo "Usage: $0 [command]"
    echo ""
    echo "Commands:"
    echo "  build     - Build the project (debug)"
    echo "  run       - Build and run"
    echo "  release   - Build with release profile"
    echo "  clean     - Remove build artifacts"
    echo "  format    - Format code with rustfmt"
    echo "  check     - Check code (cargo check)"
    echo "  test      - Run tests"
    echo "  clippy    - Run clippy lints"
    echo "  help      - Show this help"
}

cmd_build() {
    echo "Building llm-manager..."
    cargo build "$@"
}

cmd_run() {
    cmd_build "$@"
    cargo run "$@"
}

cmd_release() {
    echo "Building llm-manager (release)..."
    cargo build --release "$@"
}

cmd_clean() {
    echo "Cleaning build artifacts..."
    cargo clean "$@"
}

cmd_format() {
    echo "Formatting code..."
    cargo fmt "$@"
}

cmd_check() {
    echo "Checking code..."
    cargo check "$@"
}

cmd_test() {
    echo "Running tests..."
    cargo test "$@"
}

cmd_clippy() {
    echo "Running clippy..."
    cargo clippy "$@"
}

case "${1:-help}" in
    build)   shift; cmd_build "$@" ;;
    run)     shift; cmd_run "$@" ;;
    release) shift; cmd_release "$@" ;;
    clean)   shift; cmd_clean "$@" ;;
    format)  shift; cmd_format "$@" ;;
    check)   shift; cmd_check "$@" ;;
    test)    shift; cmd_test "$@" ;;
    clippy)  shift; cmd_clippy "$@" ;;
    help|--help|-h) usage ;;
    *)
        echo "Unknown command: $1"
        usage
        exit 1
        ;;
esac
