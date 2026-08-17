#!/usr/bin/env sh
#
# Remove what install.sh put down, and nothing else.
#
#   ./uninstall.sh                 from ~/.local
#   sudo ./uninstall.sh            from /usr/local
#   ./uninstall.sh --prefix DIR
#
# metascrub writes no configuration, no cache and no state anywhere, so there
# is nothing left behind afterwards. That is not tidiness for its own sake: a
# directory in your home folder recording that this tool was ever installed is
# the same class of trace the tool exists to remove.
set -eu

app_id=org.crake.metascrub

if [ "$(id -u)" = 0 ]; then
  prefix=/usr/local
else
  prefix="$HOME/.local"
fi

while [ $# -gt 0 ]; do
  case "$1" in
    --prefix) prefix="$2"; shift 2 ;;
    -h|--help) sed -n '2,10p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

echo "removing from $prefix"

rm -f "$prefix/bin/metascrub-gui" \
      "$prefix/bin/metascrub" \
      "$prefix/share/applications/$app_id.desktop" \
      "$prefix/share/metainfo/$app_id.metainfo.xml" \
      "$prefix/share/licenses/metascrub/LICENSE"
rmdir "$prefix/share/licenses/metascrub" 2>/dev/null || true

# Named exactly, so a shared icon theme keeps everything that is not ours.
find "$prefix/share/icons" -name "$app_id.png" -delete 2>/dev/null || true
find "$prefix/share/icons" -name "$app_id.svg" -delete 2>/dev/null || true

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$prefix/share/applications" 2>/dev/null || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -qtf "$prefix/share/icons/hicolor" 2>/dev/null || true
fi

echo "done. metascrub leaves no configuration or state behind."
