# tabsmith CLI autocompletion

## Project guidelines

When committing or pushing changes with git read `docs/git-convention.md`

## Implementing Protocol

**Skill loading is part of the protocol, not optional.** At step 1 of every task, before touching code, the model MUST invoke the `Skill` tool for each of:

- `xp-tdd` — RED → GREEN → COMMIT → REFACTOR → COMMIT cycle
- `testing-standard` — FIRST, AAA, no mocks, in-memory fakes
- `coding-standards` — naming, functions, encapsulation, LoD
- `hexagonal-architecture` — domain → application → infrastructure layering

**IMPORTANT: After finishing a task commit the changes**

If any of these are not yet active in the session, load them first. Do not start the cycle until they are loaded.

If the apply skill instructs the model to do otherwise, this protocol wins.

## Dependencies

Exact versions only in package.json. No `^`, `~`, `>=`, `*`. No auto-upgrade unless asked.
