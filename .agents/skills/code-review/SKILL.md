---
name: code-review
description: Use when reviewing a pull request or diff in Notecevia, deciding whether a change can merge, or answering review comments — maps changed paths to their owning design doc and skill, turns the three review criteria into executable checks, and separates blockers from suggestions. Not for style nits, gate reruns, or writing the fix; see ui-design, data-format, testing, and release.
---

# Code review

This skill owns the review verdict: which document governs a changed path, what blocks a merge, what must be checked by hand, and how a finding is reported. The three review criteria, the merge path, and the gates are repository rules owned by [CONTRIBUTING.md](../../../CONTRIBUTING.md) and [AGENTS.md](../../../AGENTS.md); this skill turns them into a checklist and adds the read-before-diff step, not a second copy.

## Sources of truth

| File | Owns |
| --- | --- |
| [AGENTS.md](../../../AGENTS.md) | gate commands, skill load table, layer table, red lines |
| [CONTRIBUTING.md](../../../CONTRIBUTING.md) | the three review criteria, protected main, merge path, dependency rules |
| [.github/pull_request_template.md](../../../.github/pull_request_template.md) | the sections every pull request carries; an empty section, or one ticked without a method, is a finding |
| [docs/roadmap/roadmap.md](../../../docs/roadmap/roadmap.md) | undone work and the completion criterion each item states |
| [docs/design/architecture.md](../../../docs/design/architecture.md) | layers, state ownership, load and save, crash recovery, performance budgets |
| [docs/design/format.md](../../../docs/design/format.md) | on-disk shape, version, migration, unknown content, export container |
| [docs/design/ipc.md](../../../docs/design/ipc.md) | command table, DTO shapes, error kinds, known limits |
| [docs/design/ui.md](../../../docs/design/ui.md) | theme tokens, component rules, static and dynamic rendering |
| [.agents/skills/project-rules/references/code.md](../project-rules/references/code.md) | Rust layering, error handling, naming, test placement, dependencies, budgets |
| [.agents/skills/project-rules/references/docs.md](../project-rules/references/docs.md) | prose rules for documents and skills |

## Read the owner before the diff

| Changed path | Read first |
| --- | --- |
| `crates/core` | [architecture.md](../../../docs/design/architecture.md) for the layer; [code.md](../project-rules/references/code.md) for the Rust rules |
| `crates/store` format surface: `format.rs`, `migrate.rs`, `container.rs`, `fixtures/` | [format.md](../../../docs/design/format.md) and the [data-format skill](../data-format/SKILL.md) |
| `crates/store` behavior: `index.rs`, `workspace.rs`, `trash.rs`, `volumes.rs`, `transfer.rs` | [architecture.md](../../../docs/design/architecture.md); [ipc.md](../../../docs/design/ipc.md) where a command exposes it |
| `crates/app/src/ipc`, including `dto.rs`, `error.rs` and the registration | [ipc.md](../../../docs/design/ipc.md) |
| `crates/ui`, `crates/ui/theme` | [ui.md](../../../docs/design/ui.md) and the [ui-design skill](../ui-design/SKILL.md) |
| `docs/design` | [docs.md](../project-rules/references/docs.md); the design claim itself is a human decision |
| `AGENTS.md`, `.agents/skills` | [skills.md](../project-rules/references/skills.md) for authoring; the repository text is the rule |
| `Cargo.toml`, `Cargo.lock`, `deny.toml`, `about.toml`, `THIRD_PARTY_LICENSES.md` | [CONTRIBUTING.md](../../../CONTRIBUTING.md) for dependency rules; [release](../release/SKILL.md) for regeneration |
| `.github/workflows`, `.github/actions` | [AGENTS.md](../../../AGENTS.md) for gates; [release](../release/SKILL.md) for release jobs |

Reading from memory is the common miss: a diff shows which lines changed, not which promise they break. Name the owning file in every review comment and quote the rule applied. A behavior claim with no owning file is a question to the author, not yet a blocker.

## Blockers

CONTRIBUTING.md names three criteria. Each failure below is a blocker.

### Behavior matches the roadmap completion criterion

- The PR names the roadmap item it completes, or explains why it is needed without one. A change that implements no listed item needs a scope answer before review continues.
- The observable behavior matches the completion criterion that item states, not just its description. State what the user can now do that they could not before.
- Scope did not widen silently. A new dependency, product behavior, or UI direction needs an explicit answer; AGENTS.md makes each a red line.
- A completed item is deleted from the roadmap in the same change. The roadmap holds only undone work.

### Failure cannot lose or corrupt data

- For every new or changed write path, name the on-disk state after a failure at each step. The result must be the old file or the new file, never a mix.
- Save order stays bodies → board → orphan reclaim → index. An interrupted save must not leave an entry pointing at a missing block.
- Unknown board, entry and body fields survive load and save. A change that rebuilds a map from known fields silently drops data written by a newer build.
- A board with a newer format number is refused and never overwritten; a corrupt board is refused, not replaced by an empty one; a corrupt block loses only its body.
- Paths read from files, containers, media names and export file names are validated before use. Review the rejection branch, not only the legal input.
- A released shape change carries the format bump, a migration step and a fixture. Missing any of the three is a blocker.
- Incremental save still writes every block whose bytes the change alters.

### Rules are followed

- Dependencies point one way: core ← store, core ← app, store ← app. core touches no file, clock, randomness or UI type. The command layer converts and calls; it holds no business rules.
- Library errors stay typed, the app boundary adds context, and no `unwrap()` sits on a path reachable from user data. A panic needs a stated invariant.
- Names use the domain vocabulary; no `utils`, `helpers`, or `common` module appears.
- Documents change in the same diff: format.md for a shape change, ipc.md for a command or DTO, ui.md for a token or component rule, the AGENTS.md load table for a new skill. A rule is edited where it is owned, not copied.
- A new test fails on the base commit and passes on the head. A test that would pass before the fix guards nothing.
- The IPC contract agrees at both ends: command name, camelCase arguments, return shape, error kind, and registration in `generate_handler!` match ipc.md and the caller. `crates/ui` has no invoke calls yet; the first one makes both ends reviewable.
- A rule is not restated in a second file. If the diff copies a gate command or a merge rule, point at the owner instead.

## Manual checks

- Error path: follow one changed operation to its user-visible result — the `IpcError` kind, the message, and the state left behind. A correct error on top of a half-written set of files still blocks.
- Concurrency and async: the app holds one `Workspace` behind a `Mutex`, commands are synchronous, and `Workspace` keeps its index in a `RefCell` so it is not `Sync`. A diff adding threads, async, or a second workspace needs a reason and a test.
- Path escape: test the malicious input — `..`, an absolute path, a drive letter, a media name that is not a hash — and check that the operation refuses rather than falls back.
- Performance: when the diff touches index rebuild, list, search, open or save, ask for before and after numbers with the dataset size. Budgets and measured baselines are in architecture.md and code.md; "feels fast" is not evidence.
- Legacy data: a format change is proven by the fixture tests, not by a new test that only writes and reads current files.
- What the diff cannot show: for UI behavior, run the app with `pnpm tauri dev` and follow the steps; for a crash boundary, separate what is tested from what is argued.

## Report

- One finding per item: location (`path:line`), impact, evidence. Evidence is a test name, a command and its output, or a reproduction; a finding without evidence is a question.
- Two lists: blockers that must change before merge, and suggestions the author decides. Never bury a blocker inside a suggestion.
- Do not repeat what the gates already check: formatting, lints, a red test CI will show. Run or read the gates once, then spend attention on what they cannot see.
- Verify every review comment before agreeing to it or fixing it, your own and the author's. "Good catch" with no check is noise; a comment that does not survive the check is withdrawn.
- State what was read: the owning document and the diff. A verdict without that list cannot be reviewed.
- The review itself is the two lists above. The task report that closes it uses the four sections defined in [workflow.md](../project-rules/references/workflow.md); do not restate the format.

## Boundaries

- Not a style guide: formatting and lints belong to `cargo fmt` and `cargo clippy`. Do not spend review comments there.
- Not a gate substitute: green checks show the machine-checked rules hold. They do not clear a blocker found here.
- Not the author: do not push to the branch or rewrite the diff unless asked. Propose the change and the test.
- UI behavior and visual direction: [ui-design](../ui-design/SKILL.md).
- On-disk format, migration, container: [data-format](../data-format/SKILL.md).
- Test design, fixtures, flaky tests: [testing](../testing/SKILL.md).
- Version, changelog, tag, license regeneration: [release](../release/SKILL.md).
- Commit message, branch, pull request mechanics: [git-commit](../git-commit/SKILL.md).
- Documentation and skill prose: [docs.md](../project-rules/references/docs.md).
- Stop and ask when the change implements no roadmap item and the scope is unclear; a design document and the code disagree and neither is clearly current; a format bump, new dependency, or new UI direction is missing from the PR; or a claim cannot be verified in the available environment.

## Verification

- For every blocker, give the check that proves it: `cargo test -p notecevia-store --test storage unknown_fields_survive_a_load_save_roundtrip` for field preservation, `cargo test -p notecevia-store --test legacy_fixture` for migration and old files, `cargo test -p notecevia-store newer_format_is_refused` for the refusal, or the UI steps and observed screen for interface behavior.
- For a fixed defect, run the named test on the base commit and see it fail for the reason the PR describes, then on the head and see it pass. Record both observations.
- The review is complete when every finding names its owner, its evidence, and the change that clears it, and the author can act on every blocker without asking what was meant.
- Negative case: reviewing only the diff. A PR that moves the board write before the block writes is small, compiles and passes the tests; an interrupted save then leaves entries pointing at missing blocks. Only format.md's save-order rule makes that visible, because the diff reads as a harmless reorder.
