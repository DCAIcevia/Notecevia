---
name: git-commit
description: Use when committing or splitting changes in Notecevia, naming a branch, writing a commit message, or opening a pull request — covers Conventional Commits with a Chinese subject, one-purpose commits, and the protected-main merge path. Not for tagging or publishing; use the release skill.
---

# Commit and pull request

This skill owns the shape of one commit, the branch name, the message, and the pull request body. It does not own the repository process: the protected branch, the gate commands, and the merge path live in [CONTRIBUTING.md](../../../CONTRIBUTING.md) and [AGENTS.md](../../../AGENTS.md), and are linked here, not copied.

Read the repository's own practice before writing the first commit. `git log --oneline -10` shows the types and scopes in use, and history wins wherever it disagrees with this skill.

## Sources of truth

- [CONTRIBUTING.md](../../../CONTRIBUTING.md) — protected `main`, branch shape, commit format, merge path, review.
- [AGENTS.md](../../../AGENTS.md) — the three gate commands and the skill load table.
- [.github/pull_request_template.md](../../../.github/pull_request_template.md) — the sections every pull request carries.
- [release skill](../release/SKILL.md) and [code-review skill](../code-review/SKILL.md) — the work this skill hands off.

## The repository decides first

`main` is ruleset-protected: no direct push, no force push, no merge method other than a merge commit, and merging requires the required checks to pass. Those are GitHub's constraints, not preferences — a direct push is rejected, so there is one path and no shortcut around it.

There is no commitlint config and no `commit-msg` hook in this repository. Nothing validates the message mechanically; a human does, so the format below is the only check that will happen.

## Before committing

- `git status --porcelain` for what changed, `git diff --staged` for what is about to be committed. When both are empty there is nothing to commit: say so and stop. Do not create an empty commit to signal that a task finished.
- Check `git diff --staged --name-only` for `.env`, keys, certificates, and credential files. `.gitignore` lists none of them, so nothing stops an accidental `git add`; inspection is the only guard. Report such a file instead of committing it.
- `.gitattributes` routes `*.png` and `*.vectorcraft` through Git LFS; the history stores the pointer, not the bytes. Work around the filter only with the user's agreement, and say so in the report.

## One commit, one purpose

A commit holding two purposes cannot be reverted, reviewed, or bisected on its own. When the index has collected unrelated work:

- Unstage the other purpose with `git reset -q -- <paths>`, then stage this one with `git add -- <paths>`.
- Quote globs (`git add -- '*.md'`). In a POSIX shell an unquoted pattern is expanded at the current depth, and files in nested directories are silently skipped.
- Do not use `git add -p` or `git add -i`. Both wait on stdin and hang an agent that has no one to answer.
- Re-run `git diff --staged --name-only` once more. A path you cannot explain in one sentence means the commit is still mixed.

## Message

`<type>(<scope>): <subject>`, with a Chinese subject. The subject is imperative, carries no trailing period, and stays within 50 characters.

```text
feat(store): 保存前先写临时文件再原子替换
```

- Body: why the change exists, what it affects, how it was verified. One `-m` per paragraph; for long text write a file and use `git commit -F <file>`.
- Footer: `Token: value` on its own line after a blank line, with hyphens instead of spaces inside the token — the only shape git reads back as a trailer.
- Breaking change: `!` before the colon, or a `BREAKING CHANGE:` footer.

Types: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`, `revert`, plus this repository's `rules`.

- `rules` covers `AGENTS.md`, `.agents/`, and anything else that changes what an agent must do. It is not `docs` (text written for a human reader) and not `chore` (tooling): reading the history for `rules` is how constraint changes get audited later.

Scopes: `core`, `store`, `ui`, `app`, `skills`, `ci`. Omit the scope when the change crosses modules.

## Branch

[CONTRIBUTING.md](../../../CONTRIBUTING.md) owns the `<type>/<scope>-<subject>` shape. This skill fixes the rest: kebab-case, at most 48 characters, one purpose per branch.

- Never `dev`, `test`, `tmp`, `wip`, a date, a person's name, or Chinese. These names say nothing at review time and outlive the work they were meant to hold.
- One milestone, one pull request: accumulate on the branch until the milestone's exit criteria are met, then merge. A pull request per fix buries the milestone behind its own bookkeeping.

## Pull request

The template's headings are literal — 改动, 验证, 风险. Write the description under them in Chinese; do not rename, reorder, or translate them.

- 验证 is the section a reviewer re-runs. Tick each gate and, for the manual check, state what was done. An unticked box, or a tick with no method, is an unfinished pull request.
- 风险 carries the impact scope and what the change does not cover. An empty impact scope is only honest when nothing outside the touched files can observe the difference.
- Link the template instead of paraphrasing it: a description restating the template drifts from it within a release or two.

Path: branch → pull request → required checks green and the branch synced with `main` → merge commit. Merging past a red required check is not a judgment call. With a single maintainer, no external approval is required ([CONTRIBUTING.md](../../../CONTRIBUTING.md)).

## Safety

- Do not change git config — `user.*`, `core.*`, aliases — to make a command pass.
- Do not pass `--no-verify`. The only installed hooks are Git LFS's, so skipping them lets a push land without uploading the objects it references.
- `git commit --amend` is fine before the branch is pushed. After a push, fix forward with a new commit instead of rewriting history.
- Do not run `reset --hard`, `rebase`, or any history rewrite on a branch you were not asked to repair, and never force-push `main`.
- Stop and ask when: the only way out is a history rewrite, a push would delete someone's branch, or a file that looks like a secret has already been staged.

## Report

After committing, `git log -1 --stat` shows exactly the intended paths and `git status --porcelain` shows what is left behind. Write the report in the four-section format defined in [workflow.md](../project-rules/references/workflow.md).

## Boundaries

- Tagging, version bumps, changelogs, publishing: [release skill](../release/SKILL.md).
- Reviewing a diff or a pull request: [code-review skill](../code-review/SKILL.md).
- The gate commands, the four release-prep steps, and the merge rules belong to CONTRIBUTING and AGENTS. Do not restate them here, and do not reword them in a commit without saying so.
- Commit and pull request prose beyond the format rules: [docs.md](../project-rules/references/docs.md).
- This skill does not decide what to build or how to slice a feature. Feature scope is a question for the user.

## Verification

- Run the three gates named in [AGENTS.md](../../../AGENTS.md). All three exit 0 and `cargo clippy` prints no `warning:` line.
- `git log -1 --stat`: the listed paths match the purpose stated in the subject.
- `git status --porcelain`: empty, or only files deliberately left uncommitted.
- Negative case: stage an unrelated file and commit. `git log -1 --stat` then lists a path the subject does not explain — that listing is the split signal, and catching it before the push is the whole point of the check.
