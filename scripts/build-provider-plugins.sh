#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

source "$SCRIPT_DIR/common/common.sh"

CATALOG_PATH="$REPO_ROOT/plugins/catalog.json"
CONFIG_PATH="$HOME/.gal/config/config.json"
LOCKFILE_PATH="$HOME/.gal/state/plugins.lock.json"
PROVIDERS_CSV='agy,copilot,codex,claude'
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
        --providers)
            shift
            PROVIDERS_CSV="$1"
            shift
            ;;
        *) echo "Unknown option: $1" >&2 ; exit 1 ;;
    esac
done

effective_lockfile_path="$LOCKFILE_PATH"
if [[ "$DRY_RUN" == 'true' ]]; then
    effective_lockfile_path="${TMPDIR:-/tmp}/gal-provider-build-lock-$$.json"
fi

resolved_plugins_json="$(pwsh -NoProfile -File "$SCRIPT_DIR/Resolve-GalCatalog.ps1" -CatalogPath "$CATALOG_PATH" -ConfigPath "$CONFIG_PATH" -LockfilePath "$effective_lockfile_path" -PassThru | jq '.ResolvedPlugins')"
tmp_resolved_plugins_file="${TMPDIR:-/tmp}/gal-resolved-plugins-$$.json"
printf '%s' "$resolved_plugins_json" > "$tmp_resolved_plugins_file"
trap 'rm -f "$tmp_resolved_plugins_file" "$effective_lockfile_path"' EXIT

IFS=',' read -r -a requested_providers <<< "$PROVIDERS_CSV"

if [[ "$DRY_RUN" == 'true' ]]; then
    echo '--- DRY RUN ---'
    echo "Resolved plugins: $(printf '%s' "$resolved_plugins_json" | jq -r 'map(.pluginId) | join(", ")')"
    local_provider=''
    for local_provider in "${requested_providers[@]}"; do
        case "$local_provider" in
            agy) echo "[agy] mode=managed-shortcut renderer=build-core-plugin.sh shortcut=$(get_gal_active_provider_target agy) lifecycle=implemented" ;;
            copilot) echo "[copilot] mode=native-install renderer=build-core-plugin.sh shortcut=none lifecycle=implemented" ;;
            codex) echo "[codex] mode=native-install renderer=not-yet-implemented shortcut=none lifecycle=not-implemented" ;;
            claude) echo "[claude] mode=native-install renderer=build-core-plugin.sh shortcut=none lifecycle=artifact-rendered-install-deferred" ;;
            '') ;;
            *) echo "Unsupported provider: $local_provider" >&2; exit 1 ;;
        esac
    done
    exit 0
fi

core_args=(--resolved-plugins-file "$tmp_resolved_plugins_file")
if [[ "$FORCE" == 'true' ]]; then
    core_args+=(--force)
fi

canonical_plugin_rendered=false
for local_provider in "${requested_providers[@]}"; do
    case "$local_provider" in
        agy)
            # Core renderer builds superset canonical root AND installs all 3 AGY surfaces
            "$SCRIPT_DIR/build-core-plugin.sh" "${core_args[@]}" --install
            canonical_plugin_rendered=true
            mkdir -p "$(dirname "$(get_gal_active_provider_target agy)")"
            if ! safe_link "$(get_gal_active_provider_target agy)" "$(get_agy_plugin_install_target)"; then
                echo "Failed to claim GAL-managed shortcut for provider 'agy': $(get_gal_active_provider_target agy)" >&2
                exit 1
            fi
            ;;
        claude)
            if [[ "$canonical_plugin_rendered" != 'true' ]]; then
                # Core renderer without --install (Claude uses marketplace; no surface projection needed)
                "$SCRIPT_DIR/build-core-plugin.sh" "${core_args[@]}"
                canonical_plugin_rendered=true
            fi
            ;;
        copilot|codex|'') ;;
        *) echo "Unsupported provider: $local_provider" >&2; exit 1 ;;
    esac
done
