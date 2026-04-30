#!/usr/bin/env bash
# setup-machine.sh — Orchestrate GAL machine setup by invoking concern-specific update scripts.
#
# Runs the shell concern scripts in order:
#   1. update-personalization.sh
#   2. update-skills.sh
#   3. update-commands.sh
#   4. update-mcp.sh
#
# The tracked repo remains the source of truth for agents, skills, commands, and MCP.
# Copilot skills and agents stay shared, but MCP is written separately for
# VS Code Copilot and Copilot CLI.
# Each concern script can also run standalone when you only need one slice refreshed.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
. "$SCRIPT_DIR/common/common.sh"

parse_setup_args "$@"
initialize_setup_session
ensure_ripgrep

shared_args=()
$UNINSTALL && shared_args+=(--uninstall)
$REPLACE && shared_args+=(--replace)
$DRY_RUN && shared_args+=(--dry-run)
$RECONFIGURE && shared_args+=(--reconfigure)

if ! $UNINSTALL; then
    shared_args+=(--selected-runtimes "$SELECTED_RUNTIMES_CSV" --primary-runtime "$PRIMARY_RUNTIME")
fi

step_names=(Personalization Skills Commands MCP)
step_scripts=(update-personalization.sh update-skills.sh update-commands.sh update-mcp.sh)

for index in "${!step_scripts[@]}"; do
    echo ''
    echo ">>> Running ${step_names[$index]}"
    "$SCRIPT_DIR/${step_scripts[$index]}" "${shared_args[@]}"
done

echo ''
if $UNINSTALL; then
    echo 'Uninstall complete.'
elif $DRY_RUN; then
    echo 'Dry run complete. No changes made.'
    echo "Selected runtimes: $(format_runtime_csv "$SELECTED_RUNTIMES_CSV")"
    echo "Primary runtime: $PRIMARY_RUNTIME"
else
    echo "Setup complete: runtimes=$(format_runtime_csv "$SELECTED_RUNTIMES_CSV"); primary=$PRIMARY_RUNTIME"
    echo 'Note: If SKILL.template.md or SKILL.local.md changes, rerun update-commands.sh or setup-machine.sh.'
fi