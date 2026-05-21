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

# Build a provider-neutral plugin package from the GAL repo source contracts.
# Outputs a JSON object to stdout.
build_provider_plugin_package() {
    local repo_root="${1:-$REPO_ROOT}"
    local generated_at
    generated_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

    local skills_json='[]'
    local commands_json='[]'
    local agents_json='[]'
    local corpus_sources_json='[]'
    local mcp_spec_json='null'

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
            if [ -f "$skill_file" ]; then
                local name
                name="$(basename "$skill_dir")"
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
    local mcp_local_file="$repo_root/mcp.local.json"
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

    # --- Assemble package ---
    jq -n \
        --arg name 'gal' \
        --arg displayName 'Golem Agents Legion' \
        --arg generatedAt "$generated_at" \
        --argjson skills "$skills_json" \
        --argjson commandSkills "$commands_json" \
        --argjson mcpSpec "$mcp_spec_json" \
        --argjson corpusSources "$corpus_sources_json" \
        --argjson agents "$agents_json" \
        '{
            metadata: {
                name: $name,
                displayName: $displayName,
                generatedAt: $generatedAt
            },
            skills: $skills,
            commandSkills: $commandSkills,
            mcpSpec: $mcpSpec,
            instructionCorpus: {
                sources: $corpusSources
            },
            agents: $agents,
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
    printf '%s\n' "$repo_root/dist/provider-plugins/agy/gal"
}

# Returns the AGY plugin install target path.
get_agy_plugin_install_target() {
    printf '%s\n' "$HOME/.gemini/antigravity-cli/plugins/gal"
}
