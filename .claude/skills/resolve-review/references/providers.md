# Provider recipes

Concrete commands for finding the PR/MR, reading every reviewer comment, and replying in-thread.
Read only the section for the host you detected from `git remote get-url origin`.

Placeholders: `<owner>/<repo>` (the repo slug), `<num>` (PR/MR number), `<branch>` (the target
branch — the argument the user passed, or the current branch when none was given),
`<comment_id>` (the inline comment to reply to), `<sha>` (the commit you're referencing), and
`<commit_url>` (its web page on the provider).

**Always reference a commit as a markdown link — ``[`<sha>`](<commit_url>)`` — never a bare
hash**, so the reviewer can click through to the diff. Each provider section below shows how to
resolve `<commit_url>` for that host before you post the reply.

---

## GitHub (`gh`)

### Find the PR for the target branch

```bash
gh pr view <branch> --json number,title,url,state,headRefName   # omit <branch> for the current one
```

If that fails, list open PRs: `gh pr list --head <branch> --json number,title,url`.

### Read all comments

```bash
# Inline review comments (file:line threads) — keep the id to reply in-thread
gh api repos/<owner>/<repo>/pulls/<num>/comments \
  --jq '.[] | "ID:\(.id)\nFILE:\(.path):\(.line)\nUSER:\(.user.login)\nBODY:\(.body)\n---"'

# Review summary bodies (approve / request-changes / comment)
gh api repos/<owner>/<repo>/pulls/<num>/reviews \
  --jq '.[] | "USER:\(.user.login)\nSTATE:\(.state)\nBODY:\(.body)\n---"'

# General timeline comments
gh api repos/<owner>/<repo>/issues/<num>/comments \
  --jq '.[] | "USER:\(.user.login)\nBODY:\(.body)\n---"'
```

Get `<owner>/<repo>` from `gh repo view --json nameWithOwner -q .nameWithOwner`.

### Resolve the commit URL

```bash
gh api repos/<owner>/<repo>/commits/<sha> --jq '.html_url'
# or build it directly: <repo_web_url>/commit/<sha>
#   repo_web_url comes from `gh repo view --json url -q .url`
```

### Reply in-thread to an inline comment

Link the SHA to the commit page so the reviewer can click straight to the diff:

```bash
gh api repos/<owner>/<repo>/pulls/<num>/comments \
  -f body="Fixed in [\`<sha>\`](<commit_url>) — <what changed and why>." \
  -F in_reply_to=<comment_id>
```

### Reply on the general conversation (not tied to a line)

```bash
gh pr comment <num> --body "<message>"
```

### Optionally resolve a review thread

Thread resolution is GraphQL-only. Reply first; resolve only if the user asks:

```bash
gh api graphql -f query='mutation($id:ID!){resolveReviewThread(input:{threadId:$id}){thread{isResolved}}}' -f id=<thread_node_id>
```

Thread node IDs come from a GraphQL `reviewThreads` query on the PR.

---

## GitLab (`glab`)

### Find the MR for the target branch

```bash
glab mr list --source-branch <branch>   # target a specific branch
# or
glab mr view                            # the current branch's MR, when no branch was given
```

### Read all comments (discussions)

```bash
# All discussion threads (inline + general); notes[] holds each comment with author + body
glab api projects/:id/merge_requests/<num>/discussions

# Or the flat notes list
glab api projects/:id/merge_requests/<num>/notes
```

`:id` is URL-encoded `group/project`; `glab` fills `:id` automatically inside a repo. Inline
notes carry a `position` object (`new_path`, `new_line`); general notes don't.

### Resolve the commit URL

```bash
glab api projects/:id/repository/commits/<sha> --jq '.web_url'
# or build it directly: <project_web_url>/-/commit/<sha>
#   project_web_url comes from `glab api projects/:id --jq '.web_url'`
```

### Reply in-thread to a discussion

Link the SHA to the commit page so the reviewer can click straight to the diff:

```bash
glab api --method POST \
  projects/:id/merge_requests/<num>/discussions/<discussion_id>/notes \
  -f body="Fixed in [\`<sha>\`](<commit_url>) — <what changed and why>."
```

### Add a general comment

```bash
glab mr note <num> --message "<message>"
```

---

## Bitbucket (REST API via `curl`)

No first-class CLI; use the Cloud REST API with an app password or `BITBUCKET_TOKEN`.
Base: `https://api.bitbucket.org/2.0/repositories/<workspace>/<repo>`.

### Find the PR for the current branch

```bash
curl -s -u "$BB_USER:$BB_APP_PASSWORD" \
  "https://api.bitbucket.org/2.0/repositories/<workspace>/<repo>/pullrequests?q=source.branch.name=%22<branch>%22&state=OPEN" \
  | jq '.values[] | {id, title, links}'
```

### Read all comments

```bash
curl -s -u "$BB_USER:$BB_APP_PASSWORD" \
  "https://api.bitbucket.org/2.0/repositories/<workspace>/<repo>/pullrequests/<num>/comments?pagelen=100" \
  | jq '.values[] | {id, file: .inline.path, line: .inline.to, user: .user.display_name, body: .content.raw}'
```

Comments with an `inline` object are anchored to `file:line`; without it they're general.

### Resolve the commit URL

The web page for a commit is `https://bitbucket.org/<workspace>/<repo>/commits/<sha>`.

### Reply in-thread to a comment

Bitbucket comment bodies are Markdown — link the SHA to the commit page so the reviewer can
click straight to the diff:

```bash
curl -s -u "$BB_USER:$BB_APP_PASSWORD" -X POST \
  "https://api.bitbucket.org/2.0/repositories/<workspace>/<repo>/pullrequests/<num>/comments" \
  -H "Content-Type: application/json" \
  -d '{"content":{"raw":"Fixed in [`<sha>`](<commit_url>) — <what changed and why>."},"parent":{"id":<comment_id>}}'
```

Omit `parent` to post a top-level comment.

---

## Generic REST fallback

If no CLI is available, the same data is reachable over plain HTTPS with a token:

- GitHub: `https://api.github.com/repos/<owner>/<repo>/pulls/<num>/comments` (header
  `Authorization: Bearer $GITHUB_TOKEN`).
- GitLab: `https://<host>/api/v4/projects/<url-encoded-id>/merge_requests/<num>/discussions`
  (header `PRIVATE-TOKEN: $GITLAB_TOKEN`).
- Bitbucket: see above.

Reply by POSTing to the same comment endpoints with the in-reply/parent field set as shown.
