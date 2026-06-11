# Install Convergence Follow-ups — CLOSED

Residual follow-ups from the GAL Bootstrap / Install Convergence work
(`fix-gal-bootstrap-install-convergence`, closed 2026-06-08). All items were
adjudicated and closed by the **`fix-install-followups-closeout`** plan
(2026-06-11). This file is now a compact closure index; the durable detail lives
in that plan's `## Review Results` / `## Test Results` and in the referenced
commits.

> The only items not closed *here* are the live real-machine runs, which are not a
> fix-install concern — they are owned by **core plan T-035** (pure end-user
> acceptance of the T-034 release artifact on mac-mini + Windows). Nothing below
> blocks anything except that real-machine pass.

## Closure index

| Orig. ID | Item | Resolution | Evidence |
| --- | --- | --- | --- |
| FU-01 (H-01) | Normal-mode packaged-source render | RESOLVED | commit `47f04c4` (`resolve_packaged_source_root`) |
| S-1 (Low) | Secret-guard backstop regex anchored (embedded `${SECRET}` bypass) | RESOLVED — R-01/T-001 | `crates/providers/src/lib.rs::has_unresolved_secrets` (M1 `af1bf5e`) |
| S-2 (Low) | AGY junction `to_str().unwrap()` panic on non-UTF-8 home | RESOLVED — R-V1/T-003 | `crates/providers/src/agy.rs` lossy path (M1 `af1bf5e`) |
| S-3 (Info) | cosign keyless CI trust re-review | RESOLVED — R-03/T-008 | commit `416c209`; `cosign verify-blob` → Verified OK vs Release `v0.1.0-rc1`; fixed broken documented verify identity |
| FU-03 (L-01) | Uninstall ledger imprecise + skipped on mid-removal failure | RESOLVED — R-02/T-002 | `run_uninstall` best-effort + guaranteed ledger write (M1 `af1bf5e`) |
| FU-04 (L-03) | `is_readable` UNC timeout not real | CLOSED-BY-PORT — R-V2/T-004 | `crates/base/src/mode.rs` `recv_timeout(2s)` + dead-path test |
| R5/T-013 | commit-msg scope injection | DONE — R-06/T-007 | `9dd2ef3` + architect fix `9c3d9a1` (live `fill_commit_msg_file` path) |
| AGY M2 | AGY transaction/ledger rollback | DONE — R-04/T-005 | commit `6264f7a` (3-surface rollback) |
| MCP M2 | AGY/Codex/OpenCode MCP serializers | CLOSED — R-05/T-006 | commit `267cfe4`; AGY not on the `.mcp.json` write path (evidence); Codex/OpenCode covered |
| orphan plans | confirm feat-gal-file-memory-strategy + manage-external-plugins | ADJUDICATED — R-07/T-009 | `.dev/state.md` Non-Active / Closed Plans (memory-strategy CLOSED/absorbed; manage-external-plugins STALE/DEFERRED) |
| FU-02 / x-platform | oracle-parity + macOS/Linux live install run | → **core T-035** | not a fix-install item; pure end-user acceptance of the `v0.1.0-rc1` artifact |
