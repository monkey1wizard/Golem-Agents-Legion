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
            copilot) echo "[copilot] mode=native-install renderer=build-core-plugin.sh shortcut=none lifecycle=artifact-rendered-install-deferred" ;;
            codex) echo "[codex] mode=native-install renderer=build-core-plugin.sh shortcut=none lifecycle=artifact-rendered-install-deferred" ;;
            claude) echo "[claude] mode=native-install renderer=build-core-plugin.sh shortcut=none lifecycle=artifact-rendered-install-deferred" ;;
            '') ;;
            *) echo "Unsupported provider: $local_provider" >&2; exit 1 ;;
        esac
    done
    exit 0
fi

# T-003: Resolve mode and effective source root
install_mode="$(get_configured_install_mode)" || exit 1
effective_repo_root="$REPO_ROOT"
if [[ "$install_mode" == 'source' && -f "$GAL_CONFIG_FILE" ]]; then
    effective_repo_root="$(run_python - "$GAL_CONFIG_FILE" "$REPO_ROOT" <<'PY'
import json
import sys
with open(sys.argv[1], encoding='utf-8') as f:
    config = json.load(f)
gal_root = config.get('galRoot', '').strip()
print(gal_root if gal_root else sys.argv[2])
PY
)" || effective_repo_root="$REPO_ROOT"
fi

core_args=(--resolved-plugins-file "$tmp_resolved_plugins_file" --install-mode "$install_mode")
if [[ "$FORCE" == 'true' ]]; then
    core_args+=(--force)
fi

write_provider_managed_state_from_build_plan() {
    local provider="$1"
    local mode="$2"
    local canonical_root="$3"
    local package_output_root="$4"
    local install_target="$5"
    local shortcut_target="$6"

    local projection_root=''
    if [[ "$provider" == 'agy' ]]; then
        projection_root="$install_target"
    fi

    local lifecycle_status='artifact-rendered-install-deferred'
    if [[ "$provider" == 'agy' ]]; then
        lifecycle_status='implemented'
    fi

    local status
    status="$(resolve_provider_managed_state_status "$mode" "$lifecycle_status" "$projection_root")"
    local read_surface
    read_surface="$(resolve_provider_managed_read_surface "$status")"
    local state_path
    state_path="$(get_provider_managed_state_path "$provider")"

    mkdir -p "$(dirname "$state_path")"
    jq -n \
        --arg provider "$provider" \
        --arg canonicalRoot "$canonical_root" \
        --arg packageOutputRoot "$package_output_root" \
        --arg projectionRoot "$projection_root" \
        --arg installTarget "$install_target" \
        --arg shortcutTarget "$shortcut_target" \
        --arg generatedAt "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
        --arg status "$status" \
        --arg readSurface "$read_surface" \
        --arg mode "$mode" \
        --arg lifecycleStatus "$lifecycle_status" \
        '{
            schemaVersion: 1,
            provider: $provider,
            canonicalRoot: $canonicalRoot,
            packageOutputRoot: $packageOutputRoot,
            projectionRoot: (if $projectionRoot == "" then null else $projectionRoot end),
            installTarget: $installTarget,
            shortcutTarget: (if $shortcutTarget == "" then null else $shortcutTarget end),
            generatedAt: $generatedAt,
            status: $status,
            readSurface: (if $readSurface == "" then null else $readSurface end),
            cli: {
                available: false,
                validateSupported: false,
                localArtifactInstallSupported: false,
                marketplaceInstallSupported: false,
                installScopeSupported: false,
                installHelpSummary: null
            },
            validation: {
                command: null,
                strictPassed: false
            },
            lifecycle: {
                mode: $mode,
                status: $lifecycleStatus
            },
            notes: [
                "Base provider ledger written from the provider build plan.",
                "Later lifecycle-specific tasks may enrich this file with provider-native validation and install details."
            ]
        }' > "$state_path"
    echo "[OK] Wrote $provider managed state: $state_path"
}

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
        copilot|codex|claude)
            if [[ "$canonical_plugin_rendered" != 'true' ]]; then
                # Render the shared canonical root once for any selected native-install lane.
                "$SCRIPT_DIR/build-core-plugin.sh" "${core_args[@]}"
                canonical_plugin_rendered=true
            fi
            ;;
        '') ;;
        *) echo "Unsupported provider: $local_provider" >&2; exit 1 ;;
    esac
done

for local_provider in "${requested_providers[@]}"; do
    case "$local_provider" in
        agy)
            write_provider_managed_state_from_build_plan 'agy' 'managed-shortcut' "$(get_gal_plugin_root gal)" "$(get_gal_plugin_root gal)" "$(get_agy_plugin_install_target)" "$(get_gal_active_provider_target agy)"
            ;;
        copilot)
            write_provider_managed_state_from_build_plan 'copilot' 'native-install' "$(get_gal_plugin_root gal)" "$(get_gal_plugin_root gal)" 'provider-managed via gh copilot plugin install <canonical-root>' ''
            ;;
        codex)
            write_provider_managed_state_from_build_plan 'codex' 'native-install' "$(get_gal_plugin_root gal)" "$(get_gal_plugin_root gal)" 'provider-managed via codex plugin marketplace add + codex plugin add gal@gal-marketplace' ''
            ;;
        claude)
            write_provider_managed_state_from_build_plan 'claude' 'native-install' "$(get_gal_plugin_root gal)" "$(get_gal_plugin_root gal)" 'provider-managed via claude plugin install --scope <scope>' ''
            ;;
        '') ;;
        *) echo "Unsupported provider: $local_provider" >&2; exit 1 ;;
    esac
done
