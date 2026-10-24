---
name: git-commit
description: 提交、拆分改动、写提交信息时加载。含 Conventional Commits 格式、类型、暂存、分支、门禁、合并、版本与安全。
---

# Git 提交

基线：Conventional Commits 1.0.0，模板取自 awesome-copilot 的 git-commit（MIT）。仓库自身规则优先于本 skill。

## 提交前

1. 先读仓库规则：`commitlint` 配置、`CONTRIBUTING.md`、`git log --oneline -10`。冲突时以仓库规则为准。
2. 看改动：`git status --porcelain`、`git diff --staged`、`git diff`。无改动直说，不造空提交。
3. 门禁三条全绿才提交：`cargo fmt --all --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`。

## 拆分与暂存

一个提交一个目的。暂存区混了多个目的，先 `git reset -q -- <其余文件>`，再 `git add -- <本目的文件>`。

- glob 加引号：`git add -- '*.test.*'`。
- 禁 `git add -p`、`git add -i`，它们等待人工输入。
- 冲突解决后用 `git add --resolved`。
- 提交前用 `git diff --staged --name-only` 复核；禁提交 `.env`、密钥、证书。

## 信息格式

`<type>(<scope>): <主题>`，加可选正文与 footer。

- 主题祈使句，不加句号，≤50 字符。
- 中文主题，`type` 与 `scope` 用英文。
- 正文写做了什么与为什么；每段一个 `-m`。长正文写临时文件，用 `git commit -F <file>`。
- footer 唯一被 git 识别为 trailer 的写法是 `Token: value`；token 内用 `-` 代替空格，如 `BREAKING-CHANGE`。

## 类型

`feat`、`fix`、`docs`、`style`、`refactor`、`perf`、`test`、`build`、`ci`、`chore`、`revert`，另加本项目 `rules`，用于规则与 skill 变更。

## scope

`core`、`store`、`ui`、`app`、`skills`、`ci`；跨模块省略。

## 破坏性变更

`!` 置于冒号前，或加 `BREAKING CHANGE:` footer。

## 分支

主干式，`main` 随时可构建。可直接进 `main`：规则与 skill 修订、单文件 ≤20 行且无行为变化、构建配置微调。其余走短分支，命名 `<type>/<scope>-<主题>`，kebab-case ≤48 字符，存活 ≤3 天。

禁 `dev`、`test`、`tmp`、`wip`、日期、人名、中文。

## 合并

squash merge 进 `main`，保持线性；禁 merge commit；禁 force-push `main`。

必须开 PR：`feat`、`refactor`、`perf`、依赖与数据格式变更。可直接合并：`rules`、`docs`、`chore`、`ci` 且不碰代码。

## 版本

SemVer；`0.x` 期间 minor 可破坏。流程：门禁 → CHANGELOG（新增／变更／修复／移除）→ tag `v0.1.0` → 构建 Windows x64 → 发布。

## 安全

- 不改 git 配置。
- 不用 `--no-verify`，除非用户明确要求。
- 钩子失败就修问题重新提交，不用 `--amend`。
- 不擅自 `reset --hard`、rebase、改历史。
- 不擅自 push；不 force-push 默认分支。

## 校验

`git log -1 --stat`、`git status --porcelain`。
