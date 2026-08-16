#!/usr/bin/env bash
#
# Build the metascrub core (crate metascrub-ffi) as a C-ABI library for a
# Sailfish target, ready for the Silica app to link.
#
#   sailfish/build-ffi.sh [emulator|armv7|aarch64]
#
# ## Where this has to run
#
# The cross-compile needs the Sailfish *sysroot* as its linker sysroot, so run
# this inside the Sailfish SDK build engine (via `sfdk` / `mb2`), or from a host
# where CARGO_TARGET_<triple>_LINKER points at the SDK's cross gcc. The RPM spec
# does the same cargo step during packaging; this script is for the Qt Creator /
# manual emulator workflow: build the lib, then point the app's RUST_LIB_DIR at
# the path this prints.
set -euo pipefail

alias_arg="${1:-emulator}"
case "$alias_arg" in
    emulator|i486|i686) TRIPLE=i686-unknown-linux-gnu ;;
    armv7|armv7hl)      TRIPLE=armv7-unknown-linux-gnueabihf ;;
    aarch64|arm64)      TRIPLE=aarch64-unknown-linux-gnu ;;
    *) echo "unknown target '$alias_arg' (use emulator|armv7|aarch64)" >&2; exit 2 ;;
esac

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(dirname "$here")"

echo "==> building metascrub-ffi for $TRIPLE"
cargo build --release --manifest-path "$root/Cargo.toml" -p metascrub-ffi --target "$TRIPLE"

libdir="$root/target/$TRIPLE/release"
echo
echo "built:"
ls -1 "$libdir"/libmetascrub_ffi.* 2>/dev/null || true
echo
echo "point the app at it, e.g.:"
echo "  qmake RUST_LIB_DIR=$libdir  # or set it in Qt Creator's build settings"
