# xmachine Fixture Freeze

Established by T-001 of `refactor-gal-xmachine-rust-port`.

## Scope

This fixture set freezes the current Windows control-node observable behavior that the Rust `pipeline` and `xmachine` ports must preserve before the legacy xmachine script family is deleted.

The goal of T-001 is not to freeze every remote side effect up front. It is to capture the smallest stable baseline that can be reproduced in the current environment before any Rust-native xmachine crate exists.

## Captured Baselines

- `windows-control-node/test-xmachine-no-worknode.txt`
  Current no-argument failure surface of `scripts/Test-Xmachine.ps1` on Windows.
- `windows-control-node/new-task-spec-missing-taskscope.txt`
  Current required-argument failure surface of `scripts/common/New-TaskSpec.ps1`.
- `windows-control-node/bash-wrapper-unavailable.txt`
  Current Windows environment limitation when attempting to invoke `scripts/Test-Xmachine.sh` via `bash` without a working WSL/bash path.

## Notes

- `scripts/Invoke-XmachinePipeline.ps1` produced no stable no-argument output in this environment and was not frozen in T-001. A later task can extend the fixture set with a bounded invocation once the expected argument contract is known.
- Cross-machine SSH and zellij parity remain deferred to later xmachine tasks and the TP-13 hard gate.
