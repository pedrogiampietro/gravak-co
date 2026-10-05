# Generates content/objects/gravak-statue.png: the relic of gravak-relic.png
# standing upright on its grip, at the size it was drawn and as sharp: it is
# turned with RotSprite (Scale2x three times, a nearest-neighbour turn, back
# down to 1x), which keeps pixel art crisp where an ordinary rotation blurs
# it, and nothing is stretched or resized. A soft shadow lies under it.
# The grip's tip is 12 px above the bottom of the image (the object's
# `offset` in content/maps/1002.toml puts it on the dais).
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

# 2. Upright. The long axis of the steel disc in the middle (its blue-grey
#    pixels: the grip and the curled end would pull a whole-drawing axis
#    aside) turns vertical, the grip (lower right in the icon) down.
pixels = np.array(relic).astype(np.int32)
disc = (pixels[..., 3] > 128) & (pixels[..., 2] > pixels[..., 0] + 12) & (pixels[..., 2] > 90)
ys, xs = np.nonzero(disc)
eigval, eigvec = np.linalg.eigh(np.cov(np.stack([xs - xs.mean(), ys - ys.mean()])))
axis = eigvec[:, np.argmax(eigval)]            # (dx, dy), y down
if axis[0] < 0:
    axis = -axis
angle = np.degrees(np.arctan2(axis[1], axis[0]))   # below the horizontal


def scale2x(a):
    p = np.pad(a, ((1, 1), (1, 1), (0, 0)), mode="edge")
    c = p[1:-1, 1:-1]
    up, down, left, right = p[:-2, 1:-1], p[2:, 1:-1], p[1:-1, :-2], p[1:-1, 2:]
    eq = lambda a, b: (a == b).all(-1)
    out = np.zeros((a.shape[0] * 2, a.shape[1] * 2, a.shape[2]), a.dtype)
    out[0::2, 0::2] = np.where((eq(left, up) & ~eq(left, down) & ~eq(up, right))[..., None], up, c)
    out[0::2, 1::2] = np.where((eq(up, right) & ~eq(up, left) & ~eq(right, down))[..., None], right, c)
    out[1::2, 0::2] = np.where((eq(down, left) & ~eq(down, right) & ~eq(left, up))[..., None], left, c)
    out[1::2, 1::2] = np.where((eq(right, down) & ~eq(right, up) & ~eq(down, left))[..., None], down, c)
    return out


# RotSprite: 8x with Scale2x, turn without blending, back to 1x by sampling
# the middle of every 8x8 block.
big = np.array(relic)
big[big[..., 3] == 0] = 0                      # one transparent colour
for _ in range(3):
    big = scale2x(big)
turned = np.array(Image.fromarray(big, "RGBA").rotate(angle - 90, resample=Image.NEAREST, expand=True))
small = turned[4::8, 4::8].copy()
small[small[..., 3] < 128, 3] = 0
small[small[..., 3] >= 128, 3] = 255
relic = Image.fromarray(small, "RGBA")
relic = relic.crop(relic.getbbox())
if SCALE > 1:
    relic = relic.resize((relic.width * SCALE, relic.height * SCALE), Image.NEAREST)

# 3. Canvas: the grip's tip on the ground point, a shadow under
#    it (a little to the right, as Twin City's shadows fall).
W = relic.width + 48
H = relic.height + MARGIN
canvas = Image.new("RGBA", (W, H))
ground = (W // 2, H - MARGIN)
yy, xx = np.mgrid[0:H, 0:W].astype(np.float32)
shadow = np.exp(-(((xx - ground[0] - 6) / (relic.width * 0.42)) ** 2 + ((yy - ground[1] + 2) / (6 * SCALE)) ** 2))
layer = np.zeros((H, W, 4), np.float32)
layer[..., :3] = np.array([30, 24, 20])
layer[..., 3] = shadow * 100
canvas.alpha_composite(Image.fromarray(layer.astype(np.uint8), "RGBA"))
bottom = np.nonzero(np.array(relic)[-1, :, 3])[0]
tip = int(bottom.mean()) if len(bottom) else relic.width // 2
canvas.alpha_composite(relic, (ground[0] - tip, ground[1] - relic.height))
canvas.save(out)
print(canvas.size, f"turned {angle - 90:.1f} deg, scale {SCALE}, grip tip {MARGIN} px above the bottom")
