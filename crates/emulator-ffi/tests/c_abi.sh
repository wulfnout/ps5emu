#!/usr/bin/env sh
# Compile a C program against include/emulator.h + the static library and run it.
# This exercises the same interface Swift uses on iOS.
set -eu
cd "$(dirname "$0")/../../.."
cargo build -p emulator-ffi --release
out=$(mktemp -d)
cc -std=c11 -D_GNU_SOURCE -Wall -Wextra -Werror -I crates/emulator-ffi/include \
   crates/emulator-ffi/tests/c_abi_check.c target/release/libemulator_ffi.a \
   -lpthread -ldl -lm -o "$out/c_abi_check"
"$out/c_abi_check"
