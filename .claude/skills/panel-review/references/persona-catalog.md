# Persona catalog

13 reviewer personas across always-on, cross-cutting conditional, and stack-specific conditional
layers, plus 2 always-on context agents and 1 conditional agent. The orchestrator uses this catalog
to decide which reviewers to convene for each diff.

## Always-on (4 personas + 2 context agents)

Spawned on every review regardless of diff content.

**Persona agents (structured JSON output):**

| Persona             | Focus                                                                                                                       |
| ------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| `correctness`       | Logic errors, edge cases, state bugs, error propagation, intent compliance                                                  |
| `testing`           | Coverage gaps, weak assertions, brittle tests, missing edge-case and behavior tests                                         |
| `maintainability`   | Structural quality, needless complexity, large regressions, coupling, type-boundary leaks, dead code, premature abstraction |
| `project-standards` | CLAUDE.md / AGENTS.md compliance — naming, references, frontmatter, conventions, hexagonal layering rules, tool selection   |

**Context agents (unstructured output, synthesized separately):**

| Agent                         | Focus                                                                                                                       |
| ----------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| `prior-art-researcher`        | Search `learnings/`, `docs/`, and archived `openspec/changes/` for past issues and patterns relevant to this diff's modules |
| `automation-surface-reviewer` | Verify a new user-facing capability is also reachable headlessly (CLI / API / agent), not UI-only                           |

## Cross-cutting conditional (7 personas)

Spawned when the orchestrator finds the relevant pattern in the diff. The orchestrator reads the full
diff and reasons about selection — this is judgment, not keyword matching.

| Persona             | Select when the diff touches...                                                                                                                                                                              |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `security`          | Auth/session handling, public endpoints, user input, permission/ownership checks, secrets, RLS policies                                                                                                      |
| `performance`       | DB queries and ORM/Drizzle calls, N+1 risks, loop-heavy transforms, caching layers, async/concurrent code                                                                                                    |
| `api-contract`      | Route definitions, DTO/serializer changes, exported type signatures, event/payload schemas, API versioning                                                                                                   |
| `data-migration`    | Migration files, schema changes (`projects/services/drizzle/migrations/*`, Drizzle schema, Supabase SQL), backfill or data-transform scripts — **not** model/query-only changes without a migration artifact |
| `reliability`       | Error handling, retry logic, timeouts, background jobs, async handlers, health checks, idempotency                                                                                                           |
| `adversarial`       | >=50 changed non-test, non-generated, non-lockfile executable lines, OR auth, payments, data mutations, external integrations, or other high-risk domains                                                    |
| `previous-comments` | **PR-only AND comment-gated.** A PR that already carries review comments/threads from prior rounds. Skip entirely when no PR metadata was gathered in Phase 1, OR when `hasPriorComments` is false.          |

## Stack-specific conditional (2 personas)

These cover runtime behavior the always-on personas do not specialize in. Structural and
maintainability concerns live in the always-on `maintainability` persona — do not convene a stack
reviewer for philosophy or convention-only passes.

| Persona          | Select when the diff touches...                                                                                                                                                                         |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `frontend-async` | React effects and state in `projects/frontend`, event/listener wiring, timers, data-fetching hooks, async UI flows, or render-time state transitions with race or stale-closure potential               |
| `nx-boundaries`  | nx project tags and import constraints, `project.json` / `nx.json` target wiring, new libs or cross-project imports, or moves that cross the hexagonal domain → application → infrastructure boundaries |

## Conditional agent (migration-specific)

Spawn `release-readiness-checker` when the migration-artifact gate applies **and** the change is
risky (destructive DDL, backfills, NOT NULL without default, column renames/drops). Schema drift and
migration safety themselves are the `data-migration` persona's structured findings — not this agent.

| Agent                       | Focus                                                                 |
| --------------------------- | --------------------------------------------------------------------- |
| `release-readiness-checker` | Go/no-go checklist with verification queries and a rollback procedure |

## Selection rules

1. **Always convene the 4 always-on personas** plus the 2 context agents.
2. **For each cross-cutting conditional persona**, read the diff and decide whether its domain is
   genuinely in play. A judgment call, not a keyword match.
3. **For each stack-specific conditional persona**, start from file types and changed patterns, then
   decide whether the diff introduces real work for that reviewer. Do not convene a stack reviewer
   just because one config or generated file matches the extension.
4. **For `data-migration`**, convene only when the diff includes a migration or schema artifact
   (`projects/services/drizzle/migrations/*`, a Drizzle schema file, a Supabase SQL migration, or an
   explicit backfill/data-transform script). Do **not** convene for model- or query-only changes
   without those files.
5. **For the conditional agent**, spawn `release-readiness-checker` when the migration gate applies
   and the change is risky (see above).
6. **Announce the panel** before spawning, with a one-line justification per conditional reviewer.

## Persona definition shape

When the orchestrator spawns a reviewer, it passes that persona's definition in the `<persona>` block
of the sub-agent prompt. Each definition should give the reviewer:

- **Identity** — what this reviewer is uniquely responsible for, and what it explicitly leaves to
  other personas (so the panel does not flag the same thing five times).
- **What to hunt** — the concrete failure modes in its domain, framed as behavior that breaks, not
  categories of code.
- **Calibration** — its confidence floor (most personas use the shared anchors; a few may set a
  stricter floor, e.g. anchor 75 minimum) and how it maps qualitative priority to P0-P3.
- **Suppress conditions** — the false-positive shapes specific to its domain that it must not emit,
  on top of the shared catalog in the sub-agent template.

Keep each definition tight and behavior-anchored — a reviewer that knows exactly what it owns
produces sharper findings than one handed a broad mandate.
