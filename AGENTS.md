# Notecevia

单机离线的桌面笔记应用。界面用 Web（Tauri），内核与数据用 Rust。一张画板就是一个文件，内容是块，媒体嵌在文件里。数据不离开本机。

本文件放定位、开工顺序、加载表、红线与出处。规则只在 `.agents/skills/`，不凭记忆猜。

## 开工

会话开始、动手前按这个顺序做，跳步视为违规。

1. 任何任务先加载 `skill project-rules`，包括只改一行文档。它定义四行声明、读取预算、提问与停下的条件、四段汇报；细则在 `project-rules/references/workflow.md`。
2. 写下四行声明：任务与完成条件、准备读的文件、怎么验证、哪些假设可以回退。写不出四行，任务还没定义。
3. 按加载表加载命中的 skill；命中两项就加载两项。加载后先读它的 `Sources of truth`。
4. 只读根 `AGENTS.md`、任务点名的文件、skill 指出的出处。`target/`、`node_modules/`、`dist/`、根 `design/` 不是出处，不整库扫描。
5. 收尾按四段汇报：改动、验证、风险、待定事项。验证写命令与观察到的输出，不写「通过」。

## 加载表

这张表是 skill 的注册表。触发条件写在每个 skill 的 description 里；新增、改名或删除 skill 时，同一改动更新此表，漏了就不会被加载。

| 场景 | 先执行 |
| --- | --- |
| 任何任务开始 | `skill project-rules` |
| 提交、拆分提交、发 PR | `skill git-commit` |
| 评审 PR 或改动 | `skill code-review` |
| 写或改测试 | `skill testing` |
| 改磁盘格式、迁移、导出容器 | `skill data-format` |
| 改界面、文案、主题 | `skill ui-design` |
| 发版、打标签 | `skill release` |
| 新建或修改 skill | `skill skill-creator` 与 `project-rules/references/skills.md` |

## Skill 制度

- skill 在 `.agents/skills/<name>/SKILL.md`；目录名、frontmatter `name`、`skill <name>` 三者一致，例如 `skill ui-design`。
- skill 正文用英文；仓库文档、提交主题与界面文案用中文。两者不互译，也不把中文文档搬进 skill。
- 领域 skill 只放自己的判断；规则出处指回 `AGENTS.md`、`CONTRIBUTING.md`、`docs/design/`。同一规则只留一处。
- 新建或改 skill 先加载 `skill skill-creator`，并按 `project-rules/references/skills.md` 写；`SKILL.md` 不超过 120 行，单个 reference 不超过 200 行。
- `skill-creator` 是上游目录，原样保留，不按本仓库风格改写。
- skill 与仓库文件冲突时，仓库文件胜，skill 在同一改动里修正。

## 红线

- 该加载的 skill 没加载就动手，视为违规。
- 新依赖、新功能范围、新 UI 方向、磁盘格式、IPC 契约：先问，得到明确答复再做。
- `main` 受保护：不直接推送，改动走分支与合并请求，只允许 merge commit。
- 不读 `target/`、`node_modules/`、`dist/`，不整库扫描，不凭记忆写第三方 API。
- 根 `design/` 是本地设计稿，已被忽略；不提交，也不当作事实来源。
- 缺信息先问；预算耗尽就停下，不自行猜测。
- 完成 = 门禁通过 + 四段汇报（改动／验证／风险／待定事项）。

## 命令

门禁按下列顺序执行，三条全绿才算通过：

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- 目录名与包名不同：`crates/core` 是 `notecevia-core`，`crates/store` 是 `notecevia-store`，`crates/app` 是 `notecevia`；`-p core` 无效。
- 单包测试：`cargo test -p notecevia-core <用例名>`；单文件：`cargo test -p notecevia-store --test storage`。
- 改 `crates/ui/` 或根前端配置时，另跑 `pnpm test` 与 `pnpm build`；`build` 先跑 `vue-tsc --noEmit` 再打包。Gate 不跑这两条，前端只在本地与发布构建时打包。
- 运行界面：先 `pnpm install`，再 `pnpm tauri dev`；只预览前端用 `pnpm dev`，地址带 `?mock` 时用假数据。
- 打包：`pnpm tauri build`。
- `clippy` 带 `-D warnings`，警告即失败；`fmt` 只检查，改写用 `cargo fmt --all`。
- 工具链由 `rust-toolchain.toml` 固定为 1.99.0，workspace 用 edition 2024；成员 crate 用 `.workspace = true` 继承版本与 edition。
- CI 在 Ubuntu 与 Windows 跑同一组门禁，`clippy` 与 `test` 带 `--locked`；Linux Gate 的 WebKitGTK／GTK 依赖清单在 `.github/workflows/ci.yml`。CI 另在 Linux 构建文档，并跑依赖审计。
- 依赖审计的允许列表在 `deny.toml`，`about.toml` 与之一致；依赖变化后按 `CONTRIBUTING.md` 重新生成 `THIRD_PARTY_LICENSES.md`。

## 结构

| 目录 | 包名 | 职责 |
| --- | --- | --- |
| `crates/ui` | `notecevia` | 界面层：Vue 3 + Vite + Pinia；源码在 `crates/ui`，构建入口在仓库根（`index.html`、`package.json`、`vite.config.ts`、`tsconfig*.json`、`public/`），产物在 `dist/` |
| `crates/app` | `notecevia` | 窗口壳：Tauri 2，二进制名 `notecevia`；IPC 命令在 `crates/app/src/ipc/`，注册表在 `crates/app/src/lib.rs` |
| `crates/core` | `notecevia-core` | 领域逻辑与用例；不碰文件、时钟、随机数、界面类型 |
| `crates/store` | `notecevia-store` | 工作区读写、媒体、索引、回收站、册与书架、导出导入 |

- 依赖单向：`crates/app` → `crates/store` → `crates/core`；`crates/app` 也直接依赖 `core` 与 `store`。反向调用是设计错误。
- 根 workspace 成员只有三个 Rust crate，所以 `cargo test --workspace` 会编译窗口壳；`crates/ui` 不是 cargo 成员，前端入口在仓库根。
- `notecevia` 同时是前端 npm 包名与窗口壳 cargo 包名；`cargo -p notecevia` 指窗口壳。
- 界面用「册」「页」「书架」，代码用 `volume`、`board`、`shelf`；`Board` 不因为界面叫「页」而改名。文案规则在 `docs/design/ui.md` 的「用词」。
- 工作区是程序所在目录下的 `NoteceviaData`，整个目录拷走即换机器；应用配置在系统配置目录的 `config.json`，不在工作区。
- 每个 Rust crate 顶部有 `#![deny(rustdoc::broken_intra_doc_links)]` 与 `#![forbid(unsafe_code)]`；新增 crate 照抄。
- 领域类型不带 `serde`；序列化形状只在 `crates/app/src/ipc/dto.rs`。
- 标准库能合理实现就不加依赖；新增依赖先问，许可证须在 `deny.toml` 允许列表内。

## 文档

- 当前结论在 `docs/design/`：`architecture.md` 管分层、加载、保存与性能，`format.md` 管磁盘格式与迁移，`ipc.md` 管命令契约与错误 `kind`，`ui.md` 管主题、布局与用词。
- 未完成项在 `docs/roadmap/roadmap.md`，完成时删除该条；阶段、能力与数据位置在 `README.md`。
- 改行为要在同一提交更新对应的设计文档；文档与代码冲突时停下报告两处，不擅自选一边。
- 文档写中文现在时，不写日期、变更史与计划；写法见 `project-rules/references/docs.md`。
