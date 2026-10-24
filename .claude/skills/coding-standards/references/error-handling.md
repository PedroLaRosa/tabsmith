# Error Handling

Errors are first-class citizens. Handle explicitly at boundaries, never swallow, never use null as an error signal.

## TypeScript Patterns

**Discriminated unions** (preferred for domain logic):
```ts
type MoveResult =
  | { ok: true; board: Board }
  | { ok: false; reason: 'cell-taken' | 'game-over' }
```

**Thrown exceptions** (infrastructure / unexpected errors only):
- Always throw `Error` subclasses, never bare strings
- Catch at boundary (route handler, error boundary), not inline

**Exhaustive switch** — catch missing cases at compile time:
```ts
function assertNever(x: never): never {
  throw new Error(`Unexpected: ${x}`)
}
```

## Rules

| Rule                        | Detail                                                         |
| --------------------------- | -------------------------------------------------------------- |
| No empty catch blocks       | `catch (e) {}` = swallowed error. Always handle or rethrow.    |
| No null-as-error            | Caller cannot distinguish "no data" from "error". Use Result.  |
| Validate at boundaries      | Component props via TypeScript. User input in handlers.        |
| Fail fast                   | Precondition violated → throw immediately. Don't proceed.      |
| React error boundaries      | Wrap route-level components. Let unexpected errors bubble.      |

## Example

```ts
// BAD — null hides the error, caller doesn't know why
function makeMove(index: number): Board | null {
  if (cells[index] !== null) return null
  // ...
}

// GOOD — explicit reason, caller handles the case
function makeMove(index: number): MoveResult {
  if (cells[index] !== null) return { ok: false, reason: 'cell-taken' }
  // ...
  return { ok: true, board: updatedBoard }
}
```
