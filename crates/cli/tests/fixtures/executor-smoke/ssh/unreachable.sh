#!/bin/sh
# Fake ssh transport (executor-smoke test): an unreachable target.
# Invoked as `sh unreachable.sh <ssh-target> <remote-command>` via SshProbe::with_program.
# Mimics a DNS/connect failure — every probe (reachability, install, auth) fails.
exit 255
