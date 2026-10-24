# Meaningful Names

Names replace comments. If a name needs explanation, rename it.

## Rules

| Context          | Rule                                                  | Example                             |
| ---------------- | ----------------------------------------------------- | ----------------------------------- |
| Variables        | Reveal content — reader never asks "what is this?"    | `remainingCells`, not `r` or `data` |
| Booleans         | Predicate form: `is`, `has`, `can`, `should`          | `isGameOver`, not `gameOver`        |
| Functions        | Verb + object — describes action                      | `checkWinner`, not `winner`         |
| Components       | PascalCase noun — describes what it renders           | `Board`, `Cell`, `Scoreboard`       |
| Types/Interfaces | Noun describing shape — no `I` prefix                 | `BoardProps`, not `IBoardProps`     |
| Constants        | UPPER_SNAKE for true constants, camelCase for derived | `DEFAULT_GRID_SIZE`                 |
| Files            | Match primary export name in kebab-case               | `board.tsx`, `use-game-state.ts`    |

## Anti-Patterns

| Pattern                                                 | Problem                                    |
| ------------------------------------------------------- | ------------------------------------------ |
| Single-letter (`r`)                                     | Zero intent. Exception: `i`/`j` tiny loops |
| Abbreviations (`btn`)                                   | Forces reader to decode                    |
| Type-encoded (`boardArray`)                             | Type system handles this                   |
| Context-free (`data`, `info`, `item`, `temp`, `handle`) | Means nothing                              |
