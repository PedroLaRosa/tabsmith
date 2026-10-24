---
name: hexagonal-architecture
description: Hexagonal architecture guidelines for organizing code into domain, application, and infrastructure layers with vertical slicing by business module.
---

# Hexagonal Architecture

Code organized by business modules (vertical slicing), not by technical layer. The directory structure "screams" the business domain — folders named after business concepts (orders, invoices), not technical layers (controllers, services).

## Module Structure

Check the [module structure reference here](./references/module-structure.md)

## Dependency Rule

`Infrastructure → Application → Domain` — deps always point inward.

## Layers

| Layer          | Contains                                                                                                                                | Depends on           |
| -------------- | --------------------------------------------------------------------------------------------------------------------------------------- | -------------------- |
| Domain         | Entities, ValueObjects, DomainServices, Repositories (interface+InMemory), DomainError (see [Design principles](../design-principles/)) | Nothing              |
| Application    | UseCases, external service Ports                                                                                                        | Domain only          |
| Infrastructure | Adapters, Controllers/Handlers, frameworks                                                                                              | Application + Domain |

## Ports & Adapters

- **Repositories** (Domain): contract + InMemory impl in same file — eliminates mocks in UseCase tests. See `references/domain/repositories.md`
- **External Ports** (Application): interfaces for external services
- **Adapters** (Infrastructure): concrete port implementations. See `references/infrastructure/module-di.md`

## Cross-Module Imports

Allowed: domain entities, value objects, ports. Forbidden: UseCases across modules.

## Naming

| Layer          | Suffixes                                                  |
| -------------- | --------------------------------------------------------- |
| Domain         | Entity (implicit), ValueObject, DomainService, Repository |
| Application    | UseCase, Service, DTO, Port                               |
| Infrastructure | Repository (impl), Adapter, Controller, Handler, Client   |

One file per class. File name = class name (PascalCase).

## References

- **Domain**: [entities](./references/domain/entities.md), [value objects](./references/domain/value-objects.md), [repositories](./references/domain/repositories.md), and [domain services](./references/domain/domain-services.md)
- **Application**: [use cases](./references/application/usecases.md), and [DTOs](./references/application/dtos.md)
- **Infrastructure**: [Module DI](./references/infrastructure/module-di.md), [HTTP adapters](./references/infrastructure/http-adapters.md)
- **Structure**: [Module structure](./references/module-structure.md)
- **TypeScript**: [Typescript](./references/languages/typescript.md) — load when writing code

## Non-Negotiable Rules

- No framework/library imports in Domain
- No upward imports (Domain ← App ← Infra)
- No cross-module UseCase imports
- No business logic in Infrastructure
- No god classes (OrderManager, ProductHelper)
- Create port before adapter
- Validate dependency direction on every import
- Group by business module, not technical layer
- Consult Tech Lead before creating new modules
