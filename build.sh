#!/usr/bin/env bash
# Build script for llm-manager
# Usage: ./build.sh [command]
# Commands: build, run, clean, format, check, release

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

usage() {
    echo "Usage: $0 [command] [options]"
    echo ""
    echo "Commands:"
    echo "  build          - Build the project (debug)"
    echo "  release        - Build with release profile"
    echo "  run            - Build and run the TUI (-b to bind, -p to port)"
    echo "  serve          - Build and serve a model with llama-server (-i model.gguf)"
    echo "  server         - Build and start the HTTP server (-b addr, --api-key KEY)"
    echo "  clean          - Remove build artifacts"
    echo "  format         - Format code with rustfmt"
    echo "  check          - Check code (cargo check)"
    echo "  test           - Run tests"
    echo "  clippy         - Run clippy lints"
    echo ""
    echo "Examples:"
    echo "  $0 build                 # Build debug binary"
    echo "  $0 release               # Build optimized binary"
    echo "  $0 run                   # Launch TUI"
    echo "  $0 serve -i model.gguf   # Serve a model"
    echo "  $0 server --bind 0.0.0.0:49222 --api-key abc123"
    echo "  $0 run --server http://127.0.0.1:49222 --api-key abc123  # TUI connected to remote server"
    echo "  $0 clippy -- -D warnings # Fail on all warnings"
    echo "  $0 format                # Reformat all code"
    echo "  $0 help                  # Show this help"
}

cmd_build() {
    echo "Building llm-manager..."
    cargo build "$@"
}

cmd_run() {
    cmd_build
    cargo run -- tui "$@"
}

cmd_serve() {
    cmd_build
    cargo run -- serve "$@"
}

cmd_server() {
    cmd_build
    cargo run -- server "$@"
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
    serve)   shift; cmd_serve "$@" ;;
    server)  shift; cmd_server "$@" ;;
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
