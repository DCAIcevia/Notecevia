# Notecevia

[![CI](https://github.com/HanYang06/Notecevia/actions/workflows/ci.yml/badge.svg)](https://github.com/HanYang06/Notecevia/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/HanYang06/Notecevia?include_prereleases&label=release)](https://github.com/HanYang06/Notecevia/releases)

单机离线的桌面画板笔记：一张无限画布上写字，内容存在本机，导出的文件自带全部内容。

阶段：Alpha。功能与文件格式都可能变化，变化时带迁移，不丢数据。

## 现在能做什么

- 双击空白开始写，点别处把写下的内容确认成一块；没写字就自动丢掉
- 字号、字体、文字颜色、文字底色
- 拖动摆放、拖边改宽高、绕角旋转
- 画布平移与缩放，块只在确认之后存在
- Ctrl+S 保存，Ctrl+N 新建画板，关窗时保存
- 保存后重开，内容、位置、样式全部还原

## 还不能做什么

- 图片、视频、音频、形状与撤销
- 画板列表、标签、搜索、自动保存与崩溃恢复
- 导出：私有容器、图片、HTML

对应路线图里的 A2、A3、A4。

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

画板按编号存成 `boards/<编号>.board`，文字块存成 `blocks/<画板>-<块>.block`，编码是 CBOR。格式与字段见 `docs/design/format.md`。

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
