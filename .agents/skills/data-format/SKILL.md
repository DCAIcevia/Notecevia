---
name: data-format
description: Use when changing the on-disk board format, adding or removing block types or fields, writing a format migration, making saves crash-safe, or changing the export container — states what the file format guarantees and which edits need a human decision. Not for IPC DTOs, UI, or release tagging; see ui-design, code-review, and release.
---

# Data format

This skill owns the judgment that keeps files readable across versions: when the format number changes, what one migration step must do, what an older build does with a newer file, what a crash may leave behind, and which changes need a human decision. Field layout is a specification, not a judgment: [format.md](../../../docs/design/format.md) owns the tables, block numbers and workspace layout, and this skill maps them to the code and adds the rules the tables leave implicit.

## Sources of truth

| File | Owns |
| --- | --- |
| [docs/design/format.md](../../../docs/design/format.md) | field tables, block numbering, workspace layout, version and migration policy, unknown-content policy, container format |
| [crates/store/src/format.rs](../../../crates/store/src/format.rs) | `FORMAT`, known-key lists, `extras` collection, value conversion |
| [crates/store/src/migrate.rs](../../../crates/store/src/migrate.rs) | how a lower format number is upgraded, per board and per block |
| [crates/store/src/workspace.rs](../../../crates/store/src/workspace.rs) | save order, `write_atomic`, `recover`, loading, corruption handling |
| [crates/store/src/container.rs](../../../crates/store/src/container.rs) | container magic, version byte, segments, path validation |
| [crates/store/src/cbor.rs](../../../crates/store/src/cbor.rs) | the supported CBOR subset and fixed-width encoding |
| [crates/store/tests/legacy_fixture.rs](../../../crates/store/tests/legacy_fixture.rs) | old fixtures still load and save under the current format |
| [crates/store/tests/storage.rs](../../../crates/store/tests/storage.rs) | round trip, unknown fields, temporary files, path escape, newer-format refusal |
| [fixtures/0.1.0-alpha.1/](../../../fixtures/0.1.0-alpha.1/) and [fixtures/0.2.0-alpha.1/](../../../fixtures/0.2.0-alpha.1/) | archived released samples; read-only inputs for migration tests |
| [docs/design/ipc.md](../../../docs/design/ipc.md) | the export and import command contract |

## Version

- The code holds the number in one constant, `FORMAT` in format.rs, and writes it into every board and block file as `format`. `format_of` runs before any parser; format.md states the released number.
- Change the number when a released build writes a different shape or gives an existing value a different meaning: a field added, removed or renamed, a block type added or removed, a default or unit changed, an encoding changed. A refactor that produces the same bytes does not change it.
- An added optional field is still a shape change. Old files remain readable, but the released build now writes a shape the previous release did not; ship the bump with it.
- A bump is complete only with a migration step for every older number, a `fixtures/<version>/` sample for the release, and the old fixture tests still green. Any one missing leaves existing user files on a path nothing proves.
- Never renumber downward, reuse a retired number, or ship a changed shape under an old number. The refusal in `format_of` is what stops an older build from rewriting what it cannot read.

## Migration

- Migration runs in memory when a file is read: `migrate::board` and `migrate::block` run before the current parser, and disk changes only on a later save. A failed migration cannot overwrite the source.
- migrate.rs is not a version-indexed ladder. Both functions return the value unchanged when `found >= FORMAT`; otherwise they apply one upgrade that assumes the oldest source format (board: `tags`, `updated_at`; entry: `locked`, `visible`; then rewrite `format`). That holds only while there is one older release and every added default is idempotent.
- When adding a format, dispatch on `found`: a file at each older number must pass through every step from its version upward, in order. Extending the single branch treats every older file as the oldest, and the first non-idempotent step then rewrites data it never saw.
- Steps add keys with defaults. They do not remove a key, change a key's unit, or reinterpret its meaning; those are product decisions and need their own version discussion.
- Preserve unknown pairs through the migration. `with_defaults` and `set` copy the existing map and only append or replace; do not rebuild a map from known fields.
- After loading an old fixture and saving, the board must carry the current number and the added defaults. `legacy_fixture::format_one_goes_through_migration_and_saves_as_format_two` is that observation.
- A value already at the current number must come back unchanged; keep the property as steps are added. It is what makes re-saving idempotent (the `format_two_board_is_returned_unchanged` test in migrate.rs).
- An older build meeting a newer file: `format_of` returns `NewerFormat` before parsing, loading refuses the board, and saving refuses to overwrite it. A block is the exception: `load_bodies` skips a block that fails to decode or migrate, leaving its entry without a body. That asymmetry is why an added block type needs the bump, not only field preservation.

## Unknown content

- Unknown board and entry fields are collected into `extras` when a board is read; unknown body fields are re-read from the block file at save time (`block_extras`). All of them are appended after the known fields when written, keys and values untouched. `storage::unknown_fields_survive_a_load_save_roundtrip` proves it at all three levels.
- A new field is added in three places in one change: the matching key constant (`BOARD_KEYS`, `ENTRY_KEYS`, or the per-type body list), the writer, and the reader. A field written but missing from the key list returns as unknown, and the next save writes the same key twice.
- Unknown block types are dropped when the board is read; the next save omits the entry, and its block file becomes an orphan and moves to `.trash`. Fields survive, blocks do not.
- Do not discard extras to normalize a file. One save from an older build would erase data a newer build wrote, and the user could not tell it happened.
- Extras do not replace the version number. They cover additive fields within an accepted format; they protect neither a new block type, a changed default, nor a changed meaning.

## Write order and crash state

- `save_document` writes block bodies, then the board file, then reclaims orphan blocks, then updates the index. Keep that order: a crash before the board write leaves unreferenced blocks, never an entry pointing at a missing body.
- `write_atomic` creates `<target>.tmp` beside the target, writes, calls `sync_all`, then renames. The rename is same-directory and atomic. The parent directory is not synced, so the guarantee is "a crash cannot leave a half-written target; the latest save may be lost", not "a save survives power loss".
- The temporary name comes from `with_extension("tmp")`, so `boards/1.board` becomes `boards/1.tmp`. `recover` removes `.tmp` one level under the root and under `boards`, `blocks`, `media`, `thumbnails`, `volumes` and `shelves`; it does not recurse and does not enter `.trash` or `exports`. The recovery tests pin the non-recursive scope and the `.trash` exclusion.
- A crash between block writes and the board write leaves blocks no board references; startup does not reclaim them, the next successful save of that board does. Do not add startup deletion of block files: startup cannot know which blocks a pending save was about to reference.
- `save_changed` skips bodies that did not change. A change that makes the writer emit different bytes for an existing body type must mark those bodies changed, or the old bytes stay until each block is edited.
- Media uses the same atomic write and skips a hash that already exists. Do not rewrite a media file in place.

## Corruption

- `index.cbor` is a cache. A missing file, a decode failure, a different index format, or a `boards/` modification-time and file-count mismatch rebuilds it from board files. Losing the index costs list and search speed, not content.
- A board that cannot be parsed keeps its slot in the rebuilt index with `readable: false`; do not drop the slot. The user must still see that the board exists.
- A newer-format board does not become a broken slot: `read_board_lenient` propagates `NewerFormat`, so rebuilding fails and the error reaches the user. Listing it as broken would look like data loss; refusing it points at the upgrade.
- A board parse error reaches the caller: `read_board` propagates it and `load` fails. Only a missing file produces an empty board; a corrupt one is never replaced by an empty board.
- A corrupt block loses only its body. The entry, geometry and other blocks stay. Do not turn a block failure into a board failure.
- Index write failure during load is ignored and retried on the next open; during save it is reported. The index may be stale, user content may not.

## Container

- The container has its own magic and version byte in container.rs, independent of `FORMAT`. Do not tie the two numbers together or reuse format migration for containers.
- Layout is repeated segments of path length (`u32`), path, content length (`u64`), content, with no directory table. `decode` reads until the input is empty, so trailing bytes become a malformed segment. Changing the segment layout bumps the container version; older containers then fail as `BadContainer` unless a decode branch is added for them.
- Decode validates each path with `paths::is_inside` before returning any entry: relative only, no absolute path, no drive, no `..`. A container arrives from another machine; treat it as untrusted.
- A container holds one board with the blocks and media it references, not a workspace. Import allocates a new board number, rewrites `block_ref`, writes media by hash, and never overwrites an existing board.
- Export writes through `write_atomic` into `exports/`; that directory is not indexed. Import accepts any readable path, so do not make it depend on a previous export.

## Decisions

Ask first:

- Bump the format number or change the migration policy.
- Delete or rename a stored field, reuse a block number, or change a default or unit that existing files rely on.
- Change how unknown content is handled, or change write order, atomicity, or the crash guarantee.
- Change the container layout, version, or path rules.
- Add a block type: it adds product-facing content semantics, and old builds drop it.

Implement directly:

- Add an optional field with a default: existing files stay readable without it. Add it to the key constant, reader, writer and round-trip test, and record it so the release that ships it carries the bump, the step and the fixture.

## Boundaries

- Not for IPC DTOs, command names, or error kinds: [ipc.md](../../../docs/design/ipc.md) owns them.
- Not for HTML or PNG export layout; format.md describes those, and this skill covers the container and workspace files.
- Version number, changelog, tag, and release prep: [release](../release/SKILL.md).
- Test design or flaky tests: [testing](../testing/SKILL.md).
- Reviewing the resulting diff: [code-review](../code-review/SKILL.md).
- Stop and ask when a change would delete or reinterpret stored data, when the current number holds files written by a build you cannot identify, or when a fixture and the parser disagree. Never make a test pass by editing an archived fixture; add a new fixture.
- Close the task with the four-section report defined in [workflow.md](../project-rules/references/workflow.md); this skill does not restate the format.

## Verification

- `cargo test -p notecevia-store` — all tests pass. The compatibility evidence is named: `legacy_fixture::format_one_fixture_still_loads`, `legacy_fixture::format_one_goes_through_migration_and_saves_as_format_two`, `legacy_fixture::format_two_fixture_loads_and_saves_back_unchanged`, `storage::unknown_fields_survive_a_load_save_roundtrip`, `storage::saving_leaves_no_temporary_files_and_recover_cleans_strays`, `storage::newer_format_is_refused`.
- Two generations: the oldest fixture loads through the migration and saves under the current number; the current fixture saves back byte-for-byte identical. A release adds one fixture directory and keeps the older ones; regenerating an old fixture with current code destroys the only evidence that old user files still open.
- Negative cases: remove extras collection or writing and `unknown_fields_survive_a_load_save_roundtrip` fails; remove the `format` rewrite from `migrate::board` and `format_one_goes_through_migration_and_saves_as_format_two` fails on the saved number; accept a higher number in `format_of` and `newer_format_is_refused` fails.
- No test proves power-loss behavior. The observable proxy is no `.tmp` after a save and a planted `.tmp` removed by `recover`. Do not cite a stronger guarantee as verified.
- Each relative link in this skill resolves from the repository root; `Test-Path` returns true for every one.
