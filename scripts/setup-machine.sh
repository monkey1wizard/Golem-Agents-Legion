#!/usr/bin/env bash
# setup-machine.sh — Create symlinks from golem-agents-legion to Copilot, Gemini, and Codex runtimes
#
# Links:
#   agent/*.agent.md  -> ~/.copilot/agents/*.agent.md
#   skills/*/         -> ~/.copilot/skills/*/
#   skills/*/         -> ~/.agents/skills/*/ (Gemini CLI + Codex CLI shared)
#   commands/gal/     -> ~/.copilot/skills/gal/ + ~/.agents/skills/gal/ (baked dispatcher skill)
#   <repo root>       -> ~/.copilot/gal/ + ~/.gemini/gal/ (GAL_ROOT dir symlinks)
#   Generates commands/gal/SKILL.md from SKILL.template.md (baked absolute paths)
#   Generates ~/.gemini/gal-context.md (@file skill imports, paths reference .agents/skills)
#   Merges VS Code user settings so Copilot Chat ignores ~/.agents/skills and does not double-list skills
#   Merges MCP server config from mcp-servers.example.json + mcp-servers.local.json into VS Code, Gemini, and Codex user config files
#
# Usage:
#   ./scripts/setup-machine.sh              # Install symlinks
#   ./scripts/setup-machine.sh --replace    # Replace existing real dirs with symlinks
#   ./scripts/setup-machine.sh --uninstall  # Remove symlinks
#   ./scripts/setup-machine.sh --dry-run    # Preview only

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

COPILOT_ROOT="$HOME/.copilot"
AGENTS_TARGET="$COPILOT_ROOT/agents"
SKILLS_TARGET="$COPILOT_ROOT/skills"

GEMINI_ROOT="$HOME/.gemini"
GEMINI_SKILLS_TARGET="$GEMINI_ROOT/skills"
GEMINI_CONTEXT_FILE="$GEMINI_ROOT/gal-context.md"

if [[ "${OSTYPE:-}" == darwin* ]]; then
    VSCODE_SETTINGS_FILE="$HOME/Library/Application Support/Code/User/settings.json"
    VSCODE_MCP_FILE="$HOME/Library/Application Support/Code/User/mcp.json"
else
    VSCODE_SETTINGS_FILE="${XDG_CONFIG_HOME:-$HOME/.config}/Code/User/settings.json"
    VSCODE_MCP_FILE="${XDG_CONFIG_HOME:-$HOME/.config}/Code/User/mcp.json"
fi

CODEX_SKILLS_ROOT="$HOME/.agents"
CODEX_SKILLS_TARGET="$CODEX_SKILLS_ROOT/skills"
CODEX_CONFIG_FILE="$HOME/.codex/config.toml"

MCP_MANIFEST_EXAMPLE="$REPO_ROOT/mcp-servers.example.json"
MCP_MANIFEST_LOCAL="$REPO_ROOT/mcp-servers.local.json"

GAL_SOURCE="$REPO_ROOT/commands/gal"
GAL_ROOT_COPILOT="$COPILOT_ROOT/gal"
GAL_ROOT_GEMINI="$GEMINI_ROOT/gal"
GAL_SKILL_COPILOT="$SKILLS_TARGET/gal"
GAL_SKILL_CODEX="$CODEX_SKILLS_TARGET/gal"
SKILL_TEMPLATE="$GAL_SOURCE/SKILL.template.md"
COMMAND_ALIAS_NAMES=()
while IFS= read -r -d '' _d; do
    _name="$(basename "$_d")"
    [ "$_name" = "gal" ] && continue
    COMMAND_ALIAS_NAMES+=("$_name")
done < <(find "$REPO_ROOT/commands" -mindepth 1 -maxdepth 1 -type d -print0 | sort -z)

COMMAND_SKILL_NAMES=(gal "${COMMAND_ALIAS_NAMES[@]}")

UNINSTALL=false
REPLACE=false
DRY_RUN=false

for arg in "$@"; do
    case "$arg" in
        --uninstall) UNINSTALL=true ;;
        --replace)   REPLACE=true ;;
        --dry-run)   DRY_RUN=true ;;
        *)           echo "Unknown argument: $arg"; exit 1 ;;
    esac
done

# --- Helpers ---

is_symlink() { [ -L "$1" ]; }

safe_link() {
    local link_path="$1"
    local target_path="$2"

    if $DRY_RUN; then
        echo "  [DRY RUN] link: $link_path -> $target_path"
        return 0
    fi

    if is_symlink "$link_path"; then
        local existing
        existing="$(readlink "$link_path")"
        if [ "$existing" = "$target_path" ]; then
            echo "  [SKIP] Already linked: $link_path"
            return 0
        fi
        echo "  [UPDATE] Replacing existing link: $link_path"
        rm "$link_path"
    elif [ -e "$link_path" ]; then
        if $REPLACE; then
            local bak_path="${link_path}.bak"
            if [ -e "$bak_path" ]; then
                echo "  [WARN] Backup already exists at $bak_path — skipping"
                return 1
            fi
            if $DRY_RUN; then
                echo "  [DRY RUN] Would rename $link_path -> $bak_path, then link"
                return 0
            fi
            mv "$link_path" "$bak_path"
            echo "  [BACKUP] $link_path -> $bak_path"
        else
            echo "  [WARN] Non-link item exists at $link_path — skipping (use --replace to back up and link)"
            return 1
        fi
    fi

    ln -s "$target_path" "$link_path"
    echo "  [OK] $link_path -> $target_path"
    return 0
}

safe_unlink() {
    local link_path="$1"
    if ! is_symlink "$link_path"; then return; fi

    if $DRY_RUN; then
        echo "  [DRY RUN] Would remove: $link_path"
        return
    fi

    rm "$link_path"
    echo "  [REMOVED] $link_path"
}

# --- Ensure target directories ---

if ! $UNINSTALL; then
    for dir in "$COPILOT_ROOT" "$AGENTS_TARGET" "$SKILLS_TARGET" "$GEMINI_ROOT" "$GEMINI_SKILLS_TARGET" "$CODEX_SKILLS_ROOT" "$CODEX_SKILLS_TARGET"; do
        if [ ! -d "$dir" ]; then
            if $DRY_RUN; then
                echo "[DRY RUN] Would create directory: $dir"
            else
                mkdir -p "$dir"
                echo "[OK] Created directory: $dir"
            fi
        fi
    done
fi

# --- Agent symlinks ---

agent_files=("$REPO_ROOT"/agent/*.agent.md)
agent_count=${#agent_files[@]}
agent_ok=0
agent_fail=0

echo ""
echo "=== Agents ($agent_count files) ==="

for f in "${agent_files[@]}"; do
    name="$(basename "$f")"
    link_path="$AGENTS_TARGET/$name"

    if $UNINSTALL; then
        safe_unlink "$link_path"
    else
        if safe_link "$link_path" "$f"; then
            ((agent_ok++)) || true
        else
            ((agent_fail++)) || true
        fi
    fi
done

# --- Skill symlinks ---

skill_dirs=()
while IFS= read -r -d '' d; do
    skill_dirs+=("$d")
done < <(find "$REPO_ROOT/skills" -mindepth 1 -maxdepth 1 -type d -print0)

skill_count=${#skill_dirs[@]}
skill_ok=0
skill_fail=0

echo ""
echo "=== Skills ($skill_count directories) ==="

for d in "${skill_dirs[@]}"; do
    name="$(basename "$d")"
    link_path="$SKILLS_TARGET/$name"

    if $UNINSTALL; then
        safe_unlink "$link_path"
    else
        if safe_link "$link_path" "$d"; then
            ((skill_ok++)) || true
        else
            ((skill_fail++)) || true
        fi
    fi
done

# --- Migration: remove .gemini/skills GAL symlinks (Gemini now discovers via .agents/skills) ---

echo ""
echo "=== Migration: .gemini/skills cleanup ==="

gemini_migrate_ok=0
all_gal_skill_names=("${COMMAND_SKILL_NAMES[@]}" "gal.bak")
for d in "${skill_dirs[@]}"; do
    all_gal_skill_names+=("$(basename "$d")")
done
for name in "${all_gal_skill_names[@]}"; do
    link_path="$GEMINI_SKILLS_TARGET/$name"
    [ -e "$link_path" ] || continue
    if is_symlink "$link_path"; then
        target="$(readlink "$link_path")"
        if [[ "$target" == *"$REPO_ROOT"* ]]; then
            if $DRY_RUN; then
                echo "  [DRY RUN] Would remove: $link_path"
            else
                rm -rf "$link_path"
                echo "  [REMOVED] $link_path"
                ((gemini_migrate_ok++)) || true
            fi
        fi
    elif [ "$name" = "gal.bak" ]; then
        if $DRY_RUN; then
            echo "  [DRY RUN] Would remove: $link_path"
        else
            rm -rf "$link_path"
            echo "  [REMOVED] $link_path (real dir)"
            ((gemini_migrate_ok++)) || true
        fi
    fi
done
if ! $DRY_RUN && [ "$gemini_migrate_ok" -eq 0 ]; then
    echo "  [OK] Nothing to migrate in .gemini/skills"
fi

# --- Shared Skill symlinks (Gemini + Codex both discover via .agents/skills) ---

codex_skill_ok=0
codex_skill_fail=0

echo ""
echo "=== Shared Skills - Gemini + Codex ($skill_count directories via .agents) ==="

for d in "${skill_dirs[@]}"; do
    name="$(basename "$d")"
    link_path="$CODEX_SKILLS_TARGET/$name"

    if $UNINSTALL; then
        safe_unlink "$link_path"
    else
        if safe_link "$link_path" "$d"; then
            ((codex_skill_ok++)) || true
        else
            ((codex_skill_fail++)) || true
        fi
    fi
done

# --- Gemini gal-context.md ---

echo ""
echo "=== Gemini gal-context.md ==="

if $UNINSTALL; then
    if [ -f "$GEMINI_CONTEXT_FILE" ]; then
        if $DRY_RUN; then
            echo "  [DRY RUN] Would remove: $GEMINI_CONTEXT_FILE"
        else
            rm "$GEMINI_CONTEXT_FILE"
            echo "  [REMOVED] $GEMINI_CONTEXT_FILE"
        fi
    fi
else
    context_lines=""
    for command_skill_name in "${COMMAND_SKILL_NAMES[@]}"; do
        context_lines+="@$CODEX_SKILLS_TARGET/$command_skill_name/SKILL.md"$'\n'
    done
    while IFS= read -r -d '' sd; do
        sname="$(basename "$sd")"
        context_lines+="@$CODEX_SKILLS_TARGET/$sname/SKILL.md"$'\n'
    done < <(find "$REPO_ROOT/skills" -mindepth 1 -maxdepth 1 -type d -print0 | sort -z)

    if $DRY_RUN; then
        echo "  [DRY RUN] Would write: $GEMINI_CONTEXT_FILE ($((skill_count + ${#COMMAND_SKILL_NAMES[@]})) skill imports)"
    else
        printf '%s' "$context_lines" > "$GEMINI_CONTEXT_FILE"
        echo "  [OK] $GEMINI_CONTEXT_FILE ($((skill_count + ${#COMMAND_SKILL_NAMES[@]})) skill imports)"
    fi
fi

# --- Gemini settings.json: context.fileName bridge ---

GEMINI_SETTINGS_FILE="$GEMINI_ROOT/settings.json"

echo ""
echo "=== Gemini settings.json bridge ==="

if $UNINSTALL; then
    echo "  [SKIP] settings.json not modified during uninstall (user-owned file)"
elif $DRY_RUN; then
    echo "  [DRY RUN] Would merge AGENTS.md into context.fileName in: $GEMINI_SETTINGS_FILE"
else
    if command -v jq &>/dev/null; then
        if [ -f "$GEMINI_SETTINGS_FILE" ]; then
            merged="$(jq '.context.fileName = ((.context.fileName // []) + ["AGENTS.md","GEMINI.md"] | unique)' "$GEMINI_SETTINGS_FILE")"
        else
            merged='{"context":{"fileName":["AGENTS.md","GEMINI.md"]}}'
        fi
        printf '%s\n' "$merged" > "$GEMINI_SETTINGS_FILE"
        echo "  [OK] $GEMINI_SETTINGS_FILE (context.fileName includes AGENTS.md and GEMINI.md)"
    elif [ ! -f "$GEMINI_SETTINGS_FILE" ]; then
        printf '{"context":{"fileName":["AGENTS.md","GEMINI.md"]}}\n' > "$GEMINI_SETTINGS_FILE"
        echo "  [OK] $GEMINI_SETTINGS_FILE (created; install jq for merge support on future runs)"
    else
        echo "  [WARN] jq not found and $GEMINI_SETTINGS_FILE already exists — skipping bridge (install jq and rerun)"
    fi
fi

# --- VS Code settings.json: disable duplicate .agents skill discovery ---

echo ""
echo "=== VS Code settings bridge ==="

if $UNINSTALL; then
    echo "  [SKIP] VS Code settings.json not modified during uninstall (user-owned file)"
elif $DRY_RUN; then
    echo "  [DRY RUN] Would set chat.agentSkillsLocations[\"~/.agents/skills\"]=false in: $VSCODE_SETTINGS_FILE"
else
    vscode_settings_dir="$(dirname "$VSCODE_SETTINGS_FILE")"
    mkdir -p "$vscode_settings_dir"

    if command -v python3 &>/dev/null; then
        if python3 - "$VSCODE_SETTINGS_FILE" <<'PY'
import json
import os
import sys

path = sys.argv[1]
if os.path.exists(path):
    with open(path, "r", encoding="utf-8") as fh:
        data = json.load(fh)
else:
    data = {}

locations = data.get("chat.agentSkillsLocations")
if locations is None:
    locations = {}
    data["chat.agentSkillsLocations"] = locations
elif not isinstance(locations, dict):
    raise TypeError("chat.agentSkillsLocations is not an object")

locations["~/.agents/skills"] = False

with open(path, "w", encoding="utf-8") as fh:
    json.dump(data, fh, indent=2)
    fh.write("\n")
PY
        then
            echo "  [OK] $VSCODE_SETTINGS_FILE (chat.agentSkillsLocations disables ~/.agents/skills for VS Code)"
        else
            echo "  [WARN] Could not merge $VSCODE_SETTINGS_FILE — add chat.agentSkillsLocations manually"
        fi
    elif [ ! -f "$VSCODE_SETTINGS_FILE" ]; then
        printf '{\n  "chat.agentSkillsLocations": {\n    "~/.agents/skills": false\n  }\n}\n' > "$VSCODE_SETTINGS_FILE"
        echo "  [OK] $VSCODE_SETTINGS_FILE (created; chat.agentSkillsLocations disables ~/.agents/skills for VS Code)"
    else
        echo "  [WARN] python3 not found and $VSCODE_SETTINGS_FILE already exists — add chat.agentSkillsLocations manually"
    fi
fi

# --- MCP config bridge ---

echo ""
echo "=== MCP config bridge ==="

if $UNINSTALL; then
    echo "  [SKIP] MCP config files not modified during uninstall (user-owned files)"
elif $DRY_RUN; then
    echo "  [DRY RUN] Would merge MCP servers into: $VSCODE_MCP_FILE"
    echo "  [DRY RUN] Would merge MCP servers into: $GEMINI_SETTINGS_FILE"
    echo "  [DRY RUN] Would merge MCP servers into: $CODEX_CONFIG_FILE"
else
    if [ ! -f "$MCP_MANIFEST_LOCAL" ]; then
        printf '{\n  "servers": {}\n}\n' > "$MCP_MANIFEST_LOCAL"
        echo "  [OK] Created local MCP override file: $MCP_MANIFEST_LOCAL"
    fi

    if command -v python3 &>/dev/null; then
        if python3 - "$REPO_ROOT" "$MCP_MANIFEST_EXAMPLE" "$MCP_MANIFEST_LOCAL" "$VSCODE_MCP_FILE" "$GEMINI_SETTINGS_FILE" "$CODEX_CONFIG_FILE" <<'PY'
import json
import os
import re
import sys
from pathlib import Path

repo_root = Path(sys.argv[1])
manifest_example = Path(sys.argv[2])
manifest_local = Path(sys.argv[3])
vscode_mcp = Path(sys.argv[4])
gemini_settings = Path(sys.argv[5])
codex_config = Path(sys.argv[6])


def read_json(path: Path):
    if not path.exists():
        return {}
    raw = path.read_text(encoding="utf-8").strip()
    if not raw:
        return {}
    return json.loads(raw)


def deep_merge(base, overlay):
    if isinstance(base, dict) and isinstance(overlay, dict):
        merged = dict(base)
        for key, value in overlay.items():
            if key in merged:
                merged[key] = deep_merge(merged[key], value)
            else:
                merged[key] = value
        return merged
    return overlay


def read_env_file(path: Path):
    values = {}
    if not path.exists():
        return values
    for line in path.read_text(encoding="utf-8").splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("#") or "=" not in stripped:
            continue
        key, value = stripped.split("=", 1)
        key = key.strip()
        value = value.strip()
        if key and value:
            values[key] = value
    return values


def get_value(values, name):
    return values.get(name) or os.environ.get(name)


def split_config_list(value):
    if not value:
        return []
    return [item for item in re.split(r"\s*,\s*", value) if item]


def resolve_string(value, values):
    match = re.fullmatch(r"\$\{([A-Z0-9_]+)\}", value)
    if match:
        resolved = get_value(values, match.group(1))
        if resolved is not None:
            return resolved
    return re.sub(r"\$\{([A-Z0-9_]+)\}", lambda m: get_value(values, m.group(1)) or m.group(0), value)


def resolve_node(node, values, expand_args=False):
    if isinstance(node, str):
        match = re.fullmatch(r"\$\{([A-Z0-9_]+)\[\]\}", node)
        if expand_args and match:
            return split_config_list(get_value(values, match.group(1)))
        return resolve_string(node, values)
    if isinstance(node, list):
        items = []
        for item in node:
            resolved = resolve_node(item, values, expand_args=expand_args)
            if expand_args and isinstance(resolved, list):
                items.extend(resolved)
            else:
                items.append(resolved)
        return items
    if isinstance(node, dict):
        return {key: resolve_node(value, values, expand_args=(key == "args")) for key, value in node.items()}
    return node


def provider_ready(provider, values):
    for key in provider.get("requiredEnv", []):
        if not get_value(values, key):
            return False
    return True


def toml_string(value):
    escaped = str(value).replace("\\", "\\\\").replace('"', '\\"')
    return f'"{escaped}"'


def toml_array(values):
    return "[" + ", ".join(toml_string(value) for value in values) + "]"


def codex_section(name, config):
    lines = [f"[mcp_servers.{name}]"]
    if "url" in config:
        lines.append(f"url = {toml_string(config['url'])}")
    else:
        lines.append(f"command = {toml_string(config['command'])}")
        if config.get("args"):
            lines.append(f"args = {toml_array(config['args'])}")
    if config.get("env"):
        lines.append("")
        lines.append(f"[mcp_servers.{name}.env]")
        for env_key in sorted(config["env"]):
            lines.append(f"{env_key} = {toml_string(config['env'][env_key])}")
    return "\n".join(lines)


manifest = read_json(manifest_example)
manifest = deep_merge(manifest, read_json(manifest_local))
servers = manifest.get("servers", {})

local_env = read_env_file(repo_root / "config.local.env")
if "MCP_MEMORY_FILE_PATH" not in local_env:
    local_env["MCP_MEMORY_FILE_PATH"] = str(Path.home() / "mcp-memory.json")
if "OBSIDIAN_VERIFY_SSL" not in local_env:
    local_env["OBSIDIAN_VERIFY_SSL"] = "false"
if "OBSIDIAN_ENABLE_CACHE" not in local_env:
    local_env["OBSIDIAN_ENABLE_CACHE"] = "true"
if "MCP_FILESYSTEM_PATHS" not in local_env:
    defaults = [str(repo_root.parent)]
    if local_env.get("OBSIDIAN_VAULT"):
        defaults.append(local_env["OBSIDIAN_VAULT"])
    local_env["MCP_FILESYSTEM_PATHS"] = ",".join(dict.fromkeys(defaults))

vscode_data = read_json(vscode_mcp)
vscode_data.setdefault("servers", {})
vscode_changed = False
for server_name, server in servers.items():
    provider = server.get("providers", {}).get("vscode")
    if not provider or not provider.get("enabled"):
        continue
    if not provider_ready(provider, local_env):
        print(f"  [WARN] Skipping VS Code MCP server '{server_name}' because required env is missing")
        continue
    key = provider.get("key", server_name)
    if key in vscode_data["servers"]:
        continue
    vscode_data["servers"][key] = resolve_node(provider["config"], local_env)
    vscode_changed = True
    print(f"  [ADD] VS Code MCP server: {key}")
if vscode_changed:
    vscode_mcp.parent.mkdir(parents=True, exist_ok=True)
    vscode_mcp.write_text(json.dumps(vscode_data, indent=2) + "\n", encoding="utf-8")
    print(f"  [OK] {vscode_mcp}")

gemini_data = read_json(gemini_settings)
gemini_data.setdefault("mcpServers", {})
gemini_changed = False
for server_name, server in servers.items():
    provider = server.get("providers", {}).get("gemini")
    if not provider or not provider.get("enabled"):
        continue
    if not provider_ready(provider, local_env):
        print(f"  [WARN] Skipping Gemini MCP server '{server_name}' because required env is missing")
        continue
    key = provider.get("key", server_name)
    if key in gemini_data["mcpServers"]:
        continue
    gemini_data["mcpServers"][key] = resolve_node(provider["config"], local_env)
    gemini_changed = True
    print(f"  [ADD] Gemini MCP server: {key}")
if gemini_changed:
    gemini_settings.parent.mkdir(parents=True, exist_ok=True)
    gemini_settings.write_text(json.dumps(gemini_data, indent=2) + "\n", encoding="utf-8")
    print(f"  [OK] {gemini_settings}")

codex_raw = codex_config.read_text(encoding="utf-8") if codex_config.exists() else ""
codex_sections = []
for server_name, server in servers.items():
    provider = server.get("providers", {}).get("codex")
    if not provider or not provider.get("enabled"):
        continue
    if not provider_ready(provider, local_env):
        print(f"  [WARN] Skipping Codex MCP server '{server_name}' because required env is missing")
        continue
    key = provider.get("key", server_name)
    if re.search(rf"(?m)^\[mcp_servers\.{re.escape(key)}\]\s*$", codex_raw):
        continue
    codex_sections.append(codex_section(key, resolve_node(provider["config"], local_env)))
    print(f"  [ADD] Codex MCP server: {key}")
if codex_sections:
    codex_config.parent.mkdir(parents=True, exist_ok=True)
    prefix = codex_raw.rstrip()
    if prefix:
        prefix += "\n\n"
    codex_config.write_text(prefix + "\n\n".join(codex_sections) + "\n", encoding="utf-8")
    print(f"  [OK] {codex_config}")
PY
        then
            :
        else
            echo "  [WARN] Could not merge MCP config files"
        fi
    else
        echo "  [WARN] python3 not found — skipping MCP config merge"
    fi
fi

# --- GAL_ROOT symlinks ---

gal_root_ok=0
echo ""
echo "=== GAL_ROOT symlinks ==="

if $UNINSTALL; then
    safe_unlink "$GAL_ROOT_COPILOT"
    safe_unlink "$GAL_ROOT_GEMINI"
else
    if safe_link "$GAL_ROOT_COPILOT" "$REPO_ROOT"; then ((gal_root_ok++)) || true; fi
    if safe_link "$GAL_ROOT_GEMINI"  "$REPO_ROOT"; then ((gal_root_ok++)) || true; fi
fi

# --- Generated GAL command skills (bake templates -> commands/gal*/SKILL.md) ---

echo ""
echo "=== Generated GAL command skills ==="

if $UNINSTALL; then
    for command_skill_name in "${COMMAND_SKILL_NAMES[@]}"; do
        baked_skill="$REPO_ROOT/commands/$command_skill_name/SKILL.md"
        if [ -f "$baked_skill" ]; then
            if $DRY_RUN; then echo "  [DRY RUN] Would remove baked: $baked_skill"
            else rm "$baked_skill"; echo "  [REMOVED] $baked_skill"; fi
        fi
    done
else
    for command_skill_name in "${COMMAND_SKILL_NAMES[@]}"; do
        command_skill_source="$REPO_ROOT/commands/$command_skill_name"
        command_skill_template="$command_skill_source/SKILL.template.md"
        if [ ! -f "$command_skill_template" ]; then
            echo "  [WARN] Template not found: $command_skill_template"
        else
            baked="$(sed "s|{{GAL_ROOT}}|$REPO_ROOT|g" "$command_skill_template")"
            baked_skill="$command_skill_source/SKILL.md"
            if $DRY_RUN; then
                echo "  [DRY RUN] Would write baked: $baked_skill"
            else
                printf '%s\n' "$baked" > "$baked_skill"
                echo "  [OK] $baked_skill"
            fi
        fi
    done
fi

# --- GAL command skill symlinks (commands/gal*/ -> ~/.copilot/skills/gal*/ + ~/.agents/skills/gal*/) ---

echo ""
echo "=== GAL command skill symlinks ==="

if $UNINSTALL; then
    for command_skill_name in "${COMMAND_SKILL_NAMES[@]}"; do
        safe_unlink "$SKILLS_TARGET/$command_skill_name"
        safe_unlink "$CODEX_SKILLS_TARGET/$command_skill_name"
    done
else
    for command_skill_name in "${COMMAND_SKILL_NAMES[@]}"; do
        command_skill_source="$REPO_ROOT/commands/$command_skill_name"
        safe_link "$SKILLS_TARGET/$command_skill_name" "$command_skill_source"
        safe_link "$CODEX_SKILLS_TARGET/$command_skill_name" "$command_skill_source"
    done
fi

# --- Migration: remove legacy gal-* dirs from installed locations ---

echo ""
echo "=== Migration: gal-* cleanup ==="

for skills_dir in "$SKILLS_TARGET" "$GEMINI_SKILLS_TARGET" "$CODEX_SKILLS_TARGET"; do
    for d in "$skills_dir"/gal-*/; do
        [ -e "$d" ] || continue
        d_name="$(basename "$d")"
        keep_dir=false
        for command_skill_name in "${COMMAND_SKILL_NAMES[@]}"; do
            if [ "$d_name" = "$command_skill_name" ]; then
                keep_dir=true
                break
            fi
        done
        if $keep_dir; then
            continue
        fi
        if $DRY_RUN; then
            echo "  [DRY RUN] Would remove: $d"
        else
            rm -rf "$d"
            echo "  [REMOVED] $d"
        fi
    done
done

# --- Summary ---

echo ""
if $UNINSTALL; then
    echo "Uninstall complete."
elif $DRY_RUN; then
    echo "Dry run complete. No changes made."
else
    echo "Setup complete: agents=$agent_ok/$agent_count, skills(copilot)=$skill_ok/$skill_count, skills(shared)=$codex_skill_ok/$skill_count, gal-root=$gal_root_ok/2"
    echo "Note: If SKILL.template.md changes, re-run setup-machine.sh --replace to regenerate."
    if [ "$agent_fail" -gt 0 ] || [ "$skill_fail" -gt 0 ] || [ "$codex_skill_fail" -gt 0 ]; then
        echo "Some links failed. Check warnings above."
    fi
fi

# --- Personalization: config.local.env + smudge/clean filter ---

if ! $UNINSTALL && ! $DRY_RUN; then
    echo ""
    echo "=== Personalization ==="

    EXAMPLE_ENV="$REPO_ROOT/config.example.env"
    LOCAL_ENV="$REPO_ROOT/config.local.env"

    # 1. Copy config.example.env → config.local.env if missing
    if [ ! -f "$LOCAL_ENV" ]; then
        if [ -f "$EXAMPLE_ENV" ]; then
            cp "$EXAMPLE_ENV" "$LOCAL_ENV"
            echo "  [OK] Created config.local.env from config.example.env"
            echo "  [ACTION REQUIRED] Edit config.local.env with your paths"
        else
            echo "  [WARN] config.example.env not found — skipping"
        fi
    else
        echo "  [SKIP] config.local.env already exists"
    fi

    # 2. Copy model-roles.example.md → model-roles.local.md if missing
    EXAMPLE_ROLES="$REPO_ROOT/model-roles.example.md"
    LOCAL_ROLES="$REPO_ROOT/model-roles.local.md"

    if [ ! -f "$LOCAL_ROLES" ]; then
        if [ -f "$EXAMPLE_ROLES" ]; then
            cp "$EXAMPLE_ROLES" "$LOCAL_ROLES"
            echo "  [OK] Created model-roles.local.md from model-roles.example.md"
        fi
    else
        echo "  [SKIP] model-roles.local.md already exists"
    fi

    # 3. Register git smudge/clean filter
    cd "$REPO_ROOT"
    git config filter.gal-config.smudge "bash scripts/gal-smudge.sh"
    git config filter.gal-config.clean  "bash scripts/gal-clean.sh"
    git config filter.gal-config.required true
    echo "  [OK] Registered git filter 'gal-config' (smudge/clean)"

    # 4. Set custom hooks path
    git config core.hooksPath .githooks
    echo "  [OK] Set core.hooksPath to .githooks"

    # 5. Re-checkout ONLY the smudge-filtered files (not all tracked files)
    if [ -f "$LOCAL_ENV" ]; then
        has_values=$(grep -v '^\s*#' "$LOCAL_ENV" | grep -c '=.' || true)
        if [ "$has_values" -gt 0 ]; then
            tracked_filter_files=()
            for filter_file in config.local.env model-roles.local.md; do
                if git ls-files --error-unmatch "$filter_file" >/dev/null 2>&1; then
                    tracked_filter_files+=("$filter_file")
                fi
            done

            if [ "${#tracked_filter_files[@]}" -gt 0 ]; then
                git checkout -- "${tracked_filter_files[@]}"
                echo "  [OK] Re-checked out tracked filtered files (smudge filter applied)"
            else
                echo "  [INFO] Filtered files are not tracked yet — skipping git checkout"
            fi
        else
            echo "  [INFO] config.local.env has no values yet — fill it in, then run: git checkout -- config.local.env model-roles.local.md"
        fi
    fi
fi
