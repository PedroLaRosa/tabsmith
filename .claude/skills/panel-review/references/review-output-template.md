# Review output template

Use this **exact format** when presenting synthesized findings — it is the **canonical skeleton:
copy its structure and fill it in**, do not re-derive a layout. Findings are grouped by severity, not
by reviewer.

**IMPORTANT:** Use pipe-delimited markdown tables (`| col | col |`). Never use ASCII box-drawing.

**IMPORTANT:** Escape literal pipe characters in cells. Any `|` inside a finding title, issue
description, code snippet, regex, or delimited-string example (e.g. a cache key like `userId + "|" +
role`) must be written as `\|`, so column boundaries are set only by unescaped pipes. An unescaped
pipe splits the cell across columns and corrupts the row's `Reviewer` and `Confidence` values (and
`Route` in the Actionable Findings table).

## Example

```markdown
## Panel Review Results

**Scope:** merge-base with the review base -> working tree (14 files, 342 lines)
**Intent:** Add a "My Courses" library view with enrollment-scoped course listing
**Mode:** interactive

**Panel:** correctness, testing, maintainability, security, api-contract, frontend-async

- security -- new endpoint returns enrollment-scoped data from a user-supplied id
- api-contract -- new GET /api/my-courses route with a response DTO
- frontend-async -- use-my-courses hook refetches on a changing dependency

### Applied (safe, verified)

| #   | File                                  | Fix                                                                            | Reviewer |
| --- | ------------------------------------- | ------------------------------------------------------------------------------ | -------- |
| 6   | `use-my-courses.test.ts:40`           | Added the missing test for the empty-library branch                            | testing  |
| 7   | `my-courses.controller.ts:88` (+test) | Scoped the count query to the current user (security-posture — verify in diff) | security |

Validation: my-courses tests 11 -> 13; `nx test services` 214 pass, lint clean.
Committed: `fix(review): cover empty-library branch + scope count query` (tree was clean before review).

### Triage Groups

| Group                      | Findings | Context                                             | Preferred Resolution                                                          | Why                                                                        |
| -------------------------- | -------- | --------------------------------------------------- | ----------------------------------------------------------------------------- | -------------------------------------------------------------------------- |
| Library result-set scaling | #2, #3   | Both stem from loading every enrollment in one pass | Settle the pagination contract first (#3), then page the query behind it (#2) | One page/cursor decision fixes the memory bound and the API shape together |

### P0 -- Critical

| #   | File                          | Issue                                          | Reviewer | Confidence |
| --- | ----------------------------- | ---------------------------------------------- | -------- | ---------- |
| 1   | `my-courses.controller.ts:42` | User-supplied id in lookup, no ownership scope | security | 100        |

- **#1** — `findById(params.id)` on the library path has no `userId` scope, so any signed-in learner can read another learner's library. Scope the lookup to the current user, mirroring `courses.controller.ts:38`.

### P1 -- High

| #   | File                       | Issue                                          | Reviewer                  | Confidence |
| --- | -------------------------- | ---------------------------------------------- | ------------------------- | ---------- |
| 2   | `my-courses.service.ts:87` | Loads all enrollments into memory -- unbounded | performance               | 100        |
| 3   | `my-courses.service.ts:91` | No pagination contract                         | api-contract, performance | 75         |

- **#2** — `.findMany(...)` materializes the full result set; a large account exhausts memory. Page the query or stream it.
- **#3** — the endpoint returns every row in one response; it needs a cursor/page contract before launch. Design decision — see Actionable Findings.

### P2 -- Moderate

| #   | File                       | Issue                                           | Reviewer    | Confidence |
| --- | -------------------------- | ----------------------------------------------- | ----------- | ---------- |
| 4   | `my-courses.service.ts:45` | No error handling for a failed enrollment fetch | correctness | 75         |

### P3 -- Low

| #   | File                   | Issue                                       | Reviewer        | Confidence |
| --- | ---------------------- | ------------------------------------------- | --------------- | ---------- |
| 5   | `use-my-courses.ts:12` | Status derivation could use an early return | maintainability | 75         |

### Actionable Findings

| #   | File                          | Issue                                            | Route                               | Notes                                                     |
| --- | ----------------------------- | ------------------------------------------------ | ----------------------------------- | --------------------------------------------------------- |
| 1   | `my-courses.controller.ts:42` | Missing ownership scope on the library lookup    | `gated_auto -> downstream-resolver` | `suggested_fix` present — caller decides whether to apply |
| 3   | `my-courses.service.ts:91`    | Pagination contract needs a broader API decision | `manual -> downstream-resolver`     | Needs design input before implementation                  |

### Pre-existing Issues

| #   | File                          | Issue                                          | Reviewer    |
| --- | ----------------------------- | ---------------------------------------------- | ----------- |
| 1   | `my-courses.controller.ts:12` | Broad catch swallows a failed permission check | correctness |

### Learnings & Prior Art

- [Known Pattern] `learnings/give-your-agent-the-docs-or-it-will-reinvent-them.md` -- a prior enrollment-scoping fix applies to this endpoint

### Automation-Surface Gaps

- The new library view has no CLI/API equivalent -- agent users cannot list a library headlessly

### Release Notes

- Pre-deploy: capture baseline enrollment counts before enabling the backfill
- Verify: `SELECT COUNT(*) FROM enrollments WHERE library_synced_at IS NULL;` should reach `0`
- Rollback: keep the old listing path available until the backfill has been validated

### Coverage

- Suppressed: 2 findings below anchor 75 (1 at anchor 50, 1 at anchor 25)
- Residual risks: no rate limiting on the library endpoint
- Testing gaps: no test for concurrent library reads

---

> **Verdict:** Ready with fixes
>
> **Reasoning:** 1 critical ownership bypass must be fixed. The memory/pagination issues (P1) should be addressed before launch.
>
> **Fix order:** P0 ownership scope -> P1 memory/pagination -> P2 error handling if straightforward
```

## Anti-patterns

Do NOT produce output like this. The following is wrong:

```markdown
Findings

Sev: P1
File: foo.ts:42
Issue: Some problem description
Reviewer(s): adversarial
Confidence: 75
Route: advisory -> human
────────────────────────────────────────
Sev: P2
File: bar.ts:99
Issue: Another problem
```

This fails because: no pipe-delimited tables, no severity-grouped `###` headers, uses box-drawing
horizontal rules, no numbered findings, no `## Panel Review Results` title, and the verdict is not a
blockquote. Always use the table format above. When a finding needs more than fits a terse `Issue`
cell, put the depth in the keyed detail list under the table (`- **#N** — …`) — never expand it into
`Field:`-prefixed blocks.

## Formatting rules

- **Pipe-delimited tables** for findings — never ASCII box-drawing or per-finding horizontal-rule
  separators (the report-level `---` before the verdict is still required).
- **Escape literal `|` in cells** — any `|` inside a title, description, snippet, regex, or
  delimited-string example must be `\|`. Unescaped pipes corrupt the `Reviewer` and `Confidence`
  columns (and `Route` in Actionable Findings). Watch cache-key delimiters, regex alternations, and
  logical-OR operators quoted inside findings.
- **Severity-grouped sections** — `### P0 -- Critical`, `### P1 -- High`, `### P2 -- Moderate`,
  `### P3 -- Low`. Omit empty levels.
- **Stable sequential numbers** — assign finding numbers once after sorting, continue them across
  severity sections, and reuse the same numbers when findings reappear in Actionable Findings. Never
  restart at `1` per severity or per route bucket.
- **Always include file:line** for code-review issues.
- **Reviewer column** shows which persona(s) flagged the issue. Multiple reviewers = cross-reviewer
  agreement.
- **Confidence column** shows the anchor as an integer (`50`, `75`, `100`). Never a float.
- **No `Route` column in the per-severity tables** — the synthesized route
  (`<autofix_class> -> <owner>`) appears only in the Actionable Findings table and the `mode:agent`
  JSON. The severity tables are 5 columns: `# | File | Issue | Reviewer | Confidence`.
- **Detail line (per finding, as needed)** — keep the `Issue` cell to **one short clause** (~12 words
  or fewer, no second sentence — the scannable index, not the explanation); put the full explanation
  in a bullet list immediately under the table, keyed by `#`: `- **#N** — <why it matters + concrete
fix direction>`. Add a detail line for findings whose one-liner is not self-sufficient — usually
  P0/P1; P2/P3 are typically terse-only. This keyed list is the sanctioned home for depth — never
  expand into `Field:`-prefixed blocks.
- **Header includes** scope, intent, and the panel with per-conditional justifications.
- **Mode line** — `interactive` or `agent`.
- **Triage Groups section (when groups exist)** — pipe table `| Group | Findings | Context |
Preferred Resolution | Why |` rendered after Applied and before the severity tables. The `Findings`
  cell lists `#`s (e.g. `#2, #3`); every referenced `#` must appear in a severity table. Groups are a
  triage lens — they never replace the severity tables, merge findings, or renumber them. Omit when
  `grouping:off` or no groups survived pruning.
- **Applied section (default mode only)** — when the review applied fixes, list them first, before the
  severity tables, as `# | File | Fix | Reviewer`, then a one-line validation outcome (e.g. "`nx test
services` 214 pass, lint clean") and the **commit status** — committed as an isolated review-labeled
  commit (`fix(review): …`, or the repo's nearest convention) when the tree was clean before review,
  or left uncommitted (for the user's commit) when it was already dirty. A fix spanning files is **one
  row with one `#`** (e.g. `controller.ts:88 (+test)`) — never duplicate the number. Flag
  green-but-unverifiable edits (auth/contract/concurrency) inline in the `Fix` cell, e.g.
  `(security-posture — verify in diff)`. Applied findings keep their `#` and appear only here, not in
  the severity tables. Omit in `mode:agent` and when nothing was applied.
- **Actionable Findings section** — include when the actionable queue is non-empty.
- **Pre-existing section** — separate table, no confidence column (informational).
- **Learnings & Prior Art section** — results from `prior-art-researcher`, with links to `learnings/`
  or archived `openspec/changes/` files.
- **Automation-Surface Gaps section** — results from `automation-surface-reviewer`. Omit if none.
- **Release Notes section** — key go/no-go items from `release-readiness-checker`. Omit if it did not
  run. Schema drift surfaces as `data-migration` findings — no separate section.
- **Coverage section** — suppressed count, residual risks, testing gaps, failed reviewers.
- **Summary uses blockquotes** for verdict, reasoning, and fix order.
- **Horizontal rule** (`---`) separates findings from the verdict.
- **`###` headers** for every section — never plain-text headers.

## Agent mode (JSON)

When `mode:agent` is active, **do not** emit the markdown report above. Emit **one parseable JSON
object** as the primary response and write the same payload to `review.json` under
`/tmp/panel-review/<run-id>/`.

The contract is defined in SKILL.md under **"JSON output (`mode:agent` only)"**. Minimum fields:
`status`, `verdict`, `scope`, `intent`, `reviewers`, `findings`, `actionable_findings`,
`artifact_path`, `run_id`.

Key differences from the interactive format:

- **No pipe tables** — findings are JSON arrays with merged fields (`#`, `title`, `severity`, `file`,
  `line`, `confidence`, `autofix_class`, `owner`, `suggested_fix`, `why_it_matters`, `evidence`,
  `reviewers`, etc.).
- **`actionable_findings`** — the subset for caller apply workflows (`gated_auto` / `manual` with
  `downstream-resolver`).
- **`triage_groups`** — the Triage Groups section serialized as `{title, findings: [<#s>], context,
preferred_resolution, why}` objects. Groups span the full finding set — a triage lens, not an apply
  queue — so a caller must intersect each group's `findings` with `actionable_findings` before
  applying; the apply handoff stays `actionable_findings`. Empty when `grouping:off` or no groups.
- **No `applied_fixes` and no Applied section** — `mode:agent` does not apply; the caller does.
  Applied work surfaces only in default-mode markdown. The handoff is `actionable_findings`.
- **Failure/degraded paths** — `{"status":"failed","reason":"..."}` or `"status":"degraded"` with a
  reason; never mix markdown tables into the JSON response.
- **Stable `#`** — the same numbering as Phase 5 synthesis, carried in the JSON finding objects for
  downstream apply/residual tracking.
