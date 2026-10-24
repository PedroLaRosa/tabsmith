# Law of Demeter

Only talk to your immediate friends. Don't reach through objects to access their internals.

## Allowed Call Targets

A method may only call methods on:

1. Itself (`this`)
2. Its parameters
3. Objects it creates
4. Its direct collaborator fields

## The "One Dot" Heuristic

- `a.b()` — fine
- `a.b().c()` — suspicious
- `a.b.c.d()` — violation

## Exceptions

- **Fluent APIs / builders** — designed for chaining, OK
- **Value object access** (`point.x`) — value objects are transparent by design
- **React props** — shallow prop access is fine; excessive drilling is a design smell (fix with context or composition)

## Detection

- Chained property access: `game.board.cells[i].value` — caller knows internal structure of game, board, AND cell
- "Train wreck" calls: `getGame().getBoard().getCellAt(i).getValue()`

## Fix

- **Add method to direct collaborator**: `game.cellValueAt(i)` encapsulates the traversal
- **Tell, don't ask**: tell the object to do the work instead of reading its internals

```ts
// BAD — reaches through three levels
if (game.board.cells[row * gridSize + col] === null) {
  game.board.cells[row * gridSize + col] = currentPlayer
}

// GOOD — tell the object to act
game.makeMove(row * gridSize + col, currentPlayer)
```
