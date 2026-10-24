# Transformation Priority Premise (TPP)

Use lowest-index transformation that passes current failing test. Jumped indices = wrote too much code → back up, use simpler.

## Why

Keeps GREEN minimal. Small safe steps. Jumping = guessing behavior no test demands. Smaller transformations surface design pressure earlier → cleaner code.

## Table

| #   | From → To                | Example                               |
| --- | ------------------------ | ------------------------------------- |
| 0   | `{}`→nil/null            | Empty fn returns null                 |
| 1   | nil→constant             | `return null`→`return 0`              |
| 2   | constant→constant+       | `return 0`→`return 1`                 |
| 3   | constant→scalar          | `return 1`→`return n`                 |
| 4   | statement→statements     | Add second unconditional step         |
| 5   | unconditional→if         | `return x`→`if(c) return x; return y` |
| 6   | scalar→array             | Single→collection                     |
| 7   | array→container          | Array→typed collection/object         |
| 8   | statement→tail-recursion | Loop via function call                |
| 9   | if→while                 | Conditional→loop                      |
| 10  | expression→function      | Inline→extracted function             |
| 11  | variable→assignment      | Read-only→mutable state               |

## Use

1. **GREEN** — Pick lowest # that passes. Constant before variable. `if` before `while`. Specific before general.
2. **RE-EVALUATE** — Skipped simpler transformation? Write test that forces it.
3. **Navigator** — Driver jumped #1→#5? Call out. Too much code.
