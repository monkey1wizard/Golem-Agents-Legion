#!/usr/bin/env bash

# Provider-neutral plugin package model helpers for GAL.
# Builds and validates the provider-neutral package model that represents
# the minimum shared substrate across AGY CLI, Copilot CLI, Codex, and Claude Code.
# The common model contains no provider-specific output paths, no resolved local
# secrets, and no runtimeScripts.

if [[ -n "${GAL_PROVIDER_PLUGIN_SH_LOADED:-}" ]]; then
    return 0 2>/dev/null || exit 0
fi
GAL_PROVIDER_PLUGIN_SH_LOADED=1

COMMON_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCRIPTS_DIR="$(cd "$COMMON_DIR/.." && pwd)"
REPO_ROOT="$(cd "$SCRIPTS_DIR/.." && pwd)"

get_gal_core_canonical_package_schema() {
    jq -n '{
        schemaId: "claude-compatible-gal-core-v1",
        schemaVersion: 1,
        packageId: "gal-core",
        packageKind: "canonical-plugin",
        canonicalProvider: "claude",
        compatibleProviders: ["claude", "copilot", "codex", "agy"],
        componentRoots: {
            skills: "skills",
            commands: "commands",
            agents: "agents",
            mcp: "provider-managed",
            lsp: "provider-managed"
        },
        nativeInstallProviders: ["claude", "copilot", "codex"],
        managedShortcutProviders: ["agy"]
    }'
}

get_gal_core_copied_companion_skill_patterns() {
    printf '%s\n' 'dart-*' 'flutter-*'
}

# Build a provider-neutral plugin package from the GAL repo source contracts.
# Outputs a JSON object to stdout.
build_provider_plugin_package() {
    local repo_root="${1:-$REPO_ROOT}"
    local resolved_plugins_json="${2:-}"
    local generated_at
    generated_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

    local skills_json='[]'
    local commands_json='[]'
    local agents_json='[]'
    local corpus_sources_json='[]'
    local mcp_spec_json='null'
    local package_schema_json
    package_schema_json="$(get_gal_core_canonical_package_schema)"
    local source_plugins_json='[{"pluginId":"gal-core","supportTier":"official-gal","sourceType":"official-gal"}]'
    local deferred_companions_json='[]'

    if [[ -n "$resolved_plugins_json" ]]; then
        source_plugins_json="$(printf '%s' "$resolved_plugins_json" | jq '[.[] | select(.pluginId == "gal-core" or .supportTier == "official-gal") | {pluginId, supportTier, sourceType}]')"
        deferred_companions_json="$(printf '%s' "$resolved_plugins_json" | jq '[.[] | select(.pluginId != "gal-core" and .supportTier != "official-gal") | {pluginId, supportTier, sourceType}]')"
        local has_gal_core
        has_gal_core="$(printf '%s' "$source_plugins_json" | jq 'map(select(.pluginId == "gal-core")) | length')"
        if [[ "$has_gal_core" -eq 0 ]]; then
            echo "Canonical package input must include gal-core when resolver output is provided." >&2
            return 1
        fi
    fi

    # --- Reusable skills ---
    local skills_dir="$repo_root/skills"
    if [ -d "$skills_dir" ]; then
        local skill_dirs=()
        while IFS= read -r -d '' dir; do
            skill_dirs+=("$dir")
        done < <(find "$skills_dir" -mindepth 1 -maxdepth 1 -type d -print0 | sort -z)

        local skill_entries=()
        local skill_dir
        for skill_dir in "${skill_dirs[@]}"; do
            local skill_file="$skill_dir/SKILL.md"
            local name
            name="$(basename "$skill_dir")"
            local skip_skill=false
            while IFS= read -r pattern; do
                [[ -n "$pattern" ]] || continue
                if [[ "$name" == $pattern ]]; then
                    skip_skill=true
                    break
                fi
            done < <(get_gal_core_copied_companion_skill_patterns)
            if [[ "$skip_skill" == 'true' ]]; then
                continue
            fi
            if [ -f "$skill_file" ]; then
                skill_entries+=("$(printf '{"name":%s,"sourcePath":%s}' "$(jq -R . <<< "$name")" "$(jq -R . <<< "$skill_file")")")
            fi
        done

        if [ "${#skill_entries[@]}" -gt 0 ]; then
            skills_json="$(printf '%s\n' "${skill_entries[@]}" | jq -s .)"
        fi
    fi

    # --- Command skills ---
    local commands_dir="$repo_root/commands"
    if [ -d "$commands_dir" ]; then
        local cmd_dirs=()
        while IFS= read -r -d '' dir; do
            cmd_dirs+=("$dir")
        done < <(find "$commands_dir" -mindepth 1 -maxdepth 1 -type d -print0 | sort -z)

        local cmd_entries=()
        local cmd_dir
        for cmd_dir in "${cmd_dirs[@]}"; do
            local name
            name="$(basename "$cmd_dir")"
            local skill_file="$cmd_dir/SKILL.md"
            local template_file="$cmd_dir/SKILL.template.md"
            local source_file=''
            if [ -f "$skill_file" ]; then
                source_file="$skill_file"
            elif [ -f "$template_file" ]; then
                source_file="$template_file"
            fi
            if [ -n "$source_file" ]; then
                cmd_entries+=("$(printf '{"name":%s,"sourcePath":%s}' "$(jq -R . <<< "$name")" "$(jq -R . <<< "$source_file")")")
            fi
        done

        if [ "${#cmd_entries[@]}" -gt 0 ]; then
            commands_json="$(printf '%s\n' "${cmd_entries[@]}" | jq -s .)"
        fi
    fi

    # --- MCP spec ---
    local mcp_file="$repo_root/mcp.json"
    local mcp_local_file="$GAL_CONFIG_ROOT/mcp.local.json"
    if [ -f "$mcp_file" ]; then
        local has_local=false
        [ -f "$mcp_local_file" ] && has_local=true
        mcp_spec_json="$(printf '{"canonicalSource":%s,"hasLocalOverrides":%s}' "$(jq -R . <<< "$mcp_file")" "$has_local")"
    fi

    # --- Instruction corpus sources ---
    local corpus_sources=()
    local corpus_file
    for corpus_file in "$repo_root/.dev/project.md" "$repo_root/conventions/conventions.md" "$repo_root/conventions/token-budget.md" "$repo_root/workflows/coding.md" "$repo_root/model-roles.md"; do
        if [ -f "$corpus_file" ]; then
            corpus_sources+=("$(jq -R . <<< "$corpus_file")")
        fi
    done
    if [ "${#corpus_sources[@]}" -gt 0 ]; then
        corpus_sources_json="$(printf '%s\n' "${corpus_sources[@]}" | jq -s .)"
    fi

    # --- Agents ---
    local agents_dir="$repo_root/agent"
    if [ -d "$agents_dir" ]; then
        local agent_files=()
        while IFS= read -r -d '' file; do
            agent_files+=("$file")
        done < <(find "$agents_dir" -maxdepth 1 -type f -name '*.agent.md' -print0 | sort -z)

        local agent_entries=()
        local agent_file
        for agent_file in "${agent_files[@]}"; do
            local name
            name="$(basename "$agent_file" .agent.md)"
            agent_entries+=("$(printf '{"name":%s,"sourcePath":%s}' "$(jq -R . <<< "$name")" "$(jq -R . <<< "$agent_file")")")
        done

        if [ "${#agent_entries[@]}" -gt 0 ]; then
            agents_json="$(printf '%s\n' "${agent_entries[@]}" | jq -s .)"
        fi
    fi

    # --- Provider capability flags ---
    local capabilities_json
    capabilities_json='{
        "agy": {"skills": true, "commandSkills": true, "agents": true, "instructions": true, "mcp": true, "hooks": false, "runtimeScripts": false},
        "copilot": {"skills": true, "commandSkills": true, "agents": true, "instructions": true, "mcp": true, "hooks": false, "runtimeScripts": false},
        "codex": {"skills": true, "commandSkills": true, "agents": false, "instructions": true, "mcp": true, "hooks": false, "runtimeScripts": false},
        "claude": {"skills": true, "commandSkills": true, "agents": true, "instructions": true, "mcp": true, "hooks": false, "runtimeScripts": false}
    }'

    # --- Assemble package ---
    jq -n \
        --arg name 'gal' \
        --arg displayName 'Golem Agents Legion' \
        --arg generatedAt "$generated_at" \
        --argjson packageSchema "$package_schema_json" \
        --argjson sourcePlugins "$source_plugins_json" \
        --argjson deferredCompanionPlugins "$deferred_companions_json" \
        --argjson skills "$skills_json" \
        --argjson commandSkills "$commands_json" \
        --argjson mcpSpec "$mcp_spec_json" \
        --argjson corpusSources "$corpus_sources_json" \
        --argjson agents "$agents_json" \
        --argjson capabilities "$capabilities_json" \
        '{
            packageSchema: $packageSchema,
            metadata: {
                name: $name,
                displayName: $displayName,
                generatedAt: $generatedAt
            },
            sourcePlugins: $sourcePlugins,
            deferredCompanionPlugins: $deferredCompanionPlugins,
            skills: $skills,
            commandSkills: $commandSkills,
            mcpSpec: $mcpSpec,
            instructionCorpus: {
                sources: $corpusSources
            },
            agents: $agents,
            providerCapabilities: $capabilities,
            skippedComponents: ["hooks", "runtimeScripts"]
        }'
}

# Validate a provider-neutral plugin package against the shared substrate contract.
# Reads package JSON from stdin; outputs validation result JSON to stdout.
validate_provider_plugin_package() {
    local package_json
    package_json="$(cat)"

    local errors_json='[]'

    # --- Reject provider-specific paths ---
    local provider_paths=('.codex-plugin' '.claude-plugin' 'rules/' 'mcp_config.json' 'hooks.json' 'gal-results/' 'runtimeScripts' 'scripts/')
    local path
    for path in "${provider_paths[@]}"; do
        if [ "$path" = 'runtimeScripts' ]; then
            # Allow runtimeScripts in skippedComponents since it explicitly records the v1 skip
            if printf '%s' "$package_json" | grep -q '"runtimeScripts"' && ! printf '%s' "$package_json" | grep -q '"skippedComponents".*"runtimeScripts"'; then
                local msg="Provider-specific path '$path' leaked into provider-neutral package model"
                errors_json="$(printf '%s' "$errors_json" | jq --arg msg "$msg" '. + [$msg]')"
            fi
            continue
        fi
        if printf '%s' "$package_json" | grep -qF "$path"; then
            local msg="Provider-specific path '$path' leaked into provider-neutral package model"
            errors_json="$(printf '%s' "$errors_json" | jq --arg msg "$msg" '. + [$msg]')"
        fi
    done

    # --- Name collision check ---
    local collision_errors
    collision_errors="$(printf '%s' "$package_json" | jq -r '
        [.skills[].name] + [.commandSkills[].name]
        | group_by(.)
        | map(select(length > 1))
        | map("Name collision detected: \"\(.[0])\" appears in both skills and commandSkills")
        | .[]
    ')"
    if [ -n "$collision_errors" ]; then
        while IFS= read -r msg; do
            [ -n "$msg" ] || continue
            errors_json="$(printf '%s' "$errors_json" | jq --arg msg "$msg" '. + [$msg]')"
        done <<< "$collision_errors"
    fi

    # --- Source path existence ---
    local missing_paths
    missing_paths="$(printf '%s' "$package_json" | jq -r '
        ([.skills[], .commandSkills[], .agents[]] | map(select(.sourcePath != null and (.sourcePath | test("^/")))))
        | .[]
        | select(.sourcePath | test("^/"))
        | if (test("file exists"; .sourcePath) | not) then "Source path does not exist: \(.sourcePath)" else empty end
    ')"
    # Note: jq cannot test file existence directly; we do it in shell below

    # Check file existence in shell
    local all_paths
    all_paths="$(printf '%s' "$package_json" | jq -r '[.skills[], .commandSkills[], .agents[] | .sourcePath] | .[]')"
    while IFS= read -r src_path; do
        [ -n "$src_path" ] || continue
        if [ ! -e "$src_path" ]; then
            local msg="Source path does not exist: $src_path"
            errors_json="$(printf '%s' "$errors_json" | jq --arg msg "$msg" '. + [$msg]')"
        fi
    done <<< "$all_paths"

    # --- Required fields ---
    local metadata_name
    metadata_name="$(printf '%s' "$package_json" | jq -r '.metadata.name // empty')"
    if [ -z "$metadata_name" ]; then
        errors_json="$(printf '%s' "$errors_json" | jq '. + ["Package metadata.name is required"]')"
    fi

    local package_id
    package_id="$(printf '%s' "$package_json" | jq -r '.packageSchema.packageId // empty')"
    if [ "$package_id" != 'gal-core' ]; then
        errors_json="$(printf '%s' "$errors_json" | jq '. + ["Package packageSchema.packageId must be set to gal-core"]')"
    fi

    local has_source_gal_core
    has_source_gal_core="$(printf '%s' "$package_json" | jq '[.sourcePlugins[]? | select(.pluginId == "gal-core")] | length')"
    if [ "$has_source_gal_core" -eq 0 ]; then
        errors_json="$(printf '%s' "$errors_json" | jq '. + ["Package sourcePlugins must include gal-core"]')"
    fi

    local leaked_companion_skills
    leaked_companion_skills="$(printf '%s' "$package_json" | jq -r '.skills[]?.name | select(startswith("dart-") or startswith("flutter-"))')"
    if [ -n "$leaked_companion_skills" ]; then
        while IFS= read -r skill_name; do
            [[ -n "$skill_name" ]] || continue
            local msg="Copied companion skill '$skill_name' leaked into gal-core canonical package"
            errors_json="$(printf '%s' "$errors_json" | jq --arg msg "$msg" '. + [$msg]')"
        done <<< "$leaked_companion_skills"
    fi

    # --- T-002: Unsupported component skip validation ---
    local required_skipped=('hooks' 'runtimeScripts')
    local component
    for component in "${required_skipped[@]}"; do
        local is_skipped
        is_skipped="$(printf '%s' "$package_json" | jq --arg c "$component" '.skippedComponents // [] | contains([$c])')"
        if [ "$is_skipped" != 'true' ]; then
            local msg="Required skipped component '$component' is not explicitly recorded in skippedComponents"
            errors_json="$(printf '%s' "$errors_json" | jq --arg msg "$msg" '. + [$msg]')"
        fi
    done

    # --- T-002: Verify no stubs for unsupported components ---
    local stub_indicators=('"hooks"' '"runtimeScripts"')
    for indicator in "${stub_indicators[@]}"; do
        # Count occurrences outside skippedComponents
        local outside_count
        outside_count="$(printf '%s' "$package_json" | jq -r --arg ind "$indicator" '
            [paths as $p | select(. == ($ind | fromjson)) | $p]
            | map(select($p | index("skippedComponents") | not))
            | length
        ')"
        if [ "$outside_count" -gt 0 ]; then
            local msg="Unsupported component stub detected: $indicator appears outside skippedComponents"
            errors_json="$(printf '%s' "$errors_json" | jq --arg msg "$msg" '. + [$msg]')"
        fi
    done

    # --- T-002: Explicit gal-results/ check ---
    if printf '%s' "$package_json" | grep -qF 'gal-results/'; then
        local msg="gal-results/ path leaked into provider-neutral package model"
        errors_json="$(printf '%s' "$errors_json" | jq --arg msg "$msg" '. + [$msg]')"
    fi

    # --- T-002: Local-only artifact boundary validation ---
    local mcp_disallowed_keys
    mcp_disallowed_keys="$(printf '%s' "$package_json" | jq -r '
        .mcpSpec // {}
        | keys[]
        | select(. != "canonicalSource" and . != "hasLocalOverrides")
    ')"
    if [ -n "$mcp_disallowed_keys" ]; then
        while IFS= read -r key; do
            [ -n "$key" ] || continue
            local msg="MCP spec contains disallowed key '$key'; only canonicalSource and hasLocalOverrides are permitted in the common model"
            errors_json="$(printf '%s' "$errors_json" | jq --arg msg "$msg" '. + [$msg]')"
        done <<< "$mcp_disallowed_keys"
    fi

    # Ensure hasLocalOverrides is strictly boolean
    local has_local_type
    has_local_type="$(printf '%s' "$package_json" | jq -r '.mcpSpec.hasLocalOverrides | type // "null"')"
    if [ "$has_local_type" != 'boolean' ] && [ "$has_local_type" != 'null' ]; then
        local msg="MCP spec hasLocalOverrides must be a boolean flag, not a resolved value"
        errors_json="$(printf '%s' "$errors_json" | jq --arg msg "$msg" '. + [$msg]')"
    fi

    # --- T-002: Provider capability flags validation ---
    local has_capabilities
    has_capabilities="$(printf '%s' "$package_json" | jq -r 'has("providerCapabilities")')"
    if [ "$has_capabilities" != 'true' ]; then
        errors_json="$(printf '%s' "$errors_json" | jq '. + ["Package providerCapabilities is required"]')"
    else
        local required_providers=('agy' 'copilot' 'codex' 'claude')
        for provider in "${required_providers[@]}"; do
            local has_provider
            has_provider="$(printf '%s' "$package_json" | jq --arg p "$provider" 'has("providerCapabilities") and (.providerCapabilities | has($p))')"
            if [ "$has_provider" != 'true' ]; then
                local msg="Provider capability flags missing for '$provider'"
                errors_json="$(printf '%s' "$errors_json" | jq --arg msg "$msg" '. + [$msg]')"
            fi
        done
    fi

    local valid=false
    local error_count
    error_count="$(printf '%s' "$errors_json" | jq 'length')"
    if [ "$error_count" -eq 0 ]; then
        valid=true
    fi

    jq -n \
        --argjson errors "$errors_json" \
        --argjson valid "$valid" \
        '{valid: $valid, errors: $errors}'
}

# Returns the generated artifact root for the AGY renderer.
get_agy_plugin_artifact_root() {
    local repo_root="${1:-$REPO_ROOT}"
    printf '%s\n' "$GAL_DIST_ROOT/provider-plugins/agy/gal"
}

# Returns the AGY plugin install target path.
get_agy_plugin_install_target() {
    printf '%s\n' "$HOME/.gemini/antigravity-cli/plugins/gal"
}

# Returns the generated artifact root for the Claude renderer.
get_claude_plugin_artifact_root() {
    local repo_root="${1:-$REPO_ROOT}"
    printf '%s\n' "$GAL_DIST_ROOT/provider-plugins/claude/gal"
}

# Returns the Claude plugin component layout relative to the plugin root.
get_claude_plugin_component_relative_paths() {
    jq -n '{
        manifest: ".claude-plugin/plugin.json",
        skills: "skills",
        commands: "commands",
        agents: "agents",
        mcp: ".mcp.json"
    }'
}

# Returns the manifest path for a rendered Claude plugin artifact.
get_claude_plugin_manifest_path() {
    local plugin_root="$1"
    printf '%s\n' "$plugin_root/.claude-plugin/plugin.json"
}

# Returns the documented Claude plugin install and validation contract.
get_claude_plugin_install_contract() {
    jq -n '{
        developmentLoadCommand: "claude --plugin-dir <plugin-root>",
        validationCommand: "claude plugin validate <plugin-root> --strict",
        lifecycleCommands: [
            "claude plugin install <plugin> --scope <scope>",
            "claude plugin update <plugin> --scope <scope>",
            "claude plugin uninstall <plugin> --scope <scope>"
        ],
        settingsScopes: {
            user: "~/.claude/settings.json",
            project: ".claude/settings.json",
            local: ".claude/settings.local.json",
            managed: "managed settings"
        },
        cacheRoot: "~/.claude/plugins/cache",
        dataRoot: "~/.claude/plugins/data",
        notes: [
            "Only .claude-plugin/plugin.json belongs inside .claude-plugin; all other plugin components stay at plugin root.",
            "Plugin artifact rendering and user-scope Claude CLI lifecycle operations are distinct concerns.",
            "Plugin data is persistent across updates and is deleted when the last install scope is removed unless --keep-data is used."
        ]
    }'
}
