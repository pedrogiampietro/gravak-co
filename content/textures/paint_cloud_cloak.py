"""Paints the Cloud Cloak's textures (garment 182995) over the WeddingGown's
(181335): the cloth turned black with its folds kept, the skin left as it is,
and red clouds with a white edge strewn over the cloth.

    python content/textures/paint_cloud_cloak.py <folder with 00N137350.png>

The originals come out of the installation with the client's texture dump
(`TEXTURE_DUMP=dir TEXTURE_PATHS=c3/texture/001137350.dds,...
cargo test -p conquer-sandbox texture_dump`). Writes
content/textures/182995-<body>.png for bodies 1-4.
"""
import colorsys
import math
import random
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter

HERE = Path(__file__).resolve().parent
SOURCE = Path(sys.argv[1])
CLOTH = (22, 20, 28)
CLOUD = (196, 24, 34)
EDGE = (245, 240, 235)


def is_skin(r, g, b):
    h, s, v = colorsys.rgb_to_hsv(r / 255, g / 255, b / 255)
    return 0.02 <= h <= 0.12 and 0.12 <= s <= 0.62 and v >= 0.42


def cloth_mask(image):
    """Where the texture is cloth: all but the skin (faces, hands)."""
    skin = Image.new("L", image.size, 0)
    px, sp = image.load(), skin.load()
    for y in range(image.height):
        for x in range(image.width):
            r, g, b, a = px[x, y]
            if a > 0 and is_skin(r, g, b):
                sp[x, y] = 255
    # Skin-coloured texels of the old embroidery are no skin: only patches
    # wider than a few texels are.
    skin = skin.filter(ImageFilter.MinFilter(5)).filter(ImageFilter.MaxFilter(7))
    return skin.point(lambda value: 255 - value)


def blacken(image, mask):
    """The cloth black, its folds kept from the old cloth's light."""
    out = image.copy()
    px, mp, op = image.load(), mask.load(), out.load()
    # The old cloth's light, smoothed so its embroidery does not show.
    light = image.convert("L").filter(ImageFilter.GaussianBlur(3)).load()
    for y in range(image.height):
        for x in range(image.width):
            if mp[x, y] < 128:
                continue
            shade = 0.55 + 0.9 * (light[x, y] / 255)
            op[x, y] = (*(min(255, int(c * shade)) for c in CLOTH), px[x, y][3])
    return out


def cloud(draw, cx, cy, size, rng):
    """A cloud of three to four lobes over a flat base, edged white, with a
    curl inside."""
    lobes = [(-0.55, 0.05, 0.42), (0.0, -0.22, 0.55), (0.55, 0.0, 0.45)]
    if rng.random() < 0.5:
        lobes.append((0.95, 0.18, 0.3))
    base = (cy + size * 0.32)
    for outline, colour, grow in ((True, EDGE, size * 0.09), (False, CLOUD, 0.0)):
        for dx, dy, r in lobes:
            rr = r * size + grow
            x, y = cx + dx * size, cy + dy * size
            draw.ellipse([x - rr, y - rr, min(x + rr, x + rr), min(y + rr, base + grow)], fill=colour)
        left = cx + (lobes[0][0] - lobes[0][2]) * size - grow
        right = cx + (lobes[-1][0] + lobes[-1][2]) * size + grow
        draw.rectangle([left, cy, right, base + grow], fill=colour)
    # The curl: an arc inside the middle lobe.
    r = size * 0.28
    draw.arc([cx - r, cy - size * 0.22 - r, cx + r, cy - size * 0.22 + r], 200, 470, fill=EDGE, width=max(1, int(size * 0.07)))


def strew_clouds(image, mask, rng, size):
    """Clouds where they sit whole on the cloth, apart from each other."""
    layer = Image.new("RGBA", image.size, (0, 0, 0, 0))
    draw = ImageDraw.Draw(layer)
    mp = mask.load()
    placed = []
    span = size * 1.25
    for _ in range(4000):
        # Not on the head-dress at the texture's top.
        cx, cy = rng.uniform(span, image.width - span), rng.uniform(max(span, 48), image.height - span)
        if any(math.hypot(cx - x, cy - y) < size * 3.5 for x, y in placed):
            continue
        box = [(cx + dx * span, cy + dy * span * 0.7) for dx in (-1, 0, 1) for dy in (-1, 0, 1)]
        if all(mp[int(x), int(y)] > 128 for x, y in box):
            cloud(draw, cx, cy, size, rng)
            placed.append((cx, cy))
    # Only on cloth.
    layer.putalpha(Image.composite(layer.getchannel("A"), Image.new("L", image.size, 0), mask))
    out = image.copy()
    out.alpha_composite(layer)
    return out, len(placed)


for body in (1, 2, 3, 4):
    original = Image.open(SOURCE / f"00{body}137350.png").convert("RGBA")
    mask = cloth_mask(original)
    painted = blacken(original, mask)
    painted, count = strew_clouds(painted, mask, random.Random(182995 + body), size=13)
    painted.save(HERE / f"182995-{body}.png")
    print(f"body {body}: {count} clouds")
