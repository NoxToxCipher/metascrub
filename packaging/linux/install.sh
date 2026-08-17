#!/usr/bin/env sh
#
# Install metascrub from this directory.
#
#   ./install.sh                 into ~/.local, no root needed
#   sudo ./install.sh            into /usr/local, for everyone on the machine
#   ./install.sh --prefix DIR    somewhere else
#
# Nothing here talks to the network, adds a repository, or installs a service.
# It copies files and refreshes the desktop's own caches. uninstall.sh removes
# exactly what this put down.
set -eu

here="$(cd "$(dirname "$0")" && pwd)"
app_id=org.crake.metascrub

if [ "$(id -u)" = 0 ]; then
  prefix=/usr/local
else
  prefix="$HOME/.local"
fi

while [ $# -gt 0 ]; do
  case "$1" in
    --prefix) prefix="$2"; shift 2 ;;
    -h|--help) sed -n '2,12p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

echo "installing into $prefix"

install -d "$prefix/bin" \
           "$prefix/share/applications" \
           "$prefix/share/metainfo" \
           "$prefix/share/licenses/metascrub"

install -m 0755 "$here/metascrub-gui" "$prefix/bin/metascrub-gui"
install -m 0755 "$here/metascrub"     "$prefix/bin/metascrub"
install -m 0644 "$here/LICENSE"       "$prefix/share/licenses/metascrub/LICENSE"
install -m 0644 "$here/$app_id.metainfo.xml" "$prefix/share/metainfo/$app_id.metainfo.xml"

# Every icon size, wherever the theme lives under this prefix.
find "$here/icons" -type f | while read -r icon; do
  rel="${icon#"$here"/icons/}"
  install -d "$prefix/share/icons/$(dirname "$rel")"
  install -m 0644 "$icon" "$prefix/share/icons/$rel"
done

# Exec is written as a bare command in the shipped file, which only works if
# the install prefix happens to be on PATH. ~/.local/bin usually is and
# sometimes is not, and a launcher entry that silently does nothing is worse
# than no entry, so point it at the binary that was just installed.
sed "s|^Exec=metascrub-gui|Exec=$prefix/bin/metascrub-gui|; s|^TryExec=metascrub-gui|TryExec=$prefix/bin/metascrub-gui|" \
  "$here/$app_id.desktop" > "$prefix/share/applications/$app_id.desktop"
chmod 0644 "$prefix/share/applications/$app_id.desktop"

# Refresh the caches, where the tools exist. None of this is fatal: the
# application works without it, the menu entry just takes longer to appear.
if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$prefix/share/applications" 2>/dev/null || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -qtf "$prefix/share/icons/hicolor" 2>/dev/null || true
fi

echo
echo "installed:"
echo "  $prefix/bin/metascrub-gui     the window"
echo "  $prefix/bin/metascrub         the command line tool"
echo

case ":$PATH:" in
  *":$prefix/bin:"*) ;;
  *) echo "note: $prefix/bin is not on your PATH, so the 'metascrub' command"
     echo "      will not be found by name. The menu entry works regardless." ;;
esac

if [ ! -d "$prefix/share/icons/hicolor" ]; then
  echo "note: no icon theme directory was created, which should not happen."
fi
