# Notecevia

单机离线的桌面笔记应用。界面用 Web（Tauri），内核与数据用 Rust。一个画板就是一个文件，内容是块，媒体嵌在文件里。数据不离开本机。

## 规则入口

本文件只放定位、加载表与红线。规则只在 `.agents/skills/`，不凭记忆猜。

| 场景 | 先执行 |
| --- | --- |
| 任何任务开始 | `skill project-rules` |
| 提交、分支、发布 | `skill git-commit` |
| 改界面 | `skill ui-design` |
| 新建或修改 skill | `skill skill-creator` |

## 红线

- 该加载的 skill 没加载就动手，视为违规。
- 新依赖、新功能范围、新 UI 方向：先问，得到明确答复再做。
- 不读 `target/`，不整库扫描，不凭记忆写第三方 API。
- 缺信息先问；预算耗尽就停下，不自行脑补。
- 完成 = 门禁通过 + 四段汇报（改动／验证／风险／后续）。

## 命令

门禁按下列顺序执行，三条全绿才算通过：

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- 目录名与包名不同：`crates/core` 的包是 `notecevia-core`，`-p core` 无效。
- 单包测试：`cargo test -p notecevia-core <用例名>`。
- 运行界面：先 `pnpm install`，再 `pnpm tauri dev`。
- 只预览前端：`pnpm dev`。
- 前端检查：`pnpm build`（先跑 `vue-tsc --noEmit` 再打包）。
- 打包：`pnpm tauri build`。
- `clippy` 带 `-D warnings`，警告即失败；`fmt` 只检查，改写用 `cargo fmt --all`。
- 工具链由 `rust-toolchain.toml` 固定，rustup 按该文件选择版本；workspace 用 `edition = "2024"`。
- 三条命令即 CI 门禁：推送 `main` 与合并请求时在 Ubuntu、Windows 上各跑一遍，CI 里的 `clippy` 与 `test` 带 `--locked`；CI 另在 Linux 上构建文档并运行依赖审计。
- Tauri 壳是根 workspace 成员，Linux 的 Gate 需要 GTK／WebKit 开发包：`libwebkit2gtk-4.1-dev`、`libjavascriptcoregtk-4.1-dev`、`libgtk-3-dev`、`librsvg2-dev`，另有 x11／wayland／fontconfig 开发包；本地门禁仍是上面三条。
- 依赖审计同时由 CI 与 `.github/workflows/audit.yml` 运行：依赖清单变化时比对许可证清单，每周检查漏洞与许可证，允许列表在 `deny.toml`。

## 结构

| 目录 | 包名 | 职责 |
| --- | --- | --- |
| `crates/ui` | 前端包 `notecevia` | 界面层：Vue 3 + Vite 前端源码；构建入口 `index.html`、`package.json`、`vite.config.ts`、`tsconfig.json`、`tsconfig.node.json`、`public/` 在仓库根 |
| `crates/app` | `notecevia` | 窗口壳：Tauri 2，二进制名 `notecevia`；IPC 命令层在 `crates/app/src/ipc/`，把 `core`／`store` 包成前端可调的命令 |
| `crates/core` | `notecevia-core` | 领域逻辑与用例 |
| `crates/store` | `notecevia-store` | 文件读写与索引 |

依赖单向：`crates/app` 以路径依赖引用 `crates/core`、`crates/store`，IPC 命令已经接上；`store` → `core`；`core` 不依赖内部 crate，也不碰文件、时钟、随机数、界面类型。

`greet` 是遗留示例命令，前端样板还在用，界面重写时删除。

根 workspace 的成员只有 `crates/core`、`crates/store`、`crates/app`，所以根上的 `cargo test --workspace` 会编译窗口壳。

每个 Rust crate 顶部有 `#![deny(rustdoc::broken_intra_doc_links)]`，并有 `#![forbid(unsafe_code)]`，新增模块沿用。`crates/app` 依赖 `tauri` 与 `tauri-plugin-opener`，前端依赖 Vue。其余标准库能合理实现就不加。
