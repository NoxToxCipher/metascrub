#!/usr/bin/env bash
#
# Turn built binaries into things a person can download.
#
#   packaging/linux/package.sh [--arch amd64|arm64]
#
# Expects dist/linux/<arch>/{metascrub-gui,metascrub}, which
# build-in-container.sh produces. Writes into dist/.
#
# Three formats, for three different situations:
#
#   .tar.gz    every distribution. Unpack, run install.sh, no root needed.
#              This is the one that has to work everywhere, so it depends on
#              nothing but a shell.
#   .deb       Debian, Ubuntu, Mint, and the Debian container inside ChromeOS,
#              which is the whole ChromeOS story. Puts the launcher entry in
#              the right place without the user having to know there is one.
#   AppDir     the staging tree for an AppImage. The final step needs
#              appimagetool, which is a download, so it is only run when that
#              tool is already present rather than fetched behind your back.
#
# Deliberately not here: anything that adds a repository to the system, and
# anything that phones home to check for updates.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
here="$root/packaging/linux"
arch=amd64
app_id=org.crake.metascrub

while [ $# -gt 0 ]; do
  case "$1" in
    --arch) arch="$2"; shift 2 ;;
    -h|--help) sed -n '2,25p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/Cargo.toml" | head -1)"
[ -n "$version" ] || { echo "could not read the version out of Cargo.toml" >&2; exit 1; }

bin="$root/dist/linux/$arch"
for f in metascrub-gui metascrub; do
  [ -f "$bin/$f" ] || {
    echo "missing $bin/$f" >&2
    echo "build it first: packaging/linux/build-in-container.sh --arch $arch" >&2
    exit 1
  }
done

stem="metascrub-$version-linux-$arch"
work="$root/dist/work/$arch"
rm -rf "$work"
mkdir -p "$work" "$root/dist"

echo "==> metascrub $version, $arch"

# ---------------------------------------------------------------------------
# Refuse to package something that should not be downloaded
# ---------------------------------------------------------------------------
#
# build-in-container.sh already checks its own output, but this script takes
# whatever is sitting in dist/linux/<arch>/, and it is easy to drop a plain
# `cargo build` result in there while testing and then forget. This is the last
# point at which a binary is still a build artefact rather than a download, so
# the checks run again here. Cheap, and the failure it prevents is the kind
# that only shows up after somebody has the file.
gate=0
for b in "$bin/metascrub-gui" "$bin/metascrub"; do
  for n in ".cargo/registry" "/home/" "/Users/" "/root/"; do
    if found="$(grep -a -o -F -- "$n" "$b" 2>/dev/null | sort -u | head -2)" && [ -n "$found" ]; then
      echo "   FAIL $(basename "$b") names a build path: $(echo "$found" | tr '\n' ' ')" >&2
      gate=1
    fi
  done
  need="$(readelf -V "$b" 2>/dev/null | sed -n 's/.*Name: GLIBC_\([0-9.]*\).*/\1/p' | sort -V | tail -1)"
  floor="${METASCRUB_GLIBC_FLOOR:-2.31}"
  if [ -n "$need" ] && [ "$(printf '%s\n%s\n' "$need" "$floor" | sort -V | tail -1)" != "$floor" ]; then
    echo "   FAIL $(basename "$b") needs GLIBC_$need, floor is $floor" >&2
    gate=1
  fi
done
if [ "$gate" -ne 0 ]; then
  echo >&2
  echo "These binaries were not produced by build-in-container.sh." >&2
  echo "Run: packaging/linux/build-in-container.sh --arch $arch" >&2
  exit 1
fi

# ---------------------------------------------------------------------------
# Tarball
# ---------------------------------------------------------------------------
tree="$work/$stem"
mkdir -p "$tree"
cp "$bin/metascrub-gui" "$bin/metascrub" "$tree/"
cp "$here/install.sh" "$here/uninstall.sh" "$tree/"
cp "$here/$app_id.desktop" "$here/$app_id.metainfo.xml" "$tree/"
cp "$root/LICENSE" "$tree/"
cp -r "$here/icons" "$tree/icons"
chmod 0755 "$tree/install.sh" "$tree/uninstall.sh" "$tree/metascrub-gui" "$tree/metascrub"

cat > "$tree/README" <<EOF
metascrub $version, Linux $arch

  ./install.sh          install for you, into ~/.local
  sudo ./install.sh     install for everyone, into /usr/local
  ./uninstall.sh        remove it again

Or do not install anything: ./metascrub-gui runs from this directory as it is,
and ./metascrub is the command line tool. Both are self-contained.

  ./metascrub -n photo.jpg     report what is in the file, write nothing
  ./metascrub photo.jpg        write photo.clean.jpg alongside it

The window needs a graphical session. The command line tool is statically
linked and needs nothing at all.

metascrub makes no network connections, has no account, keeps no configuration
and writes no state. Uninstalling leaves nothing behind.

Full documentation: https://github.com/NoxToxCipher/metascrub
EOF

# --sort=name and a fixed mtime, owner and group, so building the same input
# twice gives the same archive. A release nobody can reproduce is a release
# nobody can check.
tar --sort=name \
    --mtime="@${SOURCE_DATE_EPOCH:-0}" \
    --owner=0 --group=0 --numeric-owner \
    --format=gnu \
    -C "$work" -cf "$work/$stem.tar" "$stem"
# -n so the gzip header carries neither the original name nor a timestamp,
# which would otherwise put the build time inside a file whose whole point is
# that it does not record when things happened.
gzip -9n "$work/$stem.tar"
mv "$work/$stem.tar.gz" "$root/dist/$stem.tar.gz"
echo "  dist/$stem.tar.gz"

# ---------------------------------------------------------------------------
# .deb
# ---------------------------------------------------------------------------
if command -v dpkg-deb >/dev/null 2>&1; then
  deb="$work/deb"
  mkdir -p "$deb/DEBIAN" \
           "$deb/usr/bin" \
           "$deb/usr/share/applications" \
           "$deb/usr/share/metainfo" \
           "$deb/usr/share/doc/metascrub"

  install -m 0755 "$bin/metascrub-gui" "$deb/usr/bin/metascrub-gui"
  install -m 0755 "$bin/metascrub"     "$deb/usr/bin/metascrub"
  install -m 0644 "$here/$app_id.desktop" "$deb/usr/share/applications/$app_id.desktop"
  install -m 0644 "$here/$app_id.metainfo.xml" "$deb/usr/share/metainfo/$app_id.metainfo.xml"
  find "$here/icons" -type f | while read -r icon; do
    rel="${icon#"$here"/icons/}"
    install -D -m 0644 "$icon" "$deb/usr/share/icons/$rel"
  done

  # Ask for the glibc the binary actually needs rather than a number somebody
  # typed once and forgot. If this is wrong in the tight direction the package
  # refuses to install on machines it would have run on; wrong in the loose
  # direction and it installs and then will not start.
  need="$(readelf -V "$bin/metascrub-gui" 2>/dev/null \
          | sed -n 's/.*Name: GLIBC_\([0-9.]*\).*/\1/p' | sort -V | tail -1)"
  depends="libc6 (>= ${need:-2.31})"

  cat > "$deb/DEBIAN/control" <<EOF
Package: metascrub
Version: $version
Section: utils
Priority: optional
Architecture: $arch
Maintainer: ${DEB_MAINTAINER:-metascrub contributors <noreply@users.noreply.github.com>}
Depends: $depends
Recommends: xdg-desktop-portal-gtk, libgl1, libegl1, libx11-6, libx11-xcb1, libxcursor1, libxi6, libxrender1, libxkbcommon0, libxkbcommon-x11-0, libwayland-client0, libwayland-egl1, libdbus-1-3
Homepage: https://github.com/NoxToxCipher/metascrub
Description: Remove the information a file carries about you
 Photographs and documents hold a second payload that has nothing to do with
 what they look like: GPS coordinates, camera serial numbers, the author's real
 name, the editing-session identifiers that link two documents to one machine.
 Sending a file sends all of it.
 .
 metascrub removes it and reports exactly what it found. Files are rebuilt from
 an explicit keep-list rather than edited, so unknown vendor sections are
 dropped for not being on the list, and a format it cannot take apart is
 reported as such and returned untouched rather than claimed as clean.
 .
 Contains a windowed application and a command line tool. Makes no network
 connections of any kind.
EOF

  # The runtime libraries are Recommends and not Depends on purpose. apt
  # installs recommendations by default, so a desktop gets them; a server
  # installing with --no-install-recommends gets the command line tool without
  # dragging in OpenGL. The GUI opens all of them with dlopen, so none is
  # needed to install or to run the CLI.

  cat > "$deb/usr/share/doc/metascrub/copyright" <<'EOF'
Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/
Upstream-Name: metascrub
Source: https://github.com/NoxToxCipher/metascrub

Files: *
Copyright: metascrub contributors
License: GPL-3+
 This program is free software: you can redistribute it and/or modify it under
 the terms of the GNU General Public License as published by the Free Software
 Foundation, either version 3 of the License, or (at your option) any later
 version.
 .
 This program is distributed in the hope that it will be useful, but WITHOUT
 ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
 FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
 .
 On Debian systems the full text of the GNU General Public License version 3
 can be found in /usr/share/common-licenses/GPL-3.

Files: crates/metascrub-gui/fonts/Padauk-Regular.ttf
Copyright: SIL International
License: OFL-1.1
 Licensed under the SIL Open Font License, Version 1.1. The full text is in
 crates/metascrub-gui/fonts/OFL-Padauk.txt in the source distribution.
EOF

  # Fixed timestamps and owners again, for the same reason as the tarball:
  # dpkg-deb honours SOURCE_DATE_EPOCH, so two builds of the same input give
  # the same bytes.
  ( export SOURCE_DATE_EPOCH="${SOURCE_DATE_EPOCH:-0}"
    dpkg-deb --root-owner-group -Zgzip --build "$deb" "$root/dist/${stem}.deb" >/dev/null )
  echo "  dist/${stem}.deb"
else
  echo "  (no dpkg-deb, skipping the .deb)"
fi

# ---------------------------------------------------------------------------
# AppDir, and an AppImage if the tool is already here
# ---------------------------------------------------------------------------
appdir="$work/metascrub.AppDir"
mkdir -p "$appdir/usr/bin" "$appdir/usr/share/applications" "$appdir/usr/share/metainfo"
install -m 0755 "$bin/metascrub-gui" "$appdir/usr/bin/metascrub-gui"
install -m 0755 "$bin/metascrub"     "$appdir/usr/bin/metascrub"
install -m 0644 "$here/$app_id.desktop" "$appdir/usr/share/applications/$app_id.desktop"
install -m 0644 "$here/$app_id.metainfo.xml" "$appdir/usr/share/metainfo/$app_id.metainfo.xml"
find "$here/icons" -type f | while read -r icon; do
  rel="${icon#"$here"/icons/}"
  install -D -m 0644 "$icon" "$appdir/usr/share/icons/$rel"
done
# An AppImage wants the desktop file and a top-level icon at the root too.
cp "$here/$app_id.desktop" "$appdir/$app_id.desktop"
cp "$here/icons/hicolor/256x256/apps/$app_id.png" "$appdir/$app_id.png"
cat > "$appdir/AppRun" <<'EOF'
#!/bin/sh
# Run the window from inside the bundle. No library directory is set up
# because there is nothing to bundle: everything the window needs is opened
# from the host system with dlopen at runtime.
HERE="$(dirname "$(readlink -f "$0")")"
exec "$HERE/usr/bin/metascrub-gui" "$@"
EOF
chmod 0755 "$appdir/AppRun"

if command -v appimagetool >/dev/null 2>&1; then
  ARCH="$([ "$arch" = arm64 ] && echo aarch64 || echo x86_64)" \
    appimagetool --no-appstream "$appdir" "$root/dist/$stem.AppImage" >/dev/null
  echo "  dist/$stem.AppImage"
else
  cp -r "$appdir" "$root/dist/$stem.AppDir"
  echo "  dist/$stem.AppDir  (appimagetool not installed; run it on this directory)"
fi

rm -rf "$work"

echo
echo "==> sha256"
( cd "$root/dist" && sha256sum "$stem".* 2>/dev/null | grep -v '\.AppDir' || true )
