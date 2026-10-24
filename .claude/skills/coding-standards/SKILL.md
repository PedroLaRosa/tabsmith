---
name: coding-standards
description: >
  Enforces production code quality: naming, functions, encapsulation, composition,
  error handling, Law of Demeter, comments. Trigger on: writing production code,
  refactoring, code review, "clean code", "code quality", "coding standards",
  "review this code", "refactor this". Governs HOW production code is shaped.
  Skip for: test code (→ testing-standard), TDD cycle decisions (→ xp-tdd).
---

# Coding Standards

Production code = communication. Every name, function, class, and boundary decision either clarifies or obscures intent.

## Outcome

Readable, encapsulated, composable production code. Reader understands intent from names alone, functions do one thing, classes own their behavior, and nothing is exposed beyond what consumers need.

## Context Needs

- Skim existing production files → current naming/patterns

## Step 1: Naming Review

Before writing/accepting any identifier, verify it communicates intent without context.

- Names reveal intent — reader never asks "what is this?"
- No abbreviations unless universally understood (`id`, `url`, `html`)
- Booleans are predicates: `isOver`, `hasWinner`, `canMove`
- Functions = verbs. Types = nouns. Components = PascalCase nouns.

→ Full rules with anti-patterns and examples: `references/naming.md`

## Step 2: Function Design

Every function written or modified must follow single-responsibility and small-size principles.

- One thing per function — one reason to change
- ≤ 20 lines preferred, ≤ 3 params, 0 side effects when possible
- Early returns over nested conditionals
- Command/query separation: functions either do something or return something

→ Full rules with patterns: `references/functions.md`

## Step 3: Encapsulation Check

Apply minimum-scope, private-by-default, TypeScript access modifiers.

- Variables at narrowest possible scope. `const` always, `let` only when mutation required.
- Don't export unless another module imports it
- Class members `private` by default, `readonly` for fields set once
- `protected` only for documented extension points, `public` only for the contract

→ Full rules: `references/encapsulation.md`

## Step 4: Class & Composition Design

When creating or modifying classes/objects, enforce rich models and composition.

- Classes own their behavior — no getters-only data bags
- Need shared behavior → compose (inject collaborator), don't extend
- Inheritance only for genuine "is-a" with ≤ 2 levels depth
- TypeScript `interface` for contracts, not `abstract class`

→ Full rules: `references/classes-and-composition.md`

## Step 5: Law of Demeter

Check every method call chain for LoD violations.

- Only talk to: own properties, parameters, objects you created, direct collaborators
- Chained access (`a.b.c.d`) = violation — ask, don't dig
- Fix: add method to direct collaborator that encapsulates the traversal

→ Full rules: `references/law-of-demeter.md`

## Step 6: Error Handling

Verify error paths are explicit, typed, and never swallowed.

- Thrown `Error` subclasses for infrastructure — catch at boundary, not inline
- Never empty catch blocks. Never return null to signal errors.
- Exhaustive `switch` with `never` type in default case

→ Full rules: `references/error-handling.md`

## Step 7: Comment

After code is written, remove unnecessary comments. Add only where intent is non-obvious.

- Code should explain itself — if a comment explains "what", rename instead
- Allowed: "why" comments, TODO with ticket, JSDoc on public API
- Banned: commented-out code, section separators, paraphrasing the code

→ Full rules: `references/comments.md`
