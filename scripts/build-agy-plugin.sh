#!/usr/bin/env bash
#
# AGY plugin renderer for GAL.
#
# Builds the provider-neutral common package, validates it, and renders
# AGY-specific artifacts to dist/provider-plugins/agy/gal/.
#
# Outputs:
#   plugin.json          (manifest with stable name: gal)
#   skills/              (reusable skills + command skills)
#   agents/              (agent definitions)
#   rules/gal.md         (instruction corpus)
#   mcp_config.json      (MCP server configuration)
#
# Does NOT output:
#   hooks.json
#   scripts/
#   marketplace metadata
#   provider stubs

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# --- Parse args ---
INSTALL=false
FORCE=false
RESOLVED_PLUGINS_FILE=''
while [[ $# -gt 0 ]]; do
    case "$1" in
        --install) INSTALL=true ; shift ;;
        --force) FORCE=true ; shift ;;
        --resolved-plugins-file)
            shift
            if [[ $# -eq 0 ]]; then
                echo "Missing value for --resolved-plugins-file" >&2
                exit 1
            fi
            RESOLVED_PLUGINS_FILE="$1"
            shift
            ;;
        --resolved-plugins-file=*) RESOLVED_PLUGINS_FILE="${1#*=}" ; shift ;;
        *) echo "Unknown option: $1" >&2; exit 1 ;;
    esac
done

# --- Import common helpers ---
COMMON_SCRIPT="$SCRIPT_DIR/common/provider-plugin.sh"
if [[ ! -f "$COMMON_SCRIPT" ]]; then
    echo "Common helpers not found: $COMMON_SCRIPT" >&2
    exit 1
fi
source "$COMMON_SCRIPT"

# --- Build and validate common package ---
echo "Building provider-neutral package..."
resolved_plugins_json=''
if [[ -n "$RESOLVED_PLUGINS_FILE" ]]; then
    if [[ ! -f "$RESOLVED_PLUGINS_FILE" ]]; then
        echo "Resolved plugins file not found: $RESOLVED_PLUGINS_FILE" >&2
        exit 1
    fi
    resolved_plugins_json="$(cat "$RESOLVED_PLUGINS_FILE")"
fi
package_json="$(build_provider_plugin_package "$REPO_ROOT" "$resolved_plugins_json")"

echo "Validating common package..."
validation_json="$(printf '%s' "$package_json" | validate_provider_plugin_package)"
valid="$(printf '%s' "$validation_json" | jq -r '.valid')"
if [[ "$valid" != 'true' ]]; then
    echo "Validation failed:" >&2
    printf '%s' "$validation_json" | jq -r '.errors[]' >&2
    exit 1
fi
echo "Common package validated successfully."

# --- Prepare artifact root ---
artifact_root="$(get_agy_plugin_artifact_root "$REPO_ROOT")"
if [[ -d "$artifact_root" ]]; then
    if [[ "$FORCE" != 'true' ]]; then
        echo "Artifact root already exists: $artifact_root. Use --force to overwrite." >&2
        exit 1
    fi
    rm -rf "$artifact_root"
fi
mkdir -p "$artifact_root"

# --- Render plugin.json ---
echo "Rendering plugin.json..."

# Build skills array for plugin.json
skills_json='[]'
if [[ "$(printf '%s' "$package_json" | jq '.skills | length')" -gt 0 ]]; then
    skills_json="$(printf '%s' "$package_json" | jq '[.skills[] | {name, type: "skill", source: ("skills/" + .name + "/SKILL.md")}]')"
fi

# Build command skills array for plugin.json
if [[ "$(printf '%s' "$package_json" | jq '.commandSkills | length')" -gt 0 ]]; then
    cmd_skills_json="$(printf '%s' "$package_json" | jq '[.commandSkills[] | {name, type: "command-skill", source: ("skills/" + .name + "/SKILL.md")}]')"
    skills_json="$(printf '%s\n%s' "$skills_json" "$cmd_skills_json" | jq -s 'add')"
fi

# Build agents array for plugin.json
agents_json='[]'
if [[ "$(printf '%s' "$package_json" | jq '.agents | length')" -gt 0 ]]; then
    agents_json="$(printf '%s' "$package_json" | jq '[.agents[] | {name: (.name | sub("\\.agent$"; "")), source: ("agents/" + (.name | sub("\\.agent$"; "")) + ".agent.md")}]')"
fi

# Determine flags
has_mcp='false'
if [[ "$(printf '%s' "$package_json" | jq -r '.mcpSpec != null')" == 'true' ]]; then
    has_mcp='true'
fi

has_instructions='false'
if [[ "$(printf '%s' "$package_json" | jq '.instructionCorpus.sources | length')" -gt 0 ]]; then
    has_instructions='true'
fi

generated_at="$(printf '%s' "$package_json" | jq -r '.metadata.generatedAt')"
skipped_json="$(printf '%s' "$package_json" | jq '.skippedComponents')"
canonical_package_json="$(printf '%s' "$package_json" | jq '{packageId: .packageSchema.packageId, schemaId: .packageSchema.schemaId, canonicalProvider: .packageSchema.canonicalProvider, sourcePlugins: .sourcePlugins}')"
deferred_companions_json="$(printf '%s' "$package_json" | jq '.deferredCompanionPlugins')"

jq -n \
    --arg name 'gal' \
    --arg displayName 'Golem Agents Legion' \
    --arg version '1.0.0' \
    --arg generatedAt "$generated_at" \
    --arg description 'Golem Agents Legion plugin for AGY CLI' \
    --argjson canonicalPackage "$canonical_package_json" \
    --argjson deferredCompanionPlugins "$deferred_companions_json" \
    --argjson skills "$skills_json" \
    --argjson agents "$agents_json" \
    --argjson hasMcp "$has_mcp" \
    --argjson hasInstructions "$has_instructions" \
    --argjson skippedComponents "$skipped_json" \
    '{
        name: $name,
        displayName: $displayName,
        version: $version,
        generatedAt: $generatedAt,
        description: $description,
        canonicalPackage: $canonicalPackage,
        deferredCompanionPlugins: $deferredCompanionPlugins,
        skills: $skills,
        agents: $agents,
        hasMcp: $hasMcp,
        hasInstructions: $hasInstructions,
        skippedComponents: $skippedComponents
    }' > "$artifact_root/plugin.json"

echo "  -> plugin.json"

# --- Render skills ---
echo "Rendering skills..."
skills_dir="$artifact_root/skills"
mkdir -p "$skills_dir"

# Render reusable skills
while IFS= read -r skill_dir; do
    [[ -n "$skill_dir" ]] || continue
    name="$(basename "$skill_dir")"
    src_file="$skill_dir/SKILL.md"
    if [[ -f "$src_file" ]]; then
        dest_dir="$skills_dir/$name"
        mkdir -p "$dest_dir"
        cp "$src_file" "$dest_dir/SKILL.md"
        echo "  -> skills/$name/SKILL.md"
    fi
done < <(printf '%s' "$package_json" | jq -r '.skills[].sourcePath // empty')

# Render command skills
while IFS= read -r source_path; do
    [[ -n "$source_path" ]] || continue
    name="$(basename "$(dirname "$source_path")")"
    dest_dir="$skills_dir/$name"
    mkdir -p "$dest_dir"
    cp "$source_path" "$dest_dir/SKILL.md"
    echo "  -> skills/$name/SKILL.md (command-skill)"
done < <(printf '%s' "$package_json" | jq -r '.commandSkills[].sourcePath // empty')

# --- Render agents ---
echo "Rendering agents..."
agents_dir="$artifact_root/agents"
mkdir -p "$agents_dir"

while IFS= read -r source_path; do
    [[ -n "$source_path" ]] || continue
    name="$(basename "$source_path")"
    # Strip .agent.md suffix and re-add it cleanly to avoid double extension
    base_name="${name%.agent.md}"
    cp "$source_path" "$agents_dir/$base_name.agent.md"
    echo "  -> agents/$base_name.agent.md"
done < <(printf '%s' "$package_json" | jq -r '.agents[].sourcePath // empty')

# --- Render MCP config ---
mcp_spec="$(printf '%s' "$package_json" | jq -r '.mcpSpec.canonicalSource // empty')"
has_local_overrides="$(printf '%s' "$package_json" | jq -r '.mcpSpec.hasLocalOverrides // false')"
if [[ -n "$mcp_spec" && -f "$mcp_spec" ]]; then
    echo "Rendering mcp_config.json..."

    # Start with canonical source
    mcp_manifest="$(cat "$mcp_spec")"

    # Merge local overrides if present
    if [[ "$has_local_overrides" == 'true' ]]; then
        mcp_local_file="$(dirname "$mcp_spec")/mcp.local.json"
        if [[ -f "$mcp_local_file" ]]; then
            mcp_manifest="$(printf '%s\n%s' "$mcp_manifest" "$(cat "$mcp_local_file")" | jq -s 'def deep_merge(a;b):
              reduce (b | keys) as $k (.;
                if (.[$k] // null) == null then .[$k] = b[$k]
                elif (.[$k] | type) == "object" and (b[$k] | type) == "object" then .[$k] = deep_merge(.[$k]; b[$k])
                else .[$k] = b[$k]
                end
              );
              deep_merge(.[0]; .[1])')"
        fi
    fi

    # Convert to AGY format: servers -> mcpServers, url -> serverUrl, remove type field
    agy_mcp="$(printf '%s' "$mcp_manifest" | jq '
        {
            mcpServers: ((.servers // {}) | to_entries | map(
                select(.value != null) |
                {
                    key: .key,
                    value: ((.value | to_entries | map(
                        select(.key != "type") |
                        if .key == "url" then {key: "serverUrl", value: .value}
                        else {key: .key, value: .value}
                        end
                    )) | from_entries)
                }
            ) | from_entries),
            inputs: (.inputs // null)
        } | del(.. | nulls)
    ')"

    printf '%s\n' "$agy_mcp" > "$artifact_root/mcp_config.json"
    echo "  -> mcp_config.json"
fi

# --- Render rules/gal.md (instruction corpus) ---
corpus_count="$(printf '%s' "$package_json" | jq '.instructionCorpus.sources | length')"
if [[ "$corpus_count" -gt 0 ]]; then
    echo "Rendering rules/gal.md..."
    rules_dir="$artifact_root/rules"
    mkdir -p "$rules_dir"

    {
        echo "# GAL Instruction Corpus"
        echo ""
        echo "> Generated by build-agy-plugin.sh"
        echo "> DO NOT EDIT DIRECTLY — regenerate from source contracts"
        echo ""

        while IFS= read -r source; do
            [[ -n "$source" ]] || continue
            rel_path="${source#$REPO_ROOT/}"
            echo "## Source: $rel_path"
            echo ""
            cat "$source"
            echo ""
            echo "---"
            echo ""
        done < <(printf '%s' "$package_json" | jq -r '.instructionCorpus.sources[] // empty')
    } > "$rules_dir/gal.md"

    echo "  -> rules/gal.md"
fi

# --- Install if requested ---
if [[ "$INSTALL" == 'true' ]]; then
    install_target="$(get_agy_plugin_install_target)"
    echo "Installing to $install_target..."
    if [[ -d "$install_target" ]]; then
        rm -rf "$install_target"
    fi
    mkdir -p "$(dirname "$install_target")"
    cp -r "$artifact_root" "$install_target"
    echo "Installed to $install_target"
fi

# --- Summary ---
skill_count="$(printf '%s' "$package_json" | jq '.skills | length')"
cmd_skill_count="$(printf '%s' "$package_json" | jq '.commandSkills | length')"
agent_count="$(printf '%s' "$package_json" | jq '.agents | length')"
corpus_count="$(printf '%s' "$package_json" | jq '.instructionCorpus.sources | length')"
has_mcp_summary='no'
[[ -n "$mcp_spec" ]] && has_mcp_summary='yes'

echo ""
echo "AGY plugin rendered successfully to: $artifact_root"
echo "Skills: $((skill_count + cmd_skill_count))"
echo "Agents: $agent_count"
echo "MCP: $has_mcp_summary"
echo "Instructions: $corpus_count sources"
echo "Deferred companions: $(printf '%s' "$package_json" | jq '.deferredCompanionPlugins | length')"
