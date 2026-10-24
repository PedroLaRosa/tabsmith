# Domain Services

Domain Services contain business logic that does not naturally belong to a specific entity or value object. They are stateless functions that operate on domain objects.

> For code examples in your project's language, see `references/languages/`.

## When to Use

Use a Domain Service when:
- The logic involves multiple entities
- The logic does not naturally belong to any single entity
- A business rule spans aggregates

## Characteristics

- **Stateless**: Pure functions, same input always produces same output
- **Domain language**: Named using domain terminology
- **Pure domain logic**: No infrastructure concerns (no DB, no HTTP)
- **Operates on domain objects**: Receives and returns entities/value objects

## Structure

Domain services are functions, not classes. Group related functions in a module. They live in `domain/services/`.

## Domain Service vs Use Case

| Domain Service | Use Case |
|----------------|----------|
| Pure functions | Orchestrates application flow |
| No external dependencies | Uses ports (repositories, external services) |
| Lives in `domain/services/` | Lives in `application/` |
| Called by entities or use cases | Entry point from infrastructure |
