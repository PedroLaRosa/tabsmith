# Framework: Vitest

Always explicitly import every utility used:

```ts
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest'
```

- Import only what you use
- Never rely on globals even if `globals: true` in config
