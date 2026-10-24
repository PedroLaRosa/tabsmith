# Module Structure Details

> For language-specific file naming and code examples, see `references/languages/`.

## Full Directory Tree

```
src/
└── [module-name]/
    ├── domain/                        # Business core (no external dependencies)
    │   ├── entities/
    │   │   └── [Entity]
    │   ├── value-objects/
    │   │   └── [ValueObject]
    │   ├── services/
    │   │   └── [DomainService]
    │   └── repositories/
    │       └── [Repository]           # Interface + InMemory implementation
    ├── application/                   # Use cases and ports for external services
    │   ├── [UseCase]
    │   ├── [Request]                  # Request DTO (if >3 params)
    │   ├── [DTO]                      # Response DTO
    │   └── ports/
    │       └── [ExternalService]      # External service ports (driven)
    ├── infrastructure/
    │   ├── [module]-module            # registerModule(app, deps) — wiring + routes
    │   ├── [module]-schema            # ensureSchema(pool) — DDL for this module
    │   ├── adapters/                  # Adapters (concrete implementations)
    │   │   ├── [RepositoryImpl]
    │   │   └── [ApiClient]
    │   └── http/                      # Primary port (driver)
    │       └── [Controller]
    └── tests/
        ├── unit/
        │   ├── [Entity].test
        │   ├── [ValueObject].test
        │   └── [UseCase].test
        ├── integration/
        │   └── [Repository].integration.test
        └── e2e/
            └── [Controller].e2e.test
```

## Cross-Module Communication Rules

### Allowed imports from another module

- Domain entities and value objects
- Ports (interfaces)
- Domain repositories (interface + InMemory)

### Forbidden imports from another module

- UseCases (a UseCase must never call another UseCase)
- Infrastructure adapters
- Controllers

## Ports and Adapters Placement

### Repository (Domain)

Interface + InMemory implementation in `domain/repositories/`. Same file.

### External Service Port (Application)

Interface in `application/ports/`. Defines the contract for external services (email, payments, notifications).

### Adapter (Infrastructure)

Concrete implementation in `infrastructure/adapters/`. Implements the port interface with real external service logic. Includes mapping between domain entities and external formats.
