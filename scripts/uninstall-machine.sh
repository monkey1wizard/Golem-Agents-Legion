#!/usr/bin/env bash
# uninstall-machine.sh — Remove all symlinks created by setup-machine.sh
#
# Wrapper around setup-machine.sh --uninstall.
# Removes:
#   - ~/.copilot/agents/*.agent.md symlinks
#   - ~/.copilot/skills/*/ symlinks
#   - ~/.gemini/skills/*/ symlinks
#   - ~/.copilot/skills/gal/ symlink (GAL dispatcher skill)
#   - ~/.gemini/skills/gal/ symlink (GAL dispatcher skill)
#   - ~/.copilot/gal/ symlink (GAL_ROOT)
#   - ~/.gemini/gal/ symlink (GAL_ROOT)
#   - commands/gal/SKILL.md (baked from template)
#   - ~/.gemini/gal-context.md
#
# Usage:
#   ./scripts/uninstall-machine.sh
#   ./scripts/uninstall-machine.sh --dry-run

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
"$SCRIPT_DIR/setup-machine.sh" --uninstall "$@"
