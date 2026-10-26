# Notecevia

[![CI](https://github.com/HanYang06/Notecevia/actions/workflows/ci.yml/badge.svg)](https://github.com/HanYang06/Notecevia/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/HanYang06/Notecevia?include_prereleases&label=release)](https://github.com/HanYang06/Notecevia/releases)

单机离线的桌面画板笔记：一张无限画布上放各种内容，数据存在本机，导出的文件自带全部内容。

阶段：Alpha。功能与文件格式都可能变化，变化时带迁移，不丢数据。

## 现在能做什么

- 七类块：文字、图片、视频、音频、表格、形状（矩形/椭圆/直线）、高亮
- 双击空白开始写；点别处把写下的内容确认成一块；没写字就自动丢掉
- 文字样式：字号、字体、文字颜色、文字底色
- 表格：单元格直接编辑，可加行加列
- 图片：按填充/适应/拉伸三种方式放进框里
- 视频、音频：拖进画布成为一块，双击用系统播放器打开
- 摆放：拖动、拖边改宽高、绕角旋转；层级置顶置底；锁定与隐藏
- 多选：Shift 点选、空白拖框，多块可对齐与等距分布
- 撤销重做：每一步都是一步撤销，关掉程序前都能退回去
- 画布平移与缩放，块只在确认之后存在
- 画板首页：缩略图、搜索（标题/标签/正文）、按标签筛选、重命名、删除确认、刷新列表
- 自动保存与崩溃恢复：改动 2 秒后自动落盘，启动时清理没写完的临时文件
- 导出：自包含容器（可再导入）、自包含 HTML、当前画面 PNG；文件在 `NoteceviaData/exports/`
- Ctrl+S 保存，Ctrl+N 新建，Ctrl+H 回首页，关窗时保存

## 还不能做什么

- 视频与音频在应用内播放（现在交给系统播放器）
- 文字的行内样式（粗体、局部颜色、链接）与组合
- 外部改动只在停在首页时每 2 秒检查一次目录时间；原地覆盖写发现不了

## 运行

```bash
cargo run -p notecevia-app
```

提交前三条门禁全绿：

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

工具链用 Rust 1.85 以上，workspace 是 edition 2024。

## 数据位置

工作区是程序所在目录下的 `NoteceviaData`，随程序走：整个文件夹拷走即换机器。程序目录不可写时程序拒绝写盘并提示路径，不会把数据写到别处。

```text
NoteceviaData/
  boards/<编号>.board               画板目录，CBOR
  blocks/<画板>-<块>.block          私有块正文，CBOR
  media/<sha256>                    图片、视频、音频原始字节
  thumbnails/<编号>.png             列表缩略图，可删
  index.cbor                        列表与搜索索引，可删，会重建
  exports/                          导出结果
  .trash/                           删掉的画板与孤儿块
```

格式号 2，字段见 `docs/design/format.md`。格式 1 的旧文件仍能打开，保存时升到格式 2。

## 文档

| 文档 | 内容 |
| --- | --- |
| `docs/roadmap/roadmap.md` | 还没做的事与完成标准 |
| `changelog/` | 每个版本交付了什么 |
| `docs/design/format.md` | 工作区与导出容器的格式 |
| `docs/design/architecture.md` | 分层、加载策略与性能预算 |
| `AGENTS.md` | 参与开发前先读的规则入口 |

## 许可与参与

Apache-2.0，见 `LICENSE`，版权归 HanYang06（2026）。项目与内嵌字体的署名见 `NOTICE`，随程序分发的第三方 crate 及许可全文见 `THIRD_PARTY_LICENSES.md`。提交方式与评审流程见 `CONTRIBUTING.md`。
