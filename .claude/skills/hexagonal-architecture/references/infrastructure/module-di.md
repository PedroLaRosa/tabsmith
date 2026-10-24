# Module-Scoped Dependency Injection

Use module-scoped factory functions instead of a monolithic Factory class or DI containers. Why: a single Factory grows unbounded (~10 lines per module). DI containers add magic (decorators, metadata reflection, runtime resolution). Module factories keep wiring explicit, co-located with the module, and scalable.

> For code examples in your project's language, see `references/languages/`.

## SharedDependencies

A plain interface carrying shared infrastructure every module may need:

```
SharedDependencies {
  pool    — database connection pool (singleton)
  logger  — application logger (singleton)
}
```

Built once in `main` (the Composition Root), passed to every module factory.

## Module Registration Function

Each module exports one `register<Module>Module(app, deps)` function in `infrastructure/`:

- Receives the Express app and `SharedDependencies`
- Creates its own internal dependency graph (repository → use case → controller)
- Registers its own routes on the app
- Lives in `<module>/infrastructure/<module>-module`

## Wiring Flow

```
main (Composition Root)
  → build SharedDependencies { pool, logger }
  → createServer(deps)
      → registerHealthModule(app, deps)
      → registerOrdersModule(app, deps)
      → registerUsersModule(app, deps)
```

## File Locations

```
src/
├── shared/infrastructure/
│   ├── shared-dependencies    # SharedDependencies interface
│   ├── postgres               # connectToPostgres() → Pool
│   └── server                 # createServer(deps) — calls register*Module
├── orders/
│   └── infrastructure/
│       ├── orders-module      # registerOrdersModule(app, deps)
│       └── orders-schema      # ensureOrdersSchema(pool)
└── main                       # Composition Root
```

## Adding a New Module

1. Create `<module>/infrastructure/<module>-module` with `register<Module>Module`
2. Create `<module>/infrastructure/<module>-schema` if module has DB tables
3. In `server`: add import + `register<Module>Module(app, deps)` (2 lines)
4. In `main`: add `await ensure<Module>Schema(pool)` (1 line)

## Testing

- **Unit/Integration tests**: wire InMemory implementations directly in the test — no shared state, no factory override needed
- **E2e tests**: build `SharedDependencies` with a test pool, pass to `createServer(deps)` — standard constructor injection at the Composition Root level
- No `setPool()` or mutable static state needed

## Why Not a DI Container Library?

- No decorators enabled (`erasableSyntaxOnly: true` in TS 6)
- No reflection metadata needed
- Compile-time type safety — if you forget a dependency, the compiler catches it
- Explicit, readable, debuggable — the dependency graph is plain code
- Zero dependencies added
