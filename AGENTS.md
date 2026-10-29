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
- 运行旧界面：`cargo run -p notecevia-app`，二进制名 `notecevia`。
- 运行新界面：`pnpm install`，然后 `pnpm tauri dev`；只预览前端用 `pnpm dev`。
- 前端检查：`pnpm build`（先跑 `vue-tsc --noEmit` 再打包）。
- `clippy` 带 `-D warnings`，警告即失败；`fmt` 只检查，改写用 `cargo fmt --all`。
- 工具链由 `rust-toolchain.toml` 固定，rustup 按该文件选择版本；workspace 用 `edition = "2024"`。
- 三条命令即 CI 门禁：推送 `main` 与合并请求时在 Ubuntu、Windows 上各跑一遍，CI 里的命令带 `--locked`；CI 另在 Linux 上构建文档并运行依赖审计。
- 依赖审计同时由 CI 与 `.github/workflows/audit.yml` 运行：依赖清单变化时比对许可证清单，每周检查漏洞与许可证，允许列表在 `deny.toml`。
- 旧界面的改动跑两条本机验收（需要真实桌面，不进 CI）：
  - `pwsh tools/ui-shots.ps1 -Capture before` 截一组，改完 `-Capture after`，再用 `-Before before -After after` 逐像素比；有差异即非零退出。
  - `pwsh tools/window-probe.ps1` 验窗口行为：拖头部、拉边缘、松手后不再跟随、双击最大化。

## 结构

| 目录 | 包名 | 职责 |
| --- | --- | --- |
| `src` | 前端包 `notecevia` | 界面层：Vue 前端；`public/` 放随包分发的静态资源 |
| `src-tauri` | `notecevia-desktop` | 窗口壳：前端与 Rust 内核之间的桥 |
| `crates/core` | `notecevia-core` | 领域逻辑与用例 |
| `crates/store` | `notecevia-store` | 文件读写与索引 |
| `crates/ui` | `notecevia-ui` | 旧界面（egui），冻结 |
| `crates/os` | `notecevia-os` | 旧界面的系统窗口层，冻结 |
| `crates/app` | `notecevia-app` | 旧界面的入口，冻结 |

依赖单向：`src-tauri` 以路径依赖引用 `crates/core`、`crates/store`；`store`、`ui` → `core`；`core` 不依赖内部 crate，也不碰文件、时钟、随机数、界面类型。

新界面写在 `src`。`crates/ui`、`crates/app`、`crates/os` 冻结：只修缺陷，不加功能；新界面与它们一一追平后删除，代码由 git 留底。

`src-tauri` 自成 workspace（`Cargo.toml` 里有空的 `[workspace]` 表），不进根 workspace，所以根上的 `cargo test --workspace` 不编译它，Linux 的 Gate 也不需要 GTK／WebKit 开发包。它的 `fmt` 与 `clippy` 单独跑。

每个 crate 顶部有 `#![deny(rustdoc::broken_intra_doc_links)]`，`crates/os` 以外的 crate 另有 `#![forbid(unsafe_code)]`，新增模块沿用。`crates/ui`、`crates/app` 依赖 `egui`／`eframe`；`src-tauri` 依赖 `tauri`，前端依赖 Vue。其余标准库能合理实现就不加。

旧界面里，`crates/os` 是全库唯一允许 `unsafe` 的地方：窗口句柄、窗口样式的读写只能直接调系统接口。它对外只暴露"能不能做"的布尔与选项，平台没有实现就返回否，界面退回 egui 自己的通用命令——所以这一层缺了，窗口仍然能用。

旧界面自己画窗口按钮，但有两块交给系统才成立：图标（单击出系统菜单、双击关窗）与最大化按钮（Win11 贴靠布局浮窗只认系统认得的区域）。界面每帧把这两块按物理像素报上去，系统接管的区域收不到指针事件，悬停与按下状态由系统回报。没有实现的平台（Linux、macOS）不报，界面退回通用命令：拖动走 `StartDrag`，拉边走 `BeginResize`，最大化走 `Maximized`，三个窗口按钮都自己响应。
