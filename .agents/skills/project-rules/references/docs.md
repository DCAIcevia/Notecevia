# Documentation

Repository docs are Chinese prose. This file owns how to write them: facts only, a closed vocabulary, four sentence shapes, a bounded structure, one home per rule, and the rule that docs move with the code. It applies to `docs/`, `changelog/`, `README.md`, `CONTRIBUTING.md`, and any user-facing Chinese text a document has to carry verbatim. It does not own the design conclusions themselves, the report format ([workflow.md](workflow.md)), or skill prose ([skills.md](skills.md)).

The failure this prevents: a document that answers "what changed and when" instead of "what is true now". A doc that records history decays by construction — git already holds the history — and every later reader has to work out which sentences still hold.

## Sources of truth

| File | What it owns |
| --- | --- |
| [AGENTS.md](../../../../AGENTS.md) | rules entry, gate commands, repository layout |
| [CONTRIBUTING.md](../../../../CONTRIBUTING.md) | branch and pull request flow, version placement, release steps |
| [docs/design/](../../../../docs/design/) | the current conclusions: architecture, format, ipc, ui |
| [docs/roadmap/roadmap.md](../../../../docs/roadmap/roadmap.md) | unfinished work and its done condition |
| [changelog/README.md](../../../../changelog/README.md) | what one changelog file contains |
| [README.md](../../../../README.md) | current stage, capabilities, data location |

These files own their rules. When one of them and this file disagree, the repository file wins and this file is corrected in the same change.

## Facts, not a log

- Write the present tense: what is true now and why. The reader is deciding what to do next, not reading a diary.
- Do not write dates, status labels, change history, authors, TODOs, plans, or guesses. Each is already in git or will be false within a week.
- A version number appears only where the version is the subject: a `changelog/<version>.md` file, the release badge and stage line in `README.md`, and the release steps in `CONTRIBUTING.md`. Everywhere else it is history.
- Delete a paragraph or a document that no longer holds. Do not mark it deprecated, do not keep it for reference, do not annotate what it used to say. The reader pays for every stale line, and git keeps the old text.
- One document, one subject. Split by the question it answers rather than growing a file; the size limits below are ceilings, not targets.

## State narration

`docs/design/` states the current conclusion only: the rule the code follows today, without the discussion that produced it, the rejected alternative, or the order it happened in.

- Unfinished work belongs to `docs/roadmap/roadmap.md`: what is missing plus a done condition. Delete the item in the change that completes it.
- A design doc may carry a not-yet sentence only when all three hold:
  - a settled design rule exists and the code does not satisfy it;
  - the sentence names the missing behavior and the layer that owes it, not merely "not implemented";
  - without it, a reader would take the rule, or a number beside it, for a measured fact about the current build.
- Such a sentence carries no date, schedule, owner, version, or next step. The change that implements the behavior deletes the sentence; one that survives the implementation is a false statement about the code.
- A target value follows the same rule: label it as a target where it is given, and replace the label with the measurement in the change that measures it.
- A design doc that accumulates gaps is a plan. Move the unfinished part to the roadmap and keep the conclusion in the design doc.
- The deletion rule is not limited to `docs/design/`: a "not yet" line in `README.md` and a roadmap item are removed by the same change that closes the gap.

## Docs move with the code

A behavior change updates the design doc that owns it in the same commit, or the doc starts lying at merge time.

| Change | Document |
| --- | --- |
| layering, load, save, crash recovery, performance budget | `docs/design/architecture.md` |
| on-disk format, migration, export container | `docs/design/format.md` |
| commands between frontend and window shell | `docs/design/ipc.md` |
| theme, component priority, static versus dynamic, shadows and radii | `docs/design/ui.md` |

- A behavior that no design doc covers adds its conclusion to the document that owns the layer. Left only in code, the next reader re-derives it.
- When a document and the code contradict, stop and report the file, the line, and the code. Do not pick a side and do not edit the document to match the code: one of the two records a decision someone made, and the report asks which one is intended.
- A doc that was right when written and is false now is corrected in the change that made it false. A doc describing behavior removed entirely loses the paragraph; it does not gain a note that the behavior was removed.

## Vocabulary

Write for someone acting on the text: one thing gets one name. Colloquial writing gives two names to one thing, and emotional words give a rule no one can check.

Banned in every document governed by this file:

| Category | Words |
| --- | --- |
| fillers | `吧` `呢` `啊` `其实` `反正` `说白了` `差不多` |
| vague | `可能` `尽量` `基本` `通常` `一些` `稍微` |
| people | `我` `我们` `你` `大家` |
| emotional emphasis | `非常` `显然` `务必` `一定要注意` |
| relative time | `目前` `暂时` `后续` `将来` `最新` |
| marketing | `赋能` `闭环` `生态` `极致` `丝滑` `强大` `优雅` |
| colloquial verbs | `搞` `弄` `整`, when used as verbs |

- Replace a banned word with the fact it hid: `可能` becomes the condition under which it happens, `尽量` becomes the rule, `目前` disappears because the present tense already means now.
- A string that must be reproduced character for character — a message, a term, a quoted rule — is the only place a banned word may stay, and it stays inside quotation marks.

## Sentence shape and punctuation

- Four shapes only: rule (must, must not, may), definition (X is Y), fact (present tense), citation (document path plus section title).
- A bullet carries one rule; a paragraph carries one claim. Questions, exclamations, bold emphasis, and emoji fit none of the four shapes.
- Chinese punctuation: `，` `。` `；` `：` `、` `（）`. ASCII punctuation appears only inside inline code, code blocks, paths, identifiers, and URLs.
- One space separates a Chinese character from an adjacent Latin letter or digit: `Vue 3`, `12 条`, never `Vue3` or `12条`.
- Emphasis uses `「」`. Bold, italics, and scare quotes are not emphasis.
- Name a term once and reuse it exactly. A synonym for a defined term breaks both the reader and the check below.

## Structure

- One H1 per document. Heading depth stops at three.
- At most 12 bullets per section; split the section instead of exceeding it.
- A table replaces a paragraph listing parallel facts. A table is read once; parallel prose is parsed every time.
- Length is a cost. A design doc stays short enough to read in one pass, and detail moves to the document that owns it.

## One home per rule

- A rule is defined once, in the document that owns it. Other documents refer to it by name, or not at all. A second copy drifts, and the reader cannot tell which copy is current.
- Cross-document references stop at three per document. Write the path and the section title, as in `docs/design/architecture.md` 的 `保存` 一节.
- `见上文`, `如下`, and `同上` are banned: they point at a position that editing moves.
- Repeat a reference only at the point of use; a document does not open with a list of everything it mentions.

## Boundaries

- Skill files, including this file's English: [skills.md](skills.md). The Chinese docs are not translated, and an English skill is not a translation of one.
- Code comments and Rust doc comments: [code.md](code.md).
- Design conclusions: `docs/design/` owns them, and this file fixes only how they are written. When a document and the code contradict, report both; do not choose one.
- Gate commands and release steps: [AGENTS.md](../../../../AGENTS.md) and [CONTRIBUTING.md](../../../../CONTRIBUTING.md). This file names them, it does not restate them.

## Verification

Run these in the repository root.

Banned words:

```powershell
$files = (Get-ChildItem docs, changelog -Recurse -Filter *.md).FullName + 'README.md'
Select-String -Path $files -Pattern '吧|呢|啊|其实|反正|说白了|差不多|我|你|大家|显然|务必|目前|暂时|后续|将来|最新|赋能|闭环|生态|极致|丝滑|强大|优雅'
```

Observation: no hit on a line the change added. The pattern omits `搞`, `弄`, and `整` because one character also matches legitimate compounds (`整理`, `完整`); judge those in context. A hit on an untouched line is debt in that file, cleared when a change edits that section.

Heading count and depth:

```powershell
Get-ChildItem docs, changelog -Recurse -Filter *.md | ForEach-Object {
  $count = @(Select-String -Path $_.FullName -Pattern '^# ').Count
  if ($count -ne 1) { "$($_.Name): $count H1" }
}
Select-String -Path (Get-ChildItem docs -Recurse -Filter *.md).FullName -Pattern '^#{4,} '
```

Observation: both print nothing. A second H1 or a `####` prints a line.

Relative links:

```powershell
$files = (Get-ChildItem docs -Recurse -Filter *.md).FullName + 'README.md'
foreach ($file in $files) {
  foreach ($m in (Select-String -Path $file -Pattern '\]\(([^)#]+)\)' -AllMatches).Matches) {
    $t = $m.Groups[1].Value
    if ($t -notmatch '^(https?:|mailto:|#)' -and -not (Test-Path (Join-Path (Split-Path $file) $t))) { "$file : $t" }
  }
}
```

Observation: no output. A renamed document leaves its old link behind.

Negative cases:

- Implement autosave and leave `自动保存` listed as not implemented in `docs/design/architecture.md`. The three checks stay green and the doc is wrong. Check with `Select-String -Path docs/design/*.md -Pattern '尚未实现|目标值'`: every remaining hit must still name something the code does not do, and the implementing change deletes its hit.
- Add `目前` to a changed line. The first command prints it; the fix is deleting the word, not rewording around it, because the present tense already carries the meaning.
- Copy a gate command into a design doc. `Select-String -Path (Get-ChildItem docs -Recurse -Filter *.md).FullName -Pattern 'cargo |pnpm '` prints the copy; the fix is naming the owning file with its section instead.
