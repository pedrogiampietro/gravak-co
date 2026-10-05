# Generates content/objects/gravak-statue.png: an isometric stone pedestal in
# the style of Twin City's walls (light from the upper left, shadows to the
# right) with the relic of gravak-relic.png on top. The base covers 3x3 cells
# and its centre is 50 px above the bottom of the image.
#
#   pip install pillow numpy
#   python make-gravak-statue.py gravak-relic.png ../gravak-statue.png
import sys, numpy as np
from PIL import Image, ImageFilter, ImageDraw

relic_src, out = sys.argv[1], sys.argv[2]
rng = np.random.default_rng(7)
W, H = 560, 420
cx, cy = W // 2, 360            # the ground point under the statue's centre
img = np.zeros((H, W, 4), np.float32)

def put(mask, rgb, alpha=1.0):
    a = mask[..., None] * alpha
    img[..., :3] = img[..., :3] * (1 - a) + rgb * a
    img[..., 3] = np.maximum(img[..., 3], mask * alpha * 255)

ys, xs = np.mgrid[0:H, 0:W].astype(np.float32)
X, Y = xs - cx, ys - cy
noise = rng.normal(0, 1, (H, W)).astype(np.float32)
noise = np.array(Image.fromarray(((noise * 40) + 128).clip(0, 255).astype(np.uint8)).filter(ImageFilter.GaussianBlur(0.8)), np.float32) / 128 - 1
grain = rng.normal(0, 1, (H, W)).astype(np.float32)

def bricks(t, h, course, length, offset):
    row = np.floor(h / course)
    shift = (row % 2) * length * 0.5 + offset
    along = (t + shift) % length
    across = h % course
    mortar = (across < 1.3) | (along < 1.3)
    # each brick a slightly different tone
    tone = np.sin(row * 12.9898 + np.floor((t + shift) / length) * 78.233) * 43758.5453
    tone = (tone - np.floor(tone)) - 0.5
    return mortar, tone

def box(a, h0, h1, light, dark, top, brick=None, dirt=0.0):
    # left face: v = +a, u in [-a, a]
    u = X + a
    h = (u + a) / 2 - Y
    left = (u >= -a) & (u <= a) & (h >= h0) & (h <= h1)
    # right face: u = +a, v in [-a, a]
    v = a - X
    hr = (a + v) / 2 - Y
    right = (v >= -a) & (v <= a) & (hr >= h0) & (hr <= h1)
    # top: |X|/2 + |Y + h1| <= a ... diamond of half-width 2a, half-height a
    topm = (np.abs(X) / 2 + np.abs(Y + h1) <= a)
    for mask, base, t, hh, side in ((left, light, u, h, 0), (right, dark, v, hr, 1)):
        col = np.broadcast_to(np.array(base, np.float32), (H, W, 3)).copy()
        # darker towards the ground, lighter at the top edge
        k = ((hh - h0) / max(h1 - h0, 1)).clip(0, 1)[..., None]
        col *= (0.78 + 0.22 * k)
        if brick:
            mortar, tone = bricks(t, hh, *brick)
            col *= (1 + 0.10 * tone[..., None])
            col[mortar] *= 0.80
        col *= (1 + 0.06 * noise[..., None] + 0.03 * grain[..., None])
        if dirt:
            g = np.exp(-(hh - h0) / 6.0)[..., None] * dirt
            col = col * (1 - g) + np.array([72, 78, 40], np.float32) * g
        # bright top edge
        edge = (np.abs(hh - h1) < 1.2)[..., None]
        col = np.where(edge, col * 1.18, col)
        put(mask.astype(np.float32), col.clip(0, 255))
    col = np.broadcast_to(np.array(top, np.float32), (H, W, 3)).copy()
    col *= (1 + 0.05 * noise[..., None] + 0.03 * grain[..., None])
    # bevel: the two front edges of the top catch the light
    d = a - (np.abs(X) / 2 + np.abs(Y + h1))
    col = np.where((d < 1.5)[..., None] & ((Y + h1) > 0)[..., None], col * 1.12, col)
    put(topm.astype(np.float32), col.clip(0, 255))

stone_l, stone_r, stone_t = (204, 205, 196), (118, 122, 118), (222, 222, 212)
box(48, 0, 16, stone_l, stone_r, stone_t, brick=(8, 24, 3), dirt=0.55)
box(40, 16, 28, stone_l, stone_r, stone_t, brick=(12, 30, 9))
box(27, 28, 118, stone_l, stone_r, stone_t, brick=(11, 18, 5))
box(33, 118, 128, stone_l, stone_r, stone_t)

# A bronze plaque on the lit face of the column.
plaque = Image.new("L", (W, H), 0)
d = ImageDraw.Draw(plaque)
pts = []
for (u, h) in ((-15, 96), (15, 96), (15, 66), (-15, 66)):
    pts.append((cx + u - 27, cy + (u + 27) / 2 - h))
d.polygon(pts, fill=255)
pm = np.array(plaque, np.float32) / 255
grad = ((cy - 58 - ys) / 40).clip(0, 1)[..., None]
pcol = (np.array([120, 84, 34], np.float32) * (1 - grad) + np.array([176, 130, 58], np.float32) * grad) * (1 + 0.10 * noise[..., None])
put(pm, pcol)
inner = Image.new("L", (W, H), 0)
ImageDraw.Draw(inner).polygon([(x + (2 if i in (0, 3) else -2), y + (2 if i in (2, 3) else -2) + (1 if i in (0, 3) else -1)) for i, (x, y) in enumerate(pts)], outline=255)
put(np.array(inner, np.float32) / 255 * 0.8, np.array([214, 170, 84], np.float32))

statue = Image.fromarray(img.clip(0, 255).astype(np.uint8), "RGBA")

# The relic, background taken out, standing on the cap.
relic = Image.open(relic_src).convert("RGBA")
r = np.array(relic).astype(np.int32)
keys = [(248, 249, 250), (230, 231, 232), (162, 169, 177)]
h_, w_ = r.shape[:2]
bg = np.zeros((h_, w_), bool)
close = np.zeros((h_, w_), bool)
for k in keys:
    close |= (np.abs(r[..., :3] - np.array(k)).max(-1) <= 6)
stack = [(x, y) for x in range(w_) for y in (0, 1, h_ - 2, h_ - 1)] + [(x, y) for y in range(h_) for x in (0, 1, w_ - 2, w_ - 1)]
while stack:
    x, y = stack.pop()
    if x < 0 or y < 0 or x >= w_ or y >= h_ or bg[y, x] or not close[y, x]:
        continue
    bg[y, x] = True
    stack += [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)]
r[bg, 3] = 0
# soften the fringe the white background left
alpha = Image.fromarray(r[..., 3].astype(np.uint8)).filter(ImageFilter.MinFilter(3)).filter(ImageFilter.GaussianBlur(0.6))
relic = Image.fromarray(r.astype(np.uint8), "RGBA")
relic.putalpha(alpha)
relic = relic.crop(relic.getbbox())
scale = 136 / relic.width
relic = relic.resize((round(relic.width * scale), round(relic.height * scale)), Image.LANCZOS)
relic = relic.filter(ImageFilter.UnsharpMask(1.2, 60, 2))

# A warm glow behind it, as Conquer's relics and altars have.
gx, gy = cx, cy - 128 - 62
glow = np.exp(-(((xs - gx) / 60) ** 2 + ((ys - gy) / 48) ** 2))
glow_img = np.zeros((H, W, 4), np.float32)
glow_img[..., :3] = np.array([255, 196, 90])
glow_img[..., 3] = glow * 55
glow_img = Image.fromarray(glow_img.astype(np.uint8), "RGBA")

top_y = cy - 128
pos = (cx - relic.width // 2, top_y - relic.height + 30)
figure = Image.new("RGBA", (W, H))
figure.alpha_composite(glow_img)
figure.alpha_composite(statue)
figure.alpha_composite(relic, pos)

# Shadow to the lower right: the silhouette laid on the ground.
sil = np.array(figure.split()[3], np.float32)
sil = (sil > 40).astype(np.float32) * 255
sil_img = Image.fromarray(sil.astype(np.uint8))
# A point h above the ground point (cx, cy) lands at (x + 0.6h, cy - 0.2h).
sh = sil_img.transform((W, H), Image.AFFINE, (1, 3.0, -3.0 * cy, 0, 5.0, -4.0 * cy), Image.BILINEAR)
sh = np.array(sh.filter(ImageFilter.GaussianBlur(2.5)), np.float32) * 0.5
shadow = np.zeros((H, W, 4), np.uint8)
shadow[..., 3] = sh.clip(0, 255).astype(np.uint8)
shadow[..., :3] = (20, 28, 10)

final = Image.new("RGBA", (W, H))
final.alpha_composite(Image.fromarray(shadow, "RGBA"))
final.alpha_composite(figure)
bbox = final.getbbox()
# keep the ground point at the bottom centre: symmetric crop around cx
left = min(bbox[0], 2 * cx - bbox[2])
final = final.crop((left, bbox[1], 2 * cx - left, cy + 50))
final.save(out)
print(final.size, "base centre 50 px above the bottom")
