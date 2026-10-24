# Encapsulation: Scope, Exports, and Access Modifiers

Every identifier should be visible to the smallest possible audience. Default to hidden; promote visibility only when a consumer proves it needs access.

## Minimum Scope, Maximum Cohesion

- `const` always. `let` only when mutation is required. Never `var`.
- Declare variables at point of first use, not top of function.
- Block scope: variables inside `if`/`for` when only used there.
- Helpers in the narrowest file that needs them — no shared `utils/` grab-bag.
- A module's exports relate to a single concept. `useGameState.ts` exports game logic, not scoring.

## Export Discipline

Every function, type, constant starts **unexported**.

| Question                                      | Action         |
| --------------------------------------------- | -------------- |
| Is this consumed outside this file?            | `export`       |
| Only used internally?                          | No `export`    |
| Re-export from barrel (`index.ts`)?            | Only at genuine package boundaries |
| React component internal helpers/types?        | No `export` unless consumed externally |

```ts
// BAD — exporting internals nobody imports
export function checkRow(board: Cell[], row: number): boolean { ... }
export function checkColumn(board: Cell[], col: number): boolean { ... }
export function checkWinner(board: Cell[], gridSize: number): number[] | null { ... }

// GOOD — only the public API is exported
function checkRow(board: Cell[], row: number): boolean { ... }
function checkColumn(board: Cell[], col: number): boolean { ... }
export function checkWinner(board: Cell[], gridSize: number): number[] | null { ... }
```

## TypeScript Access Modifiers

| Modifier    | When                                                            |
| ----------- | --------------------------------------------------------------- |
| `private`   | Default for all class fields and methods                        |
| `readonly`  | Fields assigned once (constructor or declaration)               |
| `protected` | Only for documented subclass extension points (rare)            |
| `public`    | Only for the class's contract — methods other code calls        |

- No `public` keyword on interface implementations (interface members are public by definition).
- `#field` (ES private) for runtime enforcement when class exposed to untrusted code; otherwise `private` keyword suffices.
