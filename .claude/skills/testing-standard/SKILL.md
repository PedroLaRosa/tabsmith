---
name: testing-standard
description: >
  Enforces test quality (unit/integration/e2e). Trigger on: writing, adding, or modifying tests
  in any framework. Governs HOW tests are written. For TDD cycle → xp-tdd.
  Skip for: running tests, reading output, refactoring prod code without test edits.
---

# Testing Standard

Every test = first-class citizen. Slow, brittle, ambiguous, or coupled tests are liabilities.

## Outcome

Fast, independent, readable, trustworthy tests. Reader understands behavior verified, scenario setup, expected outcome — without reading prod code.

## Context Needs

- Skim existing test files → current naming/patterns

## On Start

Read learnings, note prior corrections. Announce:

## Step 1: FIRST Principles

Verify Fast, Independent, Repeatable, Self-Validating, Timely — all five before writing. Violates any → redesign.

→ Full table with red flags: `references/first-principles.md`

## Step 2: Naming

Test names = documentation. Reader understands intent without body.

Formats:

- `should <verb> <condition>` — e.g. `should reject order when inventory insufficient`
- `given <ctx> when <action> then <result>`

Rules:

- Plain English sentences — no camelCase fragments, no abbreviations
- Describe blocks = unit under test (class/function/module)
- Nested describes narrow scope
- Never implementation details in name (no "calls saveMethod", "returns true")

## Step 3: AAA Pattern

Structure every test as Arrange → Act → Assert, separated by blank lines. One logical assertion per test.

→ Full pattern with examples and rules: `references/aaa-pattern.md`

## Step 4: Test Doubles

Pick the lightest double that works: Real → InMemory Fake → Stub → Mock (last resort). Never mock what you own.

→ Full priority table and hard rules: `references/test-doubles.md`

## Step 5: DI + Strategy Pattern

Code only testable with mocks = coupled to infrastructure. Fix structurally via constructor injection.

→ Full pattern: `references/di-strategy.md`

## Step 6: Test Isolation

- No shared mutable state between tests
- Each test creates own instances in Arrange — no `beforeEach` shared references
- `afterEach`/teardown must be idempotent (clean state even if test crashed)
- Integration tests w/ real DB: rollback transactions or fresh schema per run

## Framework-Specific

Detect framework from project config or existing tests, then read the relevant reference:

- **Vitest** → `references/vitest.md`

## Rules

- 2026-04-07: Vitest utilities (describe, it, expect, test, vi, beforeEach, afterEach) must always be explicitly imported from 'vitest'. Never assume globals.
