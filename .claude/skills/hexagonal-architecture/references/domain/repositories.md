# Domain Repositories

> For code examples in your project's language, see `references/languages/`.

## Repository Interface + InMemory Implementation

Repository interfaces and their InMemory implementations live together in `domain/repositories/`.

`Maybe<T>` is a monadic type that replaces `null`/`undefined`. It has two states: `Some(value)` and `None`. Key methods: `isSome()`, `isNone()`, `getOrThrow()`, `fold(onNone, onSome)`, `map(fn)`, `fromNullable(value)`. Use a library or implement a simple version in the project.

Pattern: Define the repository interface and an InMemory implementation in the same file. The InMemory implementation uses an in-memory collection (map/dictionary) and is the default test double for UseCase tests.

## Repository Characteristics

- **Domain language**: Use domain terms (`save`, `findById`), not database terms (`insert`, `findOne`)
- **Entity-centric**: Work with domain entities, not primitives or DTOs
- **No implementation details**: The interface does not reveal storage mechanism
- **Return `Maybe<T>`**: For single-entity lookups instead of null/undefined

## Real Adapter (Infrastructure)

Real adapters implement the repository interface with actual database logic. They live in `infrastructure/adapters/`. They include private mapping methods (`toDocument`, `toDomain`) to convert between domain entities and storage format.

## When to Use Each

| Context | Use |
|---------|-----|
| UseCase unit tests | InMemoryRepository |
| Repository adapter tests | Real adapter + Real DB |
| E2E tests | Real adapter + Real DB |
| Production | Real adapter |

## File Structure

```
src/[module]/
├── domain/
│   └── repositories/
│       └── [Entity]Repository    # Interface + InMemory (same file)
└── infrastructure/
    └── adapters/
        └── [Store][Entity]Repository  # Real adapter for production
```
