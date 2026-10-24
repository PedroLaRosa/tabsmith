# Diff scope rules

Every reviewer follows these rules. They draw the line between "code this change is responsible for"
and "pre-existing context the change merely sits near."

## Resolving the scope

Pick the diff to review in this priority order:

1. **Caller-specified scope.** If the prompt carries `BASE:`, `FILES:`, or `DIFF:` markers, use that
   scope exactly as given.
2. **Working-copy changes.** If staged or unstaged changes exist (`git diff HEAD` is non-empty),
   review those.
3. **Unpushed commits vs the base branch.** If the working copy is clean, review `git diff $(git
merge-base HEAD <base>)..HEAD`, where `<base>` is the default branch (`main` or `master`).

The orchestrator's scope phase normally does this discovery and hands you the resolved diff — you do
not run git yourself unless remote scope (below) requires it.

## Remote scope (`pr-remote` and `branch-remote`)

When the review context carries `<pr-scope-mode>pr-remote</pr-scope-mode>` or
`<pr-scope-mode>branch-remote</pr-scope-mode>`, the working tree is **not** the head under review. Do
**not** Read/Grep workspace paths for files in the changed-file list — they may not match the branch
or PR you are reviewing.

Instead:

- Prefer `git show <remote-head-ref>:<path>` when `<pr-head-ref>` or `<branch-head-ref>` is in
  context.
- Otherwise rely on the hunks in the provided `<diff>` only.
- Never treat local workspace contents as evidence for a finding on a changed file.

## Classifying every finding

Each finding lands in one of three tiers by its relationship to the diff:

### Primary — directly changed code

Lines the diff added or modified. This is your main focus. Report against these at full confidence.

### Secondary — immediately surrounding code

Unchanged code in the same function, method, or block as a changed line. If the change introduces a
bug that is only visible by reading the surrounding context, report it — but note that the problem
lives in the interaction between the new and the existing code.

### Pre-existing — unrelated to this diff

Issues in unchanged code the diff neither touched nor interacts with. Mark these `"pre_existing":
true`. They are reported separately and do not count toward the verdict.

**The test:** if you would flag the same issue on an identical diff that did **not** include the
surrounding file, it is pre-existing. If the diff makes the issue _newly relevant_ — a new caller now
reaches an existing buggy function — it is secondary, not pre-existing.
