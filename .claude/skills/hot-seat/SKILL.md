---
name: hot-seat
description: "Relentless one-question-at-a-time interview that stress-tests a plan, design, or idea before any code is written. Trigger on: 'hot seat', 'put me in the hot seat', 'grill me', 'stress-test this plan', 'poke holes in this', 'challenge my design', 'interview me about this plan'. Skip for: writing a plan with no interview, implementing, reviewing a diff (→ panel-review)."
argument-hint: "What plan, design, or idea goes in the hot seat?"
---

# Hot Seat

Put the user's plan in the hot seat: question it until every decision in it was made on purpose, by the user. Plans break where they were vague, and it is cheaper to find those spots in conversation than in code.

## Outcome

A plan the user could defend to someone who wasn't in the room: each decision settled or explicitly parked, nothing silently assumed. This skill writes no code and no files unless the user asks for a plan document at the end.

## Step 1: Read before asking

Restate the plan in two or three sentences so the user can correct your reading of it.

Then look up everything the environment can already answer: the code the plan touches, `docs/`, `AGENTS.md`, existing plans and specs. Facts are your job. Asking the user something you could have read wastes their attention. Use a sub-agent for wide searches so the interview context stays clean.

## Step 2: Map the decision tree

Treat the plan as a tree: each decision opens the decisions that depend on it (choosing a storage format opens migration, versioning, and corruption handling). An **open** question is one whose prerequisites are all settled. Anything that hinges on an unanswered question waits.

## Step 3: Ask one question, then stop

Ask a single open question and wait for the answer. One at a time, because each answer reshapes the tree, and a wall of questions gets skimmed instead of thought about.

Choose the open question that constrains the most beneath it. Details come last.

```
**Q<n> · <branch>**: <the question, with concrete options when there are any>

Recommend: <your answer> — <why, in one line>
```

Always commit to a recommendation, and word the question so a plain "yes" accepts it. A recommendation gives the user something to push against, which is faster and more honest than an open-ended prompt.

After each answer, update the tree: mark what it settled, add what it opened, drop what it made irrelevant.

## Step 4: Don't let weak answers through

This is the hot part. An interview that only collects agreement produces a plan you wrote and the user nodded at.

- Vague answer ("it depends", "we'll handle it", "should be fine") → ask what it depends on, or what happens in the specific failing case.
- Answer that contradicts an earlier one, the code, or a doc → put the two side by side and ask which one gives.
- Several yeses in a row → say so, and ask which recommendation they would least like to defend.
- "I don't know" → a real answer. Park the question with what would settle it (a spike, a measurement, a person to ask) and move on.
- Question that talking can't settle (how something should look or feel) → propose a throwaway prototype instead of rephrasing the question.
- Tree growing faster than it closes → the scope is too big for one session. Say so, propose a split, and grill one piece.

## Step 5: Close

When no open questions remain, show the result:

- **Decided**: each decision, with the reason where it was contested
- **Parked**: each open item and what would settle it
- **Out of scope**: what was explicitly cut

Ask the user to confirm this is the shared understanding. On confirmation, offer to save it as `docs/<topic>-plan.md` alongside the existing plans. Then stop.

## Rules

- Facts are yours, decisions are the user's. Never answer your own question and move on, even when the recommendation seems obvious.
- One question per message. No "and also".
- No implementation, no file edits, and no plan document until the user confirms the close. Running out of questions is not permission to build.
- The user can end early ("wrap up"): go straight to Step 5 and list whatever is unvisited under Parked.
