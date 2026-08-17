#!/usr/bin/env bash
#
# Build a .click package without clickable, and therefore without Docker.
#
#   ubuntu-touch/build-click.sh [arm64|armhf|amd64]
#
# clickable is the normal way to package an Ubuntu Touch app, and it is the way
# to build for a phone, because it carries a container with the target's Qt in
# it. It also needs Docker, which is not always available: the machine this
# project is developed on runs Virtualization-Based Security, which stops
# VirtualBox dead and made the Sailfish SDK unusable (see sailfish/BUILD.md).
#
# So this exists as the no-container path:
#
#   * For the host architecture it needs only Qt 5 dev packages, cmake, cargo
#     and click. It produces a real, installable package.
#   * For another architecture it needs a cross Qt as well, which in practice
#     means the clickable container. The script says so rather than producing
#     something broken.
#
# Either way the package is built by `click` itself, so the manifest, the hooks
# and the payload are validated by the same tool the phone uses.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
app="$here/metascrub"

host_arch="$(dpkg --print-architecture 2>/dev/null || echo amd64)"
arch="${1:-$host_arch}"

if [ "$arch" != "$host_arch" ]; then
    cat >&2 <<EOF
This builds for the host architecture ($host_arch), and you asked for $arch.

Cross-building the app needs Qt 5 for $arch, which this script cannot conjure.
Use clickable for that:

    ubuntu-touch/build-ffi.sh $arch
    cd ubuntu-touch/metascrub && clickable build --arch $arch
EOF
    exit 2
fi

# click is a Python program using gi. On a system with several Pythons the
# default one may not be the one gi was built for, so find one that works.
find_click_python() {
    for python in python3 python3.12 python3.11 python3.10; do
        if command -v "$python" >/dev/null 2>&1 \
           && "$python" -c "import gi" >/dev/null 2>&1; then
            echo "$python"
            return 0
        fi
    done
    return 1
}

if ! command -v click >/dev/null 2>&1; then
    echo "click not found. Install it:  sudo apt install click click-dev" >&2
    exit 1
fi
python_for_click="$(find_click_python || true)"
if [ -z "$python_for_click" ]; then
    echo "no Python on this machine can 'import gi', which click needs." >&2
    echo "Install it:  sudo apt install python3-gi" >&2
    exit 1
fi

echo "==> 1/4  the Rust core"
"$here/build-ffi.sh" "$arch"

echo
echo "==> 2/4  configure"
cmake -S "$app" -B "$app/build" \
      -DCMAKE_BUILD_TYPE=Release \
      -DCMAKE_INSTALL_PREFIX="$app/build/install"

echo
echo "==> 3/4  build and lay out the package tree"
cmake --build "$app/build" -j"$(nproc)"
rm -rf "$app/build/install"
cmake --install "$app/build" > /dev/null

echo
echo "==> 4/4  click build"
# click writes the package into the working directory, so choose one rather than
# leaving it wherever the script happened to be run from. The "Ignoring missing
# framework" warning is expected off-device: the framework is declared by the
# phone image, not by the machine doing the packaging. Validation is otherwise
# left on, because checking the manifest and the hooks is the point of using
# click to build rather than tarring the tree up by hand.
out="$app/build/package"
mkdir -p "$out"
rm -f "$out"/*.click
( cd "$out" && "$python_for_click" "$(command -v click)" build "$app/build/install" ) \
    | sed "s/^/    /"

package="$(ls -t "$out"/*.click 2>/dev/null | head -1)"
echo
echo "built: $package"
echo
"$python_for_click" "$(command -v click)" info "$package"
