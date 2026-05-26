#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
. "$SCRIPT_DIR/common/common.sh"

CONFIG_PATH="$GAL_CONFIG_FILE"
LOCKFILE_PATH="$GAL_PLUGINS_LOCK_FILE"
SELECTED_RUNTIMES_CSV=""
PRIMARY_RUNTIME_OVERRIDE=""
DRY_RUN=false
UNINSTALL=false
FORCE=false
REPLACE=false
RECONFIGURE=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --config-path) shift; CONFIG_PATH="$1"; shift ;;
        --lockfile-path) shift; LOCKFILE_PATH="$1"; shift ;;
        --selected-runtimes) shift; SELECTED_RUNTIMES_CSV="$1"; shift ;;
        --selected-runtimes=*) SELECTED_RUNTIMES_CSV="${1#*=}"; shift ;;
        --primary-runtime) shift; PRIMARY_RUNTIME_OVERRIDE="$1"; shift ;;
        --primary-runtime=*) PRIMARY_RUNTIME_OVERRIDE="${1#*=}"; shift ;;
        --dry-run) DRY_RUN=true; shift ;;
        --uninstall) UNINSTALL=true; shift ;;
        --replace) REPLACE=true; shift ;;
        --reconfigure) RECONFIGURE=true; shift ;;
        --force) FORCE=true; shift ;;
        *) echo "Unknown option: $1" >&2; exit 1 ;;
    esac
done

provider_from_runtime() {
    case "$1" in
        antigravity) printf '%s\n' 'agy' ;;
        *) printf '%s\n' "$1" ;;
    esac
}

lane_from_runtime() {
    case "$1" in
        opencode) printf '%s\n' 'bridge' ;;
        gemini) printf '%s\n' 'migration' ;;
        *) printf '%s\n' 'primary' ;;
    esac
}

resolve_selection_csv() {
    local selected="$SELECTED_RUNTIMES_CSV"
    local primary="$PRIMARY_RUNTIME_OVERRIDE"
    if [ -z "$selected" ]; then
        selected="$(normalize_runtime_csv "$(read_install_state_value selectedRuntimes 2>/dev/null || true)")"
    else
        selected="$(normalize_runtime_csv "$selected")"
    fi
    if [ -z "$selected" ]; then
        selected='copilot,antigravity,codex,claude'
    fi

    if [ -z "$primary" ]; then
        primary="$(read_install_state_value primaryRuntime 2>/dev/null || true)"
    fi
    if [ -z "$primary" ]; then
        IFS=',' read -r primary _ <<< "$selected"
    fi

    printf '%s\n%s\n' "$selected" "$primary"
}

build_default_config_json() {
    local selected_csv="$1"
    local primary_runtime="$2"
    run_python - "$REPO_ROOT" "$selected_csv" "$primary_runtime" <<'PY'
import json
import sys

repo_root = sys.argv[1]
selected = [item for item in sys.argv[2].split(',') if item]
primary_runtime = sys.argv[3]

def provider_from_runtime(runtime: str) -> str:
    return 'agy' if runtime == 'antigravity' else runtime

def lane_from_runtime(runtime: str) -> str:
    if runtime == 'opencode':
        return 'bridge'
    if runtime == 'gemini':
        return 'migration'
    return 'primary'

provider_selections = {
    provider_from_runtime(runtime): {
        'enabled': True,
        'lane': lane_from_runtime(runtime),
    }
    for runtime in selected
}

preferred = []
primary_provider = provider_from_runtime(primary_runtime)
if primary_provider in provider_selections:
    preferred.append(primary_provider)
for provider in provider_selections:
    if provider not in preferred:
        preferred.append(provider)

config = {
    'schemaVersion': 1,
    'galRoot': repo_root,
    'devMode': True,
    'defaultProfile': 'default',
    'profiles': {},
    'enabledPlugins': [],
    'disabledPlugins': [],
    'providerSelections': provider_selections,
    'installMode': 'source',
    'preferredProviders': preferred,
    'userSettings': {},
}
print(json.dumps(config, indent=2))
PY
}

read_config_summary() {
    local path="$1"
    run_python - "$path" <<'PY'
import json
import sys

path = sys.argv[1]
data = json.loads(open(path, encoding='utf-8').read())
providers = data.get('providerSelections', {})

def by_lane(lane: str):
    return [name for name, value in providers.items() if value.get('enabled', True) and value.get('lane') == lane]

summary = {
    'installMode': data.get('installMode', 'source'),
    'galRoot': data.get('galRoot', ''),
    'devMode': bool(data.get('devMode', False)),
    'defaultProfile': data.get('defaultProfile', 'default'),
    'enabledPlugins': data.get('enabledPlugins', []),
    'primaryProviders': by_lane('primary'),
    'bridgeProviders': by_lane('bridge'),
    'migrationProviders': by_lane('migration'),
}
print(json.dumps(summary))
PY
}

echo ''
echo '=== GAL install orchestration ==='

if $UNINSTALL; then
    echo '  [SKIP] Mode-aware provider orchestration is not yet part of uninstall; legacy cleanup remains in the existing concern scripts.'
    exit 0
fi

mapfile -t selection_parts < <(resolve_selection_csv)
resolved_selected_csv="${selection_parts[0]}"
resolved_primary_runtime="${selection_parts[1]}"

mkdir -p "$GAL_CONFIG_ROOT" "$GAL_STATE_DIRECTORY" "$GAL_STORE_PLUGINS_ROOT" "$GAL_GENERATED_MCP_ROOT" "$GAL_GENERATED_XMACHINE_ROOT" "$GAL_GENERATED_PROVIDERS_ROOT"

config_exists=false
config_for_resolver="$CONFIG_PATH"
cleanup_paths=()
if [ -f "$CONFIG_PATH" ]; then
    config_exists=true
    echo "  [OK] Using machine config: $CONFIG_PATH"
else
    config_preview_path="${TMPDIR:-/tmp}/gal-config-preview-$$.json"
    build_default_config_json "$resolved_selected_csv" "$resolved_primary_runtime" > "$config_preview_path"
    config_for_resolver="$config_preview_path"
    cleanup_paths+=("$config_preview_path")
    install_mode_preview="$(run_python - "$config_preview_path" <<'PY'
import json, sys
print(json.loads(open(sys.argv[1], encoding='utf-8').read()).get('installMode', 'source'))
PY
)"
    if $DRY_RUN; then
        echo "  [DRY RUN] Would seed machine config: $CONFIG_PATH (installMode=$install_mode_preview)"
    else
        cp "$config_preview_path" "$CONFIG_PATH"
        echo "  [OK] Seeded machine config: $CONFIG_PATH"
        config_exists=true
        config_for_resolver="$CONFIG_PATH"
    fi
fi

resolver_lockfile_path="$LOCKFILE_PATH"
if $DRY_RUN; then
    resolver_lockfile_path="${TMPDIR:-/tmp}/gal-lock-preview-$$.json"
    cleanup_paths+=("$resolver_lockfile_path")
fi

summary_json="$(read_config_summary "$config_for_resolver")"
install_mode="$(printf '%s' "$summary_json" | jq -r '.installMode')"
default_profile="$(printf '%s' "$summary_json" | jq -r '.defaultProfile')"
resolved_plugins="$(pwsh -NoProfile -File "$SCRIPT_DIR/Resolve-GalCatalog.ps1" -CatalogPath "$REPO_ROOT/plugins/catalog.json" -ConfigPath "$config_for_resolver" -LockfilePath "$resolver_lockfile_path" -PassThru | jq -r '.ResolvedPlugins | map(.pluginId) | join(", ")')"
primary_providers_csv="$(printf '%s' "$summary_json" | jq -r '.primaryProviders | join(",")')"

echo "  [OK] Mode: $install_mode"
echo "  [OK] GAL runtime home: $GAL_STATE_ROOT"
echo "  [OK] Lockfile target: $LOCKFILE_PATH"
echo "  [OK] Active profile: $default_profile"
echo "  [OK] Explicit plugins: $(printf '%s' "$summary_json" | jq -r 'if (.enabledPlugins | length) == 0 then "none" else (.enabledPlugins | join(", ")) end')"
echo "  [OK] Resolved plugins: ${resolved_plugins:-none}"
echo "  [OK] Primary provider lanes: $(printf '%s' "$summary_json" | jq -r 'if (.primaryProviders | length) == 0 then "none" else (.primaryProviders | join(", ")) end')"
echo "  [OK] Bridge lanes: $(printf '%s' "$summary_json" | jq -r 'if (.bridgeProviders | length) == 0 then "none" else (.bridgeProviders | join(", ")) end')"
echo "  [OK] Migration lanes: $(printf '%s' "$summary_json" | jq -r 'if (.migrationProviders | length) == 0 then "none" else (.migrationProviders | join(", ")) end')"

if [ "$install_mode" = 'source' ]; then
    echo "  [OK] Source mode galRoot: $(printf '%s' "$summary_json" | jq -r '.galRoot')"
    echo "  [OK] Source mode devMode: $(printf '%s' "$summary_json" | jq -r '.devMode')"
    echo "  [OK] Local override boundary: explicit machine-local bindings via $GAL_XMACHINE_CONFIG_FILE"
    echo '  [OK] Provider-native install orchestration is deferred in source mode; repo-root links remain the contributor path until T-009.'
else
    echo "  [OK] Install mode projections root: $GAL_GENERATED_ROOT"
    if [ -n "$primary_providers_csv" ]; then
        build_provider_args=(--config-path "$config_for_resolver" --lockfile-path "$resolver_lockfile_path" --providers "$primary_providers_csv")
        $DRY_RUN && build_provider_args+=(--dry-run)
        $FORCE && build_provider_args+=(--force)
        "$SCRIPT_DIR/build-provider-plugins.sh" "${build_provider_args[@]}"
    else
        echo '  [SKIP] No primary providers selected for provider-native install orchestration.'
    fi

    bridge_output="$(printf '%s' "$summary_json" | jq -r 'if (.bridgeProviders | length) == 0 then "" else (.bridgeProviders | join(", ")) end')"
    migration_output="$(printf '%s' "$summary_json" | jq -r 'if (.migrationProviders | length) == 0 then "" else (.migrationProviders | join(", ")) end')"
    [ -n "$bridge_output" ] && echo "  [OK] Bridge lanes stay capability-only and target ~/.gal/active/<provider>: $bridge_output"
    [ -n "$migration_output" ] && echo "  [OK] Migration lanes stay compatibility-only: $migration_output"
fi

cleanup_path=''
for cleanup_path in "${cleanup_paths[@]}"; do
    [ -n "$cleanup_path" ] && rm -f "$cleanup_path"
done