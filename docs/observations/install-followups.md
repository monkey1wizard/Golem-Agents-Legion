# Install Convergence Follow-ups

Observations and deferred items from the GAL Bootstrap / Install Convergence plan
(fix-gal-bootstrap-install-convergence — closed and deleted 2026-06-08 after all 22
tasks completed; this file is the durable record of its residual follow-ups). Live
end-to-end verification of the engine is owned by `docs/plans/feat-gal-rust-native-install.md`.

These are non-blocking for P3 and documentation tasks but must be resolved before M1
is releasable or P2 (package-manager release lane) begins.

---

## M1 Blockers (must fix before M1 is releasable)

### FU-01 (H-01) — Normal-mode packaged-source render — RESOLVED (commit 47f04c4)

**File**: `crates/gal-engine/src/render.rs`, `GalMode::Normal` branch

**Was**: `render.rs` used `std::env::current_dir()` as the GAL source location in normal
mode. A `gal` binary installed via winget or Homebrew and run from an arbitrary directory
would fail to locate GAL source files and could not install (R-003/R-005).

**Fix (2026-06-03)**: Added `resolve_packaged_source_root()` resolving the source relative
to `std::env::current_exe()`. Tries flat layout (binary + source side by side), FHS
(`<prefix>/bin/gal` + `<prefix>/share/gal`), then a bounded ancestor walk-up. Each
candidate is validated by `looks_like_source_root()` (requires `skills/`+`agents/`+
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

**Done (2026-06-03)**: Render-surface smoke via `crates/gal-engine/examples/fu02_render_smoke.rs`
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

`crates/gal-engine/src/providers/mod.rs::has_unresolved_secrets` uses
`^\$\{([A-Z0-9_]+)\}$`, catching only whole-string `${SECRET}` values. Embedded
placeholders (e.g. `"Bearer ${API_KEY}"`) bypass this backstop. Not a disclosure
vuln — `McpVariableResolver::resolve_string` (un-anchored) already errors on
unresolved secret vars upstream, and any slipped value is a literal `${...}` string,
not a resolved credential. Harden by aligning the backstop regex to the embedded form.

### S-2 (Low) — AGY junction path `to_str().unwrap()` panics on non-UTF-8 home

`crates/gal-engine/src/providers/agy.rs` `create_link` / junction-removal call
`path.to_str().unwrap()` when building `cmd` argv. A non-UTF-8 home path panics.
Robustness/DoS-class (not memory-unsafe; AGY is best-effort/non-fatal). No command
injection (argv vector, trusted paths). Replace with a graceful best-effort error.

### S-3 (Info) — cosign keyless CI trust setup

Release signing (OQ-001, cosign keyless OIDC — allowed workflow identity + Rekor)
is not yet in code. **Re-review CI signing trust at T-014 implementation** before
package-manager publication.

## Low-priority Improvements (M2 or later)

### FU-03 (L-01) — Uninstall ledger record is imprecise

**File**: `crates/gal-engine/src/install.rs`, `run_uninstall()`

`run_uninstall()` records `providers: []` and `mode: "normal"` in the ledger entry
regardless of the actual last-known state. Should reuse `Ledger.last` values.

---

### FU-04 (L-03, T-005 pre-existing) — `is_readable` UNC timeout is not real

**File**: `crates/gal-engine/src/mode.rs`, `is_gal_root_usable()`

The function spawns a thread to check path readability but calls `join()` which blocks
indefinitely. The documented "dead-path short timeout" (R-003) is not enforced.
Acceptable for developer workstations; matters for UNC/network paths.

---

## Optional / Deferred Scope

### R5/T-013 — Commit-msg scope injection

The live commit-msg hook path (`gal commit-msg <file>` → `cmd_commit_msg` →
`fill_commit_msg_file`) now injects a path-derived scope into an unscoped conventional
header while preserving already-scoped and freeform author messages, and still
filling blank messages from staged changes. No-hijack behavior remains intact
because the scope is derived only from staged file paths
(`derive_scope_from_entries`, the same source the message generator uses, so
injection and blank-fill agree). Initial work (`9dd2ef3`) added the
`inject_scope_prefix` helper but only wired it into the unused `process_commit_msg`
function; the architect fix moved it onto the live `fill_commit_msg_file` path.

Known divergence (advisory, not blocking): `derive_scope_from_entries` returns the
coarse top-level `crates` bucket for crate changes, while `derive_scope_from_files`
unwraps to the crate name (`providers`, `gal-engine`) matching the repo's own
`fix(providers):` convention. Unifying them changes the tested cross-crate-rename
contract and is left as separate generator-scope cleanup.

### AGY transaction/ledger (M2) — PARTIALLY RESOLVED (commit 6264f7a)

AGY three-surface projection now rolls back already-created CLI/IDE links when a
later surface fails, so `AgyProjection::apply()` no longer leaves half-projected
state. Focused test coverage added for GUI-config failure rollback.

**Still open**: install/uninstall ledger integration at the AGY surface level is
not implemented separately from the install ledger yet.

### MCP AGY/Codex/OpenCode serializers (M2) — UPDATED (commit 267cfe4)

Claude Desktop and Copilot CLI MCP serializers are done (T-009). Codex CLI and
OpenCode serializers already exist in `crates/providers/src/codex.rs` and
`crates/providers/src/opencode.rs` and now have explicit acceptance evidence via
their focused test suites.

AGY does **not** consume the `.mcp.json` provider-write path in the current
architecture: `crates/mcp/src/lib.rs::run_mcp_update()` updates only Claude,
Copilot, OpenCode, and Codex. AGY remains a three-surface projection provider,
not an MCP serializer target. The AGY serializer portion is therefore closed as
not applicable with code evidence.

### macOS/Linux cross-platform parity (M2, T-022 — code complete, live run pending)

Windows-first for M1. macOS/Linux Rust install/render/projection vs frozen Bash oracle
is T-022 (BUG-C). **Test suite complete (2026-06-08)**: `crates/gal-engine/tests/cross_platform_oracle_parity.rs`
(9 Unix-gated tests: HOME env, `agents/` naming, AGY symlink-not-junction, render
structure, MCP serializer parity, atomic-swap cleanup) + `scripts/test-t022-ssh.sh`
(4-stage runner). Also fixed a load-bearing `agent/`→`agents/` naming bug in
`render.rs`/`mode.rs` (singular form would silently break dev-mode resolution against
the real repo, which uses `agents/` plural, matching the frozen Bash oracle).

**Still open (evidence collection, owned by `feat-gal-rust-native-install`)**: physical
TP-029 (macOS) / TP-030 (Linux) runs. Execute on the target via:
`ssh mac-mini "cd <repo> && bash scripts/test-t022-ssh.sh"`. Code passes on Windows
(non-Unix tests compile-gated out).

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
