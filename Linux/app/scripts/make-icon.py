#!/usr/bin/env python3
"""Draw the VIDSYNC app icon.

The icon mirrors the small logo shown to the left of the VIDSYNC wordmark: a
soft purple rounded square with a bright purple play triangle. Run from the app
folder with ``python3 scripts/make-icon.py``; Pillow is required.
"""
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "assets" / "vidsync.png"

# Colours sampled from the in-app logo (purple accent over the dark header).
SOFT_PURPLE = (37, 30, 52, 255)
PLAY_PURPLE = (176, 122, 239, 255)


def build(size: int) -> Image.Image:
    s = size / 512.0

    def r(*values):
        return tuple(round(v * s) for v in values)

    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    # The rounded tile and the play mark keep the same proportions as the
    # in-app logo (radius 30% of the tile, triangle 44% high).
    d.rounded_rectangle(r(12, 12, 500, 500), radius=r(146)[0], fill=SOFT_PURPLE)
    d.polygon(
        [r(202, 161), r(202, 375), r(379, 268)],
        fill=PLAY_PURPLE,
    )
    return img


def main() -> None:
    OUT.parent.mkdir(parents=True, exist_ok=True)
    build(512).resize((256, 256), Image.LANCZOS).save(OUT)
    print(f"wrote {OUT}")


if __name__ == "__main__":
    main()
