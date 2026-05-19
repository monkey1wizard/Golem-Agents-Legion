#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
. "$SCRIPT_DIR/common/common.sh"

invoke_update_mcp() {
    echo ''
    echo '=== MCP config bridge ==='

    if $UNINSTALL; then
        echo '  [SKIP] MCP config files are preserved during uninstall.'
        return 0
    fi

    if ! resolve_python_command; then
        echo '  [WARN] python3/python not found — skipping MCP config update'
        return 0
    fi

    if $DRY_RUN; then
        if $INSTALL_COPILOT; then
            echo "  [DRY RUN] Would merge MCP servers into: $VSCODE_MCP_FILE"
            echo "  [DRY RUN] Would merge MCP servers into: $COPILOT_CLI_MCP_FILE"
        fi
        if $INSTALL_GEMINI; then
            echo "  [DRY RUN] Would merge MCP servers into: $GEMINI_SETTINGS_FILE"
        fi
        if $INSTALL_ANTIGRAVITY; then
            echo "  [DRY RUN] Would merge MCP servers into: $ANTIGRAVITY_MCP_FILE"
        fi
        if $INSTALL_CODEX; then
            echo "  [DRY RUN] Would merge MCP servers into: $CODEX_CONFIG_FILE"
        fi
        if $INSTALL_OPENCODE; then
            echo "  [DRY RUN] Would merge MCP servers into: $OPENCODE_CONFIG_FILE"
        fi
        if $INSTALL_CLAUDE; then
            echo '  [DRY RUN] Would merge MCP servers through Claude CLI user scope'
        fi
    fi

    run_python - "$REPO_ROOT" "$MCP_SOURCE_FILE" "$MCP_LOCAL_FILE" "$VSCODE_MCP_FILE" "$COPILOT_CLI_MCP_FILE" "$GEMINI_SETTINGS_FILE" "$ANTIGRAVITY_MCP_FILE" "$CODEX_CONFIG_FILE" "$OPENCODE_CONFIG_FILE" "$INSTALL_COPILOT" "$INSTALL_GEMINI" "$INSTALL_ANTIGRAVITY" "$INSTALL_CODEX" "$INSTALL_OPENCODE" "$INSTALL_CLAUDE" "$DRY_RUN" <<'PY'
import json
import os
import re
import subprocess
import sys
from pathlib import Path

repo_root = Path(sys.argv[1])
mcp_source = Path(sys.argv[2])
mcp_local = Path(sys.argv[3])
vscode_mcp = Path(sys.argv[4])
copilot_cli_mcp = Path(sys.argv[5])
gemini_settings = Path(sys.argv[6])
antigravity_mcp = Path(sys.argv[7])
codex_config = Path(sys.argv[8])
opencode_config = Path(sys.argv[9])
install_copilot = sys.argv[10].lower() == 'true'
install_gemini = sys.argv[11].lower() == 'true'
install_antigravity = sys.argv[12].lower() == 'true'
install_codex = sys.argv[13].lower() == 'true'
install_opencode = sys.argv[14].lower() == 'true'
install_claude = sys.argv[15].lower() == 'true'
dry_run = sys.argv[16].lower() == 'true'

BRIDGE_PROFILES = {
    'upstash/context7': {
        'gemini': {'key': 'upstash/context7'},
        'codex': {'key': 'context7'},
    },
    'microsoftdocs/mcp': {
        'gemini': {'key': 'microsoftdocs/mcp'},
        'codex': {'key': 'microsoftdocs'},
    },
    'imageFetch': {
        'codex': {'key': 'imageFetch'},
    },
    'chromedevtools/chrome-devtools-mcp': {
        'codex': {'key': 'chrome-devtools'},
    },
    'microsoft/markitdown': {
        'codex': {'key': 'markitdown'},
    },
    'github-mcp-server': {
        'codex': {'enabled': False, 'key': None},
    },
    'playwright': {
        'codex': {'key': 'playwright'},
    },
}

LEGACY_ALIASES = {
    ('copilot-cli', 'playwright'): ['microsoft/playwright-mcp', 'microsoft-playwright-mcp', 'playwright-mcp'],
    ('gemini', 'upstash/context7'): ['context7'],
    ('gemini', 'microsoftdocs/mcp'): ['Microsoft Learn MCP Server'],
    ('gemini', 'github/github-mcp-server'): ['github'],
    ('gemini', 'chromedevtools/chrome-devtools-mcp'): ['chrome-devtools'],
    ('gemini', 'playwright'): ['microsoft/playwright-mcp', 'microsoft-playwright-mcp', 'playwright-mcp'],
    ('codex', 'upstash/context7'): ['context7'],
    ('codex', 'microsoftdocs/mcp'): ['microsoftdocs'],
    ('codex', 'imageFetch'): ['imagefetch'],
    ('codex', 'github/github-mcp-server'): ['github'],
    ('codex', 'chromedevtools/chrome-devtools-mcp'): ['chrome-devtools'],
    ('codex', 'playwright'): ['microsoft/playwright-mcp', 'microsoft-playwright-mcp', 'playwright-mcp'],
}


def read_json(path: Path):
    if not path.exists():
        return {}
    raw = path.read_text(encoding='utf-8').strip()
    if not raw:
        return {}
    return json.loads(raw)


def write_json(path: Path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, indent=2) + '\n', encoding='utf-8')


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
    for line in path.read_text(encoding='utf-8').splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith('#') or '=' not in stripped:
            continue
        key, value = stripped.split('=', 1)
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
    return [item for item in re.split(r'\s*,\s*', value) if item]


def resolve_string(value, values):
    match = re.fullmatch(r'\$\{([A-Z0-9_]+)\}', value)
    if match:
        resolved = get_value(values, match.group(1))
        if resolved is not None:
            return resolved
    return re.sub(r'\$\{([A-Z0-9_]+)\}', lambda m: get_value(values, m.group(1)) or m.group(0), value)


def resolve_node(node, values, expand_args=False):
    if isinstance(node, str):
        match = re.fullmatch(r'\$\{([A-Z0-9_]+)\[\]\}', node)
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
        return {key: resolve_node(value, values, expand_args=(key == 'args')) for key, value in node.items()}
    return node


def get_bridge_profile(server_name, runtime_name):
    profile = {'enabled': True, 'key': server_name}
    profile.update(BRIDGE_PROFILES.get(server_name, {}).get(runtime_name, {}))
    return profile


def legacy_aliases(runtime_name, server_name):
    return LEGACY_ALIASES.get((runtime_name, server_name), [])


def json_like_equal(left, right):
    return json.dumps(left, sort_keys=True) == json.dumps(right, sort_keys=True)


def convert_gemini_config(config):
    converted = {}
    for key, value in config.items():
        if key == 'type':
            continue
        if key == 'url' and config.get('type') == 'http':
            converted['httpUrl'] = value
            continue
        converted[key] = value
    return converted


def convert_antigravity_config(config):
    converted = {}
    for key, value in config.items():
        if key == 'type':
            continue
        if key == 'url':
            converted['serverUrl'] = value
            continue
        converted[key] = value
    return converted


def convert_codex_config(config):
    return {key: value for key, value in config.items() if key != 'type'}


def convert_opencode_config(config):
    transport = config.get('type') or ('http' if 'url' in config else 'stdio')
    if transport in {'http', 'sse'}:
        converted = {
            'type': 'remote',
            'url': config['url'],
            'enabled': True,
        }
        if isinstance(config.get('headers'), dict):
            converted['headers'] = config['headers']
        if 'timeout' in config:
            converted['timeout'] = config['timeout']
        return converted

    command = []
    if 'command' in config:
        command.append(str(config['command']))
    raw_args = config.get('args')
    if isinstance(raw_args, list):
        command.extend(str(item) for item in raw_args)
    elif raw_args is not None:
        command.append(str(raw_args))

    converted = {
        'type': 'local',
        'command': command,
        'enabled': True,
    }
    if isinstance(config.get('env'), dict):
        converted['environment'] = config['env']
    if 'timeout' in config:
        converted['timeout'] = config['timeout']
    return converted


def get_copilot_cli_bridge_profile(server_name):
    profiles = {
        'chromedevtools/chrome-devtools-mcp': {'enabled': True, 'key': 'chrome-devtools'},
        'github-mcp-server': {'enabled': False, 'key': None},
        'memory': {'enabled': True, 'key': 'memory'},
        'microsoftdocs/mcp': {'enabled': True, 'key': 'microsoftdocs'},
        'microsoft/markitdown': {'enabled': True, 'key': 'markitdown'},
        'playwright': {'enabled': True, 'key': 'playwright'},
        'upstash/context7': {'enabled': True, 'key': 'context7'},
        'blender': {'enabled': True, 'key': 'blender'},
        'freecad': {'enabled': True, 'key': 'freecad'},
    }
    if server_name in profiles:
        return profiles[server_name]

    normalized = re.sub(r'^[^A-Za-z0-9]+', '', server_name)
    normalized = re.sub(r'[^A-Za-z0-9_-]+', '-', normalized).lower() or 'server'
    return {'enabled': True, 'key': normalized}


def get_copilot_cli_transport(config):
    transport = config.get('type')
    if transport == 'http':
        return 'http'
    if transport == 'sse':
        return 'sse'
    if transport in {'stdio', 'local'}:
        return 'local'
    if transport:
        return str(transport)
    if 'url' in config:
        return 'http'
    return 'local'


def convert_copilot_cli_config(config):
    transport = get_copilot_cli_transport(config)
    converted = {
        'type': transport,
        'tools': ['*'],
    }
    if transport in {'http', 'sse'}:
        if 'url' in config:
            converted['url'] = config['url']
        if isinstance(config.get('headers'), dict):
            converted['headers'] = config['headers']
        return converted

    if 'command' in config:
        converted['command'] = config['command']
    raw_args = config.get('args')
    if raw_args is None:
        converted['args'] = []
    elif isinstance(raw_args, list):
        converted['args'] = [str(item) for item in raw_args]
    else:
        converted['args'] = [str(raw_args)]
    if isinstance(config.get('env'), dict):
        converted['env'] = config['env']
    return converted


def toml_string(value):
    escaped = str(value).replace('\\', '\\\\').replace('"', '\\"')
    return f'"{escaped}"'


def format_toml_key_segment(key):
    if re.fullmatch(r'[A-Za-z0-9_-]+', str(key)):
        return str(key)
    return toml_string(key)


def toml_value(value):
    if value is None:
        return '""'
    if isinstance(value, bool):
        return 'true' if value else 'false'
    if isinstance(value, (int, float)):
        return str(value)
    if isinstance(value, str):
        return toml_string(value)
    if isinstance(value, list):
        return '[' + ', '.join(toml_value(item) for item in value) + ']'
    return toml_string(str(value))


def toml_table_sections(path_segments, data):
    header = '[' + '.'.join(format_toml_key_segment(segment) for segment in path_segments) + ']'
    lines = [header]
    nested_sections = []
    for key, value in data.items():
        if isinstance(value, dict):
            nested_sections.extend(toml_table_sections(path_segments + [key], value))
        else:
            lines.append(f'{format_toml_key_segment(key)} = {toml_value(value)}')
    return ['\r\n'.join(lines)] + nested_sections


def codex_section(name, config):
    return '\r\n\r\n'.join(toml_table_sections(['mcp_servers', name], config))


def get_codex_table_server_name(line):
    match = re.match(r'^\[(?P<path>[^\]]+)\]\s*$', line)
    if not match:
        return None
    path = match.group('path')
    if not path.startswith('mcp_servers.'):
        return None
    remainder = path[len('mcp_servers.'):]
    if not remainder:
        return None
    if remainder.startswith('"'):
        token = []
        escaped = False
        for char in remainder[1:]:
            if escaped:
                token.append(char)
                escaped = False
                continue
            if char == '\\':
                escaped = True
                continue
            if char == '"':
                return ''.join(token)
            token.append(char)
        return None
    return remainder.split('.', 1)[0]


def remove_codex_managed_servers(raw_content, server_names):
    if not raw_content.strip():
        return ''
    managed = {name for name in server_names if name}
    result_lines = []
    skip_current = False
    for line in raw_content.splitlines():
        server_name = get_codex_table_server_name(line)
        if server_name is not None:
            skip_current = server_name in managed
        elif re.match(r'^\[[^\]]+\]\s*$', line):
            skip_current = False
        if not skip_current:
            result_lines.append(line)
    return '\r\n'.join(result_lines).rstrip('\r\n')


def invoke_claude(arguments, quiet=False):
    stdout = subprocess.DEVNULL if quiet else None
    stderr = subprocess.DEVNULL if quiet else None
    return subprocess.call(['claude', *arguments], stdout=stdout, stderr=stderr)


def resolved_manifest():
    if not mcp_source.exists():
        print(f'  [WARN] MCP source file not found: {mcp_source}')
        return None
    if not dry_run and not mcp_local.exists():
        write_json(mcp_local, {'servers': {}})
        print(f'  [OK] Created local MCP override file: {mcp_local}')

    manifest = read_json(mcp_source)
    if mcp_local.exists():
        manifest = deep_merge(manifest, read_json(mcp_local))

    servers = manifest.get('servers')
    if not isinstance(servers, dict):
        print('  [WARN] MCP manifest does not contain a valid servers object.')
        return None

    values = read_env_file(repo_root / 'config.local.env')
    values.setdefault('MCP_MEMORY_FILE_PATH', str(Path.home() / 'mcp-memory.json'))
    if 'MCP_FILESYSTEM_PATHS' not in values:
        defaults = [str(repo_root.parent)]
        obsidian_vault = get_value(values, 'OBSIDIAN_VAULT')
        if obsidian_vault:
            defaults.append(obsidian_vault)
        values['MCP_FILESYSTEM_PATHS'] = ','.join(dict.fromkeys(defaults))

    return {
        'servers': {server_name: resolve_node(server_config, values) for server_name, server_config in servers.items()}
    }


def update_vscode(manifest):
    data = read_json(vscode_mcp)
    servers = data.setdefault('servers', {})
    changed = False
    for server_name, server_config in manifest['servers'].items():
        profile = get_bridge_profile(server_name, 'vscode')
        if not profile.get('enabled', True):
            continue
        key = profile.get('key', server_name)
        if key not in servers or not json_like_equal(servers[key], server_config):
            servers[key] = server_config
            changed = True
            prefix = '[DRY RUN] Would set' if dry_run else '[SET]'
            print(f'  {prefix} VS Code MCP server: {key}')
    if changed and not dry_run:
        write_json(vscode_mcp, data)
        print(f'  [OK] {vscode_mcp}')


def update_copilot_cli(manifest):
    data = read_json(copilot_cli_mcp)
    servers = data.setdefault('mcpServers', {})
    changed = False
    managed_keys = {
        profile['key']
        for profile in (get_copilot_cli_bridge_profile(server_name) for server_name in manifest['servers'])
        if profile.get('key')
    }
    managed_keys.update({
        'chromedevtools/chrome-devtools-mcp',
        'github-mcp-server',
        'microsoftdocs/mcp',
        'microsoft/markitdown',
        'upstash/context7',
    })
    for server_name in manifest['servers']:
        managed_keys.update(legacy_aliases('copilot-cli', server_name))

    for existing_server_name in list(servers.keys()):
        if existing_server_name in managed_keys:
            changed = True
            prefix = '[DRY RUN] Would remove' if dry_run else '[CLEANUP]'
            print(f'  {prefix} Copilot CLI MCP server: {existing_server_name}')
            if not dry_run:
                del servers[existing_server_name]

    for server_name, server_config in manifest['servers'].items():
        profile = get_copilot_cli_bridge_profile(server_name)
        if not profile.get('enabled', True):
            continue

        target_server_name = profile['key']
        converted = convert_copilot_cli_config(server_config)
        if target_server_name not in servers or not json_like_equal(servers[target_server_name], converted):
            changed = True
            prefix = '[DRY RUN] Would set' if dry_run else '[SET]'
            print(f'  {prefix} Copilot CLI MCP server: {target_server_name}')
            if not dry_run:
                servers[target_server_name] = converted
    if changed and not dry_run:
        write_json(copilot_cli_mcp, data)
        print(f'  [OK] {copilot_cli_mcp}')


def update_gemini(manifest):
    data = read_json(gemini_settings)
    servers = data.setdefault('mcpServers', {})
    changed = False
    for server_name, server_config in manifest['servers'].items():
        for alias in legacy_aliases('gemini', server_name):
            if alias in servers:
                changed = True
                prefix = '[DRY RUN] Would remove' if dry_run else '[CLEANUP]'
                print(f'  {prefix} Gemini MCP alias: {alias}')
                if not dry_run:
                    del servers[alias]
        converted = convert_gemini_config(server_config)
        if server_name not in servers or not json_like_equal(servers[server_name], converted):
            changed = True
            prefix = '[DRY RUN] Would set' if dry_run else '[SET]'
            print(f'  {prefix} Gemini MCP server: {server_name}')
            if not dry_run:
                servers[server_name] = converted
    if changed and not dry_run:
        write_json(gemini_settings, data)
        print(f'  [OK] {gemini_settings}')


def update_antigravity(manifest):
    data = read_json(antigravity_mcp)
    servers = data.setdefault('mcpServers', {})
    changed = False
    for server_name, server_config in manifest['servers'].items():
        converted = convert_antigravity_config(server_config)
        if server_name not in servers or not json_like_equal(servers[server_name], converted):
            changed = True
            prefix = '[DRY RUN] Would set' if dry_run else '[SET]'
            print(f'  {prefix} Antigravity MCP server: {server_name}')
            if not dry_run:
                servers[server_name] = converted
    if changed and not dry_run:
        write_json(antigravity_mcp, data)
        print(f'  [OK] {antigravity_mcp}')


def update_codex(manifest):
    current_raw = codex_config.read_text(encoding='utf-8') if codex_config.exists() else ''
    managed_names = []
    for server_name in manifest['servers']:
        managed_names.append(server_name)
        profile = get_bridge_profile(server_name, 'codex')
        if profile.get('key'):
            managed_names.append(profile['key'])
        managed_names.extend(legacy_aliases('codex', server_name))

    remaining = remove_codex_managed_servers(current_raw, managed_names)
    sections = []
    written_names = []
    for server_name, server_config in manifest['servers'].items():
        profile = get_bridge_profile(server_name, 'codex')
        if not profile.get('enabled', True):
            continue
        written_names.append(profile['key'])
        sections.append(codex_section(profile['key'], convert_codex_config(server_config)))
    new_raw = remaining
    if new_raw:
        new_raw = new_raw.rstrip('\r\n') + '\r\n\r\n'
    new_raw += '\r\n\r\n'.join(sections) + '\r\n'

    if current_raw == new_raw:
        return
    for server_name in written_names:
        prefix = '[DRY RUN] Would set' if dry_run else '[SET]'
        print(f'  {prefix} Codex MCP server: {server_name}')
    if not dry_run:
        codex_config.parent.mkdir(parents=True, exist_ok=True)
        codex_config.write_text(new_raw, encoding='utf-8')
        print(f'  [OK] {codex_config}')


def update_opencode(manifest):
    data = read_json(opencode_config)
    servers = data.setdefault('mcp', {})
    changed = False
    for server_name, server_config in manifest['servers'].items():
        converted = convert_opencode_config(server_config)
        if server_name not in servers or not json_like_equal(servers[server_name], converted):
            changed = True
            prefix = '[DRY RUN] Would set' if dry_run else '[SET]'
            print(f'  {prefix} OpenCode MCP server: {server_name}')
            if not dry_run:
                servers[server_name] = converted
    if changed and not dry_run:
        write_json(opencode_config, data)
        print(f'  [OK] {opencode_config}')


def update_claude(manifest):
    if shutil.which('claude') is None:
        print('  [WARN] Claude CLI not found; skipping Claude MCP installation.')
        return
    for server_name, server_config in manifest['servers'].items():
        if dry_run:
            print(f'  [DRY RUN] Would upsert Claude MCP server: {server_name}')
            continue

        invoke_claude(['mcp', 'remove', server_name], quiet=True)
        if server_config.get('type') == 'http':
            arguments = ['mcp', 'add', '--scope', 'user', '--transport', 'http']
            for header_name, header_value in (server_config.get('headers') or {}).items():
                arguments.extend(['-H', f'{header_name}: {header_value}'])
            if 'tools' in server_config:
                print(f'  [WARN] Claude CLI does not expose a tools filter flag; installing {server_name} without tools scoping.')
            arguments.extend([server_name, str(server_config['url'])])
        else:
            arguments = ['mcp', 'add', '--scope', 'user', '--transport', 'stdio']
            for env_name, env_value in (server_config.get('env') or {}).items():
                arguments.extend(['-e', f'{env_name}={env_value}'])
            command_and_args = [str(server_config['command'])]
            command_and_args.extend(str(item) for item in server_config.get('args') or [])
            arguments.extend([server_name, '--', *command_and_args])

        exit_code = invoke_claude(arguments)
        if exit_code == 0:
            print(f'  [SET] Claude MCP server: {server_name}')
        else:
            print(f'  [WARN] Failed to configure Claude MCP server: {server_name}')


import shutil

manifest = resolved_manifest()
if manifest is None:
    raise SystemExit(0)

if install_copilot:
    update_vscode(manifest)
    update_copilot_cli(manifest)
if install_gemini:
    update_gemini(manifest)
if install_antigravity:
    update_antigravity(manifest)
if install_codex:
    update_codex(manifest)
if install_opencode:
    update_opencode(manifest)
if install_claude:
    update_claude(manifest)
PY
}

parse_setup_args "$@"
initialize_setup_session
invoke_update_mcp