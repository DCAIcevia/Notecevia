# Notecevia

单机离线的桌面笔记应用。纯 Rust，不用 WebView。一个画板就是一个文件，内容是块，媒体嵌在文件里。

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
- 运行：`cargo run -p notecevia-app`，二进制名 `notecevia`。
- `clippy` 带 `-D warnings`，警告即失败；`fmt` 只检查，改写用 `cargo fmt --all`。
- 工具链由 `rust-toolchain.toml` 固定，rustup 按该文件选择版本；workspace 用 `edition = "2024"`。
- 三条命令即 CI 门禁：推送 `main` 与合并请求时在 Ubuntu、Windows 上各跑一遍，CI 里的命令带 `--locked`。
- 依赖审计由 `.github/workflows/audit.yml` 定时运行，许可证允许列表在 `deny.toml`。

## 结构

| 目录 | 包名 | 职责 |
| --- | --- | --- |
| `crates/core` | `notecevia-core` | 领域逻辑与用例 |
| `crates/store` | `notecevia-store` | 文件读写与索引 |
| `crates/ui` | `notecevia-ui` | 状态映射成界面 |
| `crates/app` | `notecevia-app` | 组装与入口 |

依赖单向：`app` → `store`、`ui`；`store`、`ui` → `core`。`core` 不依赖内部 crate，也不碰文件、时钟、随机数、界面类型。

每个 crate 顶部有 `#![forbid(unsafe_code)]`，新增模块沿用。界面依赖 `egui`／`eframe`，其余标准库能合理实现就不加。
