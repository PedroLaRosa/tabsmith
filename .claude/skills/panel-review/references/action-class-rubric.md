# `autofix_class` rubric (for personas)

`autofix_class` describes the **intrinsic shape** of the follow-up work — it is signal, **not** an
apply gate or apply permission. In `mode:agent` the caller interprets findings and owns apply; in the
default (interactive) mode the review applies safe fixes itself by judgment (SKILL.md Phase 5c).
Either way the class informs _what to do first_ and _what to flag_ — it never mechanically decides
what gets applied.

| `autofix_class` | Meaning                                                                                                                                           |
| --------------- | ------------------------------------------------------------------------------------------------------------------------------------------------- |
| `gated_auto`    | A concrete change is proposed in `suggested_fix`. The caller may apply it after its own judgment.                                                 |
| `manual`        | Actionable work that needs a design input or decision before the code changes. Include `suggested_fix` when you can propose a defensible default. |
| `advisory`      | Report-only — a learning, a residual risk, a rollout note.                                                                                        |

## How to choose

- Prefer `gated_auto` when you can write a defensible `suggested_fix` for a localized change.
- Use `manual` when the right fix depends on product intent, architecture, or a cross-cutting
  refactor.
- Use `advisory` when nothing breaks if it is left unfixed but the observation still has value.
- Do **not** emit `safe_auto` — the caller decides what to apply; reviewers classify and propose.

## The `owner` field

| `owner`               | Meaning                                            |
| --------------------- | -------------------------------------------------- |
| `downstream-resolver` | The caller or a human should act after the review. |
| `human`               | Judgment is required before any implementation.    |
| `release`             | Operational / rollout follow-up.                   |

Do not use `review-fixer`.
