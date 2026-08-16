#!/usr/bin/env bash
#
# Build the desktop release binaries with the build machine stripped out.
#
#   ./build-desktop.sh
#
# The sibling of android/build-apk.sh, and it exists for the same reason. A
# release binary carries the absolute path of every source file that can panic,
# because those are static strings and `strip = true` only removes symbols. The
# Windows binaries carried the author's home directory 45 and 399 times, and the
# hash of one of them was already published on the download page.
#
# Remapping also finishes what rust-toolchain.toml starts. Pinning the compiler
# means two people can build the same source with the same rustc; it does not
# mean they get the same bytes, because the paths baked into those binaries
# still name whoever built them. Remapped, they match.
#
# `trim-paths` in [profile.release] would be the tidy way to say this, but it is
# not stable in Cargo 1.97.1 and this workspace pins stable on purpose.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
win() { if command -v cygpath >/dev/null; then cygpath -w "$1"; else printf '%s' "$1"; fi; }

: "${CARGO_HOME:=$HOME/.cargo}"
export RUSTFLAGS="${RUSTFLAGS:-} --remap-path-prefix=$(win "$CARGO_HOME")=/cargo --remap-path-prefix=$(win "$root")=/src"

# Every PE carries a TimeDateStamp in its header: the build time, to the
# second. The shipped binaries said 2026-08-15 11:03:19 and 11:04:48 -- ninety
# seconds apart, which is a person at a keyboard, and a release history of
# those draws the hours somebody works. It is the same class of thing as the
# Last-Modified header the site now strips, except baked into the file every
# visitor downloads, where no server config can take it back out.
#
# /Brepro tells the MSVC linker to put a hash of the content there instead, so
# the field stops being a clock and the binary becomes reproducible: build the
# same source twice and get the same bytes. MSVC-only, hence the guard.
case "$(rustc -vV | sed -n 's/^host: //p')" in
  *windows-msvc) export RUSTFLAGS="$RUSTFLAGS -C link-arg=/Brepro" ;;
esac

echo "==> RUSTFLAGS $RUSTFLAGS"
cd "$root"
cargo build --release -p metascrub -p metascrub-gui

echo
echo "==> checking the result carries no home directory"
fail=0
found=0
# Name the artifacts explicitly per host. Listing both "metascrub" and
# "metascrub.exe" looked thorough and was not: MSYS resolves a missing
# extension by trying .exe, so on Windows the loop checked the same two files
# twice and printed four reassuring lines. Anyone reading that output would
# reasonably conclude the Linux binaries had been cleared too. They had not --
# they are built in a container and are not touched by this script at all.
case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*) artifacts="target/release/metascrub.exe target/release/metascrub-gui.exe" ;;
  *)                    artifacts="target/release/metascrub target/release/metascrub-gui" ;;
esac
for b in $artifacts; do
  [ -f "$b" ] || { echo "   MISSING $b"; fail=1; continue; }
  found=$((found+1))
  # A build that still names its builder must not reach the download page, so
  # this script fails rather than printing a warning somebody scrolls past.
  if grep -a -q -E "$(basename "$HOME")" "$b" 2>/dev/null; then
    echo "   FAIL $b still contains the build user"; fail=1
  else
    echo "   ok   $b"
  fi
done
echo "   ($found artifact(s) checked for this host; the Linux container build is separate)"
[ "$fail" -eq 0 ] || { echo "refusing to call this a release"; exit 1; }

echo
echo "==> sha256"
sha256sum target/release/metascrub.exe target/release/metascrub-gui.exe 2>/dev/null || true
