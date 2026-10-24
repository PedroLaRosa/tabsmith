# Classes, Rich Models, and Composition Over Inheritance

Classes encapsulate data AND behavior. Reuse comes from composition (injecting collaborators), not inheritance (extending base classes).

## Class Design

- Hide data, expose behavior. Prefer methods that answer questions (`isGameOver()`, `canMove(index)`) over raw field access.
- Use `class` when you need identity, lifecycle, or encapsulated mutable state.
- Use plain objects/types for immutable, behavior-free data (DTOs, props, config).
- React: functional components + hooks are the standard. Classes are legacy.

## Avoid Anemic Models

Anemic model = class with only fields/getters, all logic in external functions.

**Detection:**

- Class has only fields + getters
- Separate file has functions taking the class as parameter, operating on its fields

```ts
// BAD — anemic Board + external logic
class Board {
  cells: Cell[];
}
function isFull(board: Board): boolean {
  return board.cells.every((c) => c !== null);
}
function placeMarker(board: Board, i: number, p: Player): void {
  board.cells[i] = p;
}

// GOOD — behavior lives with the data
class Board {
  private readonly cells: Cell[];
  isFull(): boolean {
    return this.cells.every((c) => c !== null);
  }
  placeMarker(index: number, player: Player): void {
    this.cells[index] = player;
  }
}
```

In React with hooks, the same principle applies: colocate logic with the state it operates on. `useGameState` owns board state AND game logic — don't split state in one hook and logic in a separate utility.

## Composition Over Inheritance

**Default: compose.** Inject collaborators via constructor or function parameters.

| Approach    | When                                                       |
| ----------- | ---------------------------------------------------------- |
| Composition | Default. Always. Swap behavior via injected interface.     |
| Inheritance | Only: genuine "is-a" AND depth ≤ 2 levels. Extremely rare. |

- Prefer `interface` for contracts, not `abstract class`.
- React: never use class inheritance for code reuse. Use hooks or render props.
- Strategy pattern: swap behavior via injected interface, not by overriding methods.

```ts
// BAD — inheritance for code reuse
class TicTacToeGame extends BaseGame {
  override checkWin(): WinResult { ... }
}

// GOOD — composition via injection
class TicTacToeGame {
  private readonly winChecker: WinChecker
  constructor( winChecker: WinChecker) {
    this.winChecker = winChecker
  }
  checkWin(): WinResult { return this.winChecker.check(this.board) }
}
```
