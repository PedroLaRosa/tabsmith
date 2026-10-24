---
name: panel-review
description: >
  Convene a panel of specialist reviewer sub-agents over a code change, gate their findings
  by confidence, and merge them into one deduplicated report. Interactively it applies the
  safe, verified fixes and commits them on a clean tree (it never pushes); with `mode:agent`
  it only reports and the caller applies. USE WHEN reviewing a branch or PR before opening it,
  after finishing a unit of work, or whenever you want a thorough multi-angle review of a diff.
argument-hint: '[mode:agent] [comment:pr] [blank for current branch, or a PR link/number/branch]'
---

# Panel Review

Reviews a change by convening a panel of specialist reviewers chosen to fit the diff. Each
reviewer runs as its own sub-agent and returns structured JSON; the orchestrator merges,
deduplicates, and confidence-gates those returns into a single report.

## When to reach for this

- Right before opening a PR.
- After completing a slice of work mid-implementation.
- Any time a diff deserves more than a quick glance.
- Standalone, or nested inside a larger flow — pass `mode:agent` when the caller wants JSON
  rather than markdown tables.

## Reading the arguments

Scan `$ARGUMENTS` for the optional tokens below. Strip every recognized token first; whatever
remains is a PR number, a GitHub URL, or a branch name.

| Token               | Example                                  | Effect                                                                                                                                                                                               |
| ------------------- | ---------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `mode:agent`        | `mode:agent`                             | **Report-only**: emit **JSON** instead of markdown tables and skip the apply step (Phase 5c) — the caller applies. Reviewer selection, merge logic, and scope rules are unchanged (see Deliverables) |
| `mode:headless`     | `mode:headless`                          | **Alias** for `mode:agent`                                                                                                                                                                           |
| `mode:report-only`  | `mode:report-only`                       | **Retired — ignored.** The old no-artifacts variant; the default is already review-without-checkout                                                                                                  |
| `base:<sha-or-ref>` | `base:abc1234`, `base:origin/main`       | Diff base on the **current checkout** (explicit; skips auto base detection)                                                                                                                          |
| `change:<path>`     | `change:openspec/changes/add-my-library` | OpenSpec change (or plan file) used for requirements verification (explicit)                                                                                                                         |
| `grouping:auto`     | `grouping:auto`                          | **Default** — build thematic triage groups when findings span distinct concerns (Phase 5, step 9b)                                                                                                   |
| `grouping:off`      | `grouping:off`                           | No triage groups: omit the section, empty `triage_groups` in JSON                                                                                                                                    |
| `grouping:always`   | `grouping:always`                        | Always build triage groups, even for a tiny review                                                                                                                                                   |
| `comment:session`   | `comment:session`                        | **Default** — keep the findings in this session (markdown report + Actionable Findings summary). No PR interaction                                                                                   |
| `comment:pr`        | `comment:pr`                             | Post the findings to the PR as inline + summary comments (default mode only; needs a resolvable PR)                                                                                                  |

**Grouping is a presentation choice, not a mode.** The `grouping:` tokens only change how the
finding set is organized for triage — never reviewer selection, merge logic, scope, or the
Phase 5c apply decision.

**Commenting is a destination choice, not a mode.** The `comment:` tokens only change _where the
findings land_ — the current session (default) or the PR. They never change reviewer selection,
merge logic, scope, or the Phase 5c apply decision.

**Alias note:** `mode:headless` normalizes to `mode:agent`; passing both is not a conflict.

**Stop without dispatching reviewers** when the arguments contradict each other:

- Two incompatible scope selectors together — e.g. `base:` **and** a PR number / branch target.
  `base:` already means "review the current checkout against this base," so a separate target is
  ambiguous.
- Two distinct `mode:` tokens beyond the `mode:agent`/`mode:headless` alias pair.
- Two distinct `grouping:` tokens (e.g. `grouping:off` **and** `grouping:always`).
- Two distinct `comment:` tokens (`comment:session` **and** `comment:pr`).
- `comment:pr` together with `mode:agent` — agent mode is a machine handoff that mutates nothing, so
  the caller (not this skill) owns posting. Combining them is contradictory.

A retired `mode:autofix` token is **not** a conflict — ignore it and proceed normally.

On a conflict, emit a single-line reason. In `mode:agent`, return `{"status":"failed","reason":"..."}`.

## Operating principles

The pipeline is identical for the default mode and `mode:agent`:

- **Apply locally, never push.** No mode pushes, opens PRs, force-pushes, or files tickets — those
  are outward steps the user owns. In the **default (interactive)** mode the review applies safe,
  verified fixes and commits them when the pre-review tree was clean (Phase 5c holds the full rule).
  In **`mode:agent`** it touches nothing — it reports, and the caller applies. The **one** outward
  action it may take is posting review comments to a PR, and only when `comment:pr` is set (see
  "Posting findings as PR comments").
- **No blocking prompts.** Never call `AskUserQuestion` or any other question tool. Infer intent,
  plan, and scope from the explicit tokens, git state, PR metadata, and conversation. Record any
  uncertainty in the Coverage section or the verdict — do not pause to ask.
- **Scope selects what to read, not what to mutate.** Never run `gh pr checkout`, `git checkout`,
  or `git switch`. Passing a PR, URL, or branch sets the **review scope** only. To review local
  uncommitted work on a feature branch, check that branch out yourself first (or stay on it) and
  pass `base:` or no target.
- **Sensible defaults.** Untracked files: review tracked changes only and list the excluded paths
  in Coverage. Requirements: use `change:` when given, otherwise discover conservatively from the
  PR body or branch keywords. A weak P2/P3 raised only by `testing` or `maintainability`: demote
  to `testing_gaps` / `residual_risks` per Phase 5.

### Posting findings as PR comments

The `comment:` token decides **where the findings land**:

- **`comment:session` (default).** Findings stay in this session — the markdown report plus the
  Actionable Findings summary (or the JSON in `mode:agent`). Nothing is posted to the PR. This keeps
  the apply-locally model intact: the user reads the report and owns every outward step.
- **`comment:pr`.** After the report is assembled (Phase 6), also post the findings to the PR — one
  top-level summary comment (verdict + the Coverage highlights) plus one inline comment per surviving
  finding, anchored to its `file:line`. Posting review comments is the **only** outward action the
  skill takes, and only when this token is set.

**`comment:pr` requires a resolvable PR.** It is valid only in default mode (never with `mode:agent`)
and only when Phase 1 found an open PR — a PR number/URL was passed, or the reviewed branch has an
open PR. If no PR resolves, do not invent one: deliver the session report as usual and note that
`comment:pr` was requested but skipped because no PR was found. Anchor inline comments only to lines
present in the PR diff; fold any finding whose line is outside the diff into the summary comment
rather than dropping it. Post through the host's review API (e.g. `gh pr review`, `gh api
repos/{owner}/{repo}/pulls/{number}/comments`).

**Comment formatting — conventional comments by default.** Lead each comment with a
`<label> [decoration]: <subject>` line (the https://conventionalcomments.org standard) whose label
matches the finding's nature, then the detail (why it matters + the concrete fix) below it:

- `issue:` for a defect or vulnerability, `suggestion:` for a maintainability or design improvement,
  `nitpick:` for a trivial preference-based remark, `todo:` for a small necessary change such as a
  missing test, `question:` when the finding needs the author's clarification. Add a `(blocking)` or
  `(non-blocking)` decoration to say whether the finding should hold up the merge — high-confidence
  defects are blocking, nitpicks and thoughts are not (e.g. `issue (blocking): scope the library
lookup to the current user`).

**Unless the project defines its own PR-comment conventions, in which case follow those.** Discover
them the same way `project-standards` discovers code standards (the governing `CLAUDE.md` /
`AGENTS.md`, plus any review/comment guide under `docs/`); the project's own convention always wins
over the conventional-comments default.

Even with `comment:pr` set, never push, open a PR, merge, or file a ticket — posting review comments
is the full extent of the outward action.

## Deliverables

| Invocation       | Output                                                                                       |
| ---------------- | -------------------------------------------------------------------------------------------- |
| **Default**      | Markdown report (pipe-delimited finding tables) + an Actionable Findings summary             |
| **`mode:agent`** | One JSON object (see "JSON output" below) + the same `/tmp/panel-review/<run-id>/` artifacts |

`mode:agent` is **report-only**: it skips the Phase 5c apply and serializes findings as JSON
rather than markdown. It changes nothing about reviewer selection, merge logic, or scope — the
JSON is the deterministic contract any programmatic caller can parse. Keep the default markdown
ASCII-safe (pipe tables, `->` not `·`, no box-drawing) so it survives every terminal.

## Quick-review short-circuit

If `$ARGUMENTS` asks for a quick, fast, or light review — and **`mode:agent` is not active** — do
not convene the panel.

**Announce the chosen path** before anything else (Quick review vs Panel review). Skip the
announcement when `mode:agent` is active.

1. **Run the harness's built-in review** (`/code-review` here) on the stripped review target, then
   stop — do not convene the panel.
2. **Exception:** if no built-in review exists, fall through to the full panel.
3. **`mode:agent` ignores this short-circuit** — always run the full panel and return JSON.

## Severity scale

Every reviewer uses P0-P3:

| Level  | Meaning                                                              | Action                 |
| ------ | -------------------------------------------------------------------- | ---------------------- |
| **P0** | Critical breakage, exploitable vulnerability, data loss/corruption   | Must fix before merge  |
| **P1** | High-impact defect hit in normal usage, or a broken contract         | Should fix             |
| **P2** | Moderate downside (edge case, perf regression, maintainability trap) | Fix if straightforward |
| **P3** | Low impact, narrow scope, minor improvement                          | Reviewer's discretion  |

## Action routing

Severity answers **urgency**. `autofix_class` and `owner` are **signals** describing the shape of
the follow-up for the caller — **not** an apply gate or apply permission. The apply decision is
judgment (Phase 5c), not a function of `autofix_class`: the default mode applies; `mode:agent`
never mutates the checkout — the caller applies. See `references/action-class-rubric.md` for the
per-persona guidance.

| `autofix_class` | Default owner                    | Meaning                                                          |
| --------------- | -------------------------------- | ---------------------------------------------------------------- |
| `gated_auto`    | `downstream-resolver` or `human` | Concrete `suggested_fix` proposed; caller applies after judgment |
| `manual`        | `downstream-resolver` or `human` | Actionable work that needs design input or a handoff             |
| `advisory`      | `human` or `release`             | Report-only — learnings, rollout notes, residual risk            |

Routing rules:

- **Synthesis owns the final route.** A persona's routing metadata is input, not the verdict.
- **On disagreement, pick the more conservative route.** A merged finding may narrow from
  `gated_auto` to `manual`, but never widen without stronger evidence.
- **Drop legacy values.** If a persona emits `safe_auto` or `review-fixer`, remap to
  `gated_auto` / `downstream-resolver` during synthesis (or drop the finding).
- **`requires_verification: true`** means any caller-applied fix needs targeted tests or a
  follow-up validation pass.

## The panel

13 reviewer personas in layered conditionals, plus 2 always-on context agents and 1 conditional
agent. A quick roster with one-line triggers follows; `references/persona-catalog.md` carries the
full per-persona selection criteria and spawn gates — read it at Phase 3.

**Always-on (every review):** `correctness`, `testing`, `maintainability`, `project-standards`,
plus the context agents `prior-art-researcher` and `automation-surface-reviewer`.

**Cross-cutting conditional (per diff):**

- `security` — auth, public endpoints, user input, permissions, secrets
- `performance` — DB queries, data transforms, caching, async/concurrent paths
- `api-contract` — routes, serializers, exported type signatures, event/DTO schemas, versioning
- `data-migration` — migration files / schema changes / backfills (see the spawn gate in Phase 3)
- `reliability` — error handling, retries, timeouts, background jobs, health checks
- `adversarial` — >=50 changed executable lines, or auth / payments / data mutations / external APIs
- `previous-comments` — a PR that already carries review comments (PR-only, comment-gated)

**Stack-specific conditional (per diff):** `frontend-async` (React effects/state, event wiring,
async UI with race potential under `projects/frontend`) and `nx-boundaries` (nx project tags and
import constraints, `project.json` target wiring, hexagonal layer leaks across domain →
application → infrastructure).

**Conditional agent (migration-specific):** `release-readiness-checker` — a go/no-go checklist
with verification queries and rollback notes when the migration gate applies and the change is
risky.

## Panel size

Every review spawns the 4 always-on personas plus the 2 context agents, then adds whichever
cross-cutting and stack-specific conditionals the diff warrants. The set right-sizes itself: a
small config tweak triggers 0 conditionals = 6 reviewers; an auth feature might add security +
reliability + adversarial = 9.

## Protected artifacts

These paths are project decision / knowledge artifacts and must **never** be flagged for
deletion, removal, or gitignore by any reviewer:

- `openspec/changes/**` — change proposals, specs, and task lists.
- `openspec/specs/**` — the captured capability specs.
- `learnings/**` — captured engineering learnings.

If a reviewer flags anything under these paths for cleanup or removal, discard that finding during
synthesis.

## Running the pipeline

### Phase 1: Determine scope

Compute the diff range, file list, and diff. Fold the git calls into as few commands as possible
to minimize permission prompts.

**If `base:` was provided (fast path):**

The caller already knows the base. Skip all base-branch detection, remote resolution, and
merge-base work — use the value directly:

```
BASE_ARG="{base_arg}"
BASE=$(git merge-base HEAD "$BASE_ARG" 2>/dev/null) || BASE="$BASE_ARG"
```

Then produce the same output the other paths produce:

```
echo "BASE:$BASE" && echo "FILES:" && git diff --name-only $BASE && echo "DIFF:" && git diff -U10 $BASE && echo "UNTRACKED:" && git ls-files --others --exclude-standard
```

This works with any ref — a SHA, `origin/main`, or a branch name. **Do not combine `base:` with a
PR number or branch target.** If both appear, stop: "Cannot combine `base:` with a PR or branch
target — `base:` already means the current checkout is the branch to review. Pass `base:` alone,
or pass the target alone and let scope detection resolve the base."

**If a PR number or GitHub URL was provided:**

Do **not** check out the PR branch. Scope comes from GitHub read APIs, plus local inspection only
when HEAD already matches the PR head.

**Skip pre-check.** Before scope detection, probe PR state:

```
gh pr view <number-or-url> --json state,title,body,files
```

Apply the skip rules in order:

- `state` is `CLOSED` or `MERGED` -> stop with reason `PR is closed/merged; not reviewing.`
- **Trivial-PR judgment**: spawn a lightweight sub-agent (`model: haiku` in Claude Code) with the
  PR title, body, and changed paths. Its task: "Is this an automated or trivial PR that does not
  warrant review? Consider dependency lock-file bumps, automated release commits, chore version
  increments with no substantive code change. When in doubt, answer no — a wrongly-skipped review
  is more costly than a needless one." If it answers yes, stop with reason `PR appears trivial /
automated; not reviewing. Run without a PR argument to review the current branch, or pass
base:<ref> if review is intended.`

When a skip rule fires, stop without dispatching. **Default mode:** emit the reason as plain text.
**`mode:agent`:** emit JSON only — `{"status":"skipped","reason":"<same message>"}`. The
**standalone**, **`base:`**, and **branch-remote** paths are unaffected. **Draft PRs are reviewed
normally.**

If nothing skips, fetch PR metadata **without checkout**:

```
gh pr view <number-or-url> --json title,body,baseRefName,headRefName,headRefOid,isCrossRepository,url,files,reviews,comments --jq '{title, body, baseRefName, headRefName, headRefOid, isCrossRepository, url, files: [.files[].path], hasPriorComments: ((.reviews | map(select(.state != "APPROVED" or .body != "")) | length) > 0 or (.comments | length) > 0)}'
```

Set `BASE:` to `pr:<number-or-url>` (a logical marker, not a git SHA). Set `UNTRACKED:` from
`git ls-files --others --exclude-standard` on the **current** checkout.

**PR scope mode.** Classify as **`local-aligned`** only when **all** of these hold; otherwise
**`pr-remote`**. A matching branch name alone is not enough — a fork PR or a stale local branch can
share a name yet point at unrelated code, and trusting the name would diff the wrong tree.

1. `git rev-parse --abbrev-ref HEAD` equals `headRefName`.
2. The PR is **not** cross-repository (`isCrossRepository` is false).
3. The PR head is contained in the local checkout: `git merge-base --is-ancestor <headRefOid> HEAD`
   exits 0 — confirming the working tree actually carries the PR head (with any unpushed local
   fixes on top) rather than an unrelated same-named branch.

- **`local-aligned`** — all three pass. Local Read/Grep/git blame against workspace files are valid
  for the PR's changed paths.
- **`pr-remote`** — any check fails. The working tree is **not** the PR head; workspace contents for
  changed paths may be stale or unrelated.

**Diff by scope mode** (never mix remote and local diffs — contradictory hunks cause false
positives):

- **`local-aligned`:** Resolve `<resolved-base-ref>` from `baseRefName` (fetch if needed). Compute
  `BASE=$(git merge-base HEAD <resolved-base-ref>)`, then `FILES:` from `git diff --name-only $BASE`
  and `DIFF:` from `git diff -U10 $BASE` (committed + staged + unstaged on the PR branch). Do **not**
  call `gh pr diff` or append remote hunks — when unpushed fixes exist, the local tree is canonical.
  Note in Coverage: `scope: local-aligned (PR; local tree diff)`.
- **`pr-remote`:** `FILES:` from the PR `files` array; `DIFF:` from `gh pr diff <number-or-url>
--color=never`. If `gh pr diff` fails, stop with an actionable error — do not fall back to checkout.

When **`pr-remote`**, before Phase 4:

1. Best-effort fetch the PR head without checkout: `git fetch --no-tags origin
<headRefName>:refs/review/pr-<number>-head`.
2. On success, set `PR_HEAD_REF=refs/review/pr-<number>-head` for reviewers and validators. On
   failure, omit it and note in Coverage — reviewers rely on diff hunks only.
3. Best-effort fetch the PR base without checkout: `git fetch --no-tags origin <baseRefName>`. On
   success, resolve a concrete SHA with `git rev-parse FETCH_HEAD` and set `PR_BASE_REF` to it — a
   **real git base ref** for file-level diffs (e.g. `data-migration` running `git diff <PR_BASE_REF>
-- projects/services/drizzle/migrations`). The `pr:<...>` marker in `BASE:` stays the scope
   marker; `PR_BASE_REF` is the diffable base. On failure, omit it and note in Coverage — schema
   drift checks fall back to diff hunks and must **not** assume `main`.
4. Include `<pr-scope-mode>pr-remote</pr-scope-mode>` and, when set, `<pr-head-ref>...</pr-head-ref>`
   and `<pr-base-ref>...</pr-base-ref>` in the Phase 4 review-context bundle.

In **`pr-remote`** mode, reviewers and Phase 5b validators must **not** Read/Grep workspace paths
for files in `FILES:` — inspect via `git show <PR_HEAD_REF>:<path>` when set, otherwise the diff
hunks only. **`local-aligned`** uses normal workspace inspection.

**If a branch name was provided:**

Substitute it as `<branch>`. Do **not** check it out.

If `git rev-parse --abbrev-ref HEAD` equals `<branch>`, use the **standalone (current branch)** path
below — same tree, explicit name; do not use a remote-only diff.

Otherwise diff the remote/local ref **without checkout**:

1. Try `gh pr view <branch> --json baseRefName,url,headRefName` — if a PR exists, prefer the **PR
   path** above (same remote diff rules).
2. Else resolve `<branch>` as `origin/<branch>` or `<branch>` after `git fetch --no-tags origin
<branch>` when needed.
3. Resolve the default base branch (same logic as standalone). Compute `BASE=$(git merge-base
<base-ref> <branch-ref>)` and `git diff -U10 $BASE <branch-ref>`.
4. If `<branch-ref>` cannot be resolved locally, stop: "Cannot diff branch `<branch>` without
   checkout. Check it out, pass its open PR URL/number, or review the current branch with `base:`."

On success, set **branch-remote scope**. The working tree is **not** `<branch>`. Include
`<pr-scope-mode>branch-remote</pr-scope-mode>` and `<branch-head-ref><branch-ref></branch-head-ref>`
in the Phase 4 bundle. Reviewers and Phase 5b validators must **not** Read/Grep workspace paths for
files in `FILES:` — inspect via `git show <branch-ref>:<path>` or diff hunks only.

```
echo "BASE:$BASE" && echo "FILES:" && git diff --name-only $BASE <branch-ref> && echo "DIFF:" && git diff -U10 $BASE <branch-ref> && echo "UNTRACKED:" && git ls-files --others --exclude-standard
```

**If no argument (standalone on the current branch):**

Apply the same base-detection logic as branch mode, using the current branch (`gh pr view
--json baseRefName,url` with no argument defaults to it).

If no base resolves, **stop**. Do not fall back to `git diff HEAD` — a standalone review without the
base would show only uncommitted changes and silently miss every committed change on the branch.

```
echo "BASE:$BASE" && echo "FILES:" && git diff --name-only $BASE && echo "DIFF:" && git diff -U10 $BASE && echo "UNTRACKED:" && git ls-files --others --exclude-standard
```

`git diff $BASE` (without `..HEAD`) diffs the merge-base against the working tree, capturing
committed, staged, and unstaged changes together.

**Untracked files:** always inspect `UNTRACKED:`. Untracked paths are out of scope unless staged.
When non-empty, list the excluded files in Coverage and continue on tracked changes only — never
stop or prompt.

### Phase 2: Discover intent

Understand what the change is trying to do. The source depends on which Phase 1 path you took:

**PR/URL mode:** the PR title, body, and linked issues from `gh pr view`. Supplement with the PR's
commit messages if the body is thin.

**Branch mode:** `git log --oneline ${BASE}..<branch-ref>` using the resolved merge-base and branch
ref from Phase 1 — use `<branch-ref>`, not the raw `<branch>` argument (a remote-only branch has no
matching local ref).

**Standalone:**

```
echo "BRANCH:" && git rev-parse --abbrev-ref HEAD && echo "COMMITS:" && git log --oneline ${BASE}..HEAD
```

Combine with conversation context (a plan summary, the PR description) and write a 2-3 line intent
summary:

```
Intent: Replace the per-tier rate lookup in enrollment pricing with a flat-rate
computation. Must not regress the tax-exempt edge cases.
```

Pass this to every reviewer. Intent shapes _how hard each reviewer looks_, not which reviewers are
chosen.

**When intent is ambiguous:** infer from the branch name, commits, PR title/body, diff, `change:`,
and conversation. Write the best-effort summary and note the uncertainty in Coverage — never block
on a clarifying question.

### Phase 2b: Locate requirements (OpenSpec change or plan)

Find the requirements source so Phase 6 can check completeness. Check these in priority order and
stop at the first hit:

1. **`change:` argument.** If the caller passed a path (an `openspec/changes/<id>/` directory or a
   plan file), use it. Confirm it exists on disk.
2. **PR body.** If PR metadata was fetched, scan the body for `openspec/changes/*` references (or a
   plan path). One unambiguous match that exists on disk -> `req_source: explicit`. Several matches
   -> `req_source: inferred` for the most relevant existing one, or skip if none clearly relate.
   Always confirm the file/dir exists — stale links in PR descriptions are common.
3. **Auto-discover.** Pull 2-3 keywords from the branch name (e.g. `ux/my-courses-opus` ->
   `my-courses`). Glob `openspec/changes/*` and filter by those keywords. Exactly one match -> use
   it. Multiple or generic-keyword matches (`fix`, `update`, `review`) -> **skip** — a wrong
   requirements source is worse than none. Zero matches -> skip.

**Confidence tagging:**

- `change:` argument -> `req_source: explicit` (high confidence)
- single unambiguous PR-body match -> `req_source: explicit`
- multiple/ambiguous PR-body matches -> `req_source: inferred`
- auto-discover single match -> `req_source: inferred`

When a source is found, read its requirements: for an OpenSpec change, read `proposal.md` (the
"What changes" / requirements) and `tasks.md` (the task checklist), plus any `specs/` deltas; for a
plain plan file, read its requirements/units section. Store the extracted requirement/task list and
`req_source` for Phase 6. Never block the review when none is found — requirements verification is
additive.

### Phase 3: Assemble the panel

Read the diff and file list from Phase 1, then read `references/persona-catalog.md`. The 4 always-on
personas and 2 context agents are automatic. For each cross-cutting and stack-specific conditional
persona, decide whether the diff warrants it. This is judgment, not keyword matching.

**File-type awareness.** Instruction/prose files (Markdown skill definitions, JSON schemas, config)
are product but do not benefit from runtime-focused reviewers. The adversarial reviewer targets
executable behavior (races, cascade failures, abuse cases). For diffs that only touch prose/config,
skip adversarial unless the prose describes auth, payment, or data-mutation behavior. Count only
executable code lines toward line-count thresholds.

**`previous-comments` is PR-only AND comment-gated.** Select it only when both hold:

1. Phase 1 gathered PR metadata (a PR was passed, or `gh pr view` returned metadata for the branch).
2. `hasPriorComments` from Phase 1 is true.

Skip it for standalone reviews with no PR, and for PRs with no feedback yet — there is nothing to
verify, and a sub-agent that returns empty findings still pays full startup cost.

**Stack-specific personas are additive when runtime behavior warrants them.** A React data-fetching
change may warrant `frontend-async`; a cross-layer move or a new lib boundary may warrant
`nx-boundaries`.

**`data-migration` spawn gate.** Select it only when the diff includes a migration or schema
artifact: `projects/services/drizzle/migrations/*`, a Drizzle schema file, a Supabase SQL migration,
or an explicit backfill/data-transform script. **Do not spawn** for model-only changes, query-only
refactors, code that references columns without a migration in the diff, or migration tests alone.

For `release-readiness-checker`, use the same migration gate **and** require a risky change
(destructive DDL, backfills, NOT NULL without default, column renames/drops).

Announce the panel before spawning:

```
Review panel:
- correctness (always)
- testing (always)
- maintainability (always)
- project-standards (always)
- prior-art-researcher (always)
- automation-surface-reviewer (always)
- security -- new route accepts a user-provided redirect URL
- frontend-async -- effect refetches on a changing dependency without cleanup
- data-migration -- adds 0020_add_index_to_enrollments.sql
- release-readiness-checker -- destructive migration with a backfill
```

This is progress reporting, not a blocking confirmation.

### Phase 3b: Locate project-standards paths

Before spawning, find the **paths** (not contents) of the standards files the `project-standards`
persona needs:

1. Use the native file-search tool (Glob in Claude Code) to find every `**/CLAUDE.md` and
   `**/AGENTS.md` in the repo.
2. Keep those whose directory is an ancestor of at least one changed file (a standards file governs
   everything below it).

Pass the resulting path list to `project-standards` inside a `<standards-paths>` block in its review
context (Phase 4). The persona reads the files itself, targeting only the sections relevant to the
changed file types — this keeps the orchestrator's work to path discovery and keeps the sub-agent
prompt lean.

### Phase 4: Dispatch the sub-agents

#### Model tiering

Three reviewers inherit the session model with no override: `correctness`, `security`, and
`adversarial`. These do the highest-stakes analysis — logic bugs, vulnerabilities, adversarial
failure scenarios — and should run at whatever capability the user configured. On Opus, they get
Opus.

Every other persona sub-agent and context agent uses the platform's mid-tier model to cut cost and
latency (in Claude Code, add `model: "sonnet"` to the `Agent` call). On a platform with no
model-override parameter, omit it — a working review on the parent model beats a broken dispatch.
Omitting the override on an Opus session silently multiplies the cost of a review several times, so
set it on every dispatch except the three above.

The orchestrator (this skill) also inherits the session model; it handles intent discovery, panel
selection, merge/dedup, and synthesis.

#### Run ID

Generate a unique run identifier before dispatching anything. It scopes every artifact file and the
final run artifact to one directory.

```bash
RUN_ID=$(date +%Y%m%d-%H%M%S)-$(head -c4 /dev/urandom | od -An -tx1 | tr -d ' ')
mkdir -p "/tmp/panel-review/$RUN_ID"
```

Pass `{run_id}` to every sub-agent so it can write its full analysis to
`/tmp/panel-review/{run_id}/{reviewer_name}.json`.

**Large shared context — pass paths, not contents.** The diff and file list go to every reviewer
and validator. When inlining them into each prompt would be wasteful (many files / a big diff),
write them once into the run dir (e.g. `full.diff`, `files.txt`) and pass those **paths** in the
diff / changed-files slots — the sub-agent and validator templates tell the child to Read a staged
path. Inline a small diff directly.

#### Spawning

Omit any permission-mode parameter on the dispatch so the user's configured settings apply.

Spawn each selected persona reviewer with `references/subagent-template.md`. Respect the harness's
active-sub-agent limit: queue the selected reviewers, dispatch as many as the harness accepts, and
fill freed slots as reviewers finish. Treat concurrency-limit spawn errors as **backpressure**, not
failure — leave the reviewer queued and retry after a slot frees. Record a reviewer as failed only
after a successful dispatch times out/errors, or when dispatch fails for a non-capacity reason.

Each persona sub-agent receives:

1. Its persona definition (identity, failure modes, calibration, suppress conditions) from
   `references/persona-catalog.md`.
2. The shared diff-scope rules from `references/diff-scope.md`.
3. The JSON output contract from `references/findings-schema.json`.
4. PR metadata — title, body, URL — in a `<pr-context>` block when reviewing a PR (empty otherwise),
   so reviewers can check code against stated intent.
5. Review context: intent summary, file list, diff, scope mode (`local-aligned` | `pr-remote` |
   `branch-remote`), and the remote head ref (`PR_HEAD_REF` or `<branch-head-ref>`) when set.
6. Run ID and reviewer name for the artifact path.
7. **For `project-standards` only:** the standards-path list from Phase 3b in a `<standards-paths>`
   block.
8. **For `data-migration` only:** the resolved review base ref from Phase 1 (`BASE:` marker) in a
   `<review-base>` block, so schema-drift checks never assume `main`.

Persona sub-agents are **read-only** toward the project: they review and return JSON. They do not
edit files or propose refactors. Their one permitted write is saving their full analysis to the run
artifact path. Read-only means **non-mutating**, not "no shell" — they may run non-mutating
inspection (`git diff`, `git show`, `git blame`, `git log`, `gh pr view`, Read/Grep/Glob). In
`pr-remote`/`branch-remote` scope, inspect changed files via `git show <remote-head-ref>:<path>` or
diff hunks — do not Read/Grep workspace paths for in-scope files. They must never edit files, switch
branches, commit, push, open PRs, or otherwise mutate state.

Each persona writes full JSON (all schema fields) to
`/tmp/panel-review/{run_id}/{reviewer_name}.json` and returns compact JSON with merge-tier fields
only:

```json
{
  "reviewer": "security",
  "findings": [
    {
      "title": "User-supplied id in enrollment lookup without ownership check",
      "severity": "P0",
      "file": "enrollment.controller.ts",
      "line": 42,
      "confidence": 100,
      "autofix_class": "gated_auto",
      "owner": "downstream-resolver",
      "requires_verification": true,
      "pre_existing": false,
      "suggested_fix": "Scope the lookup to the current user before returning the enrollment"
    }
  ],
  "residual_risks": [...],
  "testing_gaps": [...]
}
```

The artifact file **must** carry the detail-tier fields (`why_it_matters`, `evidence`); the compact
_return_ omits them. Writing the compact shape to the artifact (a common slip) silently strips the
detail that Coverage and the keyed detail lines depend on. However review context arrives — inlined,
or staged to disk — each reviewer still gets the full output contract. `suggested_fix` is optional in
both tiers; include it in compact returns when present so callers can apply fixes after review. If
the file write fails, the compact return still has everything the merge needs.

**Context agents** (`prior-art-researcher`, `automation-surface-reviewer`) are dispatched as standard
Agent calls through the same bounded scheduler. Give them the same review-context bundle the personas
get: entry mode, any PR metadata, intent summary, the review base name when known, the `BASE:`
marker, file list, diff, and `UNTRACKED:` notes. Do not invoke them with a generic "review this"
prompt. Their output is unstructured and is synthesized separately in Phase 6.

**Conditional agent** (`release-readiness-checker`) is dispatched the same way when the risky-migration
gate fires. Pass the same bundle plus the applicability reason (which migration files triggered it).
Its output is unstructured and preserved for Phase 6. Schema drift itself is the `data-migration`
persona's structured finding — not this agent's.

### Phase 5: Merge findings

Turn the reviewers' compact returns into one deduplicated, confidence-gated finding set. The compact
returns carry merge-tier fields (title, severity, file, line, confidence, autofix_class, owner,
requires_verification, pre_existing) plus optional suggested_fix. Detail-tier fields
(why_it_matters, evidence) live in the per-agent artifact files and are not loaded here.

`confidence` is one of 5 discrete anchors (`0`, `25`, `50`, `75`, `100`), defined in the findings
schema. Treat anchors as integers; never coerce to floats.

1. **Validate.** Check each compact return for required fields and value constraints. Drop malformed
   returns or findings; record the drop count.
   - **Top-level required:** reviewer (string), findings (array), residual_risks (array),
     testing_gaps (array). Drop the whole return if any are missing or the wrong type.
   - **Per-finding required:** title, severity, file, line, confidence, autofix_class, owner,
     requires_verification, pre_existing.
   - **Value constraints:** severity ∈ {P0,P1,P2,P3}; autofix_class ∈ {gated_auto, manual,
     advisory}; owner ∈ {downstream-resolver, human, release}; confidence ∈ {0,25,50,75,100}; line a
     positive integer; pre_existing & requires_verification boolean.
   - Do not validate against the full schema here — the full schema (with why_it_matters and
     evidence) applies to the artifact files, not the compact returns.
2. **Deduplicate.** Fingerprint = `normalize(file) + line_bucket(line, +/-3) + normalize(title)`. On a
   match, merge: keep the highest severity, keep the highest anchor, note which reviewers flagged it.
   Dedup runs over the full validated set (including anchor 50) so step 3 can lift matching anchor-50
   findings into the actionable tier.
3. **Cross-reviewer agreement.** When 2+ independent reviewers flag the same fingerprint, promote the
   merged finding by one anchor step (`50 -> 75`, `75 -> 100`, `100 -> 100`). Note the agreement in
   the Reviewer column (e.g. "security, correctness").
4. **Separate pre-existing.** Pull findings with `pre_existing: true` into their own list.
5. **Resolve disagreements.** When reviewers flag the same region but disagree on severity,
   autofix_class, or owner, annotate the Reviewer column (e.g. "security (P0), correctness (P1) --
   kept P0").
6. **Normalize routing.** Set each merged finding's final `autofix_class`, `owner`, and
   `requires_verification`. On disagreement keep the more conservative route. Remap any legacy
   `safe_auto` / `review-fixer` to `gated_auto` / `downstream-resolver`.
   6b. **Mode-aware demotion of weak general-quality findings.** Some output is real signal that does
   not warrant primary-findings attention. Reroute it to the soft buckets so the primary table stays
   focused. A finding qualifies for demotion when **all** hold:
   - Severity is P2 or P3 (P0/P1 always stay in primary findings).
   - `autofix_class` is `advisory` (concrete-fix findings stay in primary).
   - **All** contributing reviewers are `testing` or `maintainability` — if any other persona also
     flagged it, cross-reviewer corroboration is present and it stays in primary findings.

   When it qualifies, move it out of the primary set: if the contributing reviewer is `testing`,
   append `<file:line> -- <title>` to `testing_gaps`; if `maintainability`, append to
   `residual_risks`. Use title-only lines. Record the demotion count for Coverage.

7. **Confidence gate.** After dedup, promotion, and demotion have shaped the primary set, suppress
   remaining findings below anchor 75. **Exception:** P0 findings at anchor 50+ survive — a
   critical-but-uncertain issue must not be dropped silently. Record the suppressed count by anchor.
   The gate runs late deliberately, so anchor-50 findings get their chance to be promoted (step 3) or
   rerouted (step 6b) before any drop.
8. **Partition the work.** Build two sets:
   - actionable queue: `gated_auto` or `manual` findings owned by `downstream-resolver`.
   - report-only queue: `advisory` findings plus anything owned by `human` or `release`.
9. **Sort and number.** Order by severity (P0 first) -> anchor (descending) -> file path -> line,
   then assign monotonically increasing `#` values across the full primary set in that order. Do not
   restart numbering per severity table, triage group, or routing bucket. When later sections repeat a
   finding (e.g. Actionable Findings), reuse the same stable `#`.
   9b. **Build thematic triage groups.** After stable `#`s exist, group related findings so the reader
   can triage themes, not items. This is distinct from dedup: dedup asks "are these the same
   finding?", grouping asks "are these distinct findings that should be understood or resolved
   together?". Groups never merge findings or change a finding's severity, confidence, route, owner,
   or `#`. They span the **full primary set** — actionable and report-only.
   - **`grouping:off`:** skip.
   - **`grouping:auto` (default):** build groups when findings span distinct concerns; skip when all
     findings are about the same thing. Prefer no groups over decorative single-item groups.
   - **`grouping:always`:** always build groups; use single-finding groups only when no meaningful
     multi-finding grouping exists.
   - **Signals:** shared root cause, affected subsystem, user-facing failure mode, overlapping fix
     path, dependency ordering, repeated symptoms of one design choice.
   - **Group shape:** short title, included `#`s, one-line context, preferred resolution, and why —
     when one fix path resolves several findings, name it and say which to handle first.
   - **Ordering:** by the highest-severity finding contained, then by lowest `#`. A finding appears in
     at most one group; leave unrelated findings ungrouped.
10. **Collect coverage data.** Union residual_risks and testing_gaps across reviewers.
11. **Preserve context-agent artifacts.** Keep the prior-art, automation-surface, and
    release-readiness outputs alongside the merged set. Do not drop unstructured agent output just
    because it does not match the persona JSON schema.

### Phase 5b: Verification pass (quality gate)

An independent re-verification gate. Spawn one validator sub-agent per surviving finding with
`references/validator-template.md`. Rejected findings are dropped; confirmed ones flow through
unchanged.

**When it runs:** after Phase 5 whenever at least one finding survives — skip only when zero survive.
When more than 15 survive, do **not** skip; validate per the budget cap in step 2. The default method
is the per-finding validator wave (below); a surviving **P2/P3 at anchor 100** may instead be checked
by direct first-party verification (see below). Same rule for both modes.

**Steps:**

1. **Select.** All survivors of Phase 5.
2. **Budget cap.** If the set exceeds 15, validate the highest-severity 15 (P0, then P1, then P2,
   then P3; ties by anchor descending), dropping only from the P2/P3 tail. **Never drop a P0 or P1
   from validation** — if P0/P1 alone exceed 15, raise the cap to include all of them. Record the
   over-budget count for Coverage.
3. **Spawn validators with bounded parallelism.** One sub-agent per finding via the validator
   template and the same scheduler as Phase 4. Each validator receives:
   - the finding's title, severity, file, line, suggested_fix, original reviewer, and confidence;
   - `why_it_matters` when available — loaded from `/tmp/panel-review/{run_id}/{reviewer_name}.json`;
     omit when the file is absent or the write failed (the validator proceeds from the diff and code);
   - the full diff;
   - the scope mode and remote head ref, mirroring the Phase 4 bundle: inject
     `<pr-scope-mode>local-aligned | pr-remote | branch-remote</pr-scope-mode>` and, when set,
     `<pr-head-ref>...</pr-head-ref>` or `<branch-head-ref>...</branch-head-ref>`. The template
     defaults to local-aligned workspace inspection, so omitting these in `pr-remote`/`branch-remote`
     would make validators verify against the wrong tree;
   - inspection scoped by mode: in `local-aligned`, Read/Grep/git blame the cited code, callers,
     guards, framework defaults, and history; in `pr-remote`/`branch-remote`, `git show
<remote-head-ref>:<path>` or the diff hunks only.
4. **Collect verdicts.** Each validator returns `{ "validated": true | false, "reason": "<one
sentence>" }`.
   - `validated: true` -> survives into Phase 6.
   - `validated: false` -> dropped; record the reason in Coverage.
   - Validator **infrastructure** failure (timeout, dispatch error, malformed JSON — not a
     `validated:false` verdict): for **P2/P3**, drop with reason "validator failed" (conservative
     bias); for **P0/P1**, do **not** drop — keep it and mark validation **degraded** (note in
     Coverage). A transient failure must never silently remove a critical/high finding; a genuine
     `validated:false` still drops at any severity.
5. **Use the mid-tier model** (`sonnet`) for validators — the same class the personas use. Validators
   are read-only, same constraints as personas.
6. **Record metrics for Coverage.** Total dispatched, validated-true, validated-false (with reasons),
   infra failures (and any P0/P1 kept-on-failure as degraded), and over-budget drops.
7. **Prune triage groups after drops.** When validation dropped anything, rebuild or prune
   `triage_groups` from the validated set — a group must never reference a dropped `#`. Drop groups
   left with fewer than two findings under `grouping:auto`; under `grouping:always`, keep them as
   single-finding groups only when still meaningful.

**Orchestrator direct verification.** When a finding hinges on a fact the orchestrator can check
cheaply and authoritatively — a pinned dependency's source, a wiring/config fact in this repo, a
build flag — verify it directly with single-purpose native tools (Read/Grep/Glob, one git command at
a time), never chained or error-suppressed shell. Fold confirmed facts into synthesis. Whether it can
_replace_ the independent validator turns on one distinction: the orchestrator is **not** an
independent second opinion (it synthesized these findings), so direct verification catches a wrong
**fact** but not the orchestrator's own **bias**:

- **P0/P1, any anchor:** the per-finding validator wave is **required**; direct verification only
  complements it.
- **P2/P3 at anchor 100** (verifiable from code alone — compile/type error, definitive logic bug,
  quotable standards violation): direct verification **may stand in for** the wave; note the method
  in Coverage.
- **P2/P3 at anchor 75** (judgment call): the independent wave is **required** — this is exactly
  where a fresh second opinion filters false positives, and the orchestrator cannot supply that for
  its own findings.

**Why per-finding, not batched:** independence is the point. A single batched validator looking at
all findings together pattern-matches across them and recreates the persona-bias problem. Per-finding
dispatch preserves fresh context while the scheduler respects harness limits.

### Phase 5c: Act on findings (default mode only)

**Skip entirely in `mode:agent`** — that mode is a machine handoff and the caller owns apply. In the
default mode the review is the top-level agent, so it applies the fixes it is confident in before
presenting the report.

**Apply policy (bias to act).** Default to applying every finding that is a clear improvement and a
reversible edit, regardless of severity. The work is a tracked, visible diff that can be reverted, so
leaving a clean fix unapplied "to be safe" is the failure mode, not the safe choice. Decide by
judgment, not a checklist:

- **Apply** clear improvements — the common case (test hardening, dead-code removal, a localized fix
  with a concrete `suggested_fix`).
- **Push back** — do not apply — when the reviewer is wrong; keep the finding and state the
  disagreement with reasoning.
- **Skip with judgment** taste calls and conflicting suggestions, but surface what was skipped and
  why. Never drop silently.

Severity, confidence, and cross-reviewer agreement tell you what to do first and what to flag loudly
— they do not gate the decision. There is no deny-list: downside is controlled after the fact
(revert + visible diff + commit checkpoint), not by a precondition.

**Scope invariant.** Apply only when the working tree _is_ what was reviewed — `local-aligned` or
standalone. In `pr-remote` / `branch-remote` the working tree is not the reviewed head; do not apply —
report instead.

**Verify, then keep.** After applying, run the affected tests and lint (targeted by default per
`docs/nx-guidelines.md` — e.g. `nx test <project>` / `nx lint <project>`, or affected; broaden when
fixes span files). If they fail, revert that fix and report it as a finding instead — an unverified
fix is not finished. Never leave the tree red.

**Commit when the pre-review tree was clean.** Before applying, note whether the working tree already
had uncommitted changes (`git status --porcelain`). The permanence gate is the **push**, not the
commit — a local commit is private and reversible (`git reset --soft HEAD~1`). Follow
`docs/git-convention.md`: never push, amend, force, or add co-author attribution.

- **Clean before the review:** after applying and verifying, commit the fixes as one isolated,
  review-labeled commit — `fix(review): <summary>`, or the repo's nearest convention. Labeled and
  reversible, returning the tree to a known state.
- **Dirty before the review:** apply but do **not** commit — the fixes interleave with the user's
  in-flight work and ride along with the commit they were already going to make. The Applied section
  lists what changed.
- **Never push, open a PR, or file tickets** — that is the outward step the user owns.

**Surface green-but-unverifiable edits.** When an applied fix touches auth/authz, a public or
cross-service contract/schema, or concurrency/ordering, a passing test does not prove safety — flag it
prominently in the Applied section so the diff reviewer's attention goes there.

**Re-partition triage groups after apply.** Triage groups describe the _remaining_ work. After
Phase 5c, prune applied findings out of `triage_groups` before Phase 6 — a group must never tell the
user to handle a finding already applied. When an applied fix resolved part of a theme, note that in
the group's context line. Re-apply the minimum-size rule from Phase 5b step 7.

### Phase 6: Synthesize and present

Assemble the final report. **Default:** pipe-delimited markdown tables for findings (mandatory — see
the output template). **`mode:agent`:** skip markdown and emit JSON (see "JSON output"). Other
sections (Actionable Findings, Learnings, Coverage, etc.) use bullets and a `---` before the verdict
in markdown mode only.

**Before writing the report, load `references/review-output-template.md` and mirror it** — that file
is the canonical skeleton. The block below is the always-available fallback so the shape survives a
long session even if the template was not reloaded.

**Findings table shape (default mode — load-bearing, do not improvise).** Every finding is a row in a
pipe-delimited table grouped by severity, with a **terse** `Issue` cell; depth goes in a keyed detail
line under the table. Copy this shape:

| #   | File                 | Issue                                | Reviewer    | Confidence |
| --- | -------------------- | ------------------------------------ | ----------- | ---------- |
| 1   | `path/to/file.ts:42` | One terse line — the scannable index | correctness | 100        |

- **#1** — full explanation here (why it matters + concrete fix direction), as a keyed detail line
  under the table.

Per-severity tables are **5 columns** — `Route` is not shown here (it appears only in the Actionable
Findings table and the JSON). Keep the `Issue` cell to **one short clause** (~12 words or fewer, no
second sentence, no because/so/which) — it is the index, not the explanation. The moment a cell wants
a comma-plus-clause or a reason, move that depth into the **keyed detail line** (`- **#N** — …`) —
usually for P0/P1; P2/P3 are typically terse-only.

**Never produce these shapes (instant fail — applies to _every_ tabular section, the Applied table
included; re-render if you catch one mid-draft):**

- Any row — a finding **or** an Applied entry — rendered as `Field:`-prefixed blocks (`#:` / `Sev:` /
  `File:` / `Issue:` / `Fix:` / `Route:`) — depth goes in the keyed detail line, never a field block.
- Per-row separators made of horizontal rules or box-drawing (`────`, `———`).
- A table replaced by a plain bulleted/numbered list (the keyed `- **#N** —` detail line under a
  table is a supplement, not a replacement).
- Unicode separators or arrows in cells (middot `·`); use ASCII `->`.
- **Inconsistent treatment across severities or sections** (e.g. P1 as blocks while P2/P3 are tables)
  — every table uses the same pipe-delimited shape.

1. **Header.** Scope, intent, mode, panel with per-conditional justifications.
2. **Applied (default mode only).** When Phase 5c applied fixes, list them first — before the
   findings tables — as a pipe table `| # | File | Fix | Reviewer |` (never `Field:`-blocks or `────`
   separators), then a one-line validation outcome (e.g. "pin tests 4 -> 6; suite 94 pass, lint
   clean") and commit status (committed on a clean tree as `fix(review): …`, or left uncommitted on a
   dirty one). Flag green-but-unverifiable edits (auth/contract/concurrency) prominently. Omit in
   `mode:agent` and when nothing was applied. Applied findings appear here, not in the severity tables.
   2b. **Triage Groups.** When finalized `triage_groups` exist (post-validation, post-apply), render a
   `### Triage Groups` section before the severity tables as a pipe table `| Group | Findings |
Context | Preferred Resolution | Why |`. The `Findings` cell references stable `#`s; verify each
   appears in the severity tables. Groups supplement the tables, never replace them. Omit when
   `grouping:off` or no groups survived. In `mode:agent` this is carried by the `triage_groups` JSON
   field.
3. **Findings.** Pipe tables grouped by severity (`### P0 -- Critical`, `### P1 -- High`, `### P2 --
Moderate`, `### P3 -- Low`), using the shape above. Omit empty severity levels. Finding numbers
   come from the stable assignment in Phase 5 — never re-derive per table or group.
4. **Requirements Completeness.** Include only when a requirements source was found in Phase 2b. For
   each requirement and task/unit, report whether matching work appears in the diff — a checklist:
   met / not addressed / partially addressed. Routing depends on `req_source`:
   - **`explicit`** (caller-provided or PR-body): flag unaddressed requirements/tasks as P1 with
     `autofix_class: manual`, `owner: downstream-resolver`. These enter the actionable queue.
   - **`inferred`** (auto-discovered): flag them as P3 with `autofix_class: advisory`, `owner:
human`. Report-only — an inferred match is a hint, not a contract.
     Omit this section entirely when none was found — do not mention its absence.
5. **Actionable Findings.** Include when the actionable queue is non-empty — `gated_auto` / `manual`
   with `downstream-resolver`, plus anything Phase 5c chose not to apply. In default mode, findings
   already applied appear in the Applied section, not here.
6. **Pre-existing.** Separate section; does not count toward the verdict.
7. **Learnings & Prior Art.** Surface `prior-art-researcher` results: when a past solution or pattern
   is relevant, flag it as "Known Pattern" with a link to the `learnings/` or archived
   `openspec/changes/` file.
8. **Automation-Surface Gaps.** Surface `automation-surface-reviewer` results. Omit if none.
9. **Release Notes.** If `release-readiness-checker` ran, surface the key go/no-go items: blocking
   pre-deploy checks, the most important verification queries, rollback caveats, monitoring focus.
   Keep it actionable rather than burying it in Coverage. Schema drift appears as `data-migration`
   P1 rows — no separate Schema Drift section.
10. **Coverage.** Applied count (when Phase 5c ran), suppressed count by anchor, demotion count,
    validator drop count and reasons (when Phase 5b ran), any P0/P1 with degraded validation,
    over-budget validator drops, residual risks, testing gaps, failed/timed-out reviewers, and
    inferred-intent uncertainty when applicable.
11. **Verdict.** Ready to merge / Ready with fixes / Not ready. Fix order when applicable. When an
    `explicit` requirements source has unaddressed items, the verdict must reflect it — a PR that is
    code-clean but missing planned requirements is "Not ready" unless the omission is intentional. An
    `inferred` source with gaps is noted in the reasoning but does not block on its own.

Do not include time estimates.

**Format verification (default only — last gate).** Before delivering, scan **every table — Applied,
Triage Groups, and each severity table** for the forbidden shapes: `Field:`-prefixed blocks,
box-drawing or horizontal-rule separators (`────`), middot `·`, or a list replacing a table. **The
Applied table is the most common offender — check it explicitly.** If any table hit one, STOP and
re-render it as the same pipe shape before delivering. (The keyed `- **#N** —` detail line is
expected, not a failure.) Skip when `mode:agent` is active.

### JSON output (`mode:agent` only)

Emit **one raw JSON object** as the primary response — a single bare JSON value, **no markdown code
fence**. A leading ` ```json ` fence breaks naive `JSON.parse` consumers, so never wrap it.
Also write `review.json` under `/tmp/panel-review/<run-id>/` with the same payload.

`mode:agent` does not apply fixes — the caller does — so there is no `applied_fixes` field; the
handoff is `actionable_findings`. Applied work surfaces only in the default-mode Applied section.

Minimum shape:

```json
{
  "status": "complete",
  "verdict": "Ready to merge | Ready with fixes | Not ready",
  "scope": {
    "base": "<merge-base sha, pr:NNN marker, or base: ref>",
    "branch": "<current branch name>",
    "head_sha": "<git rev-parse HEAD>",
    "pr_url": "<url or null>",
    "files_changed": 0
  },
  "intent": "<2-3 line summary>",
  "intent_confidence": "explicit | inferred | uncertain",
  "reviewers": ["correctness", "security"],
  "findings": [],
  "actionable_findings": [],
  "triage_groups": [],
  "pre_existing_findings": [],
  "requirements_completeness": null,
  "learnings": [],
  "automation_surface_gaps": [],
  "release_notes": [],
  "residual_risks": [],
  "testing_gaps": [],
  "coverage": {},
  "artifact_path": "/tmp/panel-review/<run-id>/",
  "run_id": "<run-id>"
}
```

Each object in `findings` uses the merged fields: `#`, `title`, `severity`, `file`, `line`,
`confidence`, `autofix_class`, `owner`, `requires_verification`, `pre_existing`, `suggested_fix`,
`why_it_matters`, `evidence`, `reviewers`.

`actionable_findings` lists the `gated_auto` / `manual` + `downstream-resolver` subset with the same
fields plus stable `#`.

Each object in `triage_groups` carries `{ "title", "findings": [<stable #s>], "context",
"preferred_resolution", "why" }` — the finalized groups after Phase 5b pruning. Every referenced `#`
must exist in `findings` (the full set) — **not** necessarily in `actionable_findings`. Groups are a
triage **lens over all findings, not an apply queue**: a group can reference advisory or
`human`/`release`-owned findings the caller must not apply. So a caller batching related fixes by
theme must first intersect each group's `findings` with `actionable_findings` and act only on that
subset — the apply handoff stays `actionable_findings`, never `triage_groups`. Empty array when
`grouping:off` or no groups were built.

On failure before review completes, set `"status": "failed"` and `"reason": "<one sentence>"`. When
all reviewers fail, use `"status": "degraded"` with a reason. When a PR skip rule fires
(closed/merged/trivial), use `"status": "skipped"` with the skip reason. Never emit markdown tables
when `mode:agent` is active.

## Quality gates

Before delivering, verify:

1. **Every finding is actionable.** Re-read each one. If it says "consider", "might want to", or
   "could be improved" without a concrete fix, rewrite it with a specific action.
2. **No false positives from skimming.** For each finding, confirm the surrounding code was actually
   read — that the "bug" isn't handled elsewhere in the same function, the "unused import" isn't used
   in a type annotation, the "missing null check" isn't guarded by the caller.
3. **Severity is calibrated.** A style nit is never P0; an injection is never P3. Re-check each one.
4. **Line numbers are accurate.** Verify each cited line against the file. A finding pointing at the
   wrong line is worse than no finding.
5. **Protected artifacts are respected.** Discard findings recommending deletion or gitignore of
   `openspec/changes/`, `openspec/specs/`, or `learnings/` files.
6. **Findings don't duplicate linter output.** Don't flag what the project's linter/formatter catches
   (semicolons, indentation, import order). Focus on semantic issues.

## Stack-aware conditionals

Stack-specific reviewers fire only when the diff touches runtime behavior they specialize in (React
async UI races, nx boundary/layer leaks) — never mechanically from file extensions. Structural
quality (complexity, large regressions, type-boundary leaks) lives in the always-on `maintainability`
persona; do not spawn extra reviewers for language conventions or "strict bar" passes.

## After the review

After Phase 6, stop. Never push, open PRs, or file tickets from this skill. In the default mode,
Phase 5c has already applied and (on a clean pre-review tree) committed the safe fixes; in
`mode:agent` the review mutates nothing — the caller and the user act on the report and artifact.

When `comment:pr` is set (default mode + a resolvable PR), post the findings to the PR — per
"Posting findings as PR comments" — before stopping. That is the full extent of the outward action:
still never push, open a PR, or merge.

### Actionable Findings summary (default mode only)

After Phase 6 **in default mode**, emit a compact **Actionable Findings** summary for callers:

- List each actionable finding (`gated_auto` / `manual` with `downstream-resolver`) with stable `#`,
  severity, file:line, title, `autofix_class`, whether a `suggested_fix` is present, and `confidence`.
- Include the run-artifact path when one was written: `/tmp/panel-review/<run-id>/`.
- When the actionable queue is empty, state `Actionable findings: none.` explicitly.

In `mode:agent` do **not** emit this markdown — the actionable findings are carried solely by the
`actionable_findings` JSON field. Emit nothing after the JSON object, so the response stays a single
parseable JSON value.

Do not run post-review triage (no per-finding walk-through, bulk ticket filing, or routing
questions). The report and summary are the complete handoff.

### Mode-specific completion

| Mode                       | After Phase 6 + actionable summary                                       |
| -------------------------- | ------------------------------------------------------------------------ |
| **Default**                | Markdown tables + Actionable Findings summary                            |
| **Default + `comment:pr`** | The same, plus the findings posted to the PR (summary + inline comments) |
| **`mode:agent`**           | JSON object + `review.json` in the run artifact dir                      |

Do not offer push/PR/create-branch next steps from this skill.

### Run artifacts

Always write run artifacts under `/tmp/panel-review/<run-id>/`:

- synthesized findings
- actionable findings list
- advisory outputs
- per-agent `{reviewer_name}.json` from Phase 4
- `report.md` — the rendered markdown report exactly as presented (default mode only), so format and
  numbering stay auditable

`metadata.json` minimum fields:

```json
{
  "run_id": "<run-id>",
  "branch": "<git branch --show-current at dispatch time>",
  "head_sha": "<git rev-parse HEAD at dispatch time>",
  "verdict": "<Ready to merge | Ready with fixes | Not ready>",
  "completed_at": "<ISO 8601 UTC timestamp>"
}
```

Capture `branch` and `head_sha` at dispatch time (no in-skill fixes land afterward).

## Fallback

If the platform has no parallel sub-agents, run reviewers sequentially. If it caps concurrency, use
the bounded queueing in Phase 4 rather than treating cap-related spawn failures as reviewer failures.
Everything else (phases, output, merge pipeline) is unchanged.

## References

Load on demand at the phase that needs it — none are inlined here:

- `references/persona-catalog.md` — full per-persona selection criteria and spawn gates (Phase 3).
- `references/subagent-template.md` — the reviewer sub-agent prompt (Phase 4).
- `references/diff-scope.md` — primary / secondary / pre-existing tier rules for reviewers (Phase 4).
- `references/findings-schema.json` — the JSON contract reviewers conform to (Phase 4).
- `references/action-class-rubric.md` — `autofix_class` / `owner` guidance (routing, Phase 5).
- `references/validator-template.md` — the validator sub-agent prompt (Phase 5b).
- `references/review-output-template.md` — the canonical report skeleton (Phase 6).
