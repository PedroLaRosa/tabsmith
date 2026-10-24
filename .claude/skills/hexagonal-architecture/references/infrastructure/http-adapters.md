# HTTP API Design Detail

> For code examples in your project's language, see `references/languages/`.

## Controller Structure

A Controller receives Use Cases via constructor and exposes HTTP endpoint handlers. Each handler:
1. Extracts and validates input (types and nulls only)
2. Calls the appropriate Use Case
3. Returns the response or handles errors

## Input Validation

Validate only types and nulls in the Controller. Business rules are validated in the domain. Never duplicate domain validation in the controller — let the Use Case and domain objects enforce business rules and throw `DomainError`.

## Error Handling in Controllers

Map DomainError types to HTTP status codes:

| DomainError type | HTTP Status |
|------------------|-------------|
| notFound | 404 |
| validation | 422 |
| other | 400 |

Unknown/technical errors return 500 with a generic message (log the real error).

## Response Format

### Success
- **GET**: Return the resource directly (200)
- **POST**: Return the created resource (201 Created)
- **DELETE**: No body (204 No Content)

### Error
Return an object with an `error` field containing the message.
