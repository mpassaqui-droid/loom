#!/usr/bin/env bash
# Build loom-core -> WASM and generate the browser bindings into web/pkg/.
# The web app loads web/pkg/loom_wasm.js as window.__loomRust (non-destructive).
#
# Prereqs (one-time):
#   rustup target add wasm32-unknown-unknown
#   cargo install wasm-bindgen-cli --version 0.2.122   # MUST match Cargo.lock
#
# Usage:  ./build-wasm.sh
set -euo pipefail
cd "$(dirname "$0")"

echo "==> cargo build (wasm32, release)"
cargo build -p loom-wasm --target wasm32-unknown-unknown --release

echo "==> wasm-bindgen -> web/pkg/"
wasm-bindgen target/wasm32-unknown-unknown/release/loom_wasm.wasm \
  --out-dir web/pkg --target web

echo "==> done. web/pkg/loom_wasm.js is ready (served by the static web server)."
