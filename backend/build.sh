#!/bin/bash
set -euo pipefail

# Keep container build artifacts off the host and use a stable build path.
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/wine-cellar-target}"

echo "--- Rust version info ---"
rustup --version
rustc --version
cargo --version

echo "--- Building plugin backend ---"
cargo build --release --locked
install -Dm755 "$CARGO_TARGET_DIR/release/wine-cask" out/backend
