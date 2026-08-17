#!/usr/bin/env python3
"""Render the metascrub mark to the PNG sizes a Linux icon theme wants.

There is no image library in this repository and there is not going to be one:
the application draws its own mark from circles and a triangle precisely so it
does not have to carry an SVG renderer. The icons a desktop needs are a
build-time artefact, not a runtime one, so they are generated here once and
committed, and this script exists so the next person can regenerate them from
the same shapes rather than opening a drawing program and guessing.

The shapes are the ones in brand/metascrub.svg, kept in the same coordinate
space so the two cannot drift apart silently.

    python3 packaging/linux/make-icons.py

Writes packaging/linux/icons/hicolor/<size>x<size>/apps/org.crake.metascrub.png.
The files are named after the application ID rather than the binary because
that is what AppStream, Flatpak and the icon theme spec all expect, and it is
the same ID the Android package already uses.

No third-party modules: zlib and struct are enough to write a PNG.
"""

import math
import os
import struct
import sys
import zlib

# The mark, in the coordinate space of brand/metascrub.svg (viewBox 14 15 74 78).
TEAL = (0x5F, 0xB0, 0xBA)
VIEWBOX = (14.0, 15.0, 74.0, 78.0)
SIZES = (16, 24, 32, 48, 64, 128, 256, 512)
SUPERSAMPLE = 4  # 4x4 samples per pixel, which is enough at 16px and cheap at 512


def bezier(p0, p1, p2, p3, steps=24):
    """Flatten one cubic segment into points."""
    out = []
    for i in range(steps + 1):
        t = i / steps
        u = 1.0 - t
        x = u * u * u * p0[0] + 3 * u * u * t * p1[0] + 3 * u * t * t * p2[0] + t * t * t * p3[0]
        y = u * u * u * p0[1] + 3 * u * u * t * p1[1] + 3 * u * t * t * p2[1] + t * t * t * p3[1]
        out.append((x, y))
    return out


# The beak is the one curved shape, so it is flattened to a polygon up front.
BEAK = (
    bezier((37, 30), (29, 31), (23, 34), (19, 37))
    + bezier((19, 37), (21, 38), (29, 35), (38, 34))[1:]
)


def in_circle(x, y, cx, cy, r):
    return (x - cx) ** 2 + (y - cy) ** 2 <= r * r


def in_ellipse(x, y, cx, cy, rx, ry):
    return ((x - cx) / rx) ** 2 + ((y - cy) / ry) ** 2 <= 1.0


def in_polygon(x, y, pts):
    """Even-odd fill. The two filled paths here are both simple."""
    inside = False
    n = len(pts)
    for i in range(n):
        x0, y0 = pts[i]
        x1, y1 = pts[(i + 1) % n]
        if (y0 > y) != (y1 > y):
            xint = x0 + (y - y0) * (x1 - x0) / (y1 - y0)
            if x < xint:
                inside = not inside
    return inside


def in_capsule(x, y, x0, y0, x1, y1, half):
    """A stroked line with round caps is a capsule, which is what the legs are."""
    dx, dy = x1 - x0, y1 - y0
    length2 = dx * dx + dy * dy
    if length2 == 0:
        return in_circle(x, y, x0, y0, half)
    t = max(0.0, min(1.0, ((x - x0) * dx + (y - y0) * dy) / length2))
    px, py = x0 + t * dx, y0 + t * dy
    return (x - px) ** 2 + (y - py) ** 2 <= half * half


def covered(x, y):
    """True where the mark is drawn. Mirrors the mask in brand/metascrub.svg."""
    if in_circle(x, y, 50, 30, 3.6):  # the eye is cut back out
        return False
    return (
        in_circle(x, y, 54, 54, 19)  # body
        or in_polygon(x, y, [(70, 49), (80, 46), (72, 58)])  # tail
        or in_circle(x, y, 47, 33, 13)  # head
        or in_polygon(x, y, BEAK)
        or in_capsule(x, y, 49, 70, 49, 80, 1.3)  # legs, stroke-width 2.6
        or in_capsule(x, y, 58, 70, 58, 80, 1.3)
        or in_ellipse(x, y, 54, 83, 22, 3.2)  # ground
    )


def render(size):
    """One RGBA buffer, supersampled for the anti-aliasing a small icon needs."""
    vx, vy, vw, vh = VIEWBOX
    # Fit the viewBox into a square without distorting it, and leave a little
    # margin so the mark is not flush against the edge of a launcher tile.
    margin = 0.06
    scale = (size * (1 - 2 * margin)) / max(vw, vh)
    ox = (size - vw * scale) / 2
    oy = (size - vh * scale) / 2

    rows = []
    step = 1.0 / SUPERSAMPLE
    weight = 1.0 / (SUPERSAMPLE * SUPERSAMPLE)
    for py in range(size):
        row = bytearray()
        for px in range(size):
            hits = 0
            for sy in range(SUPERSAMPLE):
                yy = (py + (sy + 0.5) * step - oy) / scale + vy
                for sx in range(SUPERSAMPLE):
                    xx = (px + (sx + 0.5) * step - ox) / scale + vx
                    if covered(xx, yy):
                        hits += 1
            alpha = int(round(hits * weight * 255))
            # Premultiplication is not wanted here: PNG alpha is straight, and
            # leaving the colour constant keeps the edge from going grey.
            row += bytes((TEAL[0], TEAL[1], TEAL[2], alpha))
        rows.append(bytes(row))
    return rows


def write_png(path, size, rows):
    raw = b"".join(b"\x00" + r for r in rows)  # filter type 0 on every row

    def chunk(tag, data):
        body = tag + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body))

    png = b"\x89PNG\r\n\x1a\n"
    png += chunk(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0))
    # Fixed compression settings, so regenerating produces identical bytes.
    png += chunk(b"IDAT", zlib.compress(raw, 9))
    png += chunk(b"IEND", b"")
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "wb") as f:
        f.write(png)


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    for size in SIZES:
        path = os.path.join(
            here, "icons", "hicolor", f"{size}x{size}", "apps", "org.crake.metascrub.png"
        )
        write_png(path, size, render(size))
        print(f"  {size}x{size}  {os.path.relpath(path, here)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
