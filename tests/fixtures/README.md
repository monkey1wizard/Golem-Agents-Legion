# Parity Fixtures — Deletion Oracle Baseline

> Established by T-001 of `refactor-gal-core-rust-port` (capture convention; fixtures captured just-in-time per domain, not all upfront — see Deviation D-001 in the execution prompt).

## Purpose

Before a superseded `.ps1`/`.sh` script is deleted, its Rust replacement must match a **frozen fixture** of the script's current observable behavior. This directory holds those fixtures. The deletion hard-gate is: *Rust output == fixture* (not merely "compiles").

## What needs a fixture (and what does not)

- **Needs a fixture**: scripts whose **only** current implementation is the script — i.e. not yet Rust-ported. These are: `adapters` family (`update-skills`/`update-commands`/`update-personalization`/`Sync-DevContext`), `setup` (`setup-machine`/`setup-tools`), `vcs` (`gal-clean`/`gal-smudge`), catalog (`Resolve-GalCatalog`), translation (`Test-TranslationFreshness`).
- **Does NOT need a script fixture**: surfaces already Rust-ported (install/render/doctor/mode/claude-skill/bin/mcp engine). For these the **Rust behavior contract is already the oracle** — see `crates/gal-engine/tests/cross_platform_oracle_parity.rs` and `mcp_provider_oracle_parity.rs` ("the retired Bash oracle is no longer a live parity reference; these tests are the authoritative correctness bar"). Do not re-capture script fixtures for them.

## Capture convention (JIT, per domain)

Fixtures are captured **at the start of each domain's port task** (T-013 adapters, T-018 setup, T-025 vcs, T-028 catalog, T-030 translation), not all at once. Rationale: aligns with the plan's JIT decomposition; avoids capturing side-effecting scripts (setup-tools downloads tools, setup-machine is heavy) before the context to run them deterministically exists.

Per domain:

1. Run the script in an **isolated HOME** (temp dir for `HOME`/`USERPROFILE`) with `galRoot` = repo root, deterministic inputs.
2. Capture: stdout/stderr + the resulting file tree (paths + contents) under the surfaces the script writes.
3. **Normalize** machine-specific values: home paths → `$HOME`, absolute repo path → `$REPO`, timestamps/uuids → placeholders.
4. Store under `tests/fixtures/<domain>/` (e.g. `tests/fixtures/adapters/update-skills/`).
5. The capture is driven by a **Rust test helper / cargo-test harness** (not a new shell script — preserves the zero-ps1/sh terminal goal). The port task's TP asserts `Rust output == fixture`.

## Layout

```
tests/fixtures/
  README.md            (this file)
  <domain>/<script>/   stdout.txt, tree.txt, files/...   (captured JIT at the domain's port task)
```

## Note on T-002 baseline

The local dev-mode `gal install` baseline (already-Rust install/render/doctor surfaces) is verified via `gal doctor` green (2026-06-09), not via a fixture here. Real-machine hard gates (mac-mini normal via SSH, Windows normal packaged-source) remain pending — see the execution prompt Status/Deviations.
