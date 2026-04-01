# Universal Conventions

Cross-language rules that apply to all code in every repository.

---

## Git Commits

Must follows [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/).

### Format

```text
<type>: <brief description>
```

### Types

| Type | Description |
| --- | --- |
| `feat` | A new feature |
| `fix` | A bug fix |
| `refactor` | Code change that neither fixes a bug nor adds a feature |
| `docs` | Documentation only changes |
| `style` | Formatting changes (no logic change) |
| `test` | Adding or correcting tests |
| `chore` | Build process or auxiliary tools |
| `perf` | Performance improvements |

### Rules

1. **Imperative mood**: "add feature" not "added feature"
2. **Lowercase**: Start with lowercase letter
3. **No period**: Don't end with a period
4. **Under 50 chars**: Subject line
5. **Be specific**: Describe what changed, not why
6. **No footer**: No `Closes #123` or similar

### Decision Tree

```text
Ready to commit → How many files changed?
    ├─ 1-3 files, same purpose → Format A: one-liner
    └─ 4+ files, or diverse   → Format B: title + bullet list
```

### Format A — Minimal

```text
feat: add public email verification token
fix: handle missing signup time correctly
```

### Format B — Substantial

```text
feat: brief description

- File/Component 1 (purpose)
- File/Component 2 (purpose)
```

Bullet guidelines:

- **Group related files** on a single bullet when they share a purpose
- **Use semicolons** to separate multiple changes within a bullet
- **No scope in type**: Use `feat:` not `feat(scope):`
- **No body paragraph or footer**: All detail in bullets

---

## Structured Logging

### Core Principles

1. **Use structured logging** — log objects, not interpolated strings
2. **Appropriate log levels** — choose the right level
3. **Include context** — add relevant data as structured properties
4. **No sensitive data** — never log passwords, tokens, or PII

### Log Levels

| Level | Use For |
| --- | --- |
| Trace | Detailed diagnostic info (dev only) |
| Debug | Development troubleshooting |
| Info | Normal operations, milestones |
| Warning | Recoverable issues, degraded state |
| Error | Failures requiring attention |
| Critical | System-wide failures |

### Decision Tree

```text
Event occurs →
    ├─ System is crashing / unrecoverable?       → Critical
    ├─ Operation failed, needs human attention?   → Error
    ├─ Something wrong but system continues?      → Warning
    ├─ Normal business milestone?                 → Info
    ├─ Useful only during development?            → Debug
    └─ Extremely verbose (loops, raw payloads)?   → Trace
```

### Cross-Language Patterns

```csharp
// C# — structured template
_logger.LogInformation("Order {OrderId} parsed with {ItemCount} items", order.Id, order.Items.Count);
// ❌ _logger.LogInformation($"Order {order.Id} parsed");  // loses structure
```

```typescript
// TypeScript — structured object
logger.info({ orderId: order.id, itemCount: order.items.length }, 'Order parsed');
```

```python
# Python — structured kwargs
logger.info("order_parsed", order_id=order.id, item_count=len(order.items))
```

```rust
// Rust — tracing structured fields
tracing::info!(order_id = %order.id, item_count = order.items.len(), "Order parsed");
```

```go
// Go — zerolog/zap structured
log.Info().Str("orderId", order.ID).Int("itemCount", len(order.Items)).Msg("Order parsed")
```

### Anti-Patterns

- **No logging in tight loops** — log summaries instead
- **No catch-and-only-log** — always handle or re-throw after logging

---

## Result Pattern

**Use Result types for expected failures. Reserve exceptions for unexpected errors.**

### When to Use

| Scenario | Use Result | Use Exception |
| --- | --- | --- |
| Validation failure | Yes | No |
| Resource not found | Yes | No |
| Business rule violation | Yes | No |
| Parse/conversion error | Yes | No |
| Null reference | No | Yes |
| Out of memory | No | Yes |

### Decision Tree

```text
Error occurs → Expected (part of normal business logic)?
    ├─ Yes (validation, not-found, parse, business rule)
    │       └─ Return Result<T> with error details
    └─ No (null ref, OOM, infrastructure crash)
            └─ Let exception propagate (catch at system boundary)
```

### Cross-Language Examples

```csharp
// C# — custom Result<T>
public Result<Order> ParseOrder(string input)
{
    if (string.IsNullOrEmpty(input))
        return Result<Order>.Failure("Input cannot be empty");
    return Result<Order>.Success(new Order(input));
}
```

```typescript
// TypeScript — neverthrow
function parseOrder(input: string): Result<Order, string> {
  if (!input) return err('Input cannot be empty');
  return ok(new Order(input));
}
```

```rust
// Rust — std::result
fn parse_order(input: &str) -> Result<Order, ParseError> {
    if input.is_empty() { return Err(ParseError::EmptyInput); }
    Ok(Order::new(input))
}
```

```go
// Go — multiple return values
func ParseOrder(input string) (*Order, error) {
    if input == "" { return nil, errors.New("input cannot be empty") }
    return &Order{Input: input}, nil
}
```

---

## Markdown Formatting

### Headings

- Increment by one (no skipping h2 to h4)
- ATX style (`# Heading`)
- Surround with blank lines
- No duplicate heading text in same document

### Lists

- Use `-` for unordered lists
- Indent nested lists with 2 spaces
- Single space after list marker
- Surround lists with blank lines

### Code Blocks

- Use fenced code blocks (not indented)
- Always specify language
- Surround with blank lines

### Lines

- No trailing spaces
- No hard tabs
- No multiple consecutive blank lines
- Files end with single newline

### Links and Tables

- No bare URLs — use `[text](url)` format
- Tables use `| --- |` dividers (no alignment markers)

### Punctuation

- In general prose and documentation, do not use CJK fullwidth semicolon punctuation or semicolons padded with surrounding spaces
- The semicolon rule above applies only to Conventional Commit bullet lists, not to normal Markdown docs

### Recommended `.markdownlint.json`

```json
{
  "MD013": false,
  "MD033": false,
  "MD041": false
}
```
