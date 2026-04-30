#!/usr/bin/env bash
# uninstall-machine.sh — Remove GAL-managed links and generated artifacts installed by setup-machine.sh.
#
# Thin wrapper around the concern-based setup-machine.sh --uninstall flow.
# User-owned runtime settings and MCP config files are preserved during uninstall.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
"$SCRIPT_DIR/setup-machine.sh" --uninstall "$@"