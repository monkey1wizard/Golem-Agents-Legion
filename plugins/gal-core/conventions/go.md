# Go Conventions

## Core Principles

- **Simplicity**: Prefer simple, readable code over clever abstractions
- **Idiomatic Go**: Follow [Effective Go](https://go.dev/doc/effective_go). Use `gofmt` formatting.
- **Concurrency**: Share memory by communicating (channels), don't communicate by sharing memory

## Error Handling

- Handle errors immediately. Never ignore with `_` unless explicitly safe.
- Use `if err != nil` guard clauses. Return early.
- Wrap with context: `fmt.Errorf("context: %w", err)`

```go
f, err := os.Open(filename)
if err != nil {
    return fmt.Errorf("failed to open config: %w", err)
}
defer f.Close()
```

## Naming

- **Exported**: `PascalCase` (`CalculateTotal`)
- **Internal**: `camelCase` (`parseInput`)
- **Short names**: Short scopes get short names (`i`, `r`)
- **Packages**: Short, lowercase, single word. No underscores.

## Project Structure

- Follow standard layout: `cmd/`, `pkg/`, `internal/`
- Use `internal/` for code that shouldn't be imported externally

## Testing

- **Table-driven tests** for multiple inputs/outputs
- Use `t.Parallel()` for unrelated unit tests

```go
func TestAdd(t *testing.T) {
    tests := []struct {
        a, b, want int
    }{
        {1, 2, 3},
        {0, 0, 0},
    }
    for _, tt := range tests {
        t.Run(fmt.Sprintf("%d+%d", tt.a, tt.b), func(t *testing.T) {
            if got := Add(tt.a, tt.b); got != tt.want {
                t.Errorf("Add() = %v, want %v", got, tt.want)
            }
        })
    }
}
```
