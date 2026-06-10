#!/usr/bin/env bash
# uninstall-machine.sh — Remove GAL-managed links and generated artifacts installed by `gal setup`.
#
# Thin wrapper around the `gal setup --uninstall` flow.
# User-owned runtime settings and MCP config files are preserved during uninstall.
# Use --purge --confirm-purge only for an explicit destructive reset of preserved machine-local state.

set -euo pipefail

if ! command -v gal >/dev/null 2>&1; then
    echo 'gal not found on PATH.' >&2
    exit 1
fi

exec gal setup --uninstall "$@"
