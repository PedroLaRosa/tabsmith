# Inside-Out DDD Layer Order

Build innermost first. No layer until inner layers green.

## Layers

| #   | Layer           | Examples                             | Deps                              |
| --- | --------------- | ------------------------------------ | --------------------------------- |
| 1   | Value Objects   | `Money`, `Email`, `OrderId`          | None                              |
| 2   | Entities        | `Order`, `Customer`                  | VOs                               |
| 3   | Domain Services | `PricingService`                     | Entities + VOs                    |
| 4   | Use Cases       | `PlaceOrder`                         | Domain services + repo interfaces |
| 5   | InMemory Repos  | `InMemoryOrderRepo`                  | Domain interfaces                 |
| 6   | Infrastructure  | `PostgresOrderRepo`, `HttpPaymentGw` | Use cases + external              |

## Violation

Target layer w/ inner not green:

```
[NAVIGATOR] Layer violation: cannot write <target> before <missing>. Define <missing> first.
```

Stop, redirect.

## Test Doubles

Repo/stateful collaborator → `InMemory*` implementation. No mocks. Fakes exercise real behavior, not stub expectations.

- `InMemoryOrderRepository` implements `OrderRepository`
- `InMemoryNotificationService` implements `NotificationService`

Live alongside tests or `test-support` module. Same interface contract as prod.
