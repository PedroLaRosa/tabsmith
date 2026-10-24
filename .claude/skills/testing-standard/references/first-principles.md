# FIRST Principles

Verify all five before writing any test:

| Property        | Meaning                                          | Red flag                                   |
| --------------- | ------------------------------------------------ | ------------------------------------------ |
| Fast            | Milliseconds. No real I/O, sleeps, network.      | >100ms without explanation                 |
| Independent     | No dependency on other tests' state/order.        | Shared mutable vars across tests           |
| Repeatable      | Same result every machine, every run.             | Date/time, unseeded random, env vars       |
| Self-Validating | Pass/fail — no manual inspection.                 | Logging instead of asserting               |
| Timely          | Written before/alongside prod code, never after.  | Retroactive test to pass CI                |

Violates any → redesign before writing.
