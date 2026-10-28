# 标志生成脚本

输入是 `assect/notecevia.vectorcraft`（模板母版）与 `assect/notecevia.svg`（词标），
输出到 `design/logo/`（不进仓库），`--install` 时写进 `crates/app/assets/`。

## 顺序

```powershell
python tools/logo/extract.py            # 从 .vectorcraft 抽几何 -> design/logo/build/geo.json
python tools/logo/masks.py              # 生成并栅格化遮挡图（需要 Edge/Chrome）
python tools/logo/colorway.py --plate "#754c24" --name walnut --install
python tools/logo/preview.py --name walnut
```

## 换色

颜色只是参数。牌色与字母色的明度差要 ≥4.5（WCAG），否则字母糊进牌里。

```powershell
python tools/logo/colorway.py --plate "#2e3192" --name blue
```

## 产物

`design/logo/out/<name>/`：`icon.png`、`icon.ico`（16/24/32/48/64/128/256）、
`icon.svg`、`icon-small.svg`、`wordmark.svg`、`png/icon-N.png`。

32/24/16 用实字母版，48 及以上用空心母版；两者同源，只差填哪条轮廓。
