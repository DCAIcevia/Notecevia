---
name: testing
description: Use when writing, changing, or reviewing Rust tests and fixtures in this repository — decides what a test must prove, where it runs, and how fixtures, clocks and paths stay deterministic. Not for the gate commands or the on-disk format; those live in AGENTS.md and the data-format skill.
---

# Testing

This skill owns one question: what a test here has to prove, and what makes that proof repeatable. It does not own the gate commands, which [AGENTS.md](../../../AGENTS.md) defines, the bytes on disk, which [format.md](../../../docs/design/format.md) defines, or where logic belongs, which [code](../project-rules/references/code.md) defines. It applies those rules to a test and adds the judgment they do not contain.

## Sources of truth

| File | Owns |
| --- | --- |
| [AGENTS.md](../../../AGENTS.md) | gate commands, single-package test form, package names |
| [crates/store/tests/support/mod.rs](../../../crates/store/tests/support/mod.rs) | the temporary workspace every store test uses |
| [crates/store/tests/performance.rs](../../../crates/store/tests/performance.rs) | the budget harness, its data set and its verdict lines |
| [format.md](../../../docs/design/format.md) §版本与迁移, §未知内容 | what a fixture pins, what a format change must touch |
| [ci.yml](../../../.github/workflows/ci.yml) | where tests run: Ubuntu and Windows, `--locked` |
| [data-format](../data-format/SKILL.md) | format changes, migration, archived fixtures |
| [code](../project-rules/references/code.md) | layering and test placement in the crate |

## What earns a test

- Give every public `core` function at least one case. `core` keeps its tests in the same file under `#[cfg(test)] mod tests` (`board.rs`, `arrange.rs`, `history.rs`, `volume.rs`); the function needs no file, clock or interface, so there is nothing to arrange and no excuse to skip it.
- Every layer crossing gets an integration test under `crates/store/tests/`. Four data paths matter more than the rest, because breaking one loses user data:
  - atomic write — `saving_leaves_no_temporary_files_and_recover_cleans_strays`, `recover_only_cleans_the_root_and_the_four_data_directories`;
  - path escape — `load_follows_block_ref_and_refuses_escapes`, `media_names_that_are_not_hashes_are_refused`;
  - unknown content preserved — `unknown_fields_survive_a_load_save_roundtrip`, `unknown_block_types_and_fields_do_not_break_loading`;
  - trash restore — `a_deleted_note_comes_back_with_its_block`, `restore_refuses_when_the_number_is_taken_again`, `orphan_block_entries_are_not_offered_for_restore`.
- A defect fix ships with the test that reproduces it. The test is the part that stops it from coming back; the fix alone does not.
- A test that cannot fail is not a test. If deleting its `assert!` leaves the suite green, it proved nothing.
- The suite is already split by concern: `storage` for format read and write, `trash`, `volumes`, `index_cache` for rebuild, `index_transfer` for list, search and container, `media_library`, `incremental_save`, `legacy_fixture` for archived data, `performance` for budgets. A new case goes into the file whose concern it shares, not into a new one.

## Who owns the directory

- One `TempWorkspace::new(name)` per test. The name carries the process id and a counter, so tests run in parallel and in any order.
- Never point a test at the real user directory and never read or set `NOTECEVIA_HOME`: a test that depends on the environment passes or fails with the machine.
- `Drop` deletes the tree, so a panicking test still cleans up. Do not delete by hand mid-test; the next assertion then fails for the wrong reason.
- `outside_path()` builds a sibling path outside the workspace root for the escape case. Keep it outside — copying it inside would test nothing.
- Each test file declares `mod support;` itself. The helper carries `#![allow(dead_code)]` because one file uses a subset of it; adding an unused helper is not a warning.

## Determinism

- No network, no system clock, no user directory, no fixed absolute path.
- Time enters as a parameter: `save_at(id, &document, updated_at)`, `purge_trash(now)`. Pass a constant — `2_000_000_000` in the trash expiry test, `1_767_225_600` when writing a fixture back, `BASE_TIME = 1_700_000_000` in the performance harness.
- The repository has no injected clock type. A test that observes expiry through `Workspace::open` reads the same wall clock the store reads (`now_seconds()`) and derives entry names from it (`{now - 31 * DAY}`), then asserts a count rather than an absolute date. Follow that shape instead of asserting a calendar value.
- Ids come from `next_board()` and the document under test, not from a timestamp. A test that derives an id from the clock fails on the next machine.
- Leave no state behind. Two tests that need the same fixture each copy it into their own workspace (`copy_dir`); sharing one root makes the second test's result depend on the first one's order.

## Fixtures are a contract

- `fixtures/<version>/` holds one archived workspace per released version ([format.md](../../../docs/design/format.md) §版本与迁移). It is the only place where "data written by an older release still opens" is proved.
- `legacy_fixture.rs` resolves a fixture from `CARGO_MANIFEST_DIR/../../fixtures/<version>`, copies it into the temp workspace, and asserts the repository copy still carries its original format number. Fixtures are read-only input; a test that rewrites one in place destroyed the contract.
- Keep two generations paired: `0.1.0-alpha.1` is format 1 and must migrate, `0.2.0-alpha.1` is format 2 and must round-trip byte for byte. Together they prove migration happens once and only once.
- A format change updates the fixture in the same change, or the next migration test silently covers nothing. The format number, the fixture and the migration step move together — hand the change to the [data-format skill](../data-format/SKILL.md).

## Proving a regression

- Write the assertion first and watch it fail, or revert the fix and watch the new test go red. A test never seen failing may be passing for the wrong reason.
- Assert external state: bytes on disk (`fs::read` of a board file), returned values, index contents, error variants. Not private fields, not call counts, not the order of internal steps.
- Byte equality is the strongest assertion available: `format_two_fixture_loads_and_saves_back_unchanged` compares the written files against the archived ones, so any extra field, reordered key or changed number shows up.
- Name the test after the behaviour it protects (`unknown_volume_fields_survive_a_rewrite`, `a_page_lives_in_at_most_one_volume`). The name is the contract; a comment explaining a vague name means the test is trying to cover two things.

## Performance

- `crates/store/tests/performance.rs` is a harness, not a gate. It is `#[ignore]`d, so `cargo test --workspace` never runs it; it prints one line per budget with `预算内` or `超预算`.
- Its assertions are about correctness (`boards.len() == 10_000`, `latest_id == BOARDS`), not about time. A slow machine prints an over-budget line and leaves the suite green — that is deliberate.
- When the numbers look wrong: run it alone with `--release`, on an idle machine, with the same data set, and compare against the previous number from the same machine. Report the machine with the number.
- Do not turn the printed verdict into an `assert!`. A CI runner's timing noise would then fail builds for reasons unrelated to the change, and the gate loses its meaning.
- Command: `cargo test -p notecevia-store --release --test performance -- --ignored --nocapture`.
- The budgets themselves live in [architecture.md](../../../docs/design/architecture.md) §性能预算, with their measurement caveats; this file does not restate the numbers, so a budget change reaches the harness and this note together.

## Two platforms

- CI runs the gate on Ubuntu and Windows ([ci.yml](../../../.github/workflows/ci.yml)), so a test that passes locally can still fail on the other platform. Both must pass before a change is done.
- `write_atomic` writes `path.with_extension("tmp")` beside its target, calls `sync_all`, then renames. The temporary file must sit in the same directory (a rename across volumes fails) and must keep the `.tmp` extension, which is what `recover()` sweeps. Inventing a different temporary suffix hides it from recovery.
- `recover()` clears one level in the root and the four data directories only. A `.tmp` left deeper is not cleaned and will be read by the next assertion.
- Case: Linux is case-sensitive, Windows is not. Media names are matched case-insensitively (`hash_file_name` lowercases before comparing), so a test must not rely on `Boards` and `boards` being two directories.
- Escaping: include a platform-absolute reference (`C:/Windows/evil.block`) beside the `../` one. The `..` case alone says nothing about a Windows absolute path.
- Long paths: Windows caps a path at 260 characters without long-path support, and the temporary root differs per machine. Keep directory names short rather than nested deep.

## Boundaries

- Not for the gate commands, the CI matrix or the commit shape: [AGENTS.md](../../../AGENTS.md), [ci.yml](../../../.github/workflows/ci.yml) and the git-commit skill own those.
- Not for the format. Fields, the format number, migration and archived fixtures belong to [format.md](../../../docs/design/format.md) and the [data-format skill](../data-format/SKILL.md); this skill only states what a fixture proves.
- Not for judging whether an existing test earns its keep in a review: that is the code-review skill's call, with these rules as input.
- Stop and ask when a test needs a new fixture version, a new dependency (a time or randomisation crate), or a change to a public `core`/`store` API made only to be testable. Each of those is a decision above a test.

## Verification

- `cargo test -p notecevia-store` — every suite ends in `test result: ok.`, and `ten_thousand_boards_are_measured` shows as `ignored`, not as a pass.
- `cargo test -p notecevia-core` — the in-module unit tests run and print no temporary-directory path.
- A single case runs by name: `cargo test -p notecevia-store a_deleted_note_comes_back`.
- The gate is the three commands in [AGENTS.md](../../../AGENTS.md). Running one package checks nothing about `fmt` or `clippy`.
- Negative cases, each run once to see the check work:
  - Delete one `assert_eq!` (the `purged == 1` one in the trash expiry test) and the suite stays green: deleting an assertion cannot fail a test, it only removes the proof. Restore it, then mutate the code instead.
  - Break the behaviour instead — make `purge_trash` purge every entry — and the expiry test must go red. If it stays green, the test is restating the implementation.
  - Remove `rename` from `write_atomic` and the stray-temporary-file test must go red; if the build still passes, the test asserts a count of files it never wrote.
