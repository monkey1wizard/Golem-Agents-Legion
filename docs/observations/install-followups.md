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

### FU-02 (M-01) — TP-014 / TP-015 oracle-parity — PARTIALLY ADDRESSED (2026-06-03)

**Tests**: TP-014 (isolated-home `gal install` Claude+Copilot == frozen oracle),
TP-015 (entry-switch: `gal` uses Rust binary, frozen scripts not invoked)

**Done (2026-06-03)**: Render-surface smoke via `crates/gal-core/examples/fu02_render_smoke.rs`
— renders the normal-mode canonical root into an isolated home (no ledger/AGY, no
real-home pollution). PASS: 71 files, 29 skills (incl. doc-sync), 13 agents (incl.
golem-dockeeper), Claude + Copilot manifests + `.mcp.json` present; 7/7 assertions.
Human-inspected, temp dir deleted. Confirms FU-01 works in practice and install
produces a correct canonical root in a clean home.

**Still open**:
1. Byte-level oracle parity vs the frozen `Install-GalPlugins` PowerShell output —
   needs the frozen script run side-by-side and the P2 artifact layout locked.
2. TP-015 entry-switch end-to-end in an installed environment.

Do NOT claim full Claude/Copilot byte-parity until (1) and (2) run.

---

## Security Review Findings (2026-06-04, non-blocking)

Security review CLEAR for the M1 install surface. Two Low findings to harden in M2,
one Info deferred to T-014.

### S-1 (Low) — Secret-guard backstop regex is anchored

`crates/gal-core/src/providers/mod.rs::has_unresolved_secrets` uses
`^\$\{([A-Z0-9_]+)\}$`, catching only whole-string `${SECRET}` values. Embedded
placeholders (e.g. `"Bearer ${API_KEY}"`) bypass this backstop. Not a disclosure
vuln — `McpVariableResolver::resolve_string` (un-anchored) already errors on
unresolved secret vars upstream, and any slipped value is a literal `${...}` string,
not a resolved credential. Harden by aligning the backstop regex to the embedded form.

### S-2 (Low) — AGY junction path `to_str().unwrap()` panics on non-UTF-8 home

`crates/gal-core/src/providers/agy.rs` `create_link` / junction-removal call
`path.to_str().unwrap()` when building `cmd` argv. A non-UTF-8 home path panics.
Robustness/DoS-class (not memory-unsafe; AGY is best-effort/non-fatal). No command
injection (argv vector, trusted paths). Replace with a graceful best-effort error.

### S-3 (Info) — cosign keyless CI trust setup

Release signing (OQ-001, cosign keyless OIDC — allowed workflow identity + Rekor)
is not yet in code. **Re-review CI signing trust at T-014 implementation** before
package-manager publication.

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
