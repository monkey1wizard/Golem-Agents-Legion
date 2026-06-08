---
name: structured-logging
description: Apply structured logging conventions across languages. Use when adding logs, reviewing logging quality, or choosing log levels and log field structure.
---

# Structured Logging Skill

Use structured logging for application events. Prefer machine-readable fields over interpolated strings.

## Core Principles

1. Log objects or structured fields, not string-built payloads
2. Choose the right log level for the event
3. Include useful context as structured properties
4. Never log passwords, tokens, secrets, or PII

## Log Levels

| Level | Use For |
| --- | --- |
| Trace | Detailed diagnostic info, usually dev only |
| Debug | Development troubleshooting |
| Info | Normal operations and milestones |
| Warning | Recoverable issues or degraded state |
| Error | Failures that require attention |
| Critical | System-wide or unrecoverable failures |

## Decision Tree

```text
Event occurs ->
    |- System is crashing or unrecoverable?      -> Critical
    |- Operation failed and needs attention?     -> Error
    |- Something is wrong but system continues?  -> Warning
    |- Normal business milestone?                -> Info
    |- Useful only during development?           -> Debug
    `- Extremely verbose diagnostics?            -> Trace
```

## Cross-Language Patterns

```csharp
// C# - structured template
_logger.LogInformation("Order {OrderId} parsed with {ItemCount} items", order.Id, order.Items.Count);
// Bad: _logger.LogInformation($"Order {order.Id} parsed");
```

```typescript
// TypeScript - structured object
logger.info({ orderId: order.id, itemCount: order.items.length }, 'Order parsed');
```

```python
# Python - structured kwargs
logger.info("order_parsed", order_id=order.id, item_count=len(order.items))
```

```rust
// Rust - tracing fields
tracing::info!(order_id = %order.id, item_count = order.items.len(), "Order parsed");
```

```go
// Go - zerolog or zap style
log.Info().Str("orderId", order.ID).Int("itemCount", len(order.Items)).Msg("Order parsed")
```

## Anti-Patterns

- Do not log in tight loops when a summary log would do
- Do not catch an error, log it, and silently continue unless that is the explicit design
- Do not duplicate the same event at multiple levels
- Do not bury key identifiers inside freeform prose

## Output Requirements

When asked to add or improve logs:

1. Pick the correct level
2. Keep the message short
3. Put searchable data into fields
4. Avoid secrets and noisy repetition
