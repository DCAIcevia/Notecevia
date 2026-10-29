---
name: ui-design
description: Use when adding or changing a Notecevia interface — a new view, layout, hover or focus state, animation, empty state, error copy — or when a screen looks too noisy, too cramped, or off. Gives the judgments and red flags for a healthy interface. Theme values live in docs/design/ui.md and crates/ui/theme; this is not a numeric spec.
---

# UI design

It is guidance, not a checklist. This skill answers one question — is this interface healthy — and healthy means attention lands on content, every change explains a relationship, and expression uses the smallest means. A red flag is a reason to look, not a verdict; the verdict names what the flag shows and what changed.

It is not a numeric spec. Colors, spacing, type sizes, column width, control heights, radii, shadows, durations, and z-order are design output owned by [docs/design/ui.md](../../../docs/design/ui.md) and defined as variables in [crates/ui/theme/default.css](../../../crates/ui/theme/default.css). Read them there; do not invent a value here.

## Sources of truth

| File | What it owns |
| --- | --- |
| [docs/design/ui.md](../../../docs/design/ui.md) | theme layers, component priority, static versus dynamic content, shadows and radii, the preview entry |
| [crates/ui/theme/default.css](../../../crates/ui/theme/default.css) | the variables of the one light theme: surfaces, text, lines, semantics, type, spacing, controls, radii, shadows, motion, z-order |
| [crates/ui/theme/base.css](../../../crates/ui/theme/base.css) | reset, readable base, always-visible focus, reduced-motion fallback |
| [crates/ui/components/base/](../../../crates/ui/components/base/) | the primitives: BaseButton, BaseTextField, BaseSearchField, BaseCheckbox, BaseProgress, BaseSpinner, BaseEmptyState |
| [crates/ui/preview/UiPreview.vue](../../../crates/ui/preview/UiPreview.vue) | the acceptance page that lays the theme and the primitives out |

The design doc wins wherever it disagrees with this skill; correct this skill in the same change.

## Attention

- Content first, controls quiet. When a control competes with content for space, content wins; a persistent control is small, light, and does not pull the eye.
- Emphasis is scarce: one strongest element per screen. Test: squint at the screenshot and see what lands first; it must be content.
- Separation comes from position and level, not added weight. A control boxed inside body text is the failure this prevents.

## Change

- Feedback changes a value the element already has — brightness, surface depth, a 1px shift — not a new border, a saturated color, or a glow. Two or more properties changing between states is too many.
- State never changes geometry: hover, press, selected, and focus keep the box and the radius. A hover that adds a border or grows the control shifts everything beside it.
- Animation explains a relationship: where something came from, where it went, how a state changed. Drawing attention, decoration, and hiding latency are not relationships.
- Motion is removable: with it off, no information is lost. A high-frequency interaction gets no animation; a transition seen hundreds of times a day is a tax.
- Test: remove the animation. If the interface still works, it was decoration.

## Hierarchy and space

- Hierarchy takes the smallest means that works: position, then spacing, then weight, then background, then border. Stop at the first that reads.
- Alignment means related; indent means subordinate. When those two say it, a label is noise.
- Whitespace groups; it is not decoration. A spacing change must track a change in information structure.
- Nested containers inset their radius concentrically; a surface with a layered shadow does not also draw a border. The numbers are in the design doc.
- More than two layers of container, border, or background between the window edge and the text: delete a layer.

## Color

- Color carries semantics only. The theme defines accent and danger; warning and success arrive when a consumer exists, not before.
- Color is never the only signal. State, interactivity, and required fields get a second expression: position, shape, glyph, or text.
- One appearance is one theme. Dark and high contrast are each a full redesign — surface hierarchy, hover direction, semantic contrast — not the light values shifted.
- With one theme, do not follow the system appearance. Following it halfway leaves half a screen dark and half light.
- Hover direction and semantic contrast are theme decisions; a view does not choose them.

## Text

- If layout can explain it, do not write it. Delete a sentence and the user still acts correctly: it should be gone.
- Keep text for what layout cannot say: consequence and irreversibility, the reason for a failure, a non-obvious action, an empty state.
- Start with the verb and match the result. An error is what happened plus what to do, placed at the point of failure.
- Drop possessives and decoration, and use one term per thing across the app.
- An empty state always offers the next step. Explanatory text over 20% of the interface text means layout is not doing its job.
- Interface copy lives in the frontend under `crates/ui/`; user-facing strings do not go in Rust, and a shortcut is defined once rather than per view.

## Accessibility and closing the loop

- Every action is reachable by keyboard and focus is always visible. A hit target is large enough without looking heavy.
- Body text meets the readable contrast standard; check the pair in a contrast meter. Larger type must not clip, overlap, or overflow its container.
- After any action the user can tell what happened: saved, deleted, failed, loading. A missing state is worse than a missing style.
- A wait over one second shows progress; a spinner alone does not.

## Reuse before adding

- Look in `crates/ui/components/base/` and in the theme variables before writing markup or a value. A second button is a second bug.
- A new color, radius, shadow, or duration enters `crates/ui/theme/default.css` as a variable. A hex, pixel, or millisecond literal in a view or component is the failure this prevents.
- A control used by a second view becomes a component; one that stays in one view stays there.
- [crates/ui/preview/UiPreview.vue](../../../crates/ui/preview/UiPreview.vue) shows the current theme and primitives; open the preview page before judging a change.

## Red flags

Check every line; a hit is changed, not argued.

- Two or more emphasis elements on one screen, or a control louder than the content.
- Hover or press changes geometry.
- A transition whose removal changes nothing, or animation on a high-frequency interaction.
- Three or more nested containers, a border on every element, or border plus shadow on the same edge.
- More than five peer buttons in a primary view.
- One color with two meanings, or color as the only distinguishing signal.
- Two color systems on one screen (part system-linked, part theme), or dark derived by shifting the light values.
- An empty state with no next step.
- An error message in a corner, far from where the failure happened.
- Layout breaks, clips, or overlaps at a larger font size.
- Explanatory text over 20% of the interface text.

## Boundaries

- Not for pure logic, terminal output, or data-layer behavior; the on-disk format and migrations belong to [data-format](../data-format/SKILL.md). This skill judges a surface, not what the surface reports.
- Not a source of values: a question about a specific color, spacing, size, radius, or duration is answered by the design doc and the theme variables.
- Stop and ask before changing the visual direction: a second theme, a motion system, a density change, a different surface hierarchy. Each is a new direction, not a fix.
- Copy style, punctuation, and document prose: [docs.md](../project-rules/references/docs.md); the report format and task protocol: [workflow.md](../project-rules/references/workflow.md).
- Responsive and touch behavior have no skill and no target here: the app is a Tauri desktop window with no mobile build. Treat a responsive request as a new direction and ask before designing it.
- Reviewing the change as a whole: [code-review](../code-review/SKILL.md); this skill supplies only the interface judgments.

## Verification

- Run the frontend check named in [AGENTS.md](../../../AGENTS.md) (`pnpm build`); it exits 0 with no type error. `pnpm dev` renders the preview page.
- Attach a screenshot of every changed view in the shipped theme; only one theme exists, so one set covers it. Attach the empty state as well as the populated one.
- State the keyboard path: which keys reach the changed controls, and where focus lands after each action.
- Give one verdict per red flag in the change or the pull request description: hit or not, and for a hit, what changed.
- Negative case: a hover that adds a 1px border, or raises a control by one pixel, reads fine in a screenshot and still moves the layout. Compare the element's box in devtools before and after hover — `height`, `padding`, `border-width`, and `border-radius` stay identical — and fail it under the second red flag.
