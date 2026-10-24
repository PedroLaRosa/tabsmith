---
name: session-relay
description: "Sumarize this conversation into a relay brief a fresh agent can continue. EXAMPLES: 'relay this session'"
argument-hint: "What's the goal of the next session?"
---

# Session Relay

Write a short brief summarising the current conversation so a fresh agent can continue the work without re-reading everything.

## Steps

1. Read if a relay brief exist in .relays/
2. if any remove it. Unless the user request not to do so.
3. Capture only what the next agent needs to resume: the goal, progress, next action, and any decisions or gotchas that aren't obvious from the code.
4. DO NOT DUPLICATE ARTIFACTS CONTENT. Instead reference them by using path or URL
5. DO NOT INCLUDE SENSITIVE INFORMATION (API keys, passwords, tokens, and personally identifiable data)
6. Suggest skills to load when executing the relay brief
7. Save relay briefs to .relays/ at the repo root.
8. Write the brief to a descriptively named markdown file, then report its absolute path so it can be handed over.

## Notes

- If the user passed an argument, treat it as a description of what the next session will focus on and tailor the brief to that.
- Keep it short and faithful — a relay brief, not a transcript. State plainly what is done, what is in progress, and what is blocked.
