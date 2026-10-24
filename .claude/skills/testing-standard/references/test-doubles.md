# Test Doubles — Prefer Real Over Fake

Stop at first option that works:

| Priority | Type           | When                                                          |
| -------- | -------------- | ------------------------------------------------------------- |
| 1 (best) | Real object    | Fast, no side effects                                         |
| 2        | InMemory Fake  | Repositories, event stores, caches — implement interface      |
| 3        | Stub           | Read-only collaborators returning canned data — never domain  |
| 4        | Mock           | Only: external HTTP, email/SMS, payments, system clocks       |

Hard rules:
- Never mock what you own (domain objects, services, repos)
- Never mock aggregates or value objects
- "Real one too hard to set up" → design smell, fix design first
