# Rust

This reference holds only the judgment needed to write Rust here: which layer a piece of logic belongs to, how a failure travels from a crate to the frontend, when a dependency earns its place, and what a change must show to be finished. The gate commands, member crates, package names and crate headers live in [AGENTS.md](../../../../AGENTS.md); the on-disk format and migration in [format.md](../../../../docs/design/format.md); the command contract and error kinds in [ipc.md](../../../../docs/design/ipc.md); test design in the [testing skill](../../testing/SKILL.md); prose in [docs.md](docs.md). Restating those lists here would make a second copy that drifts.

## Sources of truth

| File | Owns |
| --- | --- |
| [AGENTS.md](../../../../AGENTS.md) | gate commands, member crates and package names, crate headers, edition |
| [CONTRIBUTING.md](../../../../CONTRIBUTING.md) | dependency admission: crates.io only, license in the `deny.toml` allow list |
| [architecture.md](../../../../docs/design/architecture.md) | layer table, state ownership, load and save order, performance budgets |
| [ipc.md](../../../../docs/design/ipc.md) | command contract, `kind` table, DTO field shapes |
| [format.md](../../../../docs/design/format.md) | on-disk fields, format number, migration, unknown-content policy |
| [docs.md](docs.md) | comment, doc-comment and variable-name prose |

## Layering

- `crates/core` (package `notecevia-core`) owns the document model, blocks, history and use cases. It opens no file, reads no clock, draws no randomness, names no UI type, and depends on no crate at all.
- `crates/store` (package `notecevia-store`) owns workspace files, media, index, trash, volumes and export. It depends on `core` and `thiserror`.
- `crates/app` (package `notecevia`) owns the window shell and the IPC command layer. It depends on both members plus Tauri, and holds no domain rule of its own.
- Keep the arrow one-way: `app` → `store` → `core`. A reverse call is a design error, not an import to add.
- Two reasons, both practical. The GUI stack is the layer most likely to be replaced; when it changes, nothing underneath moves. And because `core` has no IO, its tests need no temporary directory and no fixture: call the function, assert the value.
- A function that needs the current time takes it as a parameter (`save_at(id, &document, updated_at)`, `purge_trash(now)`). The wall clock is read at the edges — `crates/app/src/ipc/mod.rs` and `crates/store/src/workspace.rs` — so expiry, ordering and timestamps stay testable with fixed constants.

## Errors

- Library crates return a `thiserror` enum. One variant per failure a caller must handle differently; a caller matches the variant and never parses a message.
- `anyhow` belongs at an application edge, adding what was being attempted with `.context(...)`. It does not belong in a library return type, where it erases the classification the caller needs.
- No `unwrap()` or `expect()` on a path a user can reach. A panic prints a message the user cannot act on and skips the `kind` mapping that the interface needs to show the right state.
- `panic!` is reserved for a violated invariant. The comment above it states the invariant; if the invariant cannot be written in one sentence, the code has a different problem.
- Messages are for the log and for display. The branch key is a variant or a stable `kind` string — never a substring of a message, which changes when a message is reworded.

## The IPC boundary

- `crates/app/src/ipc/dto.rs` is the only place that gives domain types a serialized shape: camelCase field names, explicit defaults, `From` impls in both directions. `core` and `store` types carry no `serde` derives. A field is added, renamed or removed here and nowhere else.
- `crates/app/src/ipc/error.rs` turns every internal failure into `IpcError { kind, message }`. `kind` comes from the table in [ipc.md](../../../../docs/design/ipc.md) and is the branch key; `message` is display-only, so the interface stays correct when a message is reworded.
- The `From<StoreError>` match has no wildcard arm. A new store variant fails the build until it is given a fixed `kind`, which is the point: a failure that reaches the frontend unclassified is a bug the compiler now refuses.
- No internal error type crosses the boundary. `notecevia_store::Error` in a command signature or a DTO hands the frontend a shape it cannot branch on.
- Split each command into a shell and a plain function. The shell resolves `AppState` and takes `tauri::State`; the plain function takes `&Workspace` plus already-converted arguments and returns `Result<_, IpcError>` (`crates/app/src/ipc/boards.rs`). Tests call the plain function; `crates/app/src/ipc/testing.rs` supplies its temporary workspace. Logic that needs a test goes into the plain function, not into the shell.

## Naming and size

- Domain words are fixed: board, note, tag, workspace, trash, volume, shelf — and the identifier uses the same word (`Board`, `Tag`, `Workspace`, `Trash`, `Volume`, `Shelf`). One word per thing across code, docs and interface copy; a second word costs a translation at every boundary.
- Booleans read as predicates: `is_`, `has_`.
- Modules split by domain (`trash`, `volumes`, `shelves`, `index`, `media`), not by shape. No `utils`, `helpers`, `common`: a module named after its shape has no boundary to push back, so it absorbs everything and its name stops meaning anything.
- 400 lines per file and 50 per function are reminders, not limits. Past one of them, ask what second job the file or function took on; splitting by job is the fix, splitting by line count is not.

## Types and serialization

- Domain types in `core` are plain Rust: no `serde`, no stringly typed fields, one type per concept. The frontend's shape is a DTO concern.
- Unknown fields survive a round trip. `crates/store/src/format.rs` collects every key it does not know into `extras` (per board, per entry, per body) and `cbor::Value::map_with_extras` writes them back after the known keys, values untouched. A newer version's field therefore survives a read and write by an older version.
- Unknown block types are skipped on read and dropped on save; a file whose format number exceeds `FORMAT` is refused whole. Both policies are decided in [format.md](../../../../docs/design/format.md) §未知内容 — do not re-decide them at a call site.
- Adding a field or a block type is a format change, not a local edit. It raises the format number, updates the fixtures and goes through the [data-format skill](../../data-format/SKILL.md).

## Gates

- Before a commit, run the three commands in [AGENTS.md](../../../../AGENTS.md) in order. CI runs the same three on Linux and Windows, so a green local run and a green build mean the same thing.
- One package or one test: `cargo test -p notecevia-store <name>`. `crates/core` is package `notecevia-core`, so `-p core` fails with a package-ID error.
- `clippy` runs with `-D warnings`: a warning fails the build. An `#[allow(...)]` needs a stated reason in the same place, or the lint is off for good and nobody remembers why.

## Performance

- A change to startup, listing, search, saving, input or memory arrives with the measurement method and before/after numbers. Without numbers the change is not done — a green test suite says nothing about a 40ms save becoming 400ms.
- Measure with `cargo test -p notecevia-store --release --test performance -- --ignored --nocapture`. It is `#[ignore]`d, so CI never runs it; release matters, and the same data set must be used for both numbers.
- The five budgets, restated once from [architecture.md](../../../../docs/design/architecture.md) §性能预算, which owns the table, the measurement caveats and the triage order: cold start ≤500ms, listing ten thousand boards ≤1.5s, searching ten thousand boards ≤100ms, input to screen ≤16ms, idle memory ≤200MB.
- Read the caveats before quoting a budget: the startup figure includes framework initialization this code cannot control, and the idle-memory definition is still open. A verdict drawn from the number alone misstates where the time goes.

## Dependencies

- If the standard library implements it reasonably, do not add the crate. Every dependency is upgraded, audited and shipped in the licence list.
- Before adding one, confirm three things: it is maintained, its licence is in the `deny.toml` allow list, and it comes from crates.io — [CONTRIBUTING.md](../../../../CONTRIBUTING.md) owns the audit. Then state the purpose, the alternative considered and the build-time or binary-size cost.
- More than 40 direct dependencies is the point to stop and discuss, not to keep adding. Past it, the update work and the licence surface grow faster than the features.
- A new dependency is a scope decision. Ask before adding it.

## Crate headers

- Every crate root carries `#![forbid(unsafe_code)]` and `#![deny(rustdoc::broken_intra_doc_links)]` before its module declarations. A new member crate copies both.
- Edition and version come from `[workspace.package]` (`edition = "2024"`). A member crate inherits them through `.workspace = true` and sets neither itself.

## Boundaries

- Not the place for prose rules ([docs.md](docs.md)), format decisions ([data-format](../../data-format/SKILL.md)), test design ([testing](../../testing/SKILL.md)), review process ([code-review](../../code-review/SKILL.md)), commit shape ([git-commit](../../git-commit/SKILL.md)) or the gate and CI definitions ([AGENTS.md](../../../../AGENTS.md)).
- Stop and ask when a change needs a new dependency, a new workspace member, a public `core`/`store` API change, or an on-disk change — each is expensive to reverse.
- When this file and [AGENTS.md](../../../../AGENTS.md) or [CONTRIBUTING.md](../../../../CONTRIBUTING.md) disagree, the repository file wins and this file is corrected in the same change.

## Verification

Run all three; each must print no failure.

- `cargo fmt --all --check` — exits 0 and prints nothing. A printed diff means the tree is unformatted; run `cargo fmt --all` and read what it rewrote before committing.
- `cargo clippy --workspace --all-targets -- -D warnings` — exits 0 with no `warning:` line. Any warning exits non-zero with the lint name and location.
- `cargo test --workspace` — every suite ends in `test result: ok.`; the ignored performance test shows as `ignored`, not as a pass.

Negative cases, each run once to see the gate work:

- Leave one unused import: clippy exits non-zero with `unused import`, which is the difference between `-D warnings` and a lint nobody reads.
- Write a doc comment linking to an item that does not exist, and the crate-level `deny(rustdoc::broken_intra_doc_links)` fails the doc build. The three gates do not build docs; CI's Linux `cargo doc --workspace --no-deps` is where it surfaces.
- Put `unsafe` in any crate with `forbid(unsafe_code)`, and compilation fails at that block. A new member crate that omits the header still passes all three gates, because no tool checks for an attribute's presence — a header missing in a new crate is found by reading it in review, which is why the header is listed in [AGENTS.md](../../../../AGENTS.md) rather than left to memory.
- Delete an `assert!` from a test and the suite stays green. The gates prove that code compiles and that tests pass, not that the tests assert anything; that gap belongs to the [testing skill](../../testing/SKILL.md).
