# DI + Strategy Pattern

Code only testable with mocks = coupled to infrastructure. Fix structurally:

1. Interface for every side-effect dependency (e.g. `OrderRepository`, `Clock`)
2. Inject via constructor — never instantiate inside class
3. Tests → `InMemory*`/`Fake*` implementation
4. Prod → real implementation via composition root

Announce: `[TESTING-STANDARD] Applying DI + Strategy: injecting <InMemory*> for <Interface>`
