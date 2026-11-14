---
applyTo: "**/*.rs,**/Cargo.toml,**/Cargo.lock"
---

<!-- GAL-generated: gal init -->

# Rust Conventions

GAL's own development standard for the `crates/` Rust workspace — not a rule injected into downstream repos as house style.

## Baseline

Official tooling is authoritative:

- Checks are read-only: `cargo fmt --check` and `cargo clippy` must pass.
- Scoped tasks format only the changed files, with `rustfmt --edition <crate edition> --config skip_children=true <changed .rs files>`. This is the only format command for a scoped task. `skip_children=true` stops rustfmt from following `mod` declarations into files outside the change.
- In a scoped task, never run a mutating workspace-wide command: `cargo fmt`, `cargo fmt --all`, or `cargo clippy --fix`.
- A check failure that names only files outside the change is baseline debt. Report it and leave it unfixed.
- Naming follows the [Rust API Guidelines — Naming](https://rust-lang.github.io/api-guidelines/naming.html) (C-CASE, built on RFC 430): casing, acronyms (`Uuid` not `UUID`), and trait/constructor conventions.

**Precedence — common standard first, house style second.** The Rust API Guidelines above **win on any conflict**. GAL's house style in [naming.md](../../plugins/gal-core/conventions/naming.md) (word choice, word order, the `UpperCamelCase` headword form) applies **only** where the standard leaves a free choice and never overrides it.

## GAL Deltas

Real deviations from — or additions to — the official baseline, kept because they solve a problem the baseline doesn't cover:

- **Error handling**: `Result<T, E>` with `thiserror` for libraries, `anyhow` for applications. Never `unwrap()` in production.
- **Monetary values**: `rust_decimal::Decimal`, never `f32`/`f64` — floating point cannot represent currency exactly.

  ```rust
  use rust_decimal::Decimal;

  pub struct PriceItem {
      pub name: String,
      pub amount: Decimal,
  }
  ```

- **Comments and docs**: always English (CJK allowed only in string literals for testing).
- **Non-blocking check findings**: represent checks that ran and found an
  advisory issue with an explicit `Advisory` state. Advisory findings do not
  fail an aggregate receipt; `Fail` and `NotRun` remain blocking.
- **Test placement**: unit tests in `mod tests` at the bottom of the source file; integration tests in the crate's `tests/` directory.
