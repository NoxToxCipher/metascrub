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
#
# On Linux this script is also where the glibc floor is enforced. See the
# GLIBC section further down, and packaging/linux/build-in-container.sh for the
# build that actually produces a shippable Linux binary.
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

# Name the artifacts explicitly per host. Listing both "metascrub" and
# "metascrub.exe" looked thorough and was not: MSYS resolves a missing
# extension by trying .exe, so on Windows the loop checked the same two files
# twice and printed four reassuring lines.
case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*) artifacts="target/release/metascrub.exe target/release/metascrub-gui.exe"; host=windows ;;
  Darwin)               artifacts="target/release/metascrub target/release/metascrub-gui";         host=macos ;;
  *)                    artifacts="target/release/metascrub target/release/metascrub-gui";         host=linux ;;
esac

fail=0

# ---------------------------------------------------------------------------
# Does the binary name anybody's home directory?
# ---------------------------------------------------------------------------
#
# The first version of this check grepped for `basename "$HOME"` as a regular
# expression, unanchored. That was wrong twice over.
#
# Too loose: on a machine where the account is called something ordinary, the
# name turns up inside unrelated strings and the check fails a build that is
# perfectly clean. Building in a container as `root` failed on "chroot" and
# "root viewport stack underflow", printed "refusing to call this a release",
# and would have convinced anyone reading it that the remap was broken. It was
# not.
#
# Too weak: the basename is not the thing that leaks. What leaks is the whole
# path, and a path belonging to *any* account is just as bad as one belonging
# to this one. A CI machine building under /home/runner leaks "runner", which
# the old check would only have caught if the developer happened to be called
# runner too.
#
# So: fixed strings, not patterns, and look for the path prefixes themselves.
echo
echo "==> checking the result names no home directory"

needles=(".cargo/registry" ".cargo\\registry" "/home/" "/Users/" "C:\\Users\\")
# The real home and cargo directory of this build, whatever they are. Skipped
# when $HOME is implausibly short, so a stray "/" never matches everything.
[ "${#HOME}" -gt 4 ] && needles+=("$HOME")
[ "${#CARGO_HOME}" -gt 4 ] && needles+=("$CARGO_HOME")
# Under MSYS these are POSIX paths (/c/Users/someone) while the binary carries
# the Windows form (C:\Users\someone), because that is what was passed to
# --remap-path-prefix. Without converting them, the only thing catching a
# Windows leak would be the generic "C:\Users\" prefix above.
if [ "$host" = windows ]; then
  [ "${#HOME}" -gt 4 ] && needles+=("$(win "$HOME")")
  [ "${#CARGO_HOME}" -gt 4 ] && needles+=("$(win "$CARGO_HOME")")
fi

for b in $artifacts; do
  [ -f "$b" ] || { echo "   MISSING $b"; fail=1; continue; }
  hits=""
  for n in "${needles[@]}"; do
    # -a so a binary is treated as text, -F so nothing in the needle is a
    # metacharacter, -o so the failure message can show what was actually found
    # instead of leaving the reader to guess.
    found="$(grep -a -o -F -- "$n" "$b" 2>/dev/null | sort | uniq -c | head -3 || true)"
    [ -n "$found" ] && hits="$hits$found"$'\n'
  done
  if [ -n "$hits" ]; then
    # A build that still names its builder must not reach the download page, so
    # this script fails rather than printing a warning somebody scrolls past.
    echo "   FAIL $b"
    printf '%s' "$hits" | sed 's/^/        /'
    fail=1
  else
    echo "   ok   $b"
  fi
done

# ---------------------------------------------------------------------------
# GLIBC floor (Linux only)
# ---------------------------------------------------------------------------
#
# A Linux binary refuses to start if it asks for a glibc newer than the one on
# the machine, and the version it asks for is decided by the machine that built
# it, not by anything in this repository. Built on Ubuntu 24.04 the GUI came out
# requiring GLIBC_2.39, because Rust's standard library picks up pidfd_spawnp
# there. That binary will not launch on Debian 12, Ubuntu 22.04, RHEL 9, Linux
# Mint 21, or the Debian container behind ChromeOS Crostini. It is not a
# warning: the loader stops before main() runs, and the user sees nothing but a
# version error in a terminal they were never told to open.
#
# So the floor is checked here and a build above it is refused, in the same
# spirit as the path check. The fix is never to lower the standard, it is to
# build somewhere older: packaging/linux/build-in-container.sh.
GLIBC_FLOOR="${METASCRUB_GLIBC_FLOOR:-2.31}"

ver_gt() { [ "$(printf '%s\n%s\n' "$1" "$2" | sort -V | tail -1)" = "$1" ] && [ "$1" != "$2" ]; }

# Guarded on readelf, which is what the check actually runs. Guarding on
# objdump instead meant a machine with one and not the other skipped the glibc
# check silently, which is the failure mode this whole section exists to stop.
if [ "$host" = linux ] && command -v readelf >/dev/null; then
  echo
  echo "==> checking the glibc floor (max allowed $GLIBC_FLOOR)"
  for b in $artifacts; do
    [ -f "$b" ] || continue
    # readelf's version-needs table, not the symbol list. A weak *symbol* still
    # produces a hard version *reference*, and it is the reference the loader
    # refuses on.
    need="$(readelf -V "$b" 2>/dev/null \
            | sed -n 's/.*Name: GLIBC_\([0-9.]*\).*/\1/p' \
            | sort -V | tail -1)"
    need="${need:-none}"
    if [ "$need" = none ]; then
      echo "   ok   $b (no glibc version requirement)"
    elif ver_gt "$need" "$GLIBC_FLOOR"; then
      echo "   FAIL $b needs GLIBC_$need, floor is $GLIBC_FLOOR"
      echo "        build it with packaging/linux/build-in-container.sh instead"
      fail=1
    else
      echo "   ok   $b needs GLIBC_$need"
    fi
  done
fi

[ "$fail" -eq 0 ] || { echo; echo "refusing to call this a release"; exit 1; }

# ---------------------------------------------------------------------------
# Hashes
# ---------------------------------------------------------------------------
#
# Previously this named the two .exe files and swallowed the error, so a Linux
# build finished by printing nothing at all and the Linux downloads went out
# with no hashes to check them against. Hash whatever this host actually built.
echo
echo "==> sha256"
for b in $artifacts; do
  [ -f "$b" ] && sha256sum "$b"
done
