# Use Cases Detail

> For code examples in your project's language, see `references/languages/`.

## Structure

A Use Case class has:
- **Constructor** receiving ports (repositories, external services) as dependencies
- **Single `execute` method** that orchestrates the business flow
- **Private `toDTO` method** that maps domain entities to response DTOs via `toPrimitives()`

Flow: validate input -> load domain objects via ports -> execute domain logic -> persist via ports -> return DTO.

## Terminal-Friendly Parameters

- For 1-3 parameters: use primitive parameters directly (string, number)
- For >3 parameters: use a request object/interface with primitive fields
- Always convert primitives to domain types (Id, Money) inside the Use Case

## Single Entry Point

One Use Case = one action. Never create a Use Case with multiple methods (create, update, delete). Instead, create separate Use Cases: `CreateOrderUseCase`, `UpdateOrderUseCase`, `DeleteOrderUseCase`.

## Dependencies

Use Cases depend only on:
- **Ports** (repository interfaces, external service interfaces) — injected via constructor
- **Domain Services** (pure functions) — imported directly
- **Domain objects** (entities, value objects) — used within `execute`

Use Cases never depend on infrastructure or other Use Cases.
