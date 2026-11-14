#!/bin/sh
# Fake ssh transport (executor-smoke test): reachable, executor installed, but the
# remote executor is not authenticated (its --version emits a login-required marker).
# Invoked as `sh unauthenticated.sh <ssh-target> <remote-command>`.
cmd="$2"
case "$cmd" in
  true) exit 0 ;;                                   # reachable
  "command -v "*) echo "/opt/agents/bin/agent"; exit 0 ;;  # installed
  *--version*) echo "Error: not logged in. Please log in to continue."; exit 1 ;;  # unauth marker
  *) exit 1 ;;
esac
