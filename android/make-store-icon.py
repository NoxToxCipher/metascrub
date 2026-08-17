# Render the store listing icon from the launcher icon.
#
#   python android/make-store-icon.py
#   -> android/listing/icon-512.png
#
# A store listing wants a 512x512 PNG, which is the one piece of app art that
# cannot be a vector. Drawing it by hand would mean two copies of the mark that
# nobody notices have drifted apart until the icon in the store stops matching
# the icon on the phone, so it is generated from the same file the launcher uses
# (res/drawable/ic_launcher_foreground.xml) on the same background colour the
# adaptive icon uses (ic_launcher_bg).
#
# Full bleed rather than masked: a launcher applies its own shape, and so does a
# store, and an icon that arrives pre-rounded gets rounded twice.
#
# Needs cairosvg (`pip install cairosvg`). It is not a build dependency: the icon
# changes when the mark changes, which is roughly never, and the result is
# committed.
import pathlib
import xml.etree.ElementTree as ET

import cairosvg

A = "{http://schemas.android.com/apk/res/android}"
HERE = pathlib.Path(__file__).resolve().parent
VECTOR = HERE / "app/src/main/res/drawable/ic_launcher_foreground.xml"
COLORS = HERE / "app/src/main/res/values/colors.xml"
OUT = HERE / "listing/icon-512.png"
SIZE = 512


def colour(name):
    """One colour out of colors.xml, by name."""
    for c in ET.parse(COLORS).getroot().iter("color"):
        if c.get("name") == name:
            return c.text.strip()
    raise SystemExit(f"no colour named {name} in {COLORS}")


def argb_to_css(value):
    """Android writes #AARRGGBB; SVG wants #RRGGBB (every colour here is opaque)."""
    v = value.strip()
    return "#" + v[3:] if len(v) == 9 else v


def group_transform(group):
    """The VectorDrawable group matrix, written as an SVG transform.

    Android applies translate, then rotate and scale about the pivot, so the
    equivalent is translate(t) translate(pivot) scale rotate translate(-pivot).
    """
    def num(attr, default):
        return float(group.get(A + attr, default))

    px, py = num("pivotX", 0), num("pivotY", 0)
    parts = [f"translate({num('translateX', 0)},{num('translateY', 0)})",
             f"translate({px},{py})",
             f"rotate({num('rotation', 0)})",
             f"scale({num('scaleX', 1)},{num('scaleY', 1)})",
             f"translate({-px},{-py})"]
    return " ".join(parts)


def path_element(path):
    """One <path> of the drawable, as SVG. Fill and stroke are the only two
    presentations the mark uses, and a path is one or the other."""
    d = path.get(A + "pathData")
    fill = path.get(A + "fillColor")
    stroke = path.get(A + "strokeColor")
    attrs = [f'd="{d}"']
    attrs.append(f'fill="{argb_to_css(fill)}"' if fill else 'fill="none"')
    if stroke:
        attrs.append(f'stroke="{argb_to_css(stroke)}"')
        attrs.append(f'stroke-width="{path.get(A + "strokeWidth", "1")}"')
        attrs.append(f'stroke-linecap="{path.get(A + "strokeLineCap", "butt")}"')
    return "  <path " + " ".join(attrs) + " />"


def main():
    vector = ET.parse(VECTOR).getroot()
    w = vector.get(A + "viewportWidth")
    h = vector.get(A + "viewportHeight")
    background = argb_to_css(colour("ic_launcher_bg"))

    body = []
    for group in vector.iter("group"):
        body.append(f'  <g transform="{group_transform(group)}">')
        body.extend("  " + path_element(p) for p in group.iter("path"))
        body.append("  </g>")
    # A path outside any group is still a path; none today, but the drawable is
    # allowed to have them and silently dropping art would be the wrong failure.
    body.extend(path_element(p) for p in vector.findall("path"))

    svg = (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}">\n'
           f'  <rect width="{w}" height="{h}" fill="{background}"/>\n'
           + "\n".join(body) + "\n</svg>\n")

    OUT.parent.mkdir(parents=True, exist_ok=True)
    cairosvg.svg2png(bytestring=svg.encode(), write_to=str(OUT),
                     output_width=SIZE, output_height=SIZE)
    print(f"wrote {OUT} ({SIZE}x{SIZE}, background {background})")


if __name__ == "__main__":
    main()
