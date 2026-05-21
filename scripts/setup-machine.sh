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

# --- AGY legacy pre-cleanup ---
# Remove all GAL-managed AGY legacy surfaces before any concern script runs.
# This ensures a clean slate for the plugin-only install path.
agy_legacy_pre_cleanup() {
    if ! $INSTALL_ANTIGRAVITY || $UNINSTALL; then
        return 0
    fi

    echo ''
    echo '=== AGY legacy pre-cleanup ==='
    local legacy_paths=(
        "$ANTIGRAVITY_SKILLS_TARGET:legacy AGY skills directory"
        "$GAL_ROOT_ANTIGRAVITY:legacy AGY GAL_ROOT symlink"
        "$AGY_PLUGIN_INSTALL_TARGET:existing AGY plugin install"
    )
    local legacy_entry legacy_path legacy_label
    for legacy_entry in "${legacy_paths[@]}"; do
        legacy_path="${legacy_entry%%:*}"
        legacy_label="${legacy_entry##*:}"
        if [ -e "$legacy_path" ]; then
            if $DRY_RUN; then
                echo "  [DRY RUN] Would remove $legacy_label: $legacy_path"
            else
                rm -rf "$legacy_path"
                echo "  [REMOVED] $legacy_label: $legacy_path"
            fi
        fi
    done
    # Clean GAL-managed entries from global AGY mcp_config.json
    if [ -f "$ANTIGRAVITY_MCP_FILE" ]; then
        if command_exists jq; then
            local gal_keys
            gal_keys="$(jq -r '.mcpServers // {} | keys[] | select(. == "gal" or startswith("gal-"))' "$ANTIGRAVITY_MCP_FILE" 2>/dev/null || true)"
            if [ -n "$gal_keys" ]; then
                if $DRY_RUN; then
                    echo "  [DRY RUN] Would remove GAL-managed MCP entries from: $ANTIGRAVITY_MCP_FILE ($gal_keys)"
                else
                    local updated
                    updated="$(jq 'del(.mcpServers["gal"]) | del(.mcpServers | to_entries[] | select(.key | startswith("gal-"))) | if .mcpServers == {} then del(.mcpServers) else . end' "$ANTIGRAVITY_MCP_FILE")"
                    if [ -n "$updated" ]; then
                        printf '%s\n' "$updated" > "$ANTIGRAVITY_MCP_FILE"
                        echo "  [REMOVED] GAL-managed MCP entries from: $ANTIGRAVITY_MCP_FILE"
                    fi
                fi
            fi
        else
            echo "  [WARN] jq not found — cannot clean GAL-managed MCP entries from $ANTIGRAVITY_MCP_FILE"
        fi
    fi
}

agy_legacy_pre_cleanup
        fi
    done
    # Clean GAL-managed entries from global AGY mcp_config.json
    if [ -f "$ANTIGRAVITY_MCP_FILE" ]; then
        if command_exists jq; then
            local gal_keys
            gal_keys="$(jq -r '.mcpServers // {} | keys[] | select(. == "gal" or startswith("gal-"))' "$ANTIGRAVITY_MCP_FILE" 2>/dev/null || true)"
            if [ -n "$gal_keys" ]; then
                if $DRY_RUN; then
                    echo "  [DRY RUN] Would remove GAL-managed MCP entries from: $ANTIGRAVITY_MCP_FILE ($gal_keys)"
                else
                    local updated
                    updated="$(jq 'del(.mcpServers["gal"]) | del(.mcpServers | to_entries[] | select(.key | startswith("gal-"))) | .mcpServers' <<< "$(jq 'del(.mcpServers["gal"]) | del(.mcpServers | to_entries[] | select(.key | startswith("gal-")))' "$ANTIGRAVITY_MCP_FILE")" 2>/dev/null || true)"
                    # Simpler approach: delete gal- prefixed keys
                    updated="$(jq 'del(.mcpServers["gal"]) | del(.mcpServers | to_entries[] | select(.key | startswith("gal-"))) | if .mcpServers == {} then del(.mcpServers) else . end' "$ANTIGRAVITY_MCP_FILE")"
                    if [ -n "$updated" ]; then
                        printf '%s\n' "$updated" > "$ANTIGRAVITY_MCP_FILE"
                        echo "  [REMOVED] GAL-managed MCP entries from: $ANTIGRAVITY_MCP_FILE"
                    fi
                fi
            fi
        else
            echo "  [WARN] jq not found — cannot clean GAL-managed MCP entries from $ANTIGRAVITY_MCP_FILE"
        fi
    fi
fi

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