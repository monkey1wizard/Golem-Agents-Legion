# TypeScript Conventions

## Core Principles

- **Strict typing**: `strict` mode enabled. Avoid `any`; use `unknown` if necessary.
- **Modern JS/TS**: ES6+ features (arrow functions, destructuring, spread).
- **Immutability**: Prefer `const` over `let`. No `var`.

## Type System

- `interface` for public API definitions and objects
- `type` for unions, intersections, or primitives
- Descriptive generic names (`TData`, `TResponse`) for complex functions
- Leverage utility types: `Partial`, `Pick`, `Omit`, `Record`

## Naming

| Category | Convention | Example |
| --- | --- | --- |
| Variables, functions | `camelCase` | `fetchUserData` |
| Classes, interfaces, types | `PascalCase` | `UserResponse` |
| Global constants | `SCREAMING_SNAKE_CASE` | `MAX_RETRIES` |
| Booleans | `is`/`has`/`should` prefix | `isLoading`, `hasError` |

## Async

- Always `async/await` over `.then()` chains
- `Promise.all` for parallel independent tasks

```typescript
async function getUserData(id: string): Promise<User> {
  const user = await db.findUser(id);
  if (!user) throw new Error("User not found");
  return user;
}
```

## Best Practices

- Explicit return types for exported functions
- Optional chaining (`?.`) and nullish coalescing (`??`) over manual checks
- Organize imports: external libs → internal absolute → relative

## Testing

- Jest or Vitest preferred
- Mock external dependencies for unit isolation
