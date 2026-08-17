#!/usr/bin/env python3
# Generate the metascrub launcher icons for Sailfish at the harbour sizes.
#
#   python sailfish/make-icons.py
#
# Draws the sandpiper (from icons/harbour-metascrub.svg) directly with Pillow, on
# a rounded dark-teal gradient, supersampled for smooth edges. Pillow is the only
# dependency; no SVG rasteriser is needed. Output:
#   harbour-metascrub/icons/<size>x<size>/harbour-metascrub.png  for 86/108/128/172
from PIL import Image, ImageDraw
import os

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "harbour-metascrub", "icons")
SS = 4                       # supersample factor for anti-aliasing
TEAL = (0x5f, 0xb0, 0xba, 255)
EYE = (0x0e, 0x1f, 0x1e, 255)

def draw_icon(size):
    S = size * SS
    img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    # vertical dark-teal gradient
    grad = Image.new("RGB", (1, S))
    top, bot = (0x17, 0x32, 0x2f), (0x0b, 0x15, 0x17)
    for y in range(S):
        t = y / S
        grad.putpixel((0, y), tuple(int(top[i] + (bot[i] - top[i]) * t) for i in range(3)))
    grad = grad.resize((S, S))
    mask = Image.new("L", (S, S), 0)
    ImageDraw.Draw(mask).rounded_rectangle([0, 0, S - 1, S - 1], radius=int(0.23 * S), fill=255)
    img.paste(grad, (0, 0), mask)

    d = ImageDraw.Draw(img)
    k = 0.60 * S / 78.0          # bird bbox is 78 tall; occupy ~60% of the icon
    cx, cy = S / 2.0, S / 2.0
    def T(px, py): return (cx + (px - 51) * k, cy + (py - 54) * k)  # 51,54 = bird centre
    def circle(px, py, rr, fill):
        d.ellipse([*T(px - rr, py - rr), *T(px + rr, py + rr)], fill=fill)

    d.polygon([T(37, 30), T(29, 31), T(23, 34), T(19, 37), T(29, 35), T(38, 34)], fill=TEAL)  # wing wisp
    circle(54, 54, 19, TEAL)     # body
    circle(47, 33, 13, TEAL)     # head
    d.polygon([T(70, 49), T(80, 46), T(72, 58)], fill=TEAL)          # beak
    d.ellipse([*T(54 - 22, 83 - 3.2), *T(54 + 22, 83 + 3.2)], fill=TEAL)  # base
    lw = max(1, int(2.6 * k))
    d.line([T(49, 70), T(49, 80)], fill=TEAL, width=lw)              # leg
    d.line([T(58, 70), T(58, 80)], fill=TEAL, width=lw)              # leg
    circle(50, 30, 3.6, EYE)     # eye

    return img.resize((size, size), Image.LANCZOS)

for s in (86, 108, 128, 172):
    sub = os.path.join(OUT, f"{s}x{s}")
    os.makedirs(sub, exist_ok=True)
    p = os.path.join(sub, "harbour-metascrub.png")
    draw_icon(s).save(p)
    print(f"wrote {p} ({os.path.getsize(p)} bytes)")
