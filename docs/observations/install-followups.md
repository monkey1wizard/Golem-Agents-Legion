# Install Convergence Follow-ups

Observations and deferred items from the GAL Bootstrap / Install Convergence plan
(`docs/plans/fix-gal-bootstrap-install-convergence.md`).

These are non-blocking for P3 and documentation tasks but must be resolved before M1
is releasable or P2 (package-manager release lane) begins.

---

## M1 Blockers (must fix before M1 is releasable)

### FU-01 (H-01) — Normal-mode packaged-source render — RESOLVED (commit 47f04c4)

**File**: `crates/gal-core/src/render.rs`, `GalMode::Normal` branch

**Was**: `render.rs` used `std::env::current_dir()` as the GAL source location in normal
mode. A `gal` binary installed via winget or Homebrew and run from an arbitrary directory
would fail to locate GAL source files and could not install (R-003/R-005).

**Fix (2026-06-03)**: Added `resolve_packaged_source_root()` resolving the source relative
to `std::env::current_exe()`. Tries flat layout (binary + source side by side), FHS
(`<prefix>/bin/gal` + `<prefix>/share/gal`), then a bounded ancestor walk-up. Each
candidate is validated by `looks_like_source_root()` (requires `skills/`+`agent/`+
`commands/`). Returns `SourceRootNotFound` rather than silently using the working
directory. Layout logic extracted to `resolve_source_from_exe_dir()` for unit testing;
5 new tests added, 122 pass, clippy clean.

**Still open**: the concrete packaged layout (which of flat/FHS) is fixed by P2 artifact
packaging (T-014). FU-02 below must run end-to-end once that layout is locked.

**Discovered**: T-011/T-012/T-013 code review (2026-06-03), finding H-01.

---

### FU-02 (M-01) — TP-014 / TP-015 oracle-parity tests never executed

**Tests**: TP-014 (isolated-home `gal install` Claude+Copilot == frozen oracle),
TP-015 (entry-switch: `gal` uses Rust binary, frozen scripts not invoked)

**Problem**: These are the load-bearing acceptance tests for "Claude/Copilot native
parity" (M1 milestone). Neither has been run. TP-014 is also blocked on FU-01 being
fixed first (the stub makes it fail trivially in a non-repo directory).

**Required**: Run both in an isolated home after FU-01 is implemented.

---

## Low-priority Improvements (M2 or later)

### FU-03 (L-01) — Uninstall ledger record is imprecise

**File**: `crates/gal-core/src/install.rs`, `run_uninstall()`

`run_uninstall()` records `providers: []` and `mode: "normal"` in the ledger entry
regardless of the actual last-known state. Should reuse `Ledger.last` values.

---

### FU-04 (L-03, T-005 pre-existing) — `is_readable` UNC timeout is not real

**File**: `crates/gal-core/src/mode.rs`, `is_gal_root_usable()`

The function spawns a thread to check path readability but calls `join()` which blocks
indefinitely. The documented "dead-path short timeout" (R-003) is not enforced.
Acceptable for developer workstations; matters for UNC/network paths.

---

## Optional / Deferred Scope

### R5/T-013 — Commit-msg scope injection

`process_commit_msg()` preserves the message (no-hijack confirmed, TP-019 passes).
Full scope injection (auto-prefix based on changed files) is not implemented.
This is a future enhancement; no-hijack is the core R5 requirement.

### AGY transaction/ledger (M2)

AGY three-surface best-effort is complete (T-010). Full transactional rollback and
ledger integration are deferred to M2 per OE-A.

### MCP AGY/Codex/OpenCode serializers (M2)

Claude Desktop and Copilot CLI MCP serializers are done (T-009). AGY, Codex CLI,
and OpenCode (TOML) serializers are M2.

### macOS/Linux cross-platform parity (M2, T-022)

Windows-first for M1. macOS/Linux Rust install/render/projection vs frozen Bash oracle
is T-022, scoped to M2 per BUG-C.

---

## Plan Closeout Candidates (OQ-004)

These plans are verified/complete and have been moved to the `Recently Closed` section
of `.dev/state.md`. Source plan files are retained in `docs/plans/` as historical
reference per OQ-004.

| Plan | Status | Notes |
| --- | --- | --- |
| `feat-plugin-arch-migration.md` | VERIFIED | All tasks + verifier complete. |
| `fix-gal-pipeline-token-burn.md` | VERIFY complete | Numeric targets met. |
| `feat-golem-dockeeper.md` | VERIFIED | Runtime visibility pending M1 releasable install. |

### Orphan plans (pending confirmation)

These plans exist in `docs/plans/` but have no active execution context and were not
part of a recent pipeline run. Confirm whether to close or resume:

| Plan | File | Last Known State |
| --- | --- | --- |
| GAL file memory strategy | `docs/plans/feat-gal-file-memory-strategy.md` | Unknown — confirm with owner |
| Manage external plugins | `docs/plans/manage-external-plugins.md` | Unknown — confirm with owner |
