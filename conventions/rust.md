# Rust Conventions

## Core Principles

- **Safety first**: Prefer safe Rust. Avoid `unsafe` unless strictly necessary for FFI.
- **Pure & stateless**: Core logic should be `fn(Input) -> Output`.
- **Error handling**: `Result<T, E>` with `thiserror` for libs, `anyhow` for apps. Never `unwrap()` in production.

## Code Style

- Must pass `cargo fmt` and `cargo clippy`
- Use `&str` and `&T` for borrowing. Avoid `.clone()` unless ownership required.

## Naming (RFC 430)

| Category | Convention | Example |
| --- | --- | --- |
| Crates, modules | `snake_case` | `order_parser` |
| Types (structs, enums, traits) | `UpperCamelCase` | `OrderItem`, `Validate` |
| Functions, methods | `snake_case` | `calculate_total` |
| Variables | `snake_case` | `user_id` |
| Constants, statics | `SCREAMING_SNAKE_CASE` | `MAX_RETRIES` |
| Files | `snake_case.rs` | `order_validator.rs` |

## Data Models

Always derive `Debug`, `Clone`, `Serialize`, `Deserialize`, `JsonSchema`.

- `String` for text fields (UTF-8)
- `rust_decimal::Decimal` for monetary values — `f32`/`f64` **forbidden** for prices

```rust
use rust_decimal::Decimal;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PriceItem {
    pub name: String,
    pub amount: Decimal,
}
```

## Testing

- **TDD**: Write tests before implementation
- Unit tests in `mod tests` at bottom of source file
- Integration tests in `tests/` directory
- Use strict helper functions over heavy mocking frameworks

## Security

- Never commit API keys
- Always check pointers for null before dereferencing in `ffi.rs`
- All comments and docs in English (CJK allowed in string literals for testing only)
