---
name: verification-map
description: Create or maintain a repository verification driver that exercises documented entry points and keeps evidence after isolated test roots are removed.
---

# Verification Map

Use this skill to create or maintain a repository-owned verification driver. The driver turns a feature journey into repeatable commands, receipts, and cleanup evidence. It does not replace the project test plan, independent TESTER workflow, or semantic review.

## Create a driver

1. Read the repository's source index and the relevant workflow or test contract. Reuse an existing tester workflow when one defines independent test design, commit selection, or receipt requirements.
2. Identify the smallest user-facing journey that proves the feature. List prerequisites and optional environments separately. Keep unavailable optional checks as `not-run` with the reason.
3. Add the six sections `Launch`, `Doctor`, `Drive`, `Evidence`, `Cleanup`, and `Helpers` to the repository driver. Add a feature index that links journeys to their entry points, expected outcomes, and evidence.
4. Use a bound executable selected before the run. Record its canonical path and SHA-256. Do not resolve the executable again through `PATH` during that run.
5. Run each mutating command only in a newly created, attempt-owned isolated repository. Never run smoke commands with the formal repository as the working or target root.
6. Record command, working directory, exit code, executable SHA-256, input SHA-256 where applicable, receipt path, and canonical run root. Preserve receipts and command output outside the disposable repository.
7. Verify both a successful journey and a negative case whose failure is expected. A negative case passes only when the command writes the expected failure receipt and returns a nonzero exit. A setup failure is not a negative proof.
8. Remove only the exact run root created by the current attempt. Confirm the root is within the configured run directory, its ownership marker matches the attempt, and it is not a symlink or reparse point. Preserve evidence after cleanup.
9. Compare the runbook with the actual command surface and receipts. Record any unrun scenario without converting it to a pass.

## Maintain a driver

1. Read the current driver, feature index, source index, relevant CLI contract, and covering Test Plan rows.
2. Compare command syntax, receipt shape, expected status, working root, cleanup boundary, and evidence location against current sources.
3. Reproduce drift in an isolated attempt root before changing the documentation when the required executable and environment are available.
4. Classify a reproducible failure as one of:
   - `doc-drift`: the product behavior is correct and the instructions or index are stale.
   - `harness-gap`: the driver cannot safely or reliably exercise the documented behavior.
   - `product-gap`: the command or product behavior violates its contract.
5. Keep product failures visible. Do not rewrite expected results or label a product failure as documentation success. Route product changes through the owning task and its test plan.
6. Preserve the failing command, exit code, executable and input hashes, receipt, root, and evidence location before cleanup. Update the source index when the repository driver path or purpose changes.

## Evidence contract

Each attempt has a unique ID. Its durable record includes the selected executable path and SHA-256, exact commands and working directories, observed exit codes, input hashes, receipt paths and hashes, canonical smoke root, cleanup result, and evidence directory. Include stdout and stderr when they help reproduce or classify a result. Mark optional host, provider, or external-service checks `not-run` when unavailable and name the missing prerequisite.

Document inspection proves only that the contract is present. It does not prove that a command ran or that a new behavior passed.
