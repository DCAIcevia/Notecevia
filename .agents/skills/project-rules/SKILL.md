---
name: project-rules
description: Use at the start of any task in this repository, before reading code or changing files, including small edits and design questions. Sets the task protocol — declaration, read budget, when to ask, when to stop, how to report — and routes to the prose, Rust, and skill references. Load it even when the task looks trivial.
---

# Project rules

Notecevia is an offline, single-machine desktop note board: a board is a file, content is blocks, media lives inside the file, and data never leaves the machine. Every rule below grows from that position. Tool failure must not lose data, the interface must not compete with content, and rules must stay readable by the next person or model.

Before starting, write four lines: the task and its done condition, the files you plan to read, how you will verify, and the assumptions you are making reversible. If you cannot write them, the task is not defined yet.

## Routing

| Situation | Read |
| --- | --- |
| Every task | `references/workflow.md` — read budget, when to ask, when to stop, report format |
| Writing docs, comments, README, or skill prose | `references/docs.md` |
| Writing or changing Rust | `references/code.md` |
| Creating or changing a skill | `references/skills.md` |

Domain skills trigger on their own; the load table in [AGENTS.md](../../../AGENTS.md) is the registry: `git-commit`, `ui-design`, `code-review`, `testing`, `data-format`, `release`.

## Red lines

- A task that should have loaded a skill and did not is off-rule.
- Changing feature scope, adding a dependency, or changing the visual direction: ask first.
- No completion claim without evidence.

## Boundaries

- This skill is not a style guide. Prose, Rust, and UI rules have their own homes; do not restate them here.
- When this skill and a repository file disagree, the repository file wins and this skill is fixed in the same change.
- This skill sets no domain judgment. Reviewing a diff, changing the format, or cutting a release hands off to the skill that owns it.
