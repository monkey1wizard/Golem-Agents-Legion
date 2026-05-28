#!/usr/bin/env bash

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

artifact_root="$(get_claude_plugin_artifact_root "$REPO_ROOT")"
if [[ -d "$artifact_root" ]]; then
    if [[ "$FORCE" != 'true' ]]; then
        echo "Artifact root already exists: $artifact_root. Use --force to overwrite." >&2
        exit 1
    fi
    rm -rf "$artifact_root"
fi

mkdir -p "$artifact_root/.claude-plugin" "$artifact_root/skills" "$artifact_root/commands" "$artifact_root/agents"

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

echo 'Rendering skills...'
while IFS= read -r source_path; do
    [[ -n "$source_path" ]] || continue
    skill_name="$(basename "$(dirname "$source_path")")"
    mkdir -p "$artifact_root/skills/$skill_name"
    cp "$source_path" "$artifact_root/skills/$skill_name/SKILL.md"
    echo "  -> skills/$skill_name/SKILL.md"
done < <(printf '%s' "$package_json" | jq -r '.skills[].sourcePath // empty')

echo 'Rendering commands...'
while IFS= read -r command_name; do
    [[ -n "$command_name" ]] || continue
    source_path="$(printf '%s' "$package_json" | jq -r --arg n "$command_name" '.commandSkills[] | select(.name == $n) | .sourcePath')"
    cp "$source_path" "$artifact_root/commands/$command_name.md"
    echo "  -> commands/$command_name.md"
done < <(printf '%s' "$package_json" | jq -r '.commandSkills[].name // empty')

echo 'Rendering agents...'
while IFS= read -r source_path; do
    [[ -n "$source_path" ]] || continue
    agent_name="$(basename "$source_path")"
    agent_name="${agent_name%.agent.md}"
    filter_claude_agent_frontmatter "$source_path" > "$artifact_root/agents/$agent_name.md"
    echo "  -> agents/$agent_name.md"
done < <(printf '%s' "$package_json" | jq -r '.agents[].sourcePath // empty')

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

if [[ "$INSTALL" == 'true' ]]; then
    echo '  [INFO] Renderer completed; install orchestration performs Claude lifecycle validation and session-load capability checks.'
fi

echo ''
echo 'Claude plugin artifact rendered successfully.'
echo "Artifact root: $artifact_root"