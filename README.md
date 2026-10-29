# Notecevia

[![CI](https://github.com/HanYang06/Notecevia/actions/workflows/ci.yml/badge.svg)](https://github.com/HanYang06/Notecevia/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/HanYang06/Notecevia?include_prereleases&label=release)](https://github.com/HanYang06/Notecevia/releases)

单机离线的桌面画板笔记：一张无限画布上放各种内容，数据存在本机，导出的文件自带全部内容。

阶段：Alpha。功能与文件格式都可能变化，变化时带迁移，不丢数据。界面正从 egui 迁到 Tauri + Vue，新界面尚未追平旧界面。

## 现在能做什么

旧界面已删除。下列能力由 Rust 内核与数据格式提供：

- 七类块：文字、图片、视频、音频、表格、形状（矩形/椭圆/直线）、高亮
- 文字样式：字号、字体、文字颜色、文字底色
- 表格：单元格文本，可加行加列
- 图片：填充/适应/拉伸三种放置方式
- 摆放数据：位置、尺寸、旋转、层级、锁定与隐藏
- 多选后的对齐与等距分布
- 撤销重做：整份文档快照，栈最多 100 步
- 画板首页的数据：缩略图文件、搜索（标题/标签/正文/表格单元格/媒体文件名）、标签、重命名、删除进回收站
- 崩溃恢复：写盘先写临时文件再改名，启动时清理没写完的临时文件
- 导出：自包含容器（可再导入）、自包含 HTML；文件在 `NoteceviaData/exports/`

## 还不能做什么

- IPC 已接好（41 条命令，其中 `greet` 是遗留示例），Vue 界面还没写
- 自动保存与当前画面 PNG 导出：都由界面驱动，新界面尚未实现
- 自绘窗口按钮与系统命中区域：新界面用系统默认标题栏
- 视频与音频在应用内播放（内核只存原始字节）
- 文字的行内样式（粗体、局部颜色、链接）与组合
- 外部改动发现：只按 boards/ 目录时间与文件数判断，原地覆盖写发现不了

## 运行

```bash
pnpm install
pnpm tauri dev
```

只预览前端用 `pnpm dev`，打包用 `pnpm tauri build`。

提交前三条门禁全绿：

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

工具链由 `rust-toolchain.toml` 固定为 1.99.0，workspace 是 edition 2024；CI 另构建文档并审计依赖。

## 数据位置

工作区是程序所在目录下的 `NoteceviaData`，随程序走：整个文件夹拷走即换机器。程序目录不可写时程序拒绝写盘并提示路径，不会把数据写到别处。

```text
NoteceviaData/
  boards/<编号>.board               画板目录，CBOR
  blocks/<画板>-<块>.block          私有块正文，CBOR
  media/<sha256>                    图片、视频、音频原始字节
  thumbnails/<编号>.png             列表缩略图，可删
  index.cbor                        列表与搜索索引，可删，会重建
  ui.json                           两条侧栏的宽度，可删，删了回默认
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
| `docs/design/ipc.md` | 前端与窗口壳之间的命令契约 |
| `AGENTS.md` | 参与开发前先读的规则入口 |

## 许可与参与

Apache-2.0，见 `LICENSE`，版权归 HanYang06（2026）。项目署名见 `NOTICE`，随程序分发的第三方 crate 及许可全文见 `THIRD_PARTY_LICENSES.md`。提交方式与评审流程见 `CONTRIBUTING.md`。
