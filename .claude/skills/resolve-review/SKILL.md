---
name: resolve-review
description: >
  Read, analyze, and resolve code-review feedback on a pull/merge request.
argument-hint: 'Indicate branch or PR to resolve reviews? (Default: current branch)'
---

# Resolve Review

Turn reviewer feedback into resolved threads: read every comment, decide what is actually
worth doing, change the code where it improves quality or functionality, push back where it
doesn't, and close the loop by replying to each thread — referencing the commit when you made
one.

## Core principles (do not compromise these)

- **Analyze, never appease.** A reviewer comment is an input, not an order. Judge each one on
  merits: correctness, quality, functionality, maintainability, and fit with the codebase's
  conventions. If a suggestion is right, apply it. If it's wrong, outdated, based on a
  misread, or a net negative, **say so respectfully in a reply** and explain why. Satisfying a
  comment by degrading the code is a failure, not a success.
- **Always reply.** Every comment you act on gets a reply. Every comment you decline gets a
  reply with your reasoning. Silence leaves the reviewer guessing — close every thread.
- **One change → one commit → one reference.** When you accept a suggestion, implement it,
  verify it, and commit it on its own. Then reply to that thread referencing the commit as a
  link to the provider's commit page — a clickable SHA, never a bare hash — so the reviewer can
  jump straight to the change it introduced.
- **Group duplicates.** When several comments point at the same underlying problem (same fix,
  or the same pattern in multiple spots), solve them together in **one commit**. Reply to each
  grouped thread linking that shared commit.
- **Link, don't just cite.** Every commit reference is a markdown link to the commit on the
  provider — ``[`<sha>`](<commit_url>)`` — not a bare SHA. A bare hash forces the reviewer to go
  hunt for the diff; a link lands them on it. The per-provider URL form is in
  `references/providers.md`.
- **Push before you reference.** A commit link only resolves once the commit is on the remote.
  Push, then post replies that link each commit on the provider.
- **Respect repo conventions.** Follow the project's commit, branching, and testing rules
  (e.g. `docs/git-convention.md`, `AGENTS.md`/`CLAUDE.md`). Don't invent a style.

## Workflow

### 1. Identify provider, branch, and PR/MR

**Resolve the target branch first.** The skill takes an optional branch name as an argument.
If the user supplied one (e.g. "resolve review for `feat/login`"), use it verbatim. If they
didn't, fall back to the branch currently checked out. Everything downstream — finding the
PR/MR, reading comments, and pushing fixes — operates on this branch, so pin it down before
anything else.

```bash
git remote get-url origin
git rev-parse --abbrev-ref HEAD   # the fallback when no branch argument was given
```

When the user named a branch other than the one checked out, you're resolving the review
without that branch's working tree in front of you. You can still read and reply to comments
freely, but **applying** fixes means the branch's code needs to be checked out (`git switch
<branch>`) so your commits land on it. If the working tree has uncommitted changes that would
block a switch, surface that to the user rather than stashing silently.

Map the host to its CLI/API:

- `github.com` (or GitHub Enterprise) → **GitHub**, use `gh`
- `gitlab.com` or self-hosted GitLab → **GitLab**, use `glab` (or the REST API)
- `bitbucket.org` → **Bitbucket**, use the REST API via `curl`

Then locate the open PR/MR for the current branch. Concrete commands for each provider are in
`references/providers.md` — read it for the host you detected. If the matching CLI isn't
installed or authenticated, fall back to the REST API recipes in that file, or ask the user to
authenticate (e.g. suggest they run `! gh auth login`).

### 2. Read ALL reviewer comments

Collect the complete set, not just the obvious ones:

- **Inline review comments** — anchored to `file:line`, organized in threads. Capture each
  comment's ID, file, line, author, and body (the IDs are needed to reply in-thread).
- **Review summary bodies** — the top-level text a reviewer leaves when approving / requesting
  changes / commenting.
- **General PR/MR comments** — the conversation timeline.

Prefer comments left by reviewers (not the PR author). Skip threads already marked
resolved/outdated unless the user asks otherwise. `references/providers.md` has the exact fetch
commands per provider.

### 3. Analyze each comment pragmatically

For every comment, reach a clear verdict before touching anything:

- **Understand the claim.** Read the referenced code yourself — don't trust the comment's
  framing. Confirm the problem is real and still present on the current branch.
- **Decide the disposition:**
  - **Accept** — the comment identifies a genuine improvement or bug. Plan the change.
  - **Accept-with-deviation** — the underlying concern is valid but the reviewer's proposed
    fix isn't the best one. Do the better fix and explain the deviation in your reply.
  - **Decline** — the comment is incorrect, based on a misread, already handled elsewhere,
    out of scope, or would worsen the code. Plan a respectful reply with evidence.
- **Quality and functionality win.** When a suggested change trades correctness, clarity, or
  behavior for mere stylistic compliance, decline or adapt it. Explain the tradeoff.

### 4. Group related comments

Cluster the accepted comments before writing code:

- Same fix in one place → one commit.
- Same pattern/problem repeated across files or lines → one commit covering all of them.
- Distinct, unrelated concerns → separate commits.

This keeps history readable and lets one commit reference resolve several threads.

### 5. Apply changes, one commit per group

For each accepted group:

1. Make the change.
2. **Verify it** — run the relevant tests/build/lint (use the project's task runner; e.g. for
   this repo see `docs/nx-guidelines.md`). Don't commit unverified fixes.
3. Commit following the repo's convention (`docs/git-convention.md` here: conventional commit,
   no co-author attribution, never push to a protected branch without permission).

Keep declined comments out of the commits entirely — they only get replies.

### 6. Push, then reply to every thread

Push the branch so the commits resolve on the remote, then reply in-thread to each comment.
**Reference every commit as a markdown link to the provider's commit page —
``[`<sha>`](<commit_url>)`` — never a bare hash, so the reviewer can click through to the diff.**
First resolve the commit's web URL for the host you detected (`references/providers.md` has the
exact form and the command to fetch it), then build each reply around that link:

- **Accepted (changed):** reply describing what you changed and **link the commit** —
  e.g. ``Fixed in [`4f45597`](https://github.com/acme/app/commit/4f45597) — added a `.catch()`
so a transient role-lookup failure is treated as not-owner, eliminating the unhandled rejection
on the public `/workshops` path.``
  For grouped comments, every thread in the group links the same commit.
- **Accepted-with-deviation:** reply explaining the better fix you applied and why it differs
  from the suggestion, linking the commit.
- **Declined:** reply with respectful, specific reasoning and evidence (point at the code,
  existing handling, or the behavioral tradeoff). No commit reference, because there's no change.

Post replies threaded to the original comment (reply endpoints are in
`references/providers.md`), so the reviewer sees your response in context.

### 7. Summarize for the user

Close with a short recap: which comments were accepted (and the commit each maps to), which
were declined (and why), how the changes were verified, and confirmation that every thread got
a reply. Mention anything you intentionally left for the user to decide.

## Example dispositions

**Accept — real bug.**
Input: "`useHasRole` calls `.then()` with no `.catch()`; a transient 500 raises an unhandled
rejection on the now-public path."
Action: add the `.catch()`, run the suite, commit `fix(user-roles): ...`, push, reply linking
the commit (``[`<sha>`](<commit_url>)``).

**Group — same problem twice.**
Input: two comments flagging an unrestored global mock in two test files.
Action: restore the mock in both files in **one** commit; reply to both threads linking that
single commit.

**Decline — misread.**
Input: "This `useMemo` dependency array is missing `requiredRole`."
Action: confirm `requiredRole` is already in the array on the current branch; reply: "
`requiredRole` is already listed in the dependency array (line 37) — no change needed," with no
commit.
