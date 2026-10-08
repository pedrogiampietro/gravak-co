"""Makes the Seraph Bow (accessory 370001): its model, content/models/370001.obj,
the texture painted to it, content/textures/370001.png, and its icon, the bow
seen flat and turned on the diagonal as the game's bows are,
content/icons/370001.png.

    python content/models/make_seraph_bow.py

A long recurve bow of ice blue with silver edges, feather blades sweeping
out from the grip like wings, and a glowing gem at its heart. It is made in
the rest pose of the Phoenix Bow's model (500279), whose bones it takes
(content/models/500279-reference.obj, from
`gravak-content weapon-obj 500279 content/models/500279-reference.obj`):
the plane of its string and grip, its string's ends where the original's
are, so the string is pulled as the original's is.

Every part is a ribbon: a spine in the bow's plane (a along the string, b
from the string toward the grip), widened in the plane and thickened out of
it. The texture is that plane laid flat: the top half for the limbs, grip,
gem and string, the bottom half for the blades (which lie over the limbs).
"""
import math
from pathlib import Path

from PIL import Image, ImageDraw

HERE = Path(__file__).resolve().parent
REFERENCE = HERE / "500279-reference.obj"
SIZE = 512
A_RANGE = (-90.0, 90.0)
B_RANGE = (-20.0, 55.0)


# --- The original: its vertices, the bone of each, and the bow's plane. ---
def read_reference():
    places, bones, group = [], {}, None
    for line in REFERENCE.read_text().splitlines():
        words = line.split()
        if not words:
            continue
        if words[0] == "v":
            places.append(tuple(map(float, words[1:4])))
        elif words[0] == "g":
            group = int(words[1][4:])
        elif words[0] == "f":
            for corner in words[1:]:
                bones[int(corner.split("/")[0]) - 1] = group
    return [(p, bones.get(i, 0)) for i, p in enumerate(places)]


def sub(p, q):
    return tuple(a - b for a, b in zip(p, q))


def add(p, q):
    return tuple(a + b for a, b in zip(p, q))


def mul(p, k):
    return tuple(a * k for a in p)


def dot(p, q):
    return sum(a * b for a, b in zip(p, q))


def unit(p):
    length = math.sqrt(dot(p, p))
    return mul(p, 1 / length)


def cross(p, q):
    return (p[1] * q[2] - p[2] * q[1], p[2] * q[0] - p[0] * q[2], p[0] * q[1] - p[1] * q[0])


REFERENCE_VERTICES = read_reference()
STRING = [p for p, bone in REFERENCE_VERTICES if bone == 5]
TOP = max(STRING, key=lambda p: p[1])
BOTTOM = min(STRING, key=lambda p: p[1])
MIDDLE = mul(add(TOP, BOTTOM), 0.5)
GRIP = mul(
    tuple(map(sum, zip(*[p for p, bone in REFERENCE_VERTICES if bone == 0]))),
    1 / sum(1 for _, bone in REFERENCE_VERTICES if bone == 0),
)
U = unit(sub(TOP, BOTTOM))
toward_grip = sub(GRIP, MIDDLE)
V = unit(sub(toward_grip, mul(U, dot(toward_grip, U))))
N = unit(cross(U, V))
HALF = math.sqrt(dot(sub(TOP, BOTTOM), sub(TOP, BOTTOM))) / 2
GRIP_B = dot(sub(GRIP, MIDDLE), V)


def place(a, b, n):
    return add(add(add(MIDDLE, mul(U, a)), mul(V, b)), mul(N, n))


def nearest_bone(a, b):
    p = place(a, b, 0)
    return min(REFERENCE_VERTICES, key=lambda v: dot(sub(v[0], p), sub(v[0], p)))[1]


# --- The shapes, in the bow's plane. ---
# The limb's spine (a from the grip, b from the string), through which a
# Catmull-Rom curve runs: out from the grip, back to the string's nock, and
# past it a recurve, the tip curling forward.
LIMB = [
    (0.0, 0.0), (14.0, 2.9), (28.0, 2.4), (42.0, -4.6), (55.0, -13.6),
    (65.0, -21.1), (HALF, -GRIP_B), (HALF + 5.5, -GRIP_B - 1.6), (HALF + 10.0, -GRIP_B + 0.6), (HALF + 13.0, -GRIP_B + 4.6),
]
LIMB = [(a, GRIP_B + b) for a, b in LIMB]
LIMB_END = LIMB[-1][0]


def limb_b(a):
    a = min(abs(a), LIMB_END)
    points = [(-LIMB[1][0], LIMB[1][1])] + LIMB + [(2 * LIMB[-1][0] - LIMB[-2][0], 2 * LIMB[-1][1] - LIMB[-2][1])]
    for i in range(1, len(points) - 2):
        (a0, b0), (a1, b1), (a2, b2), (a3, b3) = points[i - 1 : i + 3]
        if a1 <= a <= a2:
            t = (a - a1) / (a2 - a1)
            return 0.5 * (2 * b1 + (b2 - b0) * t + (2 * b0 - 5 * b1 + 4 * b2 - b3) * t * t + (3 * b1 - b0 - 3 * b2 + b3) * t ** 3)
    return LIMB[-1][1]


def limb_width(a):
    t = abs(a) / HALF
    # Broad at the grip, slender toward the nock, a blade again in the
    # recurve, to a point at its end.
    width = 4.2 - 2.6 * min(t, 1.0) + 1.2 * max(0.0, t - 1.0) / (LIMB_END / HALF - 1.0)
    return width * min(1.0, (LIMB_END - abs(a)) / 4.0 + 0.15)


def ribbon_spine(points):
    """A spine's 2D normals, by its neighbours."""
    normals = []
    for i, (a, b) in enumerate(points):
        p0 = points[max(i - 1, 0)]
        p1 = points[min(i + 1, len(points) - 1)]
        da, db = p1[0] - p0[0], p1[1] - p0[1]
        length = math.hypot(da, db) or 1
        normals.append((-db / length, da / length))
    return normals


class Part:
    def __init__(self, name, spine, widths, thickness, region, bone=None, paint=None):
        self.name, self.spine, self.widths, self.thickness = name, spine, widths, thickness
        self.region, self.bone, self.paint = region, bone, paint
        self.normals = ribbon_spine(spine)

    def edge(self, i, side):
        (a, b), (na, nb), w = self.spine[i], self.normals[i], self.widths[i]
        return (a + na * w * side, b + nb * w * side)


def limb():
    count = 90
    spine = [(-LIMB_END + 2 * LIMB_END * i / count, 0) for i in range(count + 1)]
    spine = [(a, limb_b(a)) for a, _ in spine]
    widths = [limb_width(a) for a, _ in spine]
    thickness = [max(0.5, 1.5 - abs(a) / HALF) for a, _ in spine]
    return Part("limb", spine, widths, thickness, "main", paint="limb")


def grip():
    spine = [(a, GRIP_B + 1) for a in (-12, -6, 0, 6, 12)]
    return Part("grip", spine, [4.6] * 5, [3.0] * 5, "main", bone=0, paint="grip")


def string():
    spine = [(-HALF, 0), (0, 0), (HALF, 0)]
    return Part("string", spine, [0.35] * 3, [0.35] * 3, "main", paint="string")


def blade(anchor, angle, length, width, side, front=False):
    """A feather blade from the limb's edge, sweeping out to its tip."""
    a0 = anchor * side
    root_b = limb_b(a0) + (-limb_width(a0) * 0.6 if front else limb_width(a0) * 0.6)
    spine, widths = [], []
    count = 14
    for i in range(count + 1):
        t = i / count
        # The blade bends outward as it goes, like a wing's feathers.
        theta = math.radians(angle - 18 * t * t)
        da = math.cos(theta) * side
        db = math.sin(theta) * (-1 if front else 1)
        a = a0 + da * length * t
        b = root_b + db * length * t
        spine.append((a, b))
        widths.append(width * (0.55 + 0.45 * math.sin(math.pi * min(t * 1.25, 1))) * (1 - t) ** 0.7)
    bone = nearest_bone(*spine[len(spine) // 3])
    return Part("blade", spine, widths, [0.8] * len(spine), "feather", bone=bone, paint="blade")


PARTS = [limb(), grip(), string()]
for side in (1, -1):
    for anchor, angle, length, width in [(8, 70, 30, 6.5), (16, 56, 27, 5.8), (25, 43, 22, 5), (34, 32, 16, 3.8), (43, 24, 10, 2.6)]:
        PARTS.append(blade(anchor, angle, length, width, side))
    for anchor, angle, length, width in [(13, 40, 13, 3.2), (21, 28, 9, 2.4)]:
        PARTS.append(blade(anchor, angle, length, width, side, front=True))
GEM = (0.0, GRIP_B + 1)


# --- The texture's place for each point of the plane. ---
def texel(a, b, region):
    u = (a - A_RANGE[0]) / (A_RANGE[1] - A_RANGE[0])
    v = (b - B_RANGE[0]) / (B_RANGE[1] - B_RANGE[0]) * 0.5 + (0.5 if region == "feather" else 0.0)
    return u, v


# --- The model. ---
def write_obj(path):
    lines = ["# The Seraph Bow (accessory 370001), by content/models/make_seraph_bow.py."]
    counter = [0]

    def vertex(a, b, n, region):
        x, y, z = place(a, b, n)
        u, v = texel(a, b, region)
        lines.append(f"v {x:.4f} {y:.4f} {z:.4f}")
        lines.append(f"vt {u:.5f} {1 - v:.5f}")
        counter[0] += 1
        return counter[0]

    def face(*corners):
        lines.append("f " + " ".join(f"{c}/{c}" for c in corners))

    for part in PARTS:
        lines.append(f"g bone{part.bone}" if part.bone is not None else f"g {part.name}")
        rows = []
        for i in range(len(part.spine)):
            t = part.thickness[i]
            left, right = part.edge(i, 1), part.edge(i, -1)
            rows.append([
                vertex(*left, t, part.region), vertex(*right, t, part.region),
                vertex(*left, -t, part.region), vertex(*right, -t, part.region),
            ])
        for r0, r1 in zip(rows, rows[1:]):
            face(r0[0], r0[1], r1[1], r1[0])  # front
            face(r0[2], r1[2], r1[3], r0[3])  # back
            face(r0[0], r1[0], r1[2], r0[2])  # left wall
            face(r0[1], r0[3], r1[3], r1[1])  # right wall
        for row in (rows[0], rows[-1]):
            face(row[0], row[2], row[3], row[1])
    # The gem: a diamond standing out of both faces of the grip.
    lines.append("g bone0")
    a, b = GEM
    ring = [vertex(a + 4.2 * math.cos(k * math.pi / 3), b + 4.2 * math.sin(k * math.pi / 3), 0, "main") for k in range(6)]
    front, back = vertex(a, b, 4.6, "main"), vertex(a, b, -4.6, "main")
    for k in range(6):
        face(front, ring[k], ring[(k + 1) % 6])
        face(back, ring[(k + 1) % 6], ring[k])
    path.write_text("\n".join(lines) + "\n")


# --- The texture. ---
def mix(c0, c1, t):
    t = min(max(t, 0.0), 1.0)
    return tuple(round(x + (y - x) * t) for x, y in zip(c0, c1))


SILVER, SILVER_DARK = (226, 236, 248), (120, 138, 168)
DEEP, ICE, FROST = (28, 66, 150), (120, 196, 255), (222, 246, 255)


def shade(paint, across, along):
    """The colour `across` the ribbon (0 the spine, 1 its edge) and
    `along` it (0 its root, 1 its tip)."""
    if paint == "limb":
        if across > 0.78:
            return mix(SILVER, SILVER_DARK, (across - 0.78) / 0.22)
        if across > 0.6:
            return DEEP
        if across < 0.1:
            return FROST
        return mix(ICE, DEEP, (across - 0.1) / 0.5)
    if paint == "blade":
        if across > 0.8:
            return mix(SILVER, SILVER_DARK, (across - 0.8) / 0.2)
        if across < 0.09:
            return FROST
        body = mix(mix(FROST, ICE, along * 1.4), DEEP, (across - 0.09) / 0.71 * (0.4 + 0.6 * along))
        return body
    if paint == "grip":
        return mix((22, 36, 78), (60, 84, 140), 1 - across)
    return (210, 244, 255)


def to_pixels(a, b, region):
    u, v = texel(a, b, region)
    return (u * SIZE, v * SIZE)


def paint_texture(path):
    image = Image.new("RGB", (SIZE, SIZE), DEEP)
    draw = ImageDraw.Draw(image)
    bands = 12
    for part in sorted(PARTS, key=lambda p: p.paint == "string"):
        count = len(part.spine)
        for i in range(count - 1):
            along = i / max(count - 2, 1)
            for k in range(-bands, bands):
                f0, f1 = k / bands, (k + 1) / bands
                corners = []
                for j, f in ((i, f0), (i + 1, f0), (i + 1, f1), (i, f1)):
                    (a, b), (na, nb), w = part.spine[j], part.normals[j], part.widths[j]
                    corners.append(to_pixels(a + na * w * f, b + nb * w * f, part.region))
                draw.polygon(corners, fill=shade(part.paint, abs((f0 + f1) / 2), along))
        if part.paint == "grip":
            # Silver bands at its ends and a wrap across it.
            for a in (-11.5, -10, 10, 11.5):
                p0, p1 = to_pixels(a, GRIP_B + 1 - 4.6, "main"), to_pixels(a, GRIP_B + 1 + 4.6, "main")
                draw.line([p0, p1], fill=SILVER, width=3)
            for a in range(-8, 9, 3):
                p0, p1 = to_pixels(a - 1.5, GRIP_B + 1 - 4.6, "main"), to_pixels(a + 1.5, GRIP_B + 1 + 4.6, "main")
                draw.line([p0, p1], fill=(70, 100, 160), width=1)
    # The string's line, past the limbs.
    draw.line([to_pixels(-HALF, 0, "main"), to_pixels(HALF, 0, "main")], fill=(210, 244, 255), width=3)
    # The gem: white at its heart, cyan, deep blue at its rim.
    cx, cy = to_pixels(*GEM, "main")
    rx = 4.6 / (A_RANGE[1] - A_RANGE[0]) * SIZE
    ry = 4.6 / (B_RANGE[1] - B_RANGE[0]) * SIZE * 0.5
    for step in range(40, 0, -1):
        k = step / 40
        colour = mix((240, 255, 255), mix((70, 230, 255), (30, 90, 210), (k - 0.6) / 0.4), k / 0.6)
        draw.ellipse([cx - rx * k, cy - ry * k, cx + rx * k, cy + ry * k], fill=colour)
    image.save(path)


def paint_icon(path, texture):
    """The bow flat, each part drawn with its own texels, the blades under
    the limb, turned 45 degrees and brought down to the icon's 64 pixels."""
    scale = 6
    width, height = int((LIMB_END * 2 + 10) * scale), int((B_RANGE[1] + 8) * scale)
    canvas = Image.new("RGBA", (width, height), (0, 0, 0, 0))
    source = texture.load()
    draw = ImageDraw.Draw(canvas)

    def at(a, b):
        return ((a + LIMB_END + 5) * scale, (B_RANGE[1] - b) * scale)

    order = sorted(PARTS, key=lambda p: {"blade": 0, "limb": 1, "grip": 2, "string": 3}[p.paint])
    for part in order:
        bands = 6
        for i in range(len(part.spine) - 1):
            for k in range(-bands, bands):
                f0, f1 = k / bands, (k + 1) / bands
                corners, middle = [], [0.0, 0.0]
                for j, f in ((i, f0), (i + 1, f0), (i + 1, f1), (i, f1)):
                    (a, b), (na, nb), w = part.spine[j], part.normals[j], part.widths[j]
                    corners.append(at(a + na * w * f, b + nb * w * f))
                    middle[0] += (a + na * w * f) / 4
                    middle[1] += (b + nb * w * f) / 4
                u, v = to_pixels(middle[0], middle[1], part.region)
                draw.polygon(corners, fill=source[min(int(u), SIZE - 1), min(int(v), SIZE - 1)] + (255,))
    cx, cy = at(*GEM)
    for step in range(12, 0, -1):
        r = 4.6 * scale * step / 12
        draw.ellipse([cx - r, cy - r, cx + r, cy + r], fill=mix((240, 255, 255), (60, 200, 255), step / 12) + (255,))
    turned = canvas.rotate(45, resample=Image.BICUBIC, expand=True)
    turned = turned.crop(turned.getbbox())
    turned.thumbnail((60, 60), Image.LANCZOS)
    icon = Image.new("RGBA", (64, 64), (0, 0, 0, 0))
    icon.alpha_composite(turned, ((64 - turned.width) // 2, (64 - turned.height) // 2))
    icon.save(path)


if __name__ == "__main__":
    write_obj(HERE / "370001.obj")
    paint_texture(HERE.parent / "textures" / "370001.png")
    paint_icon(HERE.parent / "icons" / "370001.png", Image.open(HERE.parent / "textures" / "370001.png").convert("RGB"))
    print(f"string {HALF * 2:.1f} long, grip {GRIP_B:.1f} behind it")
