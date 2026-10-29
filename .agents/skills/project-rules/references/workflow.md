# Workflow

## Read budget

A task reads at most 5 files, runs 3 searches, lists 1 directory, and reads 200 lines per call. The budget is a stop-and-report trigger, not a wall: when the task crosses layers or touches a contract (IPC, on-disk format, a public crate API), the budget applies per layer, and the declaration says so before reading starts. Report the extra reads instead of hiding them.

The failure this prevents: deciding from a corner of the code. Reading more is not the fix; reading the owning file is.

## Finding files

Search by symbol name or an error string, then read the 50 lines around each hit. Read signatures, not implementations, when only a call shape is needed. For third-party APIs, read docs.rs; a remembered signature is usually one argument or one version off.

At session start, read only the root [AGENTS.md](../../../../AGENTS.md) and the files the task names. Skip `target/`, generated output, logs, full `git log`, and recursive directory listings; "familiarize with the repository" is the most common way a task spends its budget and learns nothing.

## When to ask

Ask before deciding any of these: product behavior, on-disk data format, external interface, dependency choice, feature scope, visual direction. Each is expensive to reverse — rollback of user data, or a rewrite across two or more modules.

Ask one question, with the options and their consequences. Do not present a menu of five.

Naming, local implementation, and style details are cheap to reverse: decide, and mention the call in the report. The middle ground — default sort, shortcut, default filename — goes into the declaration's assumptions.

## When to stop

Stop and report facts plus options when:

- the same problem has failed verification twice;
- a decision from the list above appears;
- two rules contradict each other;
- the budget is spent and no reversible assumption is left.

Continuing to try turns a small error into a large one. A stop with evidence is progress; a silent guess is not.

## Report format

Close every task with four sections:

1. **Changes** — file, then one line: what changed.
2. **Verification** — command run and what it printed. Not "passed"; the observation.
3. **Risks** — what the change does not cover.
4. **Follow-ups** — decisions that need a human.

A task without section 2 is unfinished. This file is the single definition of the format; other skills point here instead of restating it.
