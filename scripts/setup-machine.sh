#!/usr/bin/env bash
# setup-machine.sh — Orchestrate GAL machine setup by invoking concern-specific update scripts.
#
# Runs the shell concern scripts in order:
#   1. update-personalization.sh
#   2. update-skills.sh
#   3. update-commands.sh
#   4. update-mcp.sh
#   5. install-gal-plugins.sh
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

BOOTSTRAP_INSTALL=false
for arg in "$@"; do
    if [ "$arg" = '--bootstrap-install' ]; then
        BOOTSTRAP_INSTALL=true
        break
    fi
done

get_configured_install_mode() {
    if [ ! -f "$GAL_CONFIG_FILE" ]; then
        if $BOOTSTRAP_INSTALL; then
            printf 'install\n'
            return 0
        fi

        printf 'source\n'
        return 0
    fi

    run_python - "$GAL_CONFIG_FILE" <<'PY'
import json
import sys

with open(sys.argv[1], encoding='utf-8') as handle:
    data = json.load(handle)

mode = str(data.get('installMode') or 'source').strip()
print(mode or 'source')
PY
}

SETUP_INSTALL_MODE="$(get_configured_install_mode)"

previous_bootstrap_env="${GAL_BOOTSTRAP_INSTALL-}"
if $BOOTSTRAP_INSTALL; then
    export GAL_BOOTSTRAP_INSTALL=true
fi

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

shared_args=()
$UNINSTALL && shared_args+=(--uninstall)
$REPLACE && shared_args+=(--replace)
$DRY_RUN && shared_args+=(--dry-run)
$RECONFIGURE && shared_args+=(--reconfigure)

if ! $UNINSTALL; then
    shared_args+=(--selected-runtimes "$SELECTED_RUNTIMES_CSV" --primary-runtime "$PRIMARY_RUNTIME")
fi

step_names=(Personalization Skills Commands MCP 'Install Orchestration')
step_scripts=(update-personalization.sh update-skills.sh update-commands.sh update-mcp.sh install-gal-plugins.sh)

for index in "${!step_scripts[@]}"; do
    if [ "$SETUP_INSTALL_MODE" = 'install' ] && ! $UNINSTALL && { [ "${step_names[$index]}" = 'Skills' ] || [ "${step_names[$index]}" = 'Commands' ]; }; then
        echo ''
        echo ">>> Skipping ${step_names[$index]}"
        echo "  [SKIP] ${step_names[$index]} stay source-mode-only because install mode must not depend on repo-root links or baked {{GAL_ROOT}} paths."
        continue
    fi

    echo ''
    echo ">>> Running ${step_names[$index]}"

    step_args=("${shared_args[@]}")
    if [ "${step_names[$index]}" = 'Install Orchestration' ] && $BOOTSTRAP_INSTALL; then
        step_args+=(--bootstrap-install)
    fi

    "$SCRIPT_DIR/${step_scripts[$index]}" "${step_args[@]}"
done

if $BOOTSTRAP_INSTALL; then
    if [ -n "$previous_bootstrap_env" ]; then
        export GAL_BOOTSTRAP_INSTALL="$previous_bootstrap_env"
    else
        unset GAL_BOOTSTRAP_INSTALL
    fi
fi

echo ''
if $UNINSTALL; then
    echo 'Uninstall complete.'
elif $DRY_RUN; then
    echo 'Dry run complete. No changes made.'
    echo "Selected runtimes: $(format_runtime_csv "$SELECTED_RUNTIMES_CSV")"
    echo "Primary runtime: $PRIMARY_RUNTIME"
else
    echo "Setup complete: runtimes=$(format_runtime_csv "$SELECTED_RUNTIMES_CSV"); primary=$PRIMARY_RUNTIME"
    if [ "$SETUP_INSTALL_MODE" = 'source' ]; then
        echo 'Note: If SKILL.template.md or SKILL.local.md changes, rerun update-commands.sh or setup-machine.sh.'
    else
        echo 'Note: Source-only skills and commands updates were skipped because install mode uses provider-native projections.'
        if $BOOTSTRAP_INSTALL; then
            echo 'Note: Bootstrap install seeded install mode for first launch; switch to source mode later only if you set galRoot and devMode explicitly.'
        fi
    fi
fi