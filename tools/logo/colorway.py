"""按牌色出整套标志资产。

    python tools/logo/colorway.py --plate "#2e3192" --name blue
    python tools/logo/colorway.py --plate "#754c24" --name walnut --install

产出 design/logo/out/<name>/：icon.png、icon.ico、icon.svg、icon-small.svg、
png/icon-N.png；--install 另外写进 crates/app/assets/（含 wordmark.svg 与 wordmark.png）。
"""
import argparse
import io
import json
import re
import shutil
import struct
import sys
from pathlib import Path

from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parent))
from _browser import render

REPO = Path(__file__).resolve().parents[2]
BUILD = REPO / "design" / "logo" / "build"
OUT = REPO / "design" / "logo" / "out"
ASSETS = REPO / "crates" / "app" / "assets"
SIDES = [256, 128, 64, 48, 32, 24, 16]
SMALL_CUT = 32
SIDE = 512.0
FILL = 0.80
RADIUS = 0.225
WORDMARK_VIEWBOX = "512.4 272.8 407.1 84.3"


def hex_rgb(text):
    raw = text.lstrip("#")
    if len(raw) != 6:
        raise SystemExit(f"颜色要 #RRGGBB: {text}")
    return tuple(int(raw[i:i + 2], 16) for i in (0, 2, 4))


def squircle(side):
    rad = side * RADIUS
    return (
        f"M{rad:.2f} 0H{side - rad:.2f}"
        f"A{rad:.2f} {rad:.2f} 0 0 1 {side:.1f} {rad:.2f}"
        f"V{side - rad:.2f}A{rad:.2f} {rad:.2f} 0 0 1 {side - rad:.2f} {side:.1f}"
        f"H{rad:.2f}A{rad:.2f} {rad:.2f} 0 0 1 0 {side - rad:.2f}"
        f"V{rad:.2f}A{rad:.2f} {rad:.2f} 0 0 1 {rad:.2f} 0Z"
    )


def icon_svg(geo, plate, letters, small):
    x0, y0, x1, y1 = geo["box"]
    scale = FILL * SIDE / (x1 - x0)
    ox = (SIDE - (x1 - x0) * scale) / 2 - x0 * scale
    oy = (SIDE - (y1 - y0) * scale) / 2 - y0 * scale
    data = "".join(geo["letters"][i] for i in (1, 0)) if small else "".join(geo["letters"])
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {SIDE:.0f} {SIDE:.0f}">'
        f'<path d="{squircle(SIDE)}" fill="{plate}"/>'
        f'<path d="{data}" transform="matrix({scale:.5f} 0 0 {scale:.5f} {ox:.3f} {oy:.3f})"'
        f' fill="{letters}" fill-rule="nonzero"/></svg>'
    )


def wordmark_svg():
    cache = BUILD / "wordmark.d.txt"
    if cache.exists():
        data = cache.read_text(encoding="utf-8").strip()
    else:
        raw = (REPO / "assect" / "notecevia.svg").read_text(encoding="utf-8")
        elements = re.findall(r"<path[^>]*/?>", raw)
        data = re.search(r'd="([^"]*)"', elements[3]).group(1)
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{WORDMARK_VIEWBOX}">'
        f'<path d="{data}" fill="currentColor" fill-rule="nonzero"/></svg>'
    )


def compose(plate_m, hollow_m, solid_m, plate, letters, size):
    mask = solid_m if size <= SMALL_CUT else hollow_m
    base = Image.new("RGBA", plate_m.size, (0, 0, 0, 0))
    fill = Image.new("RGBA", plate_m.size, plate + (255,))
    fill.putalpha(plate_m)
    base.alpha_composite(fill)
    ink = Image.new("RGBA", plate_m.size, letters + (255,))
    ink.putalpha(mask)
    base.alpha_composite(ink)
    return base.resize((size, size), Image.LANCZOS)


def write_ico(target, items):
    blobs = []
    for size, image in items:
        buffer = io.BytesIO()
        image.save(buffer, format="PNG")
        blobs.append((size, buffer.getvalue()))
    entries, data, offset = b"", b"", 6 + 16 * len(blobs)
    for size, blob in blobs:
        side = 0 if size >= 256 else size
        entries += struct.pack("<BBBBHHII", side, side, 0, 0, 1, 32, len(blob), offset + len(data))
        data += blob
    target.write_bytes(struct.pack("<HHH", 0, 1, len(blobs)) + entries + data)


def tint_wordmark(svg_path, png_path, ink, width=796):
    """把词标渲成 PNG：黑字白底，再按 alpha 换成墨色。"""
    raw = render(svg_path, BUILD / "wordmark-raw.png", width, 180)
    image = Image.open(raw).convert("RGB")
    gray = image.convert("L")
    box = gray.point(lambda v: 255 if v < 247 else 0).getbbox()
    if box is None:
        return None
    image = image.crop(box)
    alpha = Image.new("L", image.size)
    src, dst = image.load(), alpha.load()
    base = sum(ink) / 3.0
    for y in range(image.height):
        for x in range(image.width):
            lum = sum(src[x, y]) / 3.0
            value = (255.0 - lum) / max(1.0, 255.0 - base)
            dst[x, y] = int(round(max(0.0, min(1.0, value)) * 255))
    out = Image.new("RGBA", image.size, ink + (0,))
    out.putalpha(alpha)
    out.save(png_path)
    return out.size


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--plate", required=True, help="牌色 #RRGGBB")
    parser.add_argument("--letters", default="#ffffff", help="字母色 #RRGGBB")
    parser.add_argument("--name", default="", help="输出目录名")
    parser.add_argument("--install", action="store_true", help="写进 crates/app/assets")
    args = parser.parse_args()

    plate, letters = hex_rgb(args.plate), hex_rgb(args.letters)
    name = args.name or args.plate.lstrip("#")
    out = OUT / name
    (out / "png").mkdir(parents=True, exist_ok=True)

    geo = json.loads((BUILD / "geo.json").read_text(encoding="utf-8"))
    plate_m = Image.open(BUILD / "mask-plate.png").convert("L")
    hollow_m = Image.open(BUILD / "mask-hollow.png").convert("L")
    solid_m = Image.open(BUILD / "mask-solid.png").convert("L")

    items = []
    for size in SIDES:
        image = compose(plate_m, hollow_m, solid_m, plate, letters, size)
        image.save(out / "png" / f"icon-{size}.png")
        items.append((size, image))
    compose(plate_m, hollow_m, solid_m, plate, letters, 512).save(out / "icon.png")
    write_ico(out / "icon.ico", list(reversed(items)))

    (out / "icon.svg").write_text(icon_svg(geo, args.plate, args.letters, False), encoding="utf-8")
    (out / "icon-small.svg").write_text(icon_svg(geo, args.plate, args.letters, True), encoding="utf-8")
    (out / "wordmark.svg").write_text(wordmark_svg(), encoding="utf-8")

    if args.install:
        for item in ("icon.png", "icon.ico", "icon.svg", "icon-small.svg", "wordmark.svg"):
            shutil.copyfile(out / item, ASSETS / item)
        tint_wordmark(out / "wordmark.svg", ASSETS / "wordmark.png", plate)
        print(f"installed -> {ASSETS}")

    print(out)


if __name__ == "__main__":
    main()