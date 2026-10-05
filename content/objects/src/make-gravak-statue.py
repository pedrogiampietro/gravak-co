# Generates content/objects/gravak-statue.png: the relic of gravak-relic.png
# standing upright on its spiked grip, a little taller than a character, with a soft
# contact shadow and a faint glow, for the centre of Twin City's plaza.
# The grip's tip is 12 px above the bottom of the image (the object's
# `offset` in content/maps/1002.toml puts it on the dais).
#
#   pip install pillow numpy
#   python make-gravak-statue.py gravak-relic.png ../gravak-statue.png
import sys
import numpy as np
from PIL import Image, ImageFilter

relic_src, out = sys.argv[1], sys.argv[2]
HEIGHT = 140          # the relic, grip to top: a little over a character
SQUASH = 0.68         # the icon draws the shield foreshortened, lying down;
                      # standing, it is rounder than the drawing
MARGIN = 12           # below the grip's tip, for the shadow

# 1. The relic without the screenshot's background (white, the two greys of a
#    "transparent" checkerboard and its frame line), flood-filled from the
#    border so the same colours inside the drawing stay.
r = np.array(Image.open(relic_src).convert("RGBA")).astype(np.int32)
h, w = r.shape[:2]
close = np.zeros((h, w), bool)
for key in ((248, 249, 250), (230, 231, 232), (162, 169, 177)):
    close |= np.abs(r[..., :3] - np.array(key)).max(-1) <= 6
background = np.zeros((h, w), bool)
stack = [(x, y) for x in range(w) for y in (0, 1, h - 2, h - 1)]
stack += [(x, y) for y in range(h) for x in (0, 1, w - 2, w - 1)]
while stack:
    x, y = stack.pop()
    if 0 <= x < w and 0 <= y < h and not background[y, x] and close[y, x]:
        background[y, x] = True
        stack += [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)]
r[background, 3] = 0
# the light fringe the background left around the outline
alpha = Image.fromarray(r[..., 3].astype(np.uint8)).filter(ImageFilter.MinFilter(3))
r[..., 3] = np.array(alpha)
relic = Image.fromarray(r.astype(np.uint8), "RGBA")
relic = relic.crop(relic.getbbox())

# 2. Scale2x twice (EPX): pixel art grows with clean diagonals instead of
#    blocks or blur, before the final smooth resize.
def scale2x(a):
    p = np.pad(a, ((1, 1), (1, 1), (0, 0)), mode="edge")
    c = p[1:-1, 1:-1]
    up, down, left, right = p[:-2, 1:-1], p[2:, 1:-1], p[1:-1, :-2], p[1:-1, 2:]
    eq = lambda a, b: (a == b).all(-1)
    e0 = np.where((eq(left, up) & ~eq(left, down) & ~eq(up, right))[..., None], up, c)
    e1 = np.where((eq(up, right) & ~eq(up, left) & ~eq(right, down))[..., None], right, c)
    e2 = np.where((eq(down, left) & ~eq(down, right) & ~eq(left, up))[..., None], left, c)
    e3 = np.where((eq(right, down) & ~eq(right, up) & ~eq(down, left))[..., None], down, c)
    out = np.zeros((a.shape[0] * 2, a.shape[1] * 2, a.shape[2]), a.dtype)
    out[0::2, 0::2], out[0::2, 1::2], out[1::2, 0::2], out[1::2, 1::2] = e0, e1, e2, e3
    return out
big = np.array(relic)
big = scale2x(scale2x(big))
relic = Image.fromarray(big, "RGBA")

# 3. Upright: the steel disc in the middle (its blue-grey pixels, not the
#    grip nor the curled end, which pull the axis aside) gives the long axis;
#    it turns vertical with the grip down, then the height is squashed.
pixels = np.array(relic).astype(np.int32)
red, green, blue, alpha_ = (pixels[..., i] for i in range(4))
disc = (alpha_ > 128) & (blue > red + 12) & (blue > 90)
ys, xs = np.nonzero(disc)
pts = np.stack([xs - xs.mean(), ys - ys.mean()], 1)
eigval, eigvec = np.linalg.eigh(np.cov(pts.T))
axis = eigvec[:, np.argmax(eigval)]            # (dx, dy), y down
if axis[0] < 0:                                # point it at the grip (right)
    axis = -axis
angle = np.degrees(np.arctan2(axis[1], axis[0]))   # below the horizontal
# PIL turns counter-clockwise; the grip end goes from `angle` to straight down
relic = relic.rotate(angle - 90, resample=Image.BICUBIC, expand=True)
relic = relic.crop(relic.getbbox())
width = round(relic.width * HEIGHT / (relic.height * SQUASH))
relic = relic.resize((width, HEIGHT), Image.LANCZOS)
relic = relic.filter(ImageFilter.UnsharpMask(0.8, 40, 2))

# 4. Canvas: the grip's tip on the bottom centre, a contact shadow under it
#    (to the right, as Twin City's shadows fall) and a faint warm glow.
W = max(relic.width, 100) + 60
H = relic.height + MARGIN + 30
canvas = Image.new("RGBA", (W, H))
ground = (W // 2, H - MARGIN)
mask = np.array(relic)[..., 3] > 128
tip_xs = np.nonzero(mask[-1])[0]
tip_x = int(tip_xs.mean()) if len(tip_xs) else relic.width // 2
pos = (ground[0] - tip_x, ground[1] - relic.height)

yy, xx = np.mgrid[0:H, 0:W].astype(np.float32)
shadow = np.exp(-(((xx - ground[0] - 8) / 22) ** 2 + ((yy - ground[1] - 2) / 7) ** 2))
glow = np.exp(-(((xx - ground[0]) / (relic.width * 0.6)) ** 2 + ((yy - (ground[1] - relic.height * 0.55)) / (relic.height * 0.5)) ** 2))
layer = np.zeros((H, W, 4), np.float32)
layer[..., :3] = np.array([255, 205, 110])
layer[..., 3] = glow * 45
canvas.alpha_composite(Image.fromarray(layer.astype(np.uint8), "RGBA"))
layer = np.zeros((H, W, 4), np.float32)
layer[..., :3] = np.array([30, 24, 20])
layer[..., 3] = shadow * 110
canvas.alpha_composite(Image.fromarray(layer.astype(np.uint8), "RGBA"))
canvas.alpha_composite(relic, pos)
# Trim, keeping the grip's tip on the bottom centre.
left, top, right, _ = canvas.getbbox()
left = min(left, 2 * ground[0] - right)
canvas = canvas.crop((left, top, 2 * ground[0] - left, H))
canvas.save(out)
print(canvas.size, f"tilt {angle:.1f} deg, grip tip {MARGIN} px above the bottom")
