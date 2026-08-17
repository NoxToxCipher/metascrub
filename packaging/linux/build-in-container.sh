#!/usr/bin/env bash
#
# Build the Linux release binaries against an old glibc.
#
#   packaging/linux/build-in-container.sh [--arch amd64|arm64]
#
# A Linux binary demands the glibc version of whatever machine compiled it, and
# refuses to start on anything older. Built on Ubuntu 24.04 the GUI came out
# needing GLIBC_2.39, which rules out Debian 12, Ubuntu 22.04, RHEL 9, Linux
# Mint 21 and the Debian container behind ChromeOS Crostini: most of the
# desktops we are trying to reach. Nothing in the source causes it and nothing
# in the application can work around it. The only fix is to compile somewhere
# older, so this does that.
#
# Debian bullseye carries glibc 2.31, which is old enough for every
# still-supported distribution and for both current Crostini images.
#
# The command line tool is built for musl instead, statically. It has no C
# dependencies at all, so it costs nothing, and the result has no glibc floor
# whatsoever: one file that runs on anything with a Linux kernel, Alpine and
# rescue images included. That trick does not extend to the GUI, which reaches
# the system OpenGL and Wayland libraries through dlopen, and those are glibc
# builds that will not load into a musl process.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
arch=amd64
image="${METASCRUB_BUILD_IMAGE:-debian:bullseye}"

while [ $# -gt 0 ]; do
  case "$1" in
    --arch) arch="$2"; shift 2 ;;
    --image) image="$2"; shift 2 ;;
    -h|--help) sed -n '2,20p' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

case "$arch" in
  amd64) platform=linux/amd64; rust_target=x86_64-unknown-linux-gnu;  musl_target=x86_64-unknown-linux-musl ;;
  arm64) platform=linux/arm64; rust_target=aarch64-unknown-linux-gnu; musl_target=aarch64-unknown-linux-musl ;;
  *) echo "unsupported arch: $arch (expected amd64 or arm64)" >&2; exit 2 ;;
esac

engine=""
for candidate in podman docker; do
  if command -v "$candidate" >/dev/null 2>&1; then engine="$candidate"; break; fi
done
if [ -z "$engine" ]; then
  echo "no container engine found. Install podman or docker." >&2
  echo "Without one there is no way to produce a binary that runs on anything" >&2
  echo "older than this machine, so there is no point building a release here." >&2
  exit 1
fi

out="$root/dist/linux/$arch"
mkdir -p "$out"

echo "==> $engine, $image, $platform"
echo "==> output $out"

# Everything below runs inside the container. It runs as root because apt does,
# and hands the results back to the invoking user at the end so the build
# directory is not left full of root-owned files.
"$engine" run --rm \
  --platform "$platform" \
  -v "$root:/src" \
  -e HOST_UID="$(id -u)" \
  -e HOST_GID="$(id -g)" \
  -e ARCH="$arch" \
  -e RUST_TARGET="$rust_target" \
  -e MUSL_TARGET="$musl_target" \
  "$image" \
  bash -euo pipefail -c '
    export DEBIAN_FRONTEND=noninteractive
    apt-get update -qq
    # build-essential for the linker, curl for rustup, ca-certificates so it
    # can verify what it downloads. No GUI development packages: everything the
    # window needs is opened with dlopen at runtime, so none of it is needed to
    # link, which is also why one binary works across desktops.
    apt-get install -y -qq --no-install-recommends build-essential curl ca-certificates >/dev/null

    # Keep the toolchain inside the mount so a second run does not re-download
    # it, and so nothing lands in the container image.
    export RUSTUP_HOME=/src/target/container/rustup
    export CARGO_HOME=/src/target/container/cargo
    export CARGO_TARGET_DIR=/src/target/container/target
    export PATH="$CARGO_HOME/bin:$PATH"

    if [ ! -x "$CARGO_HOME/bin/rustup" ]; then
      curl --proto =https --tlsv1.2 -sSf https://sh.rustup.rs \
        | sh -s -- -y --no-modify-path --default-toolchain none >/dev/null
    fi
    cd /src
    # rust-toolchain.toml pins the version, so this installs the right one.
    rustup show >/dev/null
    rustup target add "$MUSL_TARGET" >/dev/null

    # The same path remapping build-desktop.sh applies, for the same reason: a
    # release binary otherwise carries the absolute path of every source file
    # that can panic, and those paths name whoever built it.
    export RUSTFLAGS="--remap-path-prefix=$CARGO_HOME=/cargo --remap-path-prefix=/src=/src"

    echo "==> GUI, dynamic, against $(ldd --version | head -1)"
    cargo build --release --target "$RUST_TARGET" -p metascrub-gui

    echo "==> command line tool, static"
    cargo build --release --target "$MUSL_TARGET" -p metascrub

    mkdir -p "/src/dist/linux/$ARCH"
    cp "$CARGO_TARGET_DIR/$RUST_TARGET/release/metascrub-gui" "/src/dist/linux/$ARCH/"
    cp "$CARGO_TARGET_DIR/$MUSL_TARGET/release/metascrub"     "/src/dist/linux/$ARCH/"
    chown -R "$HOST_UID:$HOST_GID" "/src/dist" /src/target/container
  '

echo
echo "==> checking what came out"
fail=0
for b in "$out/metascrub-gui" "$out/metascrub"; do
  [ -f "$b" ] || { echo "   MISSING $b"; fail=1; continue; }

  # The same leak check build-desktop.sh runs. A binary built in a container is
  # not automatically clean: the container has a home directory too.
  hits=""
  for n in ".cargo/registry" "/home/" "/Users/" "/root/"; do
    found="$(grep -a -o -F -- "$n" "$b" 2>/dev/null | sort -u | head -3 || true)"
    [ -n "$found" ] && hits="$hits$found"$'\n'
  done
  if [ -n "$hits" ]; then
    echo "   FAIL $b names a home directory"
    printf '%s' "$hits" | sed 's/^/        /'
    fail=1
    continue
  fi

  need="$(readelf -V "$b" 2>/dev/null | sed -n 's/.*Name: GLIBC_\([0-9.]*\).*/\1/p' | sort -V | tail -1)"
  if [ -z "$need" ]; then
    echo "   ok   $(basename "$b")  static, no glibc requirement"
  else
    echo "   ok   $(basename "$b")  needs GLIBC_$need"
  fi
done
[ "$fail" -eq 0 ] || { echo; echo "refusing to call this a release"; exit 1; }

echo
echo "==> sha256"
( cd "$out" && sha256sum metascrub-gui metascrub )
echo
echo "Next: packaging/linux/package.sh --arch $arch"
