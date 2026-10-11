# Execution Handoff and Receipt Verification

`gal-pipeline` and `gal-finalize` consume this convention pointer-only. They do not restate it.

## Stable handoff

Every trust-bearing gate runs through the execution identity selected at guarded entry. A source worktree uses its private immutable executable generation under `target/gal-pipeline/bin/<sha256>/`. A downstream repository uses the executable already owned by its package manager. Pass the absolute executable path to the gate process. Never resolve `gal` again through `PATH` after the handoff begins.

Before trusting a stored `pass`, run `doctor --verify-receipt <path> --expect-scope <standalone|coordinator:<plan-scope>>` through that bound executable. The verifier parses the receipt envelope, hashes the executable bytes again, recomputes the execution binding, and compares every envelope field. A standalone receipt is valid only when standalone scope was requested. Coordinator scope never falls back to standalone.

For coordinator scope, require `.dev/pipeline/<plan-scope>/coordinator.json` in the current canonical worktree. Its canonical path must be contained at that exact scope. Require valid coordinator JSON, the matching plan scope, and the current complete execution binding. Missing, malformed, moved, rebound, stale, or mismatched coordinator state is a hard stop.

## Immutable generations and safe rebind

Publish a private generation under its content hash. Stage bytes on the same filesystem and publish by atomic rename. If the hash destination already exists, hash its executable and accept it only when the bytes match the directory hash. Never overwrite a generation. Earlier receipts continue to refer to the generation they recorded.

Rebind only at a safe new segment with no active attempt, unconsumed checkpoint, pending receipt, or unrecovered projection journal. Build and verify the new private generation first. Then update the coordinator to that generation. Preserve every prior generation. If any prerequisite is absent or inconsistent, stop without changing the binding.

## Explicit roots and ownership

Keep executable roots separate from acceptance roots. Build, test, gate, projection, refresh, and cache acceptance must name a worktree-private or fixture-owned root explicitly. Reject shared binary, plugin, skill, cache, configuration, and projection targets before the first write.

The package manager alone installs or promotes the shared GAL binary. Source bootstrap, tests, pipeline, and finalize never install or replace shared executables. Validate candidate releases in private worktrees and fixtures. The shared binary changes only through a package-manager release.

## Hard stops

Stop before trusted action when the executable is missing, changed, outside its approved root, belongs to another worktree, or escapes through a symlink or junction. Stop on absent or malformed receipt fields, duplicate fields, wrong scope, digest mismatch, coordinator absence, coordinator parse failure, stale binding, or safe-rebind precondition failure. A failed or unrun verifier is never a pass.
