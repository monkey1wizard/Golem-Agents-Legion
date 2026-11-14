#!/bin/sh
# Fake ssh transport (executor-smoke test): reachable, but the executor is missing
# on the remote non-interactive PATH.
# Invoked as `sh not-installed.sh <ssh-target> <remote-command>`.
cmd="$2"
case "$cmd" in
  true) exit 0 ;;              # reachability precheck: reachable
  "command -v "*) exit 1 ;;    # install probe: not found (no stdout, nonzero exit)
  *) exit 1 ;;
esac
