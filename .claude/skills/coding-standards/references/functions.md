# Function Design

A function does one thing, takes few arguments, and communicates its contract through its signature.

## Rules

| Rule                     | Guideline                                                       |
| ------------------------ | --------------------------------------------------------------- |
| Single responsibility    | One reason to change. `checkWinner` does NOT also update score. |
| Size                     | Prefer ≤ 20 lines. Longer → extract.                            |
| Parameters               | ≤ 3. More → options object.                                     |
| Command/query separation | Functions either do something OR return something. Never both.  |
| Abstraction level        | All statements at same level. Don't mix DOM and business logic. |
| Early returns            | Guard clauses at top. No deep nesting.                          |
| Purity                   | No side effects when possible. Same inputs → same outputs.      |
| No flag arguments        | `processGame(board, shouldReset)` → split into two functions.   |
