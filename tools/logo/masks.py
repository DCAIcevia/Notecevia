"""生成遮挡图 SVG 并栅格化（上色前的白形）。

    python tools/logo/masks.py
"""
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from _browser import render

REPO = Path(__file__).resolve().parents[2]
BUILD = REPO / "design" / "logo" / "build"
SIDE = 512.0
FILL = 0.80
RADIUS = 0.225
RASTER = 1024


def squircle(side):
    rad = side * RADIUS
    return (
        f"M{rad:.2f} 0H{side - rad:.2f}"
        f"A{rad:.2f} {rad:.2f} 0 0 1 {side:.1f} {rad:.2f}"
        f"V{side - rad:.2f}A{rad:.2f} {rad:.2f} 0 0 1 {side - rad:.2f} {side:.1f}"
        f"H{rad:.2f}A{rad:.2f} {rad:.2f} 0 0 1 0 {side - rad:.2f}"
        f"V{rad:.2f}A{rad:.2f} {rad:.2f} 0 0 1 {rad:.2f} 0Z"
    )


def geometry(geo):
    x0, y0, x1, y1 = geo["box"]
    scale = FILL * SIDE / (x1 - x0)
    ox = (SIDE - (x1 - x0) * scale) / 2 - x0 * scale
    oy = (SIDE - (y1 - y0) * scale) / 2 - y0 * scale
    return f"matrix({scale:.5f} 0 0 {scale:.5f} {ox:.3f} {oy:.3f})"


def main():
    geo = json.loads((BUILD / "geo.json").read_text(encoding="utf-8"))
    letters = geo["letters"]
    transform = geometry(geo)
    hollow = "".join(letters)
    solid = "".join(letters[i] for i in (1, 0))

    jobs = {
        "mask-plate": f'<path d="{squircle(SIDE)}" fill="#fff"/>',
        "mask-hollow": f'<path d="{hollow}" transform="{transform}" fill="#fff"/>',
        "mask-solid": f'<path d="{solid}" transform="{transform}" fill="#fff"/>',
    }
    for name, inner in jobs.items():
        svg = (
            f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {SIDE:.0f} {SIDE:.0f}"'
            f' width="{RASTER}" height="{RASTER}">'
            f'<rect width="{SIDE}" height="{SIDE}" fill="#000"/>{inner}</svg>'
        )
        path = BUILD / f"{name}.svg"
        path.write_text(svg, encoding="utf-8")
        render(path, BUILD / f"{name}.png", RASTER, RASTER)
        print(f"{name}.png")


if __name__ == "__main__":
    main()
