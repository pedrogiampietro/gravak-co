# Generates content/objects/gravak-statue.png: the relic of gravak-relic.png
# as it was drawn, pixel for pixel (no rotation, no resampling, so it stays
# as sharp as the original art, the way Tibia shows it), leaning on the
# ground with a soft shadow under it.
# The bottom of the drawing is 12 px above the bottom of the image (the
# object's `offset` in content/maps/1002.toml puts it on the dais).
#
#   pip install pillow numpy
#   python make-gravak-statue.py gravak-relic.png ../gravak-statue.png [scale]
#
# `scale` (default 1) enlarges by whole pixels only (nearest neighbour), so
# a bigger statue stays sharp too.
import sys
import numpy as np
from PIL import Image

relic_src, out = sys.argv[1], sys.argv[2]
SCALE = int(sys.argv[3]) if len(sys.argv) > 3 else 1
MARGIN = 12           # below the drawing, for the shadow

# 1. The relic without the screenshot's background (white, the two greys of a
#    "transparent" checkerboard and its frame line), flood-filled from the
#    border so the same colours inside the drawing stay.
r = np.array(Image.open(relic_src).convert("RGBA")).astype(np.int32)
h, w = r.shape[:2]
keys = np.array([(248, 249, 250), (230, 231, 232), (162, 169, 177)])
distance = np.abs(r[..., None, :3] - keys[None, None]).max(-1).min(-1)
background = np.zeros((h, w), bool)
stack = [(x, y) for x in range(w) for y in (0, 1, h - 2, h - 1)]
stack += [(x, y) for y in range(h) for x in (0, 1, w - 2, w - 1)]
while stack:
    x, y = stack.pop()
    if 0 <= x < w and 0 <= y < h and not background[y, x] and distance[y, x] <= 6:
        background[y, x] = True
        stack += [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)]
# The light fringe the background left: pixels touching it that are still
# close to its colours. The dark outline of the drawing stays.
padded = np.pad(background, 1)
touching = padded[:-2, 1:-1] | padded[2:, 1:-1] | padded[1:-1, :-2] | padded[1:-1, 2:]
fringe = touching & ~background & (distance <= 48)
r[background | fringe, 3] = 0
relic = Image.fromarray(r.astype(np.uint8), "RGBA")
relic = relic.crop(relic.getbbox())
if SCALE > 1:
    relic = relic.resize((relic.width * SCALE, relic.height * SCALE), Image.NEAREST)

# 2. Canvas: the drawing's bottom centre on the ground point, a shadow under
#    it (a little to the right, as Twin City's shadows fall).
W = relic.width + 24
H = relic.height + MARGIN
canvas = Image.new("RGBA", (W, H))
ground = (W // 2, H - MARGIN)
yy, xx = np.mgrid[0:H, 0:W].astype(np.float32)
shadow = np.exp(-(((xx - ground[0] - 6) / (relic.width * 0.42)) ** 2 + ((yy - ground[1] + 2) / (6 * SCALE)) ** 2))
layer = np.zeros((H, W, 4), np.float32)
layer[..., :3] = np.array([30, 24, 20])
layer[..., 3] = shadow * 100
canvas.alpha_composite(Image.fromarray(layer.astype(np.uint8), "RGBA"))
canvas.alpha_composite(relic, (ground[0] - relic.width // 2, ground[1] - relic.height))
canvas.save(out)
print(canvas.size, f"scale {SCALE}, drawing's bottom {MARGIN} px above the image's")
