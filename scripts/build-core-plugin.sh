#!/usr/bin/env bash
#
# Core provider-neutral renderer for GAL.
#
# Builds the provider-neutral common package, validates it, and renders the
# superset canonical plugin root at ~/.gal/plugins/gal/ that serves all
# supported providers from one location (link-first).
#
# Outputs (superset canonical root):
#   .claude-plugin/plugin.json   (Claude Code plugin manifest)
#   skills/                      (reusable skills)
#   commands/                    (flat command markdown files)
#   agents/<name>.md             (Claude-compatible filtered agent definitions)
#   agents/<name>.agent.md       (AGY-compatible unfiltered agent definitions)
#   .mcp.json                    (portable non-secret MCP server configuration)
#   plugin.json                  (AGY root manifest)
#   mcp_config.json              (AGY MCP configuration)
#   rules/gal.md                 (AGY instruction corpus)
#
# When --install is specified, projects to all AGY surfaces (link-first):
#   CLI junction:  ~/.gemini/antigravity-cli/plugins/gal  -> canonical root
#   IDE symlink:   ~/.gemini/antigravity-ide/plugins/gal  -> canonical root
#   GUI-config:    agy plugin install <canonical root>     (host-managed copy)
# Also removes GAL-owned vestigial artifacts (whitelist-guarded).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

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

source "$SCRIPT_DIR/common/provider-plugin.sh"

# ---------------------------------------------------------------------------
# Helper: filter Claude agent frontmatter
# ---------------------------------------------------------------------------
filter_claude_agent_frontmatter() {
    local source_path="$1"
    python_cmd=''
    if command -v python3 >/dev/null 2>&1; then
        python_cmd='python3'
    elif command -v python >/dev/null 2>&1; then
        python_cmd='python'
    else
        cat "$source_path"
        return 0
    fi

    "$python_cmd" - "$source_path" <<'PY'
from pathlib import Path
import sys

source = Path(sys.argv[1])
raw = source.read_text(encoding='utf-8')
normalized = raw.replace('\r\n', '\n')
lines = normalized.split('\n')
if len(lines) < 3 or lines[0] != '---':
    sys.stdout.write(raw)
    raise SystemExit(0)

closing = -1
for index in range(1, len(lines)):
    if lines[index] == '---':
        closing = index
        break

if closing < 1:
    sys.stdout.write(raw)
    raise SystemExit(0)

allowed = {
    'name', 'description', 'model', 'effort', 'maxTurns', 'tools',
    'disallowedTools', 'skills', 'memory', 'background', 'isolation'
}

frontmatter = []
for line in lines[1:closing]:
    if ':' not in line:
        continue
    key = line.split(':', 1)[0].strip()
    if key not in allowed:
        continue
    if key == 'isolation':
        value = line.split(':', 1)[1].strip().strip('"\'')
        if value != 'worktree':
            continue
    frontmatter.append(line)

body = lines[closing + 1:]
result = ['---', *frontmatter, '---', *body]
sys.stdout.write('\n'.join(result))
PY
}

# ---------------------------------------------------------------------------
# Build and validate common package
# ---------------------------------------------------------------------------
resolved_plugins_json=''
if [[ -n "$RESOLVED_PLUGINS_FILE" ]]; then
    if [[ ! -f "$RESOLVED_PLUGINS_FILE" ]]; then
        echo "Resolved plugins file not found: $RESOLVED_PLUGINS_FILE" >&2
        exit 1
    fi
    resolved_plugins_json="$(cat "$RESOLVED_PLUGINS_FILE")"
fi

echo 'Building provider-neutral package...'
package_json="$(build_provider_plugin_package "$REPO_ROOT" "$resolved_plugins_json")"

echo 'Validating common package...'
validation_json="$(printf '%s' "$package_json" | validate_provider_plugin_package)"
if [[ "$(printf '%s' "$validation_json" | jq -r '.valid')" != 'true' ]]; then
    echo 'Validation failed:' >&2
    printf '%s' "$validation_json" | jq -r '.errors[]' >&2
    exit 1
fi
echo 'Common package validated successfully.'

artifact_root="$(get_gal_plugin_root gal)"
if [[ -d "$artifact_root" ]]; then
    if [[ "$FORCE" != 'true' ]]; then
        echo "Artifact root already exists: $artifact_root. Use --force to overwrite." >&2
        exit 1
    fi
    rm -rf "$artifact_root"
fi

mkdir -p "$artifact_root/.claude-plugin" "$artifact_root/skills" "$artifact_root/commands" "$artifact_root/agents"

# ---------------------------------------------------------------------------
# CLAUDE: .claude-plugin/plugin.json
# ---------------------------------------------------------------------------
echo 'Rendering .claude-plugin/plugin.json...'
jq -n '{
    "$schema": "https://json.schemastore.org/claude-code-plugin-manifest.json",
    name: "gal",
    displayName: "Golem Agents Legion",
    version: "1.0.0",
    description: "Golem Agents Legion plugin for Claude Code",
    author: {name: "GAL"},
    homepage: "https://github.com/leetz/Golem-Agents-Legion",
    repository: "https://github.com/leetz/Golem-Agents-Legion",
    license: "MIT",
    keywords: ["gal", "golem-agents-legion", "claude-code", "plugin"]
}' > "$artifact_root/.claude-plugin/plugin.json"
echo '  -> .claude-plugin/plugin.json'

# ---------------------------------------------------------------------------
# SHARED: skills/
# ---------------------------------------------------------------------------
echo 'Rendering skills...'
while IFS= read -r source_path; do
    [[ -n "$source_path" ]] || continue
    skill_name="$(basename "$(dirname "$source_path")")"
    mkdir -p "$artifact_root/skills/$skill_name"
    cp "$source_path" "$artifact_root/skills/$skill_name/SKILL.md"
    echo "  -> skills/$skill_name/SKILL.md"
done < <(printf '%s' "$package_json" | jq -r '.skills[].sourcePath // empty')

# ---------------------------------------------------------------------------
# CLAUDE: commands/
# ---------------------------------------------------------------------------
echo 'Rendering commands...'
while IFS= read -r command_name; do
    [[ -n "$command_name" ]] || continue
    source_path="$(printf '%s' "$package_json" | jq -r --arg n "$command_name" '.commandSkills[] | select(.name == $n) | .sourcePath')"
    cp "$source_path" "$artifact_root/commands/$command_name.md"
    echo "  -> commands/$command_name.md"
done < <(printf '%s' "$package_json" | jq -r '.commandSkills[].name // empty')

# ---------------------------------------------------------------------------
# SHARED: agents/ — two formats for superset compatibility
#   .md        = Claude-filtered (Claude Code)
#   .agent.md  = unfiltered copy (AGY)
# ---------------------------------------------------------------------------
echo 'Rendering agents...'
while IFS= read -r source_path; do
    [[ -n "$source_path" ]] || continue
    agent_name="$(basename "$source_path")"
    agent_name="${agent_name%.agent.md}"

    # Claude format: filtered frontmatter, .md extension
    filter_claude_agent_frontmatter "$source_path" > "$artifact_root/agents/$agent_name.md"
    echo "  -> agents/$agent_name.md (Claude)"

    # AGY format: unfiltered copy, .agent.md extension
    cp "$source_path" "$artifact_root/agents/$agent_name.agent.md"
    echo "  -> agents/$agent_name.agent.md (AGY)"
done < <(printf '%s' "$package_json" | jq -r '.agents[].sourcePath // empty')

# ---------------------------------------------------------------------------
# CLAUDE: .mcp.json (portable MCP)
# ---------------------------------------------------------------------------
if [[ "$(printf '%s' "$package_json" | jq -r '.mcpSpec.canonicalSource != null')" == 'true' ]]; then
    mcp_source="$(printf '%s' "$package_json" | jq -r '.mcpSpec.canonicalSource')"
    if [[ -f "$mcp_source" ]]; then
        echo 'Rendering .mcp.json...'
        jq '
            {
                mcpServers: ((.servers // {}) | to_entries | map(
                    select(.value != null)
                    | select((.value | has("headers") | not) and (.value | has("env") | not))
                    | select((.value | tostring | test("\\$\\{[^}]+\\}") | not))
                    | {
                        key: .key,
                        value: ((.value | to_entries | map(select(.key != "type")) ) | from_entries)
                    }
                ) | from_entries)
            }
        ' "$mcp_source" > "$artifact_root/.mcp.json"
        echo '  -> .mcp.json'
    fi
fi

# ---------------------------------------------------------------------------
# CODEX: .codex-plugin/plugin.json  (Codex plugin manifest — skills only; no agents)
# ---------------------------------------------------------------------------
echo 'Rendering .codex-plugin/plugin.json (Codex manifest)...'
mkdir -p "$artifact_root/.codex-plugin"
display_name="$(printf '%s' "$package_json" | jq -r '.metadata.displayName')"
jq -n \
    --arg name 'gal' \
    --arg version '1.0.0' \
    --arg description 'Golem Agents Legion plugin for Codex CLI' \
    --arg displayName "$display_name" \
    '{
        name: $name,
        version: $version,
        description: $description,
        author: {name: "GAL"},
        homepage: "https://github.com/leetz/Golem-Agents-Legion",
        repository: "https://github.com/leetz/Golem-Agents-Legion",
        license: "MIT",
        keywords: ["gal", "golem-agents-legion", "codex", "plugin"],
        "skills": "./skills/",
        interface: {
            displayName: $displayName,
            shortDescription: "Document-driven AI working system",
            developerName: "GAL",
            category: "Engineering"
        }
    }' > "$artifact_root/.codex-plugin/plugin.json"
echo '  -> .codex-plugin/plugin.json'

# ---------------------------------------------------------------------------
# COPILOT: copilot-manifest.json  (root manifest — explicitly exposes component paths)
# Fixes BUG-02: Copilot will not load commands/ unless the path is explicitly defined.
# ---------------------------------------------------------------------------
echo 'Rendering copilot-manifest.json...'
jq -n \
    --arg name 'gal' \
    --arg displayName 'Golem Agents Legion' \
    --arg version '1.0.0' \
    --arg description 'Golem Agents Legion plugin for GitHub Copilot CLI' \
    '{
        name: $name,
        displayName: $displayName,
        version: $version,
        description: $description,
        components: {
            agents: "agents/",
            skills: "skills/",
            commands: "commands/",
            mcpConfig: ".mcp.json"
        }
    }' > "$artifact_root/copilot-manifest.json"
echo '  -> copilot-manifest.json'

# ---------------------------------------------------------------------------
# AGY: root plugin.json
# ---------------------------------------------------------------------------
echo 'Rendering plugin.json (AGY root manifest)...'

skills_json='[]'
if [[ "$(printf '%s' "$package_json" | jq '.skills | length')" -gt 0 ]]; then
    skills_json="$(printf '%s' "$package_json" | jq '[.skills[] | {name, type: "skill", source: ("skills/" + .name + "/SKILL.md")}]')"
fi
if [[ "$(printf '%s' "$package_json" | jq '.commandSkills | length')" -gt 0 ]]; then
    cmd_skills_json="$(printf '%s' "$package_json" | jq '[.commandSkills[] | {name, type: "command-skill", source: ("skills/" + .name + "/SKILL.md")}]')"
    skills_json="$(printf '%s\n%s' "$skills_json" "$cmd_skills_json" | jq -s 'add')"
fi

agents_json='[]'
if [[ "$(printf '%s' "$package_json" | jq '.agents | length')" -gt 0 ]]; then
    # AGY references .agent.md format
    agents_json="$(printf '%s' "$package_json" | jq '[.agents[] | {name: (.name | sub("\\.agent$"; "")), source: ("agents/" + (.name | sub("\\.agent$"; "")) + ".agent.md")}]')"
fi

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
echo '  -> plugin.json'

# ---------------------------------------------------------------------------
# AGY: mcp_config.json (AGY MCP format)
# ---------------------------------------------------------------------------
mcp_spec="$(printf '%s' "$package_json" | jq -r '.mcpSpec.canonicalSource // empty')"
has_local_overrides="$(printf '%s' "$package_json" | jq -r '.mcpSpec.hasLocalOverrides // false')"
if [[ -n "$mcp_spec" && -f "$mcp_spec" ]]; then
    echo 'Rendering mcp_config.json (AGY MCP)...'
    mcp_manifest="$(cat "$mcp_spec")"
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
    echo '  -> mcp_config.json'
fi

# ---------------------------------------------------------------------------
# AGY: rules/gal.md (instruction corpus)
# ---------------------------------------------------------------------------
corpus_count="$(printf '%s' "$package_json" | jq '.instructionCorpus.sources | length')"
if [[ "$corpus_count" -gt 0 ]]; then
    echo 'Rendering rules/gal.md...'
    mkdir -p "$artifact_root/rules"
    {
        echo "# GAL Instruction Corpus"
        echo ""
        echo "> Generated by build-core-plugin.sh"
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
    } > "$artifact_root/rules/gal.md"
    echo '  -> rules/gal.md'
fi

# ---------------------------------------------------------------------------
# --install: project to all AGY surfaces (link-first) + cleanup
# ---------------------------------------------------------------------------
if [[ "$INSTALL" == 'true' ]]; then
    echo ''
    echo '=== AGY Surface Projection (link-first) ==='

    # Validate superset root with agy if available
    if command -v agy >/dev/null 2>&1; then
        echo 'Validating superset canonical root with agy...'
        if agy plugin validate "$artifact_root" 2>&1; then
            echo '  [OK] agy plugin validate passed on canonical root.'
        else
            echo '  [WARN] agy plugin validate returned non-zero; continuing with install.'
        fi
    fi

    # Surface 1: CLI junction -> canonical root
    cli_target="$(get_agy_plugin_install_target)"
    echo "Projecting CLI surface: $cli_target -> $artifact_root"
    if [[ -L "$cli_target" || -e "$cli_target" ]]; then
        rm -rf "$cli_target"
    fi
    mkdir -p "$(dirname "$cli_target")"
    if ! safe_link "$cli_target" "$artifact_root"; then
        echo "Failed to project AGY CLI plugin into $cli_target" >&2
        exit 1
    fi
    echo "  [OK] CLI junction: $cli_target"

    # Surface 2: IDE symlink -> canonical root
    ide_plugins_dir="$HOME/.gemini/antigravity-ide/plugins"
    ide_target="$ide_plugins_dir/gal"
    echo "Projecting IDE surface: $ide_target -> $artifact_root"
    if [[ -L "$ide_target" || -e "$ide_target" ]]; then
        rm -rf "$ide_target"
    fi
    mkdir -p "$ide_plugins_dir"
    if safe_link "$ide_target" "$artifact_root"; then
        echo "  [OK] IDE symlink: $ide_target"
    else
        echo '  [FALLBACK] Symlink failed for IDE surface; copying...'
        cp -r "$artifact_root" "$ide_target"
        echo "  [OK] IDE copy: $ide_target"
    fi

    # Surface 3: GUI-config — agy plugin install (host-managed copy)
    if command -v agy >/dev/null 2>&1; then
        echo 'Installing into shared AGY config store via `agy plugin install`...'
        if agy plugin install "$artifact_root" 2>&1; then
            echo '  [OK] agy plugin install succeeded (GUI-config surface).'
        else
            echo '  [WARN] agy plugin install returned non-zero; check output.'
        fi
    else
        echo '  [SKIP] agy CLI not on PATH; skipping GUI-config store install.'
    fi

    # Codex marketplace descriptor + plugin install
    echo ''
    echo '=== Codex Marketplace Projection ==='
    plugins_root="$(dirname "$artifact_root")"  # ~/.gal/plugins
    codex_marketplace_dir="$plugins_root/.agents/plugins"
    codex_marketplace_file="$codex_marketplace_dir/marketplace.json"
    mkdir -p "$codex_marketplace_dir"
    jq -n '{
        name: "gal-marketplace",
        interface: {displayName: "GAL Plugin Marketplace"},
        plugins: [{
            name: "gal",
            source: {source: "local", path: "./gal"},
            policy: {installation: "AVAILABLE", authentication: "ON_INSTALL"},
            category: "Engineering"
        }]
    }' > "$codex_marketplace_file"
    echo "  [OK] Codex marketplace descriptor: $codex_marketplace_file"

    if command -v codex >/dev/null 2>&1; then
        echo 'Registering gal-marketplace with codex...'
        if codex plugin marketplace add "$plugins_root" 2>&1; then
            echo '  [OK] gal-marketplace registered.'
        else
            echo '  [WARN] codex marketplace add returned non-zero.'
        fi
        echo 'Installing gal plugin from gal-marketplace...'
        if codex plugin add "gal@gal-marketplace" 2>&1; then
            echo '  [OK] gal plugin installed from gal-marketplace.'
        else
            echo '  [WARN] codex plugin add returned non-zero.'
        fi
    else
        echo '  [SKIP] codex CLI not on PATH; skipping Codex marketplace install.'
    fi

    # R-CLEANUP: remove GAL-owned vestigial artifacts (whitelist only)
    echo ''
    echo '=== R-CLEANUP: GAL-owned vestigial artifacts ==='

    claude_dist="$HOME/.gal/dist/provider-plugins/claude"
    if [[ -d "$claude_dist" ]]; then
        rm -rf "$claude_dist"
        echo "  [REMOVED] $claude_dist (vestigial claude dist)"
    else
        echo "  [SKIP] $claude_dist not found (already clean)"
    fi

    antigravitycli="$HOME/.antigravitycli"
    if [[ -d "$antigravitycli" ]]; then
        if [[ -z "$(ls -A "$antigravitycli" 2>/dev/null)" ]]; then
            rmdir "$antigravitycli"
            echo "  [REMOVED] $antigravitycli (empty shell directory)"
        else
            echo "  [SKIP] $antigravitycli is not empty; leaving intact"
        fi
    else
        echo "  [SKIP] $antigravitycli not found (already clean)"
    fi

    # Conditional: AGY dist — only if CLI junction confirmed to point to canonical root
    agy_dist="$HOME/.gal/dist/provider-plugins/agy"
    if [[ -d "$agy_dist" ]]; then
        if [[ -L "$cli_target" ]]; then
            cli_link_target="$(readlink "$cli_target" 2>/dev/null || true)"
            canonical_norm="${artifact_root%/}"
            cli_norm="${cli_link_target%/}"
            if [[ "$cli_norm" == "$canonical_norm" ]]; then
                rm -rf "$agy_dist"
                echo "  [REMOVED] $agy_dist (link-first convergence confirmed; dist orphaned)"
            else
                echo "  [RETAIN] $agy_dist (CLI junction not confirmed to target canonical; retaining)"
            fi
        else
            echo "  [RETAIN] $agy_dist (CLI target is not a symlink; retaining)"
        fi
    fi
fi

echo ''
echo 'Core plugin artifact rendered successfully.'
echo "Artifact root: $artifact_root"
