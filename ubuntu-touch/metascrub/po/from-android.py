#!/usr/bin/env python3
"""
Fill this app's translations from the Android app's, where the English is the
same sentence.

The Ubuntu Touch interface was written with the Android one open, so most of its
words are already translated into ten languages in
android/app/src/main/res/values-*/strings.xml. Rather than translate them a
second time, this matches on the English text and copies the translation across.

It only ever matches an *exact* English string. Anything new to this platform,
and every plural form, is left untranslated, and gettext falls back to English
for it. Nothing here invents a translation: for a tool people may be using under
pressure, a confidently wrong sentence in a language nobody on the project reads
is worse than an honest English one.

    ubuntu-touch/metascrub/po/from-android.py [--report]

--report prints the coverage table and writes nothing.
"""
import re
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
ANDROID_RES = REPO / "android/app/src/main/res"
POT = HERE / "metascrub.noxtoxcipher.pot"

# Android resource directory -> gettext locale name.
LANGUAGES = {
    "values-ar": "ar",
    "values-b+ckb": "ckb",
    "values-b+kmr": "kmr",
    "values-be": "be",
    "values-eo": "eo",
    "values-fa": "fa",
    "values-la": "la",
    "values-my": "my",
    "values-ru": "ru",
    "values-uk": "uk",
}


def android_strings(path):
    """name -> text, with Android's escaping turned back into plain text."""
    out = {}
    if not path.exists():
        return out
    for node in ET.parse(path).getroot():
        if node.tag != "string" or not node.get("name"):
            continue
        text = "".join(node.itertext())
        text = text.replace("\\n", "\n").replace("\\'", "'").replace('\\"', '"')
        # Android numbers its placeholders %1$s / %1$d; Qt writes %1.
        text = re.sub(r"%(\d+)\$[sd]", r"%\1", text)
        out[node.get("name")] = text.strip()
    return out


def pot_msgids(path):
    """Every singular msgid in the template, in order, skipping plurals."""
    ids, current, in_id, has_plural = [], [], False, False
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.startswith("msgid_plural"):
            has_plural = True
            continue
        if line.startswith("msgid "):
            if in_id and current and not has_plural:
                ids.append("".join(current))
            current, in_id, has_plural = [unquote(line[6:])], True, False
        elif in_id and line.startswith('"'):
            current.append(unquote(line))
        elif in_id and (line.startswith("msgstr") or not line.strip()):
            if current and not has_plural:
                ids.append("".join(current))
            current, in_id = [], False
    if in_id and current and not has_plural:
        ids.append("".join(current))
    return [i for i in ids if i]


def unquote(fragment):
    fragment = fragment.strip()
    if not (fragment.startswith('"') and fragment.endswith('"')):
        return ""
    body = fragment[1:-1]
    return body.replace('\\"', '"').replace("\\n", "\n").replace("\\\\", "\\")


def quote(text):
    body = text.replace("\\", "\\\\").replace('"', '\\"').replace("\n", "\\n")
    return '"%s"' % body


HEADER = """# Translations for metascrub on Ubuntu Touch.
#
# Written by from-android.py: every string here was already translated for the
# Android app and carries the identical English source. Strings new to this
# platform are absent on purpose and fall back to English.
msgid ""
msgstr ""
"Project-Id-Version: metascrub.noxtoxcipher\\n"
"Report-Msgid-Bugs-To: https://github.com/NoxToxCipher/metascrub/issues\\n"
"Language: %s\\n"
"MIME-Version: 1.0\\n"
"Content-Type: text/plain; charset=UTF-8\\n"
"Content-Transfer-Encoding: 8bit\\n"
"""


def main():
    report_only = "--report" in sys.argv
    if not POT.exists():
        sys.exit("no %s yet — run update-pot.sh first" % POT.name)

    msgids = pot_msgids(POT)
    english = android_strings(ANDROID_RES / "values/strings.xml")
    # English text -> resource name, so a translation can be looked up by words.
    by_text = {text: name for name, text in english.items()}

    print("%d translatable strings in the interface" % len(msgids))
    print("%d of them exist word for word in the Android app\n"
          % sum(1 for m in msgids if m in by_text))

    for res_dir, locale in sorted(LANGUAGES.items(), key=lambda kv: kv[1]):
        translated = android_strings(ANDROID_RES / res_dir / "strings.xml")
        entries = []
        for msgid in msgids:
            name = by_text.get(msgid)
            if name and translated.get(name):
                entries.append((msgid, translated[name]))

        print("%-5s %3d / %d" % (locale, len(entries), len(msgids)))
        if report_only:
            continue

        lines = [HEADER % locale]
        for msgid, msgstr in entries:
            lines.append("\nmsgid %s\nmsgstr %s\n" % (quote(msgid), quote(msgstr)))
        (HERE / ("%s.po" % locale)).write_text("".join(lines), encoding="utf-8")

    if not report_only:
        print("\nwrote %d .po files" % len(LANGUAGES))


if __name__ == "__main__":
    try:
        main()
    except BrokenPipeError:
        pass  # the report was piped into something that stopped reading
