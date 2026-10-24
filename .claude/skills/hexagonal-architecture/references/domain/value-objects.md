# Value Objects

Value Objects are **immutable** objects defined by their **attributes**, not by identity.

> For code examples in your project's language, see `references/languages/`.

## The Id Value Object

Use a single generic `Id` value object for all entity identifiers instead of separate `OrderId`, `ProductId` types. Why: in this architecture, identifiers share identical behavior (create, generate, equals, toPrimitives) — separate types add boilerplate without meaningful type safety, since cross-entity ID comparisons are caught by the domain logic, not the type system.

Pattern: private constructor, static `create(value)` factory with validation, static `generate()` for new IDs, `equals(other)`, `toPrimitives()`.

## Structure

A Value Object has:
- **Private constructor** with readonly fields
- **Static factory method** (`create`) that validates and returns a new instance
- **Convenience factories** (e.g., `zero()` for Money)
- **Operations** that return new instances (never mutate)
- **`equals(other)`** for equality by attributes
- **`toPrimitives()`** for serialization to plain types

## Immutability

Operations must return new instances, never mutate the original. A method like `add(other)` returns a new Value Object rather than modifying `this`.

## Self-Validation

Value objects validate constraints in factory methods, keeping constructors simple. The factory method throws a `DomainError` if the input is invalid.

## Common Value Objects

- **Identifier**: `Id` (single generic identifier for all entities)
- **Money**: `Money`, `Price`, `Amount`
- **Dates**: `DateRange`, `Period`
- **Contact**: `Email`, `PhoneNumber`, `Address`
- **Quantities**: `Quantity`, `Percentage`
