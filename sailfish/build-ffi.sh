#!/usr/bin/env bash
#
# Build the metascrub core (crate metascrub-ffi) as a C-ABI STATIC library for a
# Sailfish target, staged where the RPM spec links it.
#
#   sailfish/build-ffi.sh [emulator|i486|armv7|armv7hl|aarch64]
#
# ## Why this runs OUTSIDE the Sailfish build engine
#
# The Sailfish 5.1.0.11 build engine ships Rust 1.75, which is older than the
# workspace's edition-2024 dependencies (e.g. lopdf 0.44) can be compiled by. So
# the core is cross-compiled here with a modern Rust instead. A `staticlib` is
# just an archive of object files, so producing it needs no cross-linker at all,
# only `rustup target add <triple>`; the final link against the Sailfish sysroot
# happens when the RPM builds the app. The .a is dropped into rustlib/<cpu>/,
# where the spec's %build looks for it (see BUILD.md).
set -euo pipefail

alias_arg="${1:-aarch64}"
case "$alias_arg" in
    emulator|i486|i686) TRIPLE=i686-unknown-linux-gnu;        CPU=i486 ;;
    armv7|armv7hl)      TRIPLE=armv7-unknown-linux-gnueabihf; CPU=armv7hl ;;
    aarch64|arm64)      TRIPLE=aarch64-unknown-linux-gnu;     CPU=aarch64 ;;
    *) echo "unknown target '$alias_arg' (use emulator|armv7|aarch64)" >&2; exit 2 ;;
esac

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(dirname "$here")"
libdst="$here/harbour-metascrub/rustlib/$CPU"

echo "==> building metascrub-ffi (staticlib) for $TRIPLE"
rustup target add "$TRIPLE"
# --crate-type staticlib overrides the crate's [staticlib, cdylib] so only the
# archive is produced: no cdylib means no linker is invoked for this cross-build.
cargo rustc --release --manifest-path "$root/Cargo.toml" \
    -p metascrub-ffi --target "$TRIPLE" --lib --crate-type staticlib

mkdir -p "$libdst"
cp "$root/target/$TRIPLE/release/libmetascrub_ffi.a" "$libdst/"
echo
echo "staged:"
ls -la "$libdst/libmetascrub_ffi.a"
echo
echo "now build the RPM in the Sailfish engine (Docker), e.g.:"
echo "  mb2 -t SailfishOS-5.1.0.11-$CPU build"
