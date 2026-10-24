# Domain Entities

Entities are objects with **identity** and **lifecycle**. Two entities are equal if they have the same identity, regardless of their attributes.

> For code examples in your project's language, see `references/languages/`.

## Characteristics

- **Identity**: Has a unique identifier (use `Id` value object)
- **Lifecycle**: Can change state over time while maintaining its identity
- **Behavior**: Encapsulates business logic related to itself
- **Consistency**: Protects its invariants (business rules that must always be true)
- **Serializable**: Provides `toPrimitives()` method for serialization

## Structure

An Entity has:
- **Private constructor** with identity and state fields
- **Static factory method** (`create`) for construction with initial state
- **Behavior methods** that enforce invariants before state transitions
- **`toPrimitives()`** returning a plain object with only primitive types

## Identity

Use the generic `Id` value object for all entity identifiers. Never use primitive types (string, int) directly as identity.

## toPrimitives Method

Every entity must provide a `toPrimitives()` method that returns a plain object with only primitive types (string, number, boolean, arrays, nested plain objects). This is the single serialization contract — UseCases call `toPrimitives()` to build DTOs, and infrastructure adapters use it for persistence mapping. No coupling to specific DTO or document types.

## Invariants

Invariants are validated in factory methods, keeping constructors simple (assignment only). The factory method throws a `DomainError` if invariants are violated.

## Complete Objects (No Setters)

Entities must be complete at construction. No setters, no partial initialization. Encapsulate behavior through domain methods that enforce invariants on state transitions.
