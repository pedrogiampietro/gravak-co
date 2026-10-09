"""Prepares AI-generated decorations for the game: takes images with a plain
white (or light grey) background, makes the background transparent with a
soft edge, crops to the object and saves PNGs.

    python content/art/tirar_fundo.py <folder or images...> [--out <folder>]

Each image keeps its name (as .png) in --out (by default, beside it). Images
already transparent are only cropped. See docs/conteudo-custom.md.
"""
import sys
from pathlib import Path

import numpy as np
from PIL import Image

# How light and how grey a pixel must be to count as background.
LIGHT = 205
GREY = 30


def cut(image: Image.Image) -> Image.Image:
    rgba = np.asarray(image.convert("RGBA")).astype(float)
    rgb, alpha = rgba[..., :3], rgba[..., 3]
    if alpha.min() < 250:
        out = image.convert("RGBA")
    else:
        low, high = rgb.min(2), rgb.max(2)
        grey = (high - low) < GREY
        # Fully transparent on the background, fading in over 30 levels.
        alpha = np.clip((235 - low) / 30, 0, 1) * 255
        alpha[(low > LIGHT) & grey] = 0
        alpha[~grey] = 255
        out = Image.fromarray(np.dstack([rgb, alpha]).astype(np.uint8), "RGBA")
    box = out.getbbox()
    return out.crop(box) if box else out


def main(arguments: list[str]) -> None:
    out_dir = None
    if "--out" in arguments:
        index = arguments.index("--out")
        out_dir = Path(arguments[index + 1])
        del arguments[index : index + 2]
    files = []
    for argument in arguments:
        path = Path(argument)
        if path.is_dir():
            files += [p for p in path.iterdir() if p.suffix.lower() in (".png", ".jpg", ".jpeg", ".webp")]
        else:
            files.append(path)
    if not files:
        sys.exit(__doc__)
    for path in files:
        target = (out_dir or path.parent) / (path.stem + ".png")
        target.parent.mkdir(parents=True, exist_ok=True)
        result = cut(Image.open(path))
        result.save(target)
        print(f"{path.name} -> {target} {result.size[0]}x{result.size[1]}")


if __name__ == "__main__":
    main(sys.argv[1:])
