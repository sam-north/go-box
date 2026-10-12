#!/usr/bin/env bash
# Builds the browser client into web/ (served by go-box-server).
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build -p go-box-client --release --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/go-box-client.wasm web/
echo "web/ is ready: run the server and open http://127.0.0.1:7878"
