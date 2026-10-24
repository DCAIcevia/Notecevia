---
name: git-commit
description: 提交、分支、合并、发布前加载。规定分支命名、提交信息格式与类型、粒度、门禁、合并与 PR、版本发布。
---

# Git 与提交

## 分支

主干式，`main` 随时可构建。

可直接进 `main`：规则与 skill 修订、单文件 ≤20 行且无行为变化、构建配置微调。其余走短分支，合并后删除，存活 ≤3 天。

命名 `<type>/<scope>-<主题>`，kebab-case，≤48 字符，如 `feat/editor-autosave`、`fix/search-empty`。

禁：`dev`、`test`、`tmp`、`wip`、日期、人名、中文。

## 提交信息

`type(scope): 中文主题`。主题 ≤50 字符，祈使句，不加句号。

- type：`feat`、`fix`、`rules`、`docs`、`refactor`、`perf`、`test`、`build`、`ci`、`chore`、`revert`。
- scope：`core`、`store`、`ui`、`app`、`skills`、`ci`；跨模块省略。
- 正文 ≥3 行才写；footer 用 `Refs: #12`、`BREAKING CHANGE: 说明`。

一次提交一个目的，拆开后每个提交都能独立通过门禁。纯格式化单独提交 `chore(fmt): …`。

不提交：构建失败的半成品、生成物、调试输出、临时脚本、`target/`、密钥与 token、个人绝对路径、>5MB 文件。

## 门禁

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

三条全绿；本地与 CI 同一套；禁止用 `allow` 或 `--cap-lints` 压警告。

## 合并与 PR

squash merge 进 `main`，保持线性；禁止 merge commit；禁止 force-push `main`。

必须开 PR：`feat`、`refactor`、`perf`、依赖与数据格式变更，自审也开。可直接合并：`rules`、`docs`、`chore`、`ci` 且不碰代码。

PR 描述：目的、做法、影响面、验证方式、界面截图。

## 版本

SemVer；`0.x` 期间 minor 可破坏兼容。

流程：门禁 → CHANGELOG（按 新增／变更／修复／移除）→ tag `v0.1.0` → 构建 Windows x64 → 发布。
