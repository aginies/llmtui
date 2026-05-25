#!/usr/bin/env bash
# Build script for llm-manager
# Usage: ./build.sh [command]
# Commands: build, run, clean, format, check, release, doc, servedoc

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

# Add cargo bin to PATH if available
if [[ -d "$HOME/.cargo/bin" ]]; then
    export PATH="$HOME/.cargo/bin:$PATH"
fi

usage() {
    echo "Usage: $0 [command]"
    echo ""
    echo "Commands:"
    echo "  build     - Build the project (debug)"
    echo "  run       - Build and run (TUI mode)"
    echo "  serve     - Build and serve a model (llama-server)"
    echo "  servedoc  - Serve documentation with watch mode"
    echo "  release   - Build with release profile"
    echo "  clean     - Remove build artifacts"
    echo "  format    - Format code with rustfmt"
    echo "  check     - Check code (cargo check)"
    echo "  test      - Run tests (all features, verbose)"
    echo "  clippy    - Run clippy lints"
    echo "  doc       - Build documentation"
    echo "  servedoc  - Serve documentation with watch mode"
    echo "  help      - Show this help"
}

examples() {
    echo ""
    echo "Examples:"
    echo "  $0                    # Show help"
    echo "  $0 build              # Build debug binary"
    echo "  $0 release            # Build release binary"
    echo "  $0 run                # Build and launch TUI"
    echo "  $0 serve --model /path/to/model.gguf  # Serve a GGUF model"
    echo "  $0 serve --model model.gguf --profile qwen  # Serve with profile"
    echo "  $0 serve --model model.gguf --api-port 49222  # Serve + API proxy"
    echo "  $0 serve --model model.gguf --api-port 49222 --api-key secret  # Serve + auth"
    echo "  $0 format             # Format source code"
    echo "  $0 clippy             # Run clippy lints"
    echo "  $0 check              # Quick compilation check"
    echo "  $0 test               # Run all tests (verbose, all features)"
    echo "  $0 clean              # Remove target/"
    echo "  $0 doc                # Build documentation"
    echo "  $0 servedoc           # Serve docs with watch mode"
    echo "  $0 release --features vulkan  # Release with Vulkan feature"
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
    echo "Running tests (all features, verbose)..."
    cargo test --all-features -v "$@"
}

cmd_clippy() {
    echo "Running clippy..."
    cargo clippy "$@"
}

cmd_doc() {
    echo "Building documentation..."
    mdbook build docs
    echo "Docs built in docs/book/"
}

cmd_servedoc() {
    echo "Serving documentation..."
    mdbook serve docs --port "${1:-3000}"
}

case "${1:-help}" in
    build)     shift; cmd_build "$@" ;;
    run)       shift; cmd_run "$@" ;;
    serve)     shift; cmd_serve "$@" ;;
    release)   shift; cmd_release "$@" ;;
    clean)     shift; cmd_clean "$@" ;;
    format)    shift; cmd_format "$@" ;;
    check)     shift; cmd_check "$@" ;;
    test)      shift; cmd_test "$@" ;;
    clippy)    shift; cmd_clippy "$@" ;;
    doc)       shift; cmd_doc "$@" ;;
    servedoc)  shift; cmd_servedoc "$@" ;;
    help|--help|-h) usage; examples ;;
    *)
        echo "Unknown command: $1"
        usage
        examples
        exit 1
        ;;
esac
