#!/usr/bin/env sh
# Compile crates/emulator-ffi to WebAssembly and place it where Vite serves it.
set -eu
here=$(cd "$(dirname "$0")/.." && pwd)
root=$(cd "$here/../.." && pwd)
cd "$root"
rustup target add wasm32-unknown-unknown >/dev/null 2>&1 || true
cargo build -p emulator-ffi --release --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/emulator_ffi.wasm "$here/public/emulator_core.wasm"
echo "wrote apps/browser/public/emulator_core.wasm ($(wc -c < "$here/public/emulator_core.wasm") bytes)"
