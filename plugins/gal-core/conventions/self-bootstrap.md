# Self-Bootstrap
`gal-pipeline` and `gal-finalize` consume this convention pointer-only and never restate it.

**Self-bootstrap first (mandatory when the plan touches `crates/`).** If any of the plan's `## Files to Create or Modify` entries (or task-block affected-file paths for individual tasks) are under `crates/`, **rebuild + reinstall `gal` before running any gate command in the task loop**, wrapping the rebuild and every install target inside an advisory install lease. Running a gate binary against pre-change code produces a receipt that describes the old binary, not the work under implementation — any `pass` from a stale binary is evidence of nothing. The lease covers every install target: canonical root `~/.gal/plugins/gal/`, `~/.cargo/bin/gal.exe`, Claude plugin-cache copy when present. Rebuild and installation sit inside the lease in this order:

1. Create the marker's parent `~/.gal/.locks/` (this step may use `-Force` / `-p`).
2. Acquire the lease by creating `~/.gal/.locks/gal-install/` with the exact commands in the table below and no `-Force` / `-p`.
3. On "already exists" poll every `5 seconds` up to `900 seconds`, then stop and surface the marker path without removing it.
4. Run `cargo build --release` and install to all three targets — canonical root, `~/.cargo/bin/gal.exe`, Claude plugin-cache copy when present.
5. Record the pinned hash while still holding the lease by resolving the executable with the command in the table below and hashing it with SHA-256, storing `gal --version` output beside it as context that never participates in the comparison.
6. Release the marker on both the success and the error path, reporting the exact path if the release itself fails.

A later self-bootstrap repeats this cycle and replaces the pinned hash. With no Rust source touched the orchestrator takes no lease and runs no build, but still resolves and hashes the executable with the commands in the table below before its first gate command, so a pinned hash always exists.

| Step | PowerShell | Bash |
| --- | --- | --- |
| Create the marker's parent (may already exist) | `New-Item -ItemType Directory -Force -Path "$env:USERPROFILE\.gal\.locks"` | `mkdir -p "$HOME/.gal/.locks"` |
| Acquire the lease (never `-Force` / `-p`) | `New-Item -ItemType Directory -Path "$env:USERPROFILE\.gal\.locks\gal-install"` | `mkdir "$HOME/.gal/.locks/gal-install"` |
| Resolve the executable | `(Get-Command -CommandType Application gal \| Select-Object -First 1).Source` | `command -v gal` |
| Hash it | `(Get-FileHash -Algorithm SHA256 <path>).Hash` | `sha256sum <path>`, or `shasum -a 256 <path>` on macOS |

Three traps stated as rules, not left to the reader:
- `-Force` and `-p` are banned on the acquire step only. Both return success when the directory exists, turning the lease into a no-op with no error and no signal. They are required on the parent step, which is not the lock.
- `-CommandType Application` is mandatory in the PowerShell resolve. Bare `Get-Command gal` resolves to PowerShell's `gal` alias for `Get-Alias` and returns an empty `Source`, failing the hash step.
- Hash comparison is case-insensitive. `Get-FileHash` returns uppercase hex, `sha256sum` lowercase; same digest for the same file, so normalise before comparing.

Stale-marker recovery is documented in `docs/workflows.md` under `### Clearing Stalled Installation Leases`.

After the manual prerequisite check above, run:

```powershell
gal.exe pipeline-preflight <execution-prompt-path>
```

Read the receipt. Before trusting any gate command's receipt, re-resolve the executable and re-hash it with the commands in the self-bootstrap paragraph above, and compare case-insensitively against the pinned hash recorded during self-bootstrap. On mismatch, stop, do not trust the gate result that produced it, and report both `gal --version` strings to the human as context. **The receipt is the sole pass-basis — self-report is not accepted.**
