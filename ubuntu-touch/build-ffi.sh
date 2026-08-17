#!/usr/bin/env bash
#
# Build the metascrub core (crate metascrub-ffi) as a C-ABI static library for an
# Ubuntu Touch architecture, and drop it where the click's CMake build looks.
#
#   ubuntu-touch/build-ffi.sh [arm64|armhf|amd64]
#
# With no argument it takes the architecture from $ARCH (clickable sets it), then
# from dpkg, then from the host. The result lands in
#
#   ubuntu-touch/metascrub/rustlib/<arch triplet>/libmetascrub_ffi.a
#
# one directory per architecture, so builds for the phone and for the desktop can
# sit side by side.
#
# ## Where this has to run
#
# The static library is linked into the app's binary by the click build, so it
# must be built for the same target and against a compatible glibc. Two ways:
#
#   * Inside the clickable container (`clickable script build-ffi`, or the
#     library entry in clickable.yaml). Needs a Rust toolchain in the image.
#   * On the host with the cross target installed:
#         rustup target add aarch64-unknown-linux-gnu
#         sudo apt install gcc-aarch64-linux-gnu
#     This is fine because the app's own Qt code is what needs the Ubuntu Touch
#     sysroot; the Rust side only needs the target's glibc symbols, and focal's
#     glibc 2.31 is older than any host this is likely to be built on. If a link
#     error mentions a GLIBC version, build it in the container instead.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(dirname "$here")"

arch="${1:-}"
if [ -z "$arch" ]; then
    if [ -n "${ARCH:-}" ]; then
        arch="$ARCH"
    elif command -v dpkg-architecture >/dev/null 2>&1; then
        arch="$(dpkg-architecture -qDEB_HOST_ARCH)"
    else
        arch="$(dpkg --print-architecture 2>/dev/null || echo amd64)"
    fi
fi

case "$arch" in
    arm64|aarch64)
        TRIPLE=aarch64-unknown-linux-gnu; ARCH_TRIPLET=aarch64-linux-gnu ;;
    armhf|armv7|armv7l|arm)
        TRIPLE=armv7-unknown-linux-gnueabihf; ARCH_TRIPLET=arm-linux-gnueabihf ;;
    amd64|x86_64)
        TRIPLE=x86_64-unknown-linux-gnu; ARCH_TRIPLET=x86_64-linux-gnu ;;
    *)
        echo "unknown architecture '$arch' (use arm64, armhf or amd64)" >&2; exit 2 ;;
esac

# Point cargo at the cross linker when one is installed and we are not native.
linker="${ARCH_TRIPLET}-gcc"
if [ "$ARCH_TRIPLET" != "$(gcc -dumpmachine 2>/dev/null || echo none)" ] \
   && command -v "$linker" >/dev/null 2>&1; then
    var="CARGO_TARGET_$(echo "$TRIPLE" | tr 'a-z-' 'A-Z_')_LINKER"
    export "$var=$linker"
    echo "==> cross linker: $linker"
fi

# Keep the build machine out of the shipped binary.
#
# `strip` removes symbols, not data. Panic locations are &'static str literals
# in .rodata, so a stripped library still carries the absolute path of every
# source file that can panic, including the building user's home directory. The
# first click built here held 108 of them. That is the same leak the Android
# build already fixes, in a tool whose entire job is removing exactly this kind
# of thing from other people's files, and it also breaks the reproducibility
# that pinning the compiler exists to support: two people with the same
# toolchain and the same source get different bytes if their home directories
# differ.
#
# `trim-paths` in [profile.release] is the tidy form and is still unstable in
# Cargo 1.97.1, which this workspace pins on purpose, so it is done with flags
# here exactly as android/build-apk.sh does it.
win() { if command -v cygpath >/dev/null 2>&1; then cygpath -w "$1"; else printf '%s' "$1"; fi; }
: "${CARGO_HOME:=$HOME/.cargo}"
export RUSTFLAGS="${RUSTFLAGS:-} --remap-path-prefix=$(win "$CARGO_HOME")=/cargo --remap-path-prefix=$(win "$root")=/src"

echo "==> building metascrub-ffi for $arch ($TRIPLE)"
cargo build --release --manifest-path "$root/Cargo.toml" -p metascrub-ffi --target "$TRIPLE"

dest="$here/metascrub/rustlib/$ARCH_TRIPLET"
mkdir -p "$dest"
cp "$root/target/$TRIPLE/release/libmetascrub_ffi.a" "$dest/"

echo
echo "installed: $dest/libmetascrub_ffi.a"
echo "the click build finds it there automatically; override with"
echo "  cmake -DMETASCRUB_LIB_DIR=<dir>"
