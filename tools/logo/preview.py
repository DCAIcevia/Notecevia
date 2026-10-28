"""出一个变体的总览图：任务栏实尺寸、尺寸梯、词标两种底。

    python tools/logo/preview.py --name walnut
"""
import argparse
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

REPO = Path(__file__).resolve().parents[2]
OUT = REPO / "design" / "logo" / "out"
SIDES = [256, 128, 64, 48, 32, 24, 16]


def font(size):
    for name in ("msyh.ttc", "simhei.ttf", "segoeui.ttf", "arial.ttf"):
        try:
            return ImageFont.truetype(f"C:/Windows/Fonts/{name}", size)
        except OSError:
            continue
    return ImageFont.load_default()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--name", required=True)
    args = parser.parse_args()

    folder = OUT / args.name
    icons = {size: Image.open(folder / "png" / f"icon-{size}.png").convert("RGBA") for size in SIDES}
    small, title = font(11), font(13)

    width, height = 900, 560
    sheet = Image.new("RGB", (width, height), (236, 236, 236))
    draw = ImageDraw.Draw(sheet)
    text = (60, 60, 66)

    y = 24
    draw.text((20, y), "任务栏实尺寸（上浅下深）", font=title, fill=text)
    y += 22
    for background in ((247, 247, 250), (32, 32, 36)):
        draw.rectangle([20, y, width - 20, y + 44], fill=background)
        x = 40
        for size in (16, 24, 32, 48):
            sheet.paste(icons[size], (x, y + (44 - size) // 2), icons[size])
            x += size + 40
        x = 420
        for size in (16, 24, 32, 48):
            draw.text((x, y + 16), f"{size}px", font=small, fill=(150, 150, 150))
            x += 60
        y += 52

    y += 16
    draw.text((20, y), "尺寸梯：48 以上空心，32/24/16 实字小裁切", font=title, fill=text)
    y += 22
    x = 20
    for size in SIDES:
        sheet.paste(icons[size], (x, y + (256 - size)), icons[size])
        draw.text((x, y + 258), str(size), font=small, fill=text)
        x += size + 18

    y = 470
    draw.text((20, y), "词标（浅色面棕墨 / 深色面纸白）", font=title, fill=text)
    wordmark = folder / "wordmark.png"
    if wordmark.exists():
        mark = Image.open(wordmark).convert("RGBA")
        scale = 300 / mark.width
        mark = mark.resize((300, int(mark.height * scale)), Image.LANCZOS)
        draw.rectangle([20, y + 20, 340, y + 80], fill=(255, 255, 255))
        sheet.paste(mark, (36, y + 26), mark)
        light = Image.new("RGBA", mark.size, (247, 244, 239, 255))
        light.putalpha(mark.getchannel("A"))
        draw.rectangle([360, y + 20, 680, y + 80], fill=(28, 26, 24))
        sheet.paste(light, (376, y + 26), light)
    else:
        draw.text((20, y + 24), "（没有 wordmark.png，--install 时才会生成）", font=small, fill=text)

    target = REPO / "design" / "logo" / f"preview-{args.name}.png"
    sheet.save(target)
    print(target)


if __name__ == "__main__":
    main()