---
name: result-pattern
description: Apply the Result pattern for expected failures and reserve exceptions for unexpected faults. Use when designing error handling, validation flows, parsing, or business-rule outcomes.
---

# Result Pattern Skill

Use Result types for expected failures. Reserve exceptions for unexpected errors.

## When to Use Result vs Exception

| Scenario | Use Result | Use Exception |
| --- | --- | --- |
| Validation failure | Yes | No |
| Resource not found | Yes | No |
| Business rule violation | Yes | No |
| Parse or conversion error | Yes | No |
| Null reference | No | Yes |
| Out of memory | No | Yes |

## Decision Tree

```text
Error occurs -> Is it expected as part of normal business logic?
    |- Yes: validation, not-found, parse, business rule
    |    `- Return Result<T> with error details
    `- No: null ref, OOM, infrastructure crash
         `- Let exception propagate and catch at the system boundary
```

## Cross-Language Examples

```csharp
// C# - custom Result<T>
public Result<Order> ParseOrder(string input)
{
    if (string.IsNullOrEmpty(input))
        return Result<Order>.Failure("Input cannot be empty");

    return Result<Order>.Success(new Order(input));
}
```

```typescript
// TypeScript - neverthrow style
function parseOrder(input: string): Result<Order, string> {
  if (!input) return err('Input cannot be empty');
  return ok(new Order(input));
}
```

```rust
// Rust - std::result::Result
fn parse_order(input: &str) -> Result<Order, ParseError> {
    if input.is_empty() {
        return Err(ParseError::EmptyInput);
    }

    Ok(Order::new(input))
}
```

```go
// Go - value plus error
func ParseOrder(input string) (*Order, error) {
    if input == "" {
        return nil, errors.New("input cannot be empty")
    }

    return &Order{Input: input}, nil
}
```

## Guidelines

- Model expected business failures as values
- Keep error states explicit and typed where possible
- Convert infrastructure faults to boundary-level handling
- Avoid using exceptions for normal control flow

## Output Requirements

When designing or reviewing error handling:

1. Classify the failure as expected or unexpected
2. Use a Result-style return for expected outcomes
3. Use exceptions only for abnormal faults
