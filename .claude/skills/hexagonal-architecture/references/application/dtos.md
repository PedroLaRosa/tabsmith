# DTOs & Boundaries

> For code examples in your project's language, see `references/languages/`.

## Where DTOs Live

DTOs live in the Application layer, next to the UseCases that use them.

```
src/[module]/
├── application/
│   ├── CreateOrderUseCase
│   ├── GetOrderUseCase
│   ├── CreateOrderRequest     # Request DTO (if >3 params)
│   ├── OrderDTO               # Response DTO
│   └── ports/
└── ...
```

## Response DTOs

Response DTOs are plain data structures with only primitive fields (string, number, boolean, arrays, nested DTOs). They represent the output contract of a Use Case.

### Mapping in UseCase

The Use Case maps domain entities to DTOs using `toPrimitives()`. If the DTO needs a different structure than `toPrimitives()` provides, the Use Case selects and reshapes the needed fields.

## DTO Characteristics

- **Plain objects**: No methods, no behavior
- **Primitive types only**: string, number, boolean, arrays, nested DTOs
- **No domain types**: No Money, no Id, no entities
- **Serializable**: Can be directly serialized to JSON or any transport format
