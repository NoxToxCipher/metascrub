#!/usr/bin/env bash
#
# Rebuild metascrub.noxtoxcipher.pot from the QML, then merge the new strings
# into every existing translation.
#
#   ubuntu-touch/metascrub/po/update-pot.sh
#
# Run this whenever a string in the interface changes. The .po files keep their
# translations; new and changed strings arrive untranslated, and gettext falls
# back to the English source for those, so a half-translated build is never a
# broken one.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
app="$(dirname "$here")"
domain="metascrub.noxtoxcipher"

# QML is close enough to JavaScript for xgettext, which also concatenates the
# "one line " + "and the next" strings the interface uses for long copy.
# Run from the app directory and hand xgettext relative paths: it copies the
# path it is given into every source reference, and an absolute one would put
# the building machine's directories into a committed file.
cd "$app"

xgettext \
    --language=JavaScript \
    --keyword=tr:1 \
    --keyword=tr:1,2 \
    --from-code=UTF-8 \
    --package-name="$domain" \
    --copyright-holder="metascrub contributors" \
    --msgid-bugs-address="https://github.com/NoxToxCipher/metascrub/issues" \
    --sort-by-file \
    -o "$here/$domain.pot" \
    qml/*.qml

echo "wrote $here/$domain.pot"

for po in "$here"/*.po; do
    [ -e "$po" ] || continue
    msgmerge --quiet --update --backup=none "$po" "$here/$domain.pot"
    printf '%-12s ' "$(basename "$po" .po)"
    msgfmt --statistics -o /dev/null "$po"
done
