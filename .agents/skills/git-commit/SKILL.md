---
name: git-commit
description: 在 Notecevia 仓库里提交改动、拆分提交、写提交信息或发 PR 时加载。它说明本仓库的 Conventional Commits 格式与类型、提交怎么拆、门禁怎么跑，以及 main 受保护后必须走"分支 → PR → 批准 → merge commit"的流程。只要任务最后要落成一个提交，就先读它。
---

# Git 提交

仓库由他人维护（acvrac），`main` 上有 ruleset 保护：禁止直接推送、禁止强推、必须走 PR 且至少一个批准、合并方式只允许 merge commit。这不是本项目的偏好，是仓库的硬约束——直接推会被 GitHub 拒绝，所以流程只能照它来。

## 提交前先看仓库

`commitlint` 配置、`CONTRIBUTING.md`、`git log --oneline -10`：仓库自身的约定优先于本 skill。然后用 `git status --porcelain`、`git diff --staged`、`git diff` 看清改了什么；没有改动就直说，不要造空提交。

门禁三条全绿再提交：`cargo fmt --all --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`。

## 一个提交只做一件事

混在一起的提交无法单独回滚、无法 review、出问题时也无法定位。暂存区混了多个目的时，先 `git reset -q -- <其余文件>` 把无关文件退出来，再 `git add -- <本目的文件>`。

- glob 要加引号（`git add -- '*.test.*'`），否则 shell 先展开，子目录里的文件会漏掉。
- 不用 `git add -p`、`git add -i`，它们等待人工输入，在自动化里会卡住。
- 冲突解决后用 `git add --resolved`，它只暂存真正解决了的文件。
- 提交前 `git diff --staged --name-only` 复核，确认没有 `.env`、密钥、证书混进去。

## 提交信息

格式 `<type>(<scope>): <主题>`，主题是祈使句、不加句号、≤50 字符。

```text
feat(store): 保存前先写临时文件并原子替换

原地重写在断电时会留下半个文件。改为写 .tmp、flush 后 rename，
启动时清理残留的临时文件。
```

- 中文主题，`type` 与 `scope` 用英文，工具链能解析、人也读得懂。
- 正文写清为什么改、影响面、怎么验证；每段一个 `-m`。长正文写进临时文件用 `git commit -F <file>`。
- footer 用 `Token: value`（唯一会被 git 识别成 trailer 的写法），token 里用 `-` 代替空格。

## 类型与 scope

类型：`feat`、`fix`、`docs`、`style`、`refactor`、`perf`、`test`、`build`、`ci`、`chore`、`revert`，外加本项目的 `rules`——规则与 skill 的改动不属于上面任何一类。

scope：`core`、`store`、`ui`、`app`、`skills`、`ci`；跨模块时省略。

破坏性变更在冒号前加 `!`，或写 `BREAKING CHANGE:` footer。

## 分支与合并

改动一律走短分支：`<type>/<scope>-<主题>`，kebab-case、≤48 字符，例如 `feat/editor-autosave`。分支名要让人一眼看出对应什么工作，所以禁用 `dev`、`test`、`tmp`、`wip`、日期、人名、中文。

流程：推分支 → 开 PR → 至少一个批准（acvrac）→ merge commit 合并 → 删除分支。

一个里程碑一个 PR：改动先在分支上累积，达到里程碑的退出标准再合并；不要每件事开一个 PR。仓库没有开启合并后自动删分支，所以要自己删。

推送前确认提交邮箱已关联 GitHub 账号（本仓库开启了"未归属提交需要额外批准"）。不 force-push，不改写别人的分支历史。

## 安全

不改 git 配置；不用 `--no-verify`（钩子失败说明有问题，修好再提交）；不擅自 `reset --hard`、rebase、改历史；不擅自 push，不 force-push 默认分支。

提交后用 `git log -1 --stat` 和 `git status --porcelain` 复核结果。
