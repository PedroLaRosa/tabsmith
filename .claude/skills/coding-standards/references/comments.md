# Comments

Good code is self-documenting. Comments explain "why", never "what".

## Allowed

| Type                       | Example                                                         |
| -------------------------- | --------------------------------------------------------------- |
| Why (intent non-obvious)   | `// Win requires N in a row because grid is configurable`       |
| TODO with ticket           | `// TODO(#42): support AI opponent`                             |
| JSDoc on public API        | Only on exported functions/types consumed by other modules       |
| Regex/algorithm clarification | `// Diagonal: indices where row === col`                     |

## Banned

| Type                       | Why                                                             |
| -------------------------- | --------------------------------------------------------------- |
| Paraphrasing code          | `// increment counter` before `counter++` — rename instead      |
| Commented-out code         | Delete it. Git has history.                                     |
| Section separators         | `// ===== HELPERS =====` — extract to a file instead            |
| Journal comments           | `// 2024-01-15: fixed bug` — git log has this                   |
| Closing-brace comments     | `} // end if` — reduce nesting instead                          |
| JSDoc on private functions | The name should suffice internally                              |

## Decision Rule

Writing a comment that explains "what" the code does? Stop. Rename or restructure until the code says it.
