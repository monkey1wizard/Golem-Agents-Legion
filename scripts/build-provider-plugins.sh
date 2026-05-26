#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

source "$SCRIPT_DIR/common/common.sh"

CATALOG_PATH="$REPO_ROOT/plugins/catalog.json"
CONFIG_PATH="$HOME/.gal/config/config.json"
LOCKFILE_PATH="$HOME/.gal/state/plugins.lock.json"
DRY_RUN=false
FORCE=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --dry-run) DRY_RUN=true ; shift ;;
        --force) FORCE=true ; shift ;;
        --catalog-path)
            shift
            CATALOG_PATH="$1"
            shift
            ;;
        --config-path)
            shift
            CONFIG_PATH="$1"
            shift
            ;;
        --lockfile-path)
            shift
            LOCKFILE_PATH="$1"
            shift
            ;;
        *) echo "Unknown option: $1" >&2 ; exit 1 ;;
    esac
done

resolved_plugins_json="$(pwsh -NoProfile -File "$SCRIPT_DIR/Resolve-GalCatalog.ps1" -CatalogPath "$CATALOG_PATH" -ConfigPath "$CONFIG_PATH" -LockfilePath "$LOCKFILE_PATH" -PassThru | jq '.ResolvedPlugins')"
tmp_resolved_plugins_file="${TMPDIR:-/tmp}/gal-resolved-plugins-$$.json"
printf '%s' "$resolved_plugins_json" > "$tmp_resolved_plugins_file"
trap 'rm -f "$tmp_resolved_plugins_file"' EXIT

if [[ "$DRY_RUN" == 'true' ]]; then
    echo '--- DRY RUN ---'
    echo "Resolved plugins: $(printf '%s' "$resolved_plugins_json" | jq -r 'map(.pluginId) | join(", ")')"
    echo "[agy] mode=managed-shortcut renderer=build-agy-plugin.sh shortcut=$(get_gal_active_provider_target agy)"
    echo "[copilot] mode=native-install renderer=not-yet-implemented shortcut=none"
    echo "[codex] mode=native-install renderer=not-yet-implemented shortcut=none"
    echo "[claude] mode=native-install renderer=not-yet-implemented shortcut=none"
    exit 0
fi

agy_args=(--resolved-plugins-file "$tmp_resolved_plugins_file")
if [[ "$FORCE" == 'true' ]]; then
    agy_args+=(--force)
fi

"$SCRIPT_DIR/build-agy-plugin.sh" "${agy_args[@]}"