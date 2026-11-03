#!/usr/bin/env bash
# Builds ../ext/slipstream.wasm; needs `rustup target add wasm32-unknown-unknown`.
set -euo pipefail
cd "$(dirname "$0")"
cargo build --release --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/slipstream.wasm ../ext/slipstream.wasm
ls -l ../ext/slipstream.wasm
