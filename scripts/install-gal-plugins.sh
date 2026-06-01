#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
. "$SCRIPT_DIR/common/common.sh"
. "$SCRIPT_DIR/common/provider-plugin.sh"

CONFIG_PATH="$GAL_CONFIG_FILE"
LOCKFILE_PATH="$GAL_PLUGINS_LOCK_FILE"
SELECTED_RUNTIMES_CSV=""
PRIMARY_RUNTIME_OVERRIDE=""
DRY_RUN=false
UNINSTALL=false
PURGE=false
CONFIRM_PURGE=false
FORCE=false
REPLACE=false
RECONFIGURE=false
BOOTSTRAP_INSTALL=false

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
        --purge) PURGE=true; shift ;;
        --confirm-purge) CONFIRM_PURGE=true; shift ;;
        --bootstrap-install) BOOTSTRAP_INSTALL=true; shift ;;
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
    local bootstrap_install="$3"
    run_python - "$REPO_ROOT" "$selected_csv" "$primary_runtime" "$bootstrap_install" <<'PY'
import json
import sys

repo_root = sys.argv[1]
selected = [item for item in sys.argv[2].split(',') if item]
primary_runtime = sys.argv[3]
bootstrap_install = sys.argv[4].lower() == 'true'

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
    'galRoot': '' if bootstrap_install else repo_root,
    'devMode': False if bootstrap_install else True,
    'defaultProfile': 'default',
    'profiles': {},
    'enabledPlugins': [],
    'disabledPlugins': [],
    'providerSelections': provider_selections,
    'installMode': 'install' if bootstrap_install else 'source',
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

get_claude_lifecycle_state_path() {
    printf '%s\n' "$GAL_GENERATED_PROVIDERS_ROOT/claude/managed.json"
}

get_codex_lifecycle_state_path() {
    printf '%s\n' "$GAL_GENERATED_PROVIDERS_ROOT/codex/managed.json"
}

write_copilot_lifecycle_state() {
    local state_json="$1"
    local state_path
    state_path="$(get_provider_managed_state_path copilot)"
    if $DRY_RUN; then
        echo "  [DRY RUN] Would write Copilot lifecycle state: $state_path"
        return 0
    fi

    mkdir -p "$(dirname "$state_path")"
    printf '%s\n' "$state_json" > "$state_path"
    echo "  [OK] Wrote Copilot lifecycle state: $state_path"
}

write_claude_lifecycle_state() {
    local state_json="$1"
    local state_path
    state_path="$(get_provider_managed_state_path claude)"
    if $DRY_RUN; then
        echo "  [DRY RUN] Would write Claude lifecycle state: $state_path"
        return 0
    fi

    mkdir -p "$(dirname "$state_path")"
    printf '%s\n' "$state_json" > "$state_path"
    echo "  [OK] Wrote Claude lifecycle state: $state_path"
}

write_codex_lifecycle_state() {
    local state_json="$1"
    local state_path
    state_path="$(get_provider_managed_state_path codex)"
    if $DRY_RUN; then
        echo "  [DRY RUN] Would write Codex lifecycle state: $state_path"
        return 0
    fi

    mkdir -p "$(dirname "$state_path")"
    printf '%s\n' "$state_json" > "$state_path"
    echo "  [OK] Wrote Codex lifecycle state: $state_path"
}

ensure_codex_marketplace_manifest() {
    local manifest_path="$GAL_PLUGINS_ROOT/.agents/plugins/marketplace.json"
    mkdir -p "$(dirname "$manifest_path")"

    if $DRY_RUN; then
        echo "  [DRY RUN] Would refresh Codex marketplace descriptor: $manifest_path" >&2
        printf '%s\n' "$manifest_path"
        return 0
    fi

    jq -n '{
        name: "gal-marketplace",
        interface: { displayName: "GAL Plugin Marketplace" },
        plugins: [
            {
                name: "gal",
                source: { source: "local", path: "./gal" },
                policy: { installation: "AVAILABLE", authentication: "ON_INSTALL" },
                category: "Engineering"
            }
        ]
    }' > "$manifest_path"
    echo "  [OK] Refreshed Codex marketplace descriptor: $manifest_path" >&2
    printf '%s\n' "$manifest_path"
}

refresh_copilot_host_copy() {
    local canonical_root="$1"
    local target_path="$2"
    local manifest_path="$target_path/copilot-manifest.json"

    if $DRY_RUN; then
        echo "  [DRY RUN] Would refresh Copilot host copy: $target_path" >&2
        echo "  [DRY RUN] Would bump Copilot manifest version in host copy: $manifest_path" >&2
        printf '%s\n' 'preview-host-bump'
        return 0
    fi

    rm -rf "$target_path"
    mkdir -p "$(dirname "$target_path")"
    cp -R "$canonical_root" "$target_path"

    run_python - "$manifest_path" <<'PY'
import json
import sys
from datetime import datetime

path = sys.argv[1]
with open(path, encoding='utf-8') as f:
    data = json.load(f)
current = data.get('version') or '1.0.0'
bumped = f"{current}.host{datetime.utcnow().strftime('%Y%m%d%H%M%S')}"
data['version'] = bumped
with open(path, 'w', encoding='utf-8', newline='\n') as f:
    json.dump(data, f, indent=2)
    f.write('\n')
print(bumped)
PY
}

invoke_copilot_plugin_lifecycle() {
    if ! $INSTALL_COPILOT; then
        return 0
    fi

    echo '  [OK] Evaluating Copilot plugin lifecycle.'
    local canonical_root package_output_root manifest_path support_json
    canonical_root="$(get_gal_plugin_root gal)"
    package_output_root="$(get_copilot_plugin_package_output_root "$REPO_ROOT")"
    manifest_path="$(get_copilot_plugin_manifest_path "$canonical_root")"
    support_json="$(get_copilot_cli_lifecycle_support_json)"

    if [[ ! -d "$canonical_root" || ! -f "$manifest_path" ]]; then
        echo '  [WARN] Copilot canonical artifact was missing; rerendering shared plugin root before lifecycle projection.'
        "$SCRIPT_DIR/build-core-plugin.sh" >/dev/null
    fi

    if [[ ! -d "$canonical_root" ]]; then
        echo "Copilot canonical root not found: $canonical_root" >&2
        exit 1
    fi

    if [[ ! -f "$manifest_path" ]]; then
        echo "Copilot manifest not found: $manifest_path" >&2
        exit 1
    fi

    local cli_available=false
    [[ "$(printf '%s' "$support_json" | jq -r '.cliAvailable')" == 'true' ]] && cli_available=true
    local local_artifact_install_supported=false
    [[ "$(printf '%s' "$support_json" | jq -r '.localArtifactInstallSupported')" == 'true' ]] && local_artifact_install_supported=true
    local install_scope_supported=false
    [[ "$(printf '%s' "$support_json" | jq -r '.installScopeSupported')" == 'true' ]] && install_scope_supported=true
    local marketplace_install_supported=false
    [[ "$(printf '%s' "$support_json" | jq -r '.marketplaceInstallSupported')" == 'true' ]] && marketplace_install_supported=true
    local install_help_summary
    install_help_summary="$(printf '%s' "$support_json" | jq -r '.installHelpSummary // empty')"
    local lifecycle_mode
    lifecycle_mode="$(printf '%s' "$support_json" | jq -r '.installMode')"

    if ! $cli_available; then
        echo '  [WARN] GitHub CLI not found on PATH; projecting Copilot plugin without CLI lifecycle validation.'
    elif ! $local_artifact_install_supported; then
        echo '  [WARN] GitHub CLI is present but local Copilot artifact install support was not detected; keeping projection-managed lifecycle.'
    else
        echo '  [OK] GitHub CLI reports local Copilot artifact install support.'
    fi

    safe_unlink "$GAL_ROOT_COPILOT"
    mkdir -p "$(dirname "$COPILOT_PLUGIN_INSTALL_TARGET")"

    local refreshed_copy_to_host=false
    local version_bumped_to=''
    if ensure_symlink "$COPILOT_PLUGIN_INSTALL_TARGET" "$canonical_root" 'directory'; then
        echo "  [OK] Projected Copilot plugin root: $COPILOT_PLUGIN_INSTALL_TARGET"
    else
        echo '  [WARN] Copilot projection link was unavailable; refreshing host copy instead.'
        refreshed_copy_to_host=true
        version_bumped_to="$(refresh_copilot_host_copy "$canonical_root" "$COPILOT_PLUGIN_INSTALL_TARGET")"
        echo "  [OK] Refreshed Copilot host copy: $COPILOT_PLUGIN_INSTALL_TARGET"
    fi

    local provider_state_status='linked-projection'
    if $refreshed_copy_to_host; then
        provider_state_status='refreshed-copy2-host'
    fi
    local provider_read_surface
    provider_read_surface="$(resolve_provider_managed_read_surface "$provider_state_status")"

    local state_json
    state_json="$(jq -n \
        --arg canonicalRoot "$canonical_root" \
        --arg packageOutputRoot "$package_output_root" \
        --arg projectionRoot "$COPILOT_PLUGIN_INSTALL_TARGET" \
        --arg installTarget "$COPILOT_PLUGIN_INSTALL_TARGET" \
        --arg manifestPath "$manifest_path" \
        --arg generatedAt "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
        --arg status "$provider_state_status" \
        --arg readSurface "$provider_read_surface" \
        --arg installHelpSummary "$install_help_summary" \
        --arg mode "$lifecycle_mode" \
        --arg versionBumpedTo "$version_bumped_to" \
        --arg sessionLoadCommand 'GitHub Copilot reads the projected plugin from ~/.copilot/installed-plugins/gal-copilot/gal' \
        --arg installTemplate 'gh copilot plugin install <plugin-root>' \
        --arg updateTemplate 'gh copilot plugin update <plugin-id>' \
        --arg uninstallTemplate 'gh copilot plugin uninstall <plugin-id>' \
        --argjson cliAvailable "$cli_available" \
        --argjson localArtifactInstallSupported "$local_artifact_install_supported" \
        --argjson marketplaceInstallSupported "$marketplace_install_supported" \
        --argjson installScopeSupported "$install_scope_supported" \
        --argjson refreshedCopyToHost "$refreshed_copy_to_host" \
        '{
            schemaVersion: 1,
            provider: "copilot",
            canonicalRoot: $canonicalRoot,
            packageOutputRoot: $packageOutputRoot,
            projectionRoot: $projectionRoot,
            installTarget: $installTarget,
            manifestPath: $manifestPath,
            generatedAt: $generatedAt,
            status: $status,
            readSurface: (if $readSurface == "" then null else $readSurface end),
            cli: {
                available: $cliAvailable,
                validateSupported: false,
                localArtifactInstallSupported: $localArtifactInstallSupported,
                marketplaceInstallSupported: $marketplaceInstallSupported,
                installScopeSupported: $installScopeSupported,
                installHelpSummary: $installHelpSummary
            },
            validation: {
                command: null,
                strictPassed: false
            },
            lifecycle: {
                mode: $mode,
                stagedPluginRoot: $projectionRoot,
                refreshedCopyToHost: $refreshedCopyToHost,
                versionBumpedTo: (if $versionBumpedTo == "" then null else $versionBumpedTo end),
                sessionLoadCommand: $sessionLoadCommand,
                installCommandTemplate: $installTemplate,
                updateCommandTemplate: $updateTemplate,
                uninstallCommandTemplate: $uninstallTemplate
            },
            notes: [
                "Copilot CLI probing is best-effort; missing gh only downgrades lifecycle metadata, not artifact projection.",
                "Persistent install prefers a link projection at ~/.copilot/installed-plugins/gal-copilot/gal so VS Code Copilot and Copilot CLI read the same payload.",
                "~/.copilot/gal is treated as a legacy GAL_ROOT link and is removed when GAL refreshes the Copilot projection.",
                "When link projection is unavailable, GAL refreshes a host copy and bumps copilot-manifest.json version to avoid stale Copilot cache reuse."
            ]
        }')"

    write_copilot_lifecycle_state "$state_json"
}

invoke_claude_plugin_lifecycle() {
    if ! $INSTALL_CLAUDE; then
        return 0
    fi

    echo '  [OK] Evaluating Claude plugin lifecycle.'
    local canonical_root package_output_root manifest_path session_load_command
    canonical_root="$(get_gal_plugin_root gal)"
    package_output_root="$(get_claude_plugin_package_output_root "$REPO_ROOT")"
    manifest_path="$(get_claude_plugin_manifest_path "$canonical_root")"
    session_load_command="claude --plugin-dir \"$canonical_root\""

    if [[ ! -d "$canonical_root" ]]; then
        echo "Claude canonical root not found: $canonical_root" >&2
        exit 1
    fi

    if [[ ! -f "$manifest_path" ]]; then
        echo "Claude manifest not found: $manifest_path" >&2
        exit 1
    fi

    local support_json
    support_json="$(get_claude_cli_lifecycle_support_json)"

    local cli_available=false
    if [[ "$(printf '%s' "$support_json" | jq -r '.cliAvailable')" == 'true' ]]; then
        cli_available=true
    fi

    local validate_supported=false
    if [[ "$(printf '%s' "$support_json" | jq -r '.validateSupported')" == 'true' ]]; then
        validate_supported=true
    fi

    local local_artifact_install_supported=false
    if [[ "$(printf '%s' "$support_json" | jq -r '.localArtifactInstallSupported')" == 'true' ]]; then
        local_artifact_install_supported=true
    fi

    local marketplace_install_supported=false
    if [[ "$(printf '%s' "$support_json" | jq -r '.marketplaceInstallSupported')" == 'true' ]]; then
        marketplace_install_supported=true
    fi

    local install_scope_supported=false
    if [[ "$(printf '%s' "$support_json" | jq -r '.installScopeSupported')" == 'true' ]]; then
        install_scope_supported=true
    fi

    local install_help_summary
    install_help_summary="$(printf '%s' "$support_json" | jq -r '.installHelpSummary // empty')"

    local lifecycle_mode='artifact-only'
    local provider_state_status='linked-projection'
    local provider_read_surface
    provider_read_surface="$(resolve_provider_managed_read_surface "$provider_state_status")"

    local strict_passed=false
    if ! $cli_available; then
        echo '  [WARN] Claude CLI not found on PATH; artifact is built but lifecycle validation is unavailable.'
        if $DRY_RUN; then
            echo "  [DRY RUN] Would project Claude skills root into: $CLAUDE_PLUGIN_INSTALL_TARGET"
        else
            mkdir -p "$CLAUDE_SKILLS_ROOT"
            safe_unlink "$CLAUDE_LEGACY_PLUGIN_INSTALL_TARGET"
            ensure_symlink "$CLAUDE_PLUGIN_INSTALL_TARGET" "$canonical_root" 'directory'
            echo "  [OK] Projected Claude skills root: $CLAUDE_PLUGIN_INSTALL_TARGET"
        fi
    elif ! $validate_supported; then
        echo '  [WARN] Claude CLI is present but `claude plugin validate` is unavailable; recording artifact-only lifecycle status.'
        if $DRY_RUN; then
            echo "  [DRY RUN] Would project Claude skills root into: $CLAUDE_PLUGIN_INSTALL_TARGET"
        else
            mkdir -p "$CLAUDE_SKILLS_ROOT"
            safe_unlink "$CLAUDE_LEGACY_PLUGIN_INSTALL_TARGET"
            ensure_symlink "$CLAUDE_PLUGIN_INSTALL_TARGET" "$canonical_root" 'directory'
            echo "  [OK] Projected Claude skills root: $CLAUDE_PLUGIN_INSTALL_TARGET"
        fi
    else
        if $DRY_RUN; then
            echo "  [DRY RUN] Would run Claude plugin validation: claude plugin validate --strict \"$canonical_root\""
        else
            claude plugin validate --strict "$canonical_root"
            strict_passed=true
            lifecycle_mode='session-load-only'
            echo '  [OK] Claude plugin validation passed.'
        fi

        if $DRY_RUN; then
            echo "  [DRY RUN] Would project Claude skills root into: $CLAUDE_PLUGIN_INSTALL_TARGET"
        else
            mkdir -p "$CLAUDE_SKILLS_ROOT"
            safe_unlink "$CLAUDE_LEGACY_PLUGIN_INSTALL_TARGET"
            ensure_symlink "$CLAUDE_PLUGIN_INSTALL_TARGET" "$canonical_root" 'directory'
            echo "  [OK] Projected Claude skills root: $CLAUDE_PLUGIN_INSTALL_TARGET"
        fi
    fi

    local state_json
    state_json="$(jq -n \
        --arg canonicalRoot "$canonical_root" \
        --arg packageOutputRoot "$package_output_root" \
        --arg projectionRoot "$CLAUDE_PLUGIN_INSTALL_TARGET" \
        --arg installTarget "$CLAUDE_PLUGIN_INSTALL_TARGET" \
        --arg manifestPath "$manifest_path" \
        --arg generatedAt "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
        --arg status "$provider_state_status" \
        --arg readSurface "$provider_read_surface" \
        --arg installHelpSummary "$install_help_summary" \
        --arg sessionLoadCommand "$session_load_command" \
        --arg installTemplate 'claude plugin install <plugin> --scope <scope>' \
        --arg updateTemplate 'claude plugin update <plugin> --scope <scope>' \
        --arg uninstallTemplate 'claude plugin uninstall <plugin> --scope <scope>' \
        --arg mode "$lifecycle_mode" \
        --arg marketplaceRoot '' \
        --argjson cliAvailable "$cli_available" \
        --argjson validateSupported "$validate_supported" \
        --argjson localArtifactInstallSupported "$local_artifact_install_supported" \
        --argjson marketplaceInstallSupported "$marketplace_install_supported" \
        --argjson installScopeSupported "$install_scope_supported" \
        --argjson strictPassed "$strict_passed" \
        '{
            schemaVersion: 1,
            provider: "claude",
            canonicalRoot: $canonicalRoot,
            packageOutputRoot: $packageOutputRoot,
            projectionRoot: $projectionRoot,
            installTarget: $installTarget,
            manifestPath: $manifestPath,
            generatedAt: $generatedAt,
            status: $status,
            readSurface: (if $readSurface == "" then null else $readSurface end),
            cli: {
                available: $cliAvailable,
                validateSupported: $validateSupported,
                localArtifactInstallSupported: $localArtifactInstallSupported,
                marketplaceInstallSupported: $marketplaceInstallSupported,
                installScopeSupported: $installScopeSupported,
                installHelpSummary: $installHelpSummary
            },
            validation: {
                command: "claude plugin validate <artifact> --strict",
                strictPassed: $strictPassed
            },
            lifecycle: {
                mode: $mode,
                marketplaceRoot: (if $marketplaceRoot == "" then null else $marketplaceRoot end),
                marketplaceName: "gal",
                sessionLoadCommand: $sessionLoadCommand,
                installCommandTemplate: $installTemplate,
                updateCommandTemplate: $updateTemplate,
                uninstallCommandTemplate: $uninstallTemplate
            },
            notes: [
                "Artifact validation is supported when the local Claude CLI exposes claude plugin validate <path>.",
                "Persistent install projects ~/.claude/skills/gal to the canonical GAL plugin root.",
                "~/.claude/plugins/gal is treated as a legacy location and is removed when GAL refreshes the Claude projection."
            ]
        }')"

    write_claude_lifecycle_state "$state_json"
}

invoke_codex_plugin_lifecycle() {
    if ! $INSTALL_CODEX; then
        return 0
    fi

    echo '  [OK] Evaluating Codex plugin lifecycle.'
    local canonical_root package_output_root manifest_path support_json marketplace_root marketplace_name plugin_selector
    canonical_root="$(get_gal_plugin_root gal)"
    package_output_root="$(get_codex_plugin_package_output_root "$REPO_ROOT")"
    manifest_path="$(get_codex_plugin_manifest_path "$canonical_root")"
    support_json="$(get_codex_cli_lifecycle_support_json)"
    marketplace_root="$GAL_PLUGINS_ROOT"
    marketplace_name='gal-marketplace'
    plugin_selector='gal@gal-marketplace'

    if [[ ! -d "$canonical_root" || ! -f "$manifest_path" ]]; then
        echo '  [WARN] Codex canonical artifact was missing; rerendering shared plugin root before marketplace registration.'
        "$SCRIPT_DIR/build-core-plugin.sh" >/dev/null
    fi

    if [[ ! -d "$canonical_root" ]]; then
        echo "Codex canonical root not found: $canonical_root" >&2
        exit 1
    fi

    if [[ ! -f "$manifest_path" ]]; then
        echo "Codex manifest not found: $manifest_path" >&2
        exit 1
    fi

    local marketplace_manifest_path
    marketplace_manifest_path="$(ensure_codex_marketplace_manifest)"

    local cli_available=false
    [[ "$(printf '%s' "$support_json" | jq -r '.cliAvailable')" == 'true' ]] && cli_available=true
    local marketplace_install_supported=false
    [[ "$(printf '%s' "$support_json" | jq -r '.marketplaceInstallSupported')" == 'true' ]] && marketplace_install_supported=true
    local install_help_summary
    install_help_summary="$(printf '%s' "$support_json" | jq -r '.installHelpSummary // empty')"
    local lifecycle_mode
    lifecycle_mode="$(printf '%s' "$support_json" | jq -r '.installMode')"

    if ! $cli_available; then
        echo '  [WARN] Codex CLI not found on PATH; refreshed marketplace descriptor only and recorded artifact-only lifecycle state.'
        lifecycle_mode='artifact-only'
    elif ! $marketplace_install_supported; then
        echo '  [WARN] Codex CLI is present but marketplace install support was not detected; recording artifact-only lifecycle state.'
        lifecycle_mode='artifact-only'
    elif $DRY_RUN; then
        echo "  [DRY RUN] Would register Codex marketplace: codex plugin marketplace add \"$marketplace_root\""
        echo "  [DRY RUN] Would refresh Codex plugin install: codex plugin remove $plugin_selector ; codex plugin add $plugin_selector"
    else
        local marketplace_add_output
        if ! marketplace_add_output="$(codex plugin marketplace add "$marketplace_root" 2>&1)"; then
            echo '  [WARN] Codex marketplace add returned non-zero; attempting marketplace re-registration.'
            codex plugin marketplace remove "$marketplace_name" >/dev/null 2>&1 || true
            marketplace_add_output="$(codex plugin marketplace add "$marketplace_root" 2>&1)" || {
                echo "Codex marketplace registration failed for $marketplace_root" >&2
                echo "$marketplace_add_output" >&2
                exit 1
            }
        fi
        echo "  [OK] Registered Codex marketplace '$marketplace_name'."

        if codex plugin remove "$plugin_selector" >/dev/null 2>&1; then
            echo "  [OK] Removed existing Codex plugin '$plugin_selector' before refresh."
        fi

        local plugin_add_output
        plugin_add_output="$(codex plugin add "$plugin_selector" 2>&1)" || {
            echo "Codex plugin install failed for $plugin_selector" >&2
            echo "$plugin_add_output" >&2
            exit 1
        }
        echo "  [OK] Installed Codex plugin '$plugin_selector'."
    fi

    local state_json
    state_json="$(jq -n \
        --arg canonicalRoot "$canonical_root" \
        --arg packageOutputRoot "$package_output_root" \
        --arg installTarget "$plugin_selector" \
        --arg manifestPath "$manifest_path" \
        --arg generatedAt "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
        --arg status 'unprojected-artifact' \
        --arg readSurface 'unprojected-artifact' \
        --arg installHelpSummary "$install_help_summary" \
        --arg mode "$lifecycle_mode" \
        --arg marketplaceRoot "$marketplace_root" \
        --arg marketplaceManifestPath "$marketplace_manifest_path" \
        --arg marketplaceName "$marketplace_name" \
        --arg sessionLoadCommand 'codex plugin marketplace add <plugins-root> ; codex plugin add gal@gal-marketplace' \
        --arg installTemplate 'codex plugin marketplace add <plugins-root>' \
        --arg updateTemplate 'codex plugin remove gal@gal-marketplace ; codex plugin add gal@gal-marketplace' \
        --arg uninstallTemplate 'codex plugin remove gal@gal-marketplace ; codex plugin marketplace remove gal-marketplace' \
        --argjson cliAvailable "$cli_available" \
        --argjson marketplaceInstallSupported "$marketplace_install_supported" \
        '{
            schemaVersion: 1,
            provider: "codex",
            canonicalRoot: $canonicalRoot,
            packageOutputRoot: $packageOutputRoot,
            projectionRoot: null,
            installTarget: $installTarget,
            manifestPath: $manifestPath,
            generatedAt: $generatedAt,
            status: $status,
            readSurface: $readSurface,
            cli: {
                available: $cliAvailable,
                validateSupported: false,
                localArtifactInstallSupported: false,
                marketplaceInstallSupported: $marketplaceInstallSupported,
                installScopeSupported: false,
                installHelpSummary: $installHelpSummary
            },
            validation: {
                command: null,
                strictPassed: false
            },
            lifecycle: {
                mode: $mode,
                stagedPluginRoot: $canonicalRoot,
                marketplaceRoot: $marketplaceRoot,
                marketplaceManifestPath: $marketplaceManifestPath,
                marketplaceName: $marketplaceName,
                installedSelector: $installTarget,
                sessionLoadCommand: $sessionLoadCommand,
                installCommandTemplate: $installTemplate,
                updateCommandTemplate: $updateTemplate,
                uninstallCommandTemplate: $uninstallTemplate,
                pluginRemovedBeforeAdd: false
            },
            notes: [
                "Codex lifecycle is independent from AGY and is routed through install-gal-plugins.sh.",
                "GAL refreshes the local marketplace descriptor before each Codex registration so codex plugin marketplace add reads the canonical plugin root.",
                "Codex updates are delivered by marketplace-copy semantics; the canonical .codex-plugin/plugin.json version must change on each render to satisfy version-gated refreshes."
            ]
        }')"

    write_codex_lifecycle_state "$state_json"
}

if $UNINSTALL; then
    install_mode='source'
    if ! install_mode="$(get_configured_install_mode 2>/dev/null)"; then
        echo '  [WARN] Could not read machine config during uninstall; falling back to managed-state detection.'
        install_mode='source'
    fi

    owns_managed_uninstall=false
    for managed_target in \
        "$INSTALL_STATE_FILE" \
        "$AGY_PLUGIN_INSTALL_TARGET" \
        "$COPILOT_PLUGIN_INSTALL_TARGET" \
        "$GAL_ROOT_COPILOT" \
        "$GAL_GENERATED_PROVIDERS_ROOT/codex/managed.json" \
        "$GAL_PLUGINS_ROOT/.agents/plugins/marketplace.json" \
        "$GAL_PLUGINS_ROOT" \
        "$GAL_GENERATED_MCP_ROOT" \
        "$GAL_GENERATED_XMACHINE_ROOT" \
        "$GAL_GENERATED_PROVIDERS_ROOT" \
        "$(get_gal_active_provider_target agy)"; do
        if [ -L "$managed_target" ] || [ -e "$managed_target" ]; then
            owns_managed_uninstall=true
            break
        fi
    done

    if [ "$install_mode" != 'install' ] && ! $owns_managed_uninstall && ! $PURGE; then
        echo '  [SKIP] Source-mode uninstall remains owned by the legacy concern scripts.'
        exit 0
    fi

    if [ "$install_mode" != 'install' ] && $owns_managed_uninstall; then
        echo '  [OK] Falling back to install-mode uninstall because GAL-managed runtime artifacts are present.'
    fi

    echo '  [OK] Install-mode uninstall owns AGY, Copilot, Codex, and Claude provider-lifecycle metadata cleanup.'
    if $DRY_RUN; then
        echo '  [DRY RUN] Would remove Codex plugin: codex plugin remove gal@gal-marketplace'
        echo '  [DRY RUN] Would unregister Codex marketplace: codex plugin marketplace remove gal-marketplace'
    elif command_exists codex; then
        codex plugin remove 'gal@gal-marketplace' >/dev/null 2>&1 || true
        codex plugin marketplace remove 'gal-marketplace' >/dev/null 2>&1 || true
    else
        echo '  [WARN] Codex CLI not found on PATH; skipping Codex plugin unregister/remove during uninstall.'
    fi

    agy_shortcut_target="$(get_gal_active_provider_target agy)"
    if [ -L "$agy_shortcut_target" ] || [ -e "$agy_shortcut_target" ]; then
        safe_unlink "$agy_shortcut_target"
    else
        echo '  [SKIP] No GAL-managed agy shortcut to remove'
    fi

    if [ -e "$AGY_PLUGIN_INSTALL_TARGET" ]; then
        if $DRY_RUN; then
            echo "  [DRY RUN] Would remove AGY plugin install target: $AGY_PLUGIN_INSTALL_TARGET"
        else
            rm -rf "$AGY_PLUGIN_INSTALL_TARGET"
            echo "  [REMOVED] AGY plugin install target: $AGY_PLUGIN_INSTALL_TARGET"
        fi
    else
        echo '  [SKIP] No AGY plugin install target to remove'
    fi

    if [ -L "$COPILOT_PLUGIN_INSTALL_TARGET" ]; then
        safe_unlink "$COPILOT_PLUGIN_INSTALL_TARGET"
    elif [ -e "$COPILOT_PLUGIN_INSTALL_TARGET" ]; then
        if $DRY_RUN; then
            echo "  [DRY RUN] Would remove Copilot plugin install target: $COPILOT_PLUGIN_INSTALL_TARGET"
        else
            rm -rf "$COPILOT_PLUGIN_INSTALL_TARGET"
            echo "  [REMOVED] Copilot plugin install target: $COPILOT_PLUGIN_INSTALL_TARGET"
        fi
    else
        echo '  [SKIP] No Copilot plugin install target to remove'
    fi

    safe_unlink "$GAL_ROOT_COPILOT"
    safe_unlink "$CLAUDE_PLUGIN_INSTALL_TARGET"
    safe_unlink "$CLAUDE_LEGACY_PLUGIN_INSTALL_TARGET"

    remove_managed_dir() {
        local path="$1"
        local label="$2"
        if [ ! -e "$path" ]; then
            echo "  [SKIP] No $label to remove"
            return
        fi

        if $DRY_RUN; then
            echo "  [DRY RUN] Would remove $label: $path"
            return
        fi

        rm -rf "$path"
        echo "  [REMOVED] $label: $path"
    }

    remove_managed_dir "$GAL_PLUGINS_ROOT" 'GAL canonical plugin root'
    remove_managed_dir "$GAL_GENERATED_MCP_ROOT" 'GAL-managed MCP projections'
    remove_managed_dir "$GAL_GENERATED_XMACHINE_ROOT" 'GAL-managed xmachine projections'
    remove_managed_dir "$GAL_GENERATED_PROVIDERS_ROOT" 'GAL-managed provider projections'

    if $PURGE; then
        if ! $DRY_RUN && ! $CONFIRM_PURGE; then
            echo 'Explicit purge requires --confirm-purge unless you are running with --dry-run.' >&2
            exit 1
        fi

        echo '  [OK] Explicit purge requested; removing preserved machine-local state.'
        remove_managed_dir "$GAL_CONFIG_ROOT" 'GAL config root'
        remove_managed_dir "$GAL_STATE_DIRECTORY" 'GAL state directory'
        remove_managed_dir "$INSTALL_STATE_FILE" 'GAL install-state file'
    fi
    exit 0
fi

mapfile -t selection_parts < <(resolve_selection_csv)
resolved_selected_csv="${selection_parts[0]}"
resolved_primary_runtime="${selection_parts[1]}"

mkdir -p "$GAL_CONFIG_ROOT" "$GAL_STATE_DIRECTORY" "$GAL_PLUGINS_ROOT" "$GAL_DATA_ROOT" "$GAL_CACHE_ROOT" "$GAL_GENERATED_MCP_ROOT" "$GAL_GENERATED_XMACHINE_ROOT" "$GAL_GENERATED_PROVIDERS_ROOT"

config_exists=false
config_for_resolver="$CONFIG_PATH"
cleanup_paths=()
if [ -f "$CONFIG_PATH" ]; then
    config_exists=true
    echo "  [OK] Using machine config: $CONFIG_PATH"
else
    config_preview_path="${TMPDIR:-/tmp}/gal-config-preview-$$.json"
    build_default_config_json "$resolved_selected_csv" "$resolved_primary_runtime" "$BOOTSTRAP_INSTALL" > "$config_preview_path"
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
bridge_providers_csv="$(printf '%s' "$summary_json" | jq -r '.bridgeProviders | join(",")')"
migration_providers_csv="$(printf '%s' "$summary_json" | jq -r '.migrationProviders | join(",")')"

if [[ -z "$primary_providers_csv" && -n "$resolved_selected_csv" ]]; then
    IFS=',' read -r -a fallback_runtimes <<< "$resolved_selected_csv"
    fallback_primary=()
    fallback_bridge=()
    fallback_migration=()
    for runtime in "${fallback_runtimes[@]}"; do
        case "$runtime" in
            opencode) fallback_bridge+=(opencode) ;;
            gemini) fallback_migration+=(gemini) ;;
            antigravity) fallback_primary+=(agy) ;;
            copilot|codex|claude) fallback_primary+=("$runtime") ;;
        esac
    done
    primary_providers_csv="$(join_by ',' "${fallback_primary[@]}")"
    bridge_providers_csv="$(join_by ',' "${fallback_bridge[@]}")"
    migration_providers_csv="$(join_by ',' "${fallback_migration[@]}")"
fi

echo "  [OK] Mode: $install_mode"
echo "  [OK] GAL runtime home: $GAL_STATE_ROOT"
echo "  [OK] Lockfile target: $LOCKFILE_PATH"
echo "  [OK] Active profile: $default_profile"
echo "  [OK] Explicit plugins: $(printf '%s' "$summary_json" | jq -r 'if (.enabledPlugins | length) == 0 then "none" else (.enabledPlugins | join(", ")) end')"
echo "  [OK] Resolved plugins: ${resolved_plugins:-none}"
if [[ -n "$primary_providers_csv" ]]; then
    echo "  [OK] Primary provider lanes: ${primary_providers_csv//,/ , }"
else
    echo '  [OK] Primary provider lanes: none'
fi
if [[ -n "$bridge_providers_csv" ]]; then
    echo "  [OK] Bridge lanes: ${bridge_providers_csv//,/ , }"
else
    echo '  [OK] Bridge lanes: none'
fi
if [[ -n "$migration_providers_csv" ]]; then
    echo "  [OK] Migration lanes: ${migration_providers_csv//,/ , }"
else
    echo '  [OK] Migration lanes: none'
fi

if [ "$install_mode" = 'source' ]; then
    echo "  [OK] Source mode galRoot: $(printf '%s' "$summary_json" | jq -r '.galRoot')"
    echo "  [OK] Source mode devMode: $(printf '%s' "$summary_json" | jq -r '.devMode')"
    echo "  [OK] Local override boundary: explicit machine-local bindings via $GAL_XMACHINE_CONFIG_FILE"
    echo '  [OK] Repo-root links and local overrides stay source-mode-only contributor paths.'
else
    echo "  [OK] Install mode projections root: $GAL_GENERATED_ROOT"
    echo '  [OK] Install mode disables repo-root links and source-only local overrides.'
    if $BOOTSTRAP_INSTALL && ! $config_exists; then
        echo '  [OK] First launch bootstrap path seeded install mode because no machine config existed yet.'
    fi
    if [ -n "$primary_providers_csv" ]; then
        build_provider_args=(--config-path "$config_for_resolver" --lockfile-path "$resolver_lockfile_path" --providers "$primary_providers_csv")
        $DRY_RUN && build_provider_args+=(--dry-run)
        $FORCE && build_provider_args+=(--force)
        "$SCRIPT_DIR/build-provider-plugins.sh" "${build_provider_args[@]}"
        if printf '%s' ",$primary_providers_csv," | grep -q ',copilot,'; then
            invoke_copilot_plugin_lifecycle
        fi
        if printf '%s' ",$primary_providers_csv," | grep -q ',codex,'; then
            invoke_codex_plugin_lifecycle
        fi
        if printf '%s' ",$primary_providers_csv," | grep -q ',claude,'; then
            invoke_claude_plugin_lifecycle
        fi
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