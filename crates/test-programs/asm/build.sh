#!/usr/bin/env sh
# Re-assemble every reference program with GNU binutils.
# Requires: as, objcopy (x86-64 binutils).
set -eu
cd "$(dirname "$0")"
tmp=$(mktemp -d)
for src in *.s; do
  name=${src%.s}
  as --64 -o "$tmp/$name.o" "$src"
  objcopy -O binary -j .text "$tmp/$name.o" "$name.bin"
  echo "assembled $name.bin ($(wc -c < "$name.bin") bytes)"
done
rm -rf "$tmp"
