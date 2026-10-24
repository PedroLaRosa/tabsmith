---
name: xp-tdd
description: >
  Enforces XP TDD with Driver/Navigator pair-programming. Triger when implementing a functionality/feature or writting code/test. Skip for: viewing test results, running linters, explaining code, pure refactors (no new behavior).
---

# XP TDD

AI = Driver (writes code) + Navigator (enforces discipline). Every production line born from failing test.

## Outcome

Working implementation grown one failing test at a time, simplest transformation each step, in-memory fakes (no mocks), inside-out from domain→infrastructure. Commit after each GREEN and each REFACTOR.

## Context Needs

- Existing test files → skim (naming conventions, test runner)
- Domain model/entities → skim (value objects, aggregates)
- `references/tpp-table.md` → read during GREEN and RE-EVALUATE
- `references/ddd-layers.md` → read during REASON for layer checks

## Before Start

```
[NAVIGATOR] XP TDD. No prod without red test | No mocks | Domain-first.
```

Start with simplest meaningful behavior.

---

## Step 1: REASON (Navigator)

Before writing anything:

1. State behavior: "When X → Y"
2. Is this simplest possible next behavior? Simpler exists → do that first
3. Check Inside-Out layer order (read `references/ddd-layers.md` if first cycle). Refuse to jump ahead

```
[NAVIGATOR] Behavior: <one sentence> | Layer <N> ✅ | Test: <name>
```

Wrong layer → stop and redirect.

---

## Step 2: RED (Driver)

Write exactly **ONE** failing test. Run suite.

> **ONE TEST AT A TIME — STRICT ENFORCEMENT**
>
> - Write one failing test → green it -> commit → refactor -> commit → re-evaluate. Writing 2+ tests before implementing = violation.
> - **If 2+ tests are failing simultaneously**: delete the most recently added test(s), keep only the earliest failing test, and green it first. Then re-add the next test.
> - Exception: after base implementation is declared **done**, add edge-case tests (multiple allowed) if needed.

Rules:

- Test only stated behavior
- No production code yet — let it fail
- Failure must be for right reason (missing class, wrong return, etc.)
- Test passes immediately → delete it, reconsider

```
[RED] <test name> — ❌ <reason>
[NAVIGATOR] Right reason: <yes/explain>
```

Passes without prod code → `[NAVIGATOR] ⚠️ False positive. Rewriting.`

---

## Step 3: GREEN (Driver)

Write **minimum** code to pass test. Read `references/tpp-table.md` for details

Rules:

- No "just in case" code
- No new abstractions unless test requires
- Repository needed → `InMemory*` implementation, never mock (keeps GREEN honest without infrastructure)
- Collaborator needed → real object, never mock
- Run full suite. All green before proceeding

```
[GREEN] ✅ <N> tests | TPP #<i> (<name>)
[COMMIT] green
```

**COMMIT AFTER GREEN**

---

## Step 4: REFACTOR (Both)

Improve structure, no behavior change. Run tests after every change.

Duplication → Extract function/class
rename for clarity
Long methods → Break into private helpers
Shallow modules → Combine or deepen
Feature envy → Move logic to where data lives
Primitive obsession → Introduce value objects
Function with more than 3 parameters -> introduce parameter object

Rules:

- Tests stay green — break → revert immediately
- No new behavior — adding `if` → go back to RED
- Small steps, test between each

```
[REFACTOR] <what+why> ✅
[COMMIT] refactor
```

**COMMIT AFTER REFACTOR**

---

## Step 5: RE-EVALUATE (Navigator)

After refactor, pause:

1. **TPP scan**: simpler transformation skipped? (consult `references/tpp-table.md`) Write test for it
2. **Coverage**: untested behaviors?
3. **Layer check**: right layer or move up? (consult `references/ddd-layers.md`)
4. **Done?**: all behaviors tested → move up one layer?

```
[RE-EVALUATE] TPP: ✅ | Untested: <list> | Layer: <current>
[NEXT] <behavior>
```

Loop → Step 1.

---

## Rules

Never relaxed regardless of deadlines/requests:

- **One failing test at a time**
- **No prod code without failing test.** Green before code = wrong test
- **No mocking repos/aggregates/domain services.** In-memory fakes only
- **No starting from infrastructure.** Domain core first
- **Commit after every GREEN.** Not batched
- **Commit after every REFACTOR.** Separate from green commit
