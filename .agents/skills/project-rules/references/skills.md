# Skill authoring

## Language

Skills in this repository are written in English, whatever the language of the repository's own docs. English instruction text is followed more reliably by models, and it matches the convention every upstream skill already uses. Quote Chinese repository text only when the exact string matters, such as a message the model must reproduce.

## Structure

One skill, one topic. The directory name equals the frontmatter `name` in kebab-case. `SKILL.md` is the entry: what the skill does, when it triggers, what to read next. Put detail in `references/`; put a deterministic repeated step in `scripts/`; keep templates in `templates/` only when the output shape must be copied exactly.

```markdown
---
name: kebab-case-name
description: Use when <situation> — <what it does>. <What it does not cover, and where that lives.>
---

# Title

<Thesis. What this skill owns, what it does not, and the repository file that owns the rules it applies.>

## Sources of truth
## <Body>
## Boundaries
## Verification
```

## Description

The description is the only text always in context and the only trigger. Write it as: the situations a user would actually name, then the capability, then the fence. Models under-trigger, so name the situation even when it looks obvious.

One paragraph, no line breaks, roughly 40–60 words. Do not open with "Load this before..." alone: that states an obligation without stating a trigger.

## Size

`SKILL.md` stays within 120 lines. A single reference stays within 200; past that, split it by the question it answers and give it a table of contents. Length is a context cost, not a style preference.

## Sources of truth

A skill that restates a rule owned by `AGENTS.md`, `CONTRIBUTING.md`, or `docs/design/` creates a second copy that will drift. Point at the owning file with a relative link and add only the judgment it does not contain. When a rule must appear locally to be usable, keep it to one sentence and mark it as a pointer.

## Boundaries

Every skill ends with a Boundaries section stating: what it is not for, when to stop and ask, and which skill to hand off to. A skill without a fence absorbs tasks it should not.

## Verification

Name the command and the observation that proves the work. Where the skill guards a failure, add the negative case: what a broken implementation would do that the check must catch. A verification section that only says "review the result" has no content.

## Maintenance

Adding, renaming, or deleting a skill updates the load table in [AGENTS.md](../../../../AGENTS.md) in the same change, or the skill never loads. A skill whose practice has changed is rewritten; one that no longer matches practice is deleted rather than left to rot. Skills imported from upstream (`skill-creator`) stay as upstream supplies them and are exempt from the size limits here.
