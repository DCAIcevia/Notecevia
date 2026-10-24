---
name: git-commit
description: 提交、分支、合并、发布前加载。规定分支命名、提交信息格式与类型、提交粒度、门禁命令、合并与 PR 要求、版本发布、禁止提交的内容。
---

# Git 与提交

## 分支

- 主干式：`main` 随时可构建、可发布。
- 允许直接进 `main`：规则与 skill 修订、单文件 ≤20 行且无行为变化、构建配置微调。
- 其余走短分支，合并后删除；存活期 ≤3 天，超了拆任务。
- 命名 `<type>/<scope>-<主题>`，kebab-case，≤48 字符，如 `feat/editor-autosave`、`fix/search-empty`。
- 禁止 `dev`、`test`、`tmp`、`wip`、日期、人名、中文。

## 提交信息

格式 `type(scope): 中文主题`。

- 主题 ≤50 字符，祈使句，不加句号，说清做了什么。
- type：`feat` 新功能，`fix` 修缺陷，`rules` 规则与 skill，`docs` 说明文字，`refactor`，`perf`，`test`，`build`，`ci`，`chore`，`revert`。
- scope：`core`、`store`、`ui`、`app`、`skills`、`ci`；跨模块时省略。
- 正文 ≥3 行才写，写清为什么改、影响面、怎么验证。
- footer：`Refs: #12`、`BREAKING CHANGE: 说明`。

```text
feat(store): 保存前先写临时文件并原子替换

原地重写在断电时会留下半个文件。改为写 .tmp、flush 后 rename，
启动时清理残留临时文件。
```

错误示例：`update code`、`feat: 修 bug 并重构存储层`。

## 粒度

- 一次提交一个目的；拆开后每个提交都能独立通过门禁。
- 纯格式化单独提交 `chore(fmt): …`。
- 不提交构建失败的半成品、生成物、调试输出、临时脚本。

## 门禁

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

三条全绿才算完成；本地与 CI 同一套命令。禁止用 `allow` 或 `--cap-lints` 压警告。

## 合并与 PR

- squash merge 进 `main`，保持线性；禁止 merge commit；禁止 force-push `main`。
- 必须开 PR：`feat`、`refactor`、`perf`、依赖或数据格式变更，自审也要开。
- 可直接合并：`rules`、`docs`、`chore`、`ci` 且不碰代码。
- PR 描述：目的、做法、影响面、验证方式、界面截图（界面改动）。

## 版本与发布

- SemVer；`0.x` 期间 minor 允许破坏兼容。
- 流程：门禁 → 写 CHANGELOG（中文，按 新增／变更／修复／移除）→ tag `v0.1.0` → 构建 Windows x64 → 发布。

## 禁止提交

`target/`、密钥与 token、个人绝对路径、>5MB 文件、无关格式化。

