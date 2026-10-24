# AAA Pattern

Three phases separated by blank line:

```
const repo = new InMemoryOrderRepository()
const service = new OrderService(repo)
const order = Order.create({ productId: 'p1', quantity: 2 })

const result = service.place(order)

expect(result.status).toBe('confirmed')
```

- No AAA comments in actual tests (example above was illustrative only)
- One logical assertion per test. Multiple `expect` fine if verifying same outcome. Two unrelated outcomes = violation
- Arrange creates all objects from scratch — never shared `beforeAll` mutable state
- Arrange >10 lines → extract factory helper, not shared fixture
