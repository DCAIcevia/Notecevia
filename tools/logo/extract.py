"""从 VectorCraft 母版里抽几何。

    python tools/logo/extract.py                # 自动找方牌记号
    python tools/logo/extract.py --id 345       # 指定对象
    python tools/logo/extract.py --rev HEAD     # 读某个 git 版本

读 assect/notecevia.vectorcraft，把记号对象的锚点转成 SVG 路径数据，
写 design/logo/build/geo.json；顺带把 assect/notecevia.svg 里的词标抽出来。
"""
import argparse
import json
import re
import subprocess
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
BUILD = REPO / "design" / "logo" / "build"
SOURCE = REPO / "assect" / "notecevia.vectorcraft"
WORDMARK = REPO / "assect" / "notecevia.svg"


def num(value):
    return f"{value:.2f}".rstrip("0").rstrip(".")


def to_path_data(path):
    out = []
    for sub in path["subpaths"]:
        anchors = sub["anchors"]
        if not anchors:
            continue
        out.append(f"M{num(anchors[0]['p'][0])} {num(anchors[0]['p'][1])}")
        count = len(anchors) if sub.get("closed") else len(anchors) - 1
        for i in range(count):
            a = anchors[i]
            b = anchors[(i + 1) % len(anchors)]
            c1 = a.get("out", a["p"])
            c2 = b.get("in", b["p"])
            out.append(
                f"C{num(c1[0])} {num(c1[1])} {num(c2[0])} {num(c2[1])} "
                f"{num(b['p'][0])} {num(b['p'][1])}"
            )
        out.append("Z")
    return "".join(out)


def collect_paths(node, found):
    if isinstance(node, dict):
        if node.get("type") == "path" and "path" in node:
            found.append(node["path"])
        for value in node.values():
            collect_paths(value, found)
    elif isinstance(node, list):
        for value in node:
            collect_paths(value, found)


def read_source(rev):
    if not rev:
        return SOURCE.read_text(encoding="utf-8")
    result = subprocess.run(
        ["git", "-C", str(REPO), "show", f"{rev}:assect/notecevia.vectorcraft"],
        capture_output=True,
        check=False,
    )
    if result.returncode != 0:
        raise SystemExit(f"读不到 {rev} 版本：{result.stderr.decode('utf-8', 'replace').strip()}")
    return result.stdout.decode("utf-8")


def describe(node):
    paths = []
    collect_paths(node["kind"], paths)
    xs, ys = [], []
    for path in paths:
        for sub in path["subpaths"]:
            for anchor in sub["anchors"]:
                xs.append(anchor["p"][0])
                ys.append(anchor["p"][1])
    if not xs:
        return None
    return len(paths), (min(xs), min(ys), max(xs), max(ys))


def pick(children):
    """方牌记号 = 5 条轮廓（1 牌 + 4 字母）且接近 1.5:1。"""
    scored = []
    for child in children:
        info = describe(child)
        if not info:
            continue
        count, (x0, y0, x1, y1) = info
        ratio = (x1 - x0) / (y1 - y0)
        if count == 5 and 1.3 <= ratio <= 1.8:
            scored.append((abs(ratio - 1.53), child))
    if len(scored) == 1:
        return scored[0][1]
    lines = ["环境里找不到方牌记号（5 条轮廓、约 1.5:1）。现有对象："]
    for child in children:
        info = describe(child)
        if info:
            count, (x0, y0, x1, y1) = info
            lines.append(
                f"  id={child.get('id')} 轮廓 {count} 条 "
                f"{(x1 - x0):.0f}×{(y1 - y0):.0f} 比 {(x1 - x0) / (y1 - y0):.2f}"
            )
    lines.append("用 --id 指定，或用 --rev 读回旧版本。")
    raise SystemExit("\n".join(lines))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--id", type=int, default=None)
    parser.add_argument("--rev", default=None, help="从 git 版本读，例如 HEAD 或某个提交")
    args = parser.parse_args()

    document = json.loads(read_source(args.rev))["document"]
    children = [child for layer in document["layers"] for child in layer["kind"].get("children", [])]
    if args.id is None:
        target = pick(children)
    else:
        target = next((c for c in children if c.get("id") == args.id), None)
        if target is None:
            raise SystemExit(f"没找到对象 id={args.id}")

    paths = []
    collect_paths(target["kind"], paths)
    data = [to_path_data(path) for path in paths]
    if len(data) < 5:
        raise SystemExit(f"对象应含 1 条牌 + 4 条字母轮廓，实际 {len(data)}")

    xs, ys = [], []
    for path in paths[1:]:
        for sub in path["subpaths"]:
            for anchor in sub["anchors"]:
                xs.append(anchor["p"][0])
                ys.append(anchor["p"][1])

    BUILD.mkdir(parents=True, exist_ok=True)
    (BUILD / "geo.json").write_text(
        json.dumps(
            {
                "plate": data[0],
                "letters": data[1:],
                "box": [min(xs), min(ys), max(xs), max(ys)],
            }
        ),
        encoding="utf-8",
    )

    raw = WORDMARK.read_text(encoding="utf-8")
    elements = [m.group(0) for m in re.finditer(r"<path[^>]*/?>", raw)]
    wordmark = re.search(r'd="([^"]*)"', elements[3]).group(1)
    (BUILD / "wordmark.d.txt").write_text(wordmark, encoding="utf-8")

    print(f"geo.json: {len(data)} 条路径；letters 包围盒 "
          f"{max(xs) - min(xs):.1f}×{max(ys) - min(ys):.1f}")


if __name__ == "__main__":
    main()