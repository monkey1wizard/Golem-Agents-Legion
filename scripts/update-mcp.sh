#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
. "$SCRIPT_DIR/common/common.sh"

invoke_update_mcp() {
    echo ''
    echo '=== MCP config bridge ==='

    if $UNINSTALL; then
        # Remove AGY plugin root MCP config
        plugin_mcp_file="$HOME/.gemini/antigravity-cli/plugins/gal/mcp_config.json"
        if [[ -f "$plugin_mcp_file" ]]; then
            if $DRY_RUN; then
                echo "  [DRY RUN] Would remove AGY plugin MCP config: $plugin_mcp_file"
            else
                rm -f "$plugin_mcp_file"
                echo "  [CLEANUP] AGY plugin MCP config removed: $plugin_mcp_file"
            fi
        fi
        echo '  [SKIP] Global MCP config files are preserved during uninstall.'
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
        if $INSTALL_GEMINI || $INSTALL_ANTIGRAVITY; then
            echo "  [DRY RUN] Would merge MCP servers into: $ANTIGRAVITY_MCP_FILE"
            echo "  [DRY RUN] Would remove GAL-managed legacy Gemini MCP entries from: $GEMINI_SETTINGS_FILE"
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

    run_python - "$REPO_ROOT" "$MCP_SOURCE_FILE" "$MCP_LOCAL_FILE" "$VSCODE_MCP_FILE" "$COPILOT_CLI_MCP_FILE" "$GEMINI_SETTINGS_FILE" "$ANTIGRAVITY_MCP_FILE" "$CODEX_CONFIG_FILE" "$OPENCODE_CONFIG_FILE" "$INSTALL_COPILOT" "$INSTALL_GEMINI" "$INSTALL_ANTIGRAVITY" "$INSTALL_CODEX" "$INSTALL_OPENCODE" "$INSTALL_CLAUDE" "$DRY_RUN" "$GAL_CONFIG_FILE" "$GAL_XMACHINE_CONFIG_FILE" "$GAL_GENERATED_MCP_FILE" "$GAL_GENERATED_XMACHINE_FILE" <<'PY'
import json
import os
import re
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

repo_root = Path(sys.argv[1])
mcp_source = Path(sys.argv[2])
mcp_local = Path(sys.argv[3])
vscode_mcp = Path(sys.argv[4])
copilot_cli_mcp = Path(sys.argv[5])
gemini_settings = Path(sys.argv[6])
agy_mcp = Path(sys.argv[7])
codex_config = Path(sys.argv[8])
opencode_config = Path(sys.argv[9])
install_copilot = sys.argv[10].lower() == 'true'
install_gemini = sys.argv[11].lower() == 'true'
install_antigravity = sys.argv[12].lower() == 'true'
install_codex = sys.argv[13].lower() == 'true'
install_opencode = sys.argv[14].lower() == 'true'
install_claude = sys.argv[15].lower() == 'true'
dry_run = sys.argv[16].lower() == 'true'
gal_config = Path(sys.argv[17])
gal_xmachine_config = Path(sys.argv[18])
generated_mcp = Path(sys.argv[19])
generated_xmachine = Path(sys.argv[20])

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
    ('vscode', 'github'): ['github-mcp-server'],
    ('copilot-cli', 'github'): ['github-mcp-server'],
    ('copilot-cli', 'playwright'): ['microsoft/playwright-mcp', 'microsoft-playwright-mcp', 'playwright-mcp'],
    ('gemini', 'github'): ['github-mcp-server', 'github/github-mcp-server'],
    ('gemini', 'upstash/context7'): ['context7'],
    ('gemini', 'microsoftdocs/mcp'): ['Microsoft Learn MCP Server'],
    ('gemini', 'github/github-mcp-server'): ['github'],
    ('gemini', 'chromedevtools/chrome-devtools-mcp'): ['chrome-devtools'],
    ('gemini', 'playwright'): ['microsoft/playwright-mcp', 'microsoft-playwright-mcp', 'playwright-mcp'],
    ('antigravity', 'github'): ['github-mcp-server'],
    ('codex', 'github'): ['github-mcp-server', 'github/github-mcp-server'],
    ('codex', 'upstash/context7'): ['context7'],
    ('codex', 'microsoftdocs/mcp'): ['microsoftdocs'],
    ('codex', 'imageFetch'): ['imagefetch'],
    ('codex', 'github/github-mcp-server'): ['github'],
    ('codex', 'chromedevtools/chrome-devtools-mcp'): ['chrome-devtools'],
    ('codex', 'playwright'): ['microsoft/playwright-mcp', 'microsoft-playwright-mcp', 'playwright-mcp'],
    ('claude', 'github'): ['github-mcp-server'],
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


def resolve_inputs(inputs, values):
    if not isinstance(inputs, list):
        return []
    return [resolve_node(item, values) for item in inputs]


def normalize_servers(servers):
    normalized = dict(servers)
    if 'github' in normalized:
        normalized.pop('github-mcp-server', None)
    return normalized


def read_machine_config():
    data = read_json(gal_config)
    return data if isinstance(data, dict) else {}


def build_mcp_values(legacy_values, machine_config):
    values = dict(legacy_values)
    machine_mappings = {
        'OBSIDIAN_VAULT': 'obsidianVault',
        'OBSIDIAN_VAULT_NAME': 'obsidianVaultName',
        'OBSIDIAN_GUIDE_PATH': 'obsidianGuidePath',
        'OBSIDIAN_GUIDE_MODE': 'obsidianGuideMode',
        'CONTEXT7_API_KEY': 'context7ApiKey',
        'TEMP_DIR': 'tempDir',
        'LOCAL_SEARCH_PROJECT': 'localSearchProject',
    }
    for env_name, config_key in machine_mappings.items():
        raw_value = machine_config.get(config_key)
        if raw_value is not None and str(raw_value).strip():
            values[env_name] = str(raw_value).strip()

    filesystem_paths = machine_config.get('mcpFilesystemPaths')
    if isinstance(filesystem_paths, list):
        joined_paths = ','.join(str(item).strip() for item in filesystem_paths if str(item).strip())
        if joined_paths:
            values['MCP_FILESYSTEM_PATHS'] = joined_paths
    elif filesystem_paths is not None and str(filesystem_paths).strip():
        values['MCP_FILESYSTEM_PATHS'] = str(filesystem_paths).strip()

    if 'MCP_FILESYSTEM_PATHS' not in values:
        defaults = [str(repo_root.parent)]
        obsidian_vault = get_value(values, 'OBSIDIAN_VAULT')
        if obsidian_vault:
            defaults.append(obsidian_vault)
        values['MCP_FILESYSTEM_PATHS'] = ','.join(dict.fromkeys(defaults))

    return values


def manifest_contains_filesystem_server(manifest):
    servers = manifest.get('servers')
    if not isinstance(servers, dict):
        return False

    for server_name, server_config in servers.items():
        if re.search(r'filesystem', str(server_name), re.IGNORECASE):
            return True
        if isinstance(server_config, dict):
            if re.search(r'filesystem', str(server_config.get('command', '')), re.IGNORECASE):
                return True
            for argument in server_config.get('args') or []:
                if re.search(r'filesystem', str(argument), re.IGNORECASE):
                    return True
    return False


def convert_projection_server(config):
    projection = {}
    if 'url' in config:
        projection['serverUrl'] = str(config['url'])
    if 'command' in config:
        projection['command'] = str(config['command'])
    if 'args' in config:
        projection['args'] = [str(item) for item in config.get('args') or []]
    if isinstance(config.get('env'), dict):
        projection['env'] = config['env']
    if isinstance(config.get('headers'), dict):
        projection['headers'] = config['headers']
    if 'tools' in config:
        projection['tools'] = config['tools']
    return projection


def build_generated_mcp_projection(manifest, values, runtime_entries):
    projection_servers = {
        server_name: convert_projection_server(server_config)
        for server_name, server_config in manifest['servers'].items()
    }
    secret_bearing = any(
        isinstance(server_config.get('headers'), dict) and 'CONTEXT7_API_KEY' in server_config['headers']
        for server_config in projection_servers.values()
    )
    projection = {
        'schemaVersion': 1,
        'generatedAt': datetime.now(timezone.utc).isoformat(),
        'generatedBy': 'update-mcp.sh',
        'mcpServers': projection_servers,
        '_metadata': {
            'ownership': 'gal-managed',
            'secretBearing': secret_bearing,
            'sourceFiles': {
                'trackedManifest': str(mcp_source),
                'localOverrideManifest': str(mcp_local),
                'machineConfig': str(gal_config),
            },
            'runtimeEntries': runtime_entries,
            'preservation': 'Runtime config updates replace only GAL-managed MCP entries and preserve unrelated user-owned entries.',
        },
    }
    if isinstance(manifest.get('inputs'), list):
        projection['inputs'] = manifest['inputs']
    if manifest_contains_filesystem_server(manifest):
        projection['mcpFilesystemPaths'] = split_config_list(get_value(values, 'MCP_FILESYSTEM_PATHS'))
    return projection


def default_xmachine_binding():
    return {
        'schemaVersion': 1,
        'defaultXmachineNode': '',
        'xmachineNodeAliases': {},
        'machineProfiles': {},
        'localPluginPaths': [],
        'providerPathOverrides': {},
        'additionalBindings': {},
    }


def read_xmachine_binding():
    binding = default_xmachine_binding()
    legacy_config = repo_root / 'xmachine.config.json'
    if gal_xmachine_config.exists():
        configured_binding = read_json(gal_xmachine_config)
        if isinstance(configured_binding, dict):
            if isinstance(configured_binding.get('nodes'), dict) and 'xmachineNodeAliases' not in configured_binding:
                configured_binding['xmachineNodeAliases'] = configured_binding['nodes']
            return deep_merge(binding, configured_binding)
        return binding

    if legacy_config.exists():
        legacy_binding = read_json(legacy_config)
        if isinstance(legacy_binding, dict) and isinstance(legacy_binding.get('nodes'), dict):
            binding['xmachineNodeAliases'] = legacy_binding['nodes']

    return binding


def build_generated_xmachine_projection(binding):
    projection = {
        'schemaVersion': 1,
        'generatedAt': datetime.now(timezone.utc).isoformat(),
        'generatedBy': 'update-mcp.sh',
        'defaultXmachineNode': str(binding.get('defaultXmachineNode') or ''),
        'nodes': binding.get('xmachineNodeAliases', {}),
        '_metadata': {
            'ownership': 'gal-managed',
            'secretBearing': False,
            'sourceFiles': {
                'machineBinding': str(gal_xmachine_config),
                'legacyRepoConfig': str(repo_root / 'xmachine.config.json'),
            },
        },
    }
    for optional_key in ('machineProfiles', 'localPluginPaths', 'providerPathOverrides', 'additionalBindings'):
        if optional_key in binding:
            projection[optional_key] = binding[optional_key]
    return projection


def write_generated_projections(manifest, values, runtime_entries):
    xmachine_binding = read_xmachine_binding()
    if dry_run:
        print(f'  [DRY RUN] Would render managed MCP projection: {generated_mcp}')
        print(f'  [DRY RUN] Would render managed xmachine projection: {generated_xmachine}')
        return

    if not gal_xmachine_config.exists():
        write_json(gal_xmachine_config, xmachine_binding)
        print(f'  [OK] Created machine xmachine binding file: {gal_xmachine_config}')

    write_json(generated_mcp, build_generated_mcp_projection(manifest, values, runtime_entries))
    print(f'  [OK] {generated_mcp}')

    write_json(generated_xmachine, build_generated_xmachine_projection(xmachine_binding))
    print(f'  [OK] {generated_xmachine}')


def read_previous_projection():
    data = read_json(generated_mcp)
    return data if isinstance(data, dict) else {}


def get_projection_runtime_entries(projection, runtime_name):
    metadata = projection.get('_metadata')
    if not isinstance(metadata, dict):
        return {}
    runtime_entries = metadata.get('runtimeEntries')
    if not isinstance(runtime_entries, dict):
        return {}
    entries = runtime_entries.get(runtime_name)
    return entries if isinstance(entries, dict) else {}


def can_replace_managed_runtime_entry(previous_runtime_entries, key, existing_config, desired_config, runtime_label):
    if existing_config is None:
        return True
    if key in previous_runtime_entries:
        return True
    if desired_config is not None and json_like_equal(existing_config, desired_config):
        return True
    print(f'  [WARN] Preserving user-owned {runtime_label} MCP entry: {key}')
    return False


def find_copilot_managed_config_for_key(manifest, key):
    for server_name, server_config in manifest['servers'].items():
        profile = get_copilot_cli_bridge_profile(server_name)
        converted = convert_copilot_cli_config(server_config)
        if profile.get('enabled', True) and profile.get('key') == key:
            return converted
        if key in legacy_aliases('copilot-cli', server_name):
            return converted
    return None


def sync_managed_inputs(data, manifest):
    managed_inputs = manifest.get('inputs')
    if not isinstance(managed_inputs, list):
        return False

    managed_ids = {
        str(item.get('id'))
        for item in managed_inputs
        if isinstance(item, dict) and item.get('id')
    }
    current_inputs = data.get('inputs')
    if not isinstance(current_inputs, list):
        current_inputs = []

    preserved = []
    for existing in current_inputs:
        if isinstance(existing, dict) and str(existing.get('id')) in managed_ids:
            continue
        preserved.append(existing)

    merged_inputs = preserved + managed_inputs
    if json_like_equal(current_inputs, merged_inputs):
        return False

    data['inputs'] = merged_inputs
    return True


def remove_managed_inputs(data, manifest):
    current_inputs = data.get('inputs')
    if not isinstance(current_inputs, list):
        return False

    managed_inputs = manifest.get('inputs')
    if not isinstance(managed_inputs, list):
        return False

    managed_ids = {
        str(item.get('id'))
        for item in managed_inputs
        if isinstance(item, dict) and item.get('id')
    }
    if not managed_ids:
        return False

    preserved = []
    for existing in current_inputs:
        if isinstance(existing, dict) and str(existing.get('id')) in managed_ids:
            continue
        preserved.append(existing)

    if json_like_equal(current_inputs, preserved):
        return False

    if preserved:
        data['inputs'] = preserved
    else:
        data.pop('inputs', None)
    return True


def get_bridge_profile(server_name, runtime_name):
    profile = {'enabled': True, 'key': server_name}
    profile.update(BRIDGE_PROFILES.get(server_name, {}).get(runtime_name, {}))
    return profile


def legacy_aliases(runtime_name, server_name):
    return LEGACY_ALIASES.get((runtime_name, server_name), [])


def deprecated_managed_definitions():
    return {
        'memory': {
            'raw_config': {
                'command': 'npx',
                'args': ['-y', '@modelcontextprotocol/server-memory'],
                'env': {
                    'MEMORY_FILE_PATH': str(Path.home() / 'mcp-memory.json'),
                },
            },
            'match': {
                'command': 'npx',
                'args': ['-y', '@modelcontextprotocol/server-memory'],
            },
        },
    }


def deprecated_managed_keys():
    return list(deprecated_managed_definitions().keys())


def deprecated_managed_config(runtime_name, key):
    definition = deprecated_managed_definitions().get(key)
    if definition is None:
        return None

    raw = dict(definition['raw_config'])
    if runtime_name == 'vscode':
        return raw
    if runtime_name == 'copilot-cli':
        return convert_copilot_cli_config(raw)
    return None


def is_deprecated_managed_entry(runtime_name, key, existing_config):
    if not isinstance(existing_config, dict):
        return False

    definition = deprecated_managed_definitions().get(key)
    if definition is None:
        return False

    if runtime_name == 'copilot-cli':
        if 'command' not in existing_config or 'args' not in existing_config:
            return False
        if 'type' in existing_config and str(existing_config['type']) != 'local':
            return False
    elif 'command' not in existing_config or 'args' not in existing_config:
        return False

    raw_args = existing_config.get('args') or []
    args = [str(item) for item in raw_args] if isinstance(raw_args, list) else [str(raw_args)]
    expected = definition['match']
    return str(existing_config.get('command')) == str(expected['command']) and args == [str(item) for item in expected['args']]


def json_like_equal(left, right):
    return json.dumps(left, sort_keys=True) == json.dumps(right, sort_keys=True)


def convert_agy_config(config):
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

    machine_config = read_machine_config()
    env_file = gal_config / 'config.local.env'
    legacy_env_file = repo_root / 'config.local.env'
    if not env_file.exists() and legacy_env_file.exists():
        env_file = legacy_env_file

    values = build_mcp_values(read_env_file(env_file), machine_config)

    resolved = {
        'servers': normalize_servers({server_name: resolve_node(server_config, values) for server_name, server_config in servers.items()})
    }
    if 'inputs' in manifest:
        resolved['inputs'] = resolve_inputs(manifest.get('inputs'), values)
    return {
        'runtime_manifest': resolved,
        'values': values,
    }


def update_vscode(manifest, previous_projection):
    data = read_json(vscode_mcp)
    servers = data.setdefault('servers', {})
    previous_runtime_entries = get_projection_runtime_entries(previous_projection, 'vscode')
    changed = False
    managed_runtime_entries = {}
    for deprecated_key in deprecated_managed_keys():
        if deprecated_key not in servers:
            continue
        desired_config = deprecated_managed_config('vscode', deprecated_key)
        if not is_deprecated_managed_entry('vscode', deprecated_key, servers[deprecated_key]) and not can_replace_managed_runtime_entry(previous_runtime_entries, deprecated_key, servers[deprecated_key], desired_config, 'VS Code'):
            continue
        changed = True
        prefix = '[DRY RUN] Would remove' if dry_run else '[CLEANUP]'
        print(f'  {prefix} deprecated VS Code MCP server: {deprecated_key}')
        if not dry_run:
            del servers[deprecated_key]

    for server_name in manifest['servers']:
        desired_config = manifest['servers'][server_name]
        for alias in legacy_aliases('vscode', server_name):
            if alias in servers and can_replace_managed_runtime_entry(previous_runtime_entries, alias, servers[alias], desired_config, 'VS Code'):
                changed = True
                prefix = '[DRY RUN] Would remove' if dry_run else '[CLEANUP]'
                print(f'  {prefix} VS Code MCP alias: {alias}')
                if not dry_run:
                    del servers[alias]
    for server_name, server_config in manifest['servers'].items():
        profile = get_bridge_profile(server_name, 'vscode')
        if not profile.get('enabled', True):
            continue
        key = profile.get('key', server_name)
        existing_config = servers.get(key)
        if not can_replace_managed_runtime_entry(previous_runtime_entries, key, existing_config, server_config, 'VS Code'):
            continue
        if key not in servers or not json_like_equal(existing_config, server_config):
            servers[key] = server_config
            changed = True
            prefix = '[DRY RUN] Would set' if dry_run else '[SET]'
            print(f'  {prefix} VS Code MCP server: {key}')
    if sync_managed_inputs(data, manifest):
        changed = True
        prefix = '[DRY RUN] Would set' if dry_run else '[SET]'
        print(f'  {prefix} VS Code MCP inputs')

    for server_name, server_config in manifest['servers'].items():
        profile = get_bridge_profile(server_name, 'vscode')
        if not profile.get('enabled', True):
            continue
        key = profile.get('key', server_name)
        existing_config = servers.get(key)
        if can_replace_managed_runtime_entry(previous_runtime_entries, key, existing_config, server_config, 'VS Code') and (existing_config is None or key in previous_runtime_entries):
            managed_runtime_entries[key] = server_config
    if changed and not dry_run:
        write_json(vscode_mcp, data)
        print(f'  [OK] {vscode_mcp}')
    return managed_runtime_entries


def update_copilot_cli(manifest, previous_projection):
    data = read_json(copilot_cli_mcp)
    servers = data.setdefault('mcpServers', {})
    previous_runtime_entries = get_projection_runtime_entries(previous_projection, 'copilot-cli')
    changed = False
    managed_runtime_entries = {}
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
    managed_keys.update(deprecated_managed_keys())
    for server_name in manifest['servers']:
        managed_keys.update(legacy_aliases('copilot-cli', server_name))

    for existing_server_name in list(servers.keys()):
        if existing_server_name in managed_keys:
            desired_config = find_copilot_managed_config_for_key(manifest, existing_server_name)
            if desired_config is None:
                desired_config = deprecated_managed_config('copilot-cli', existing_server_name)
            if not is_deprecated_managed_entry('copilot-cli', existing_server_name, servers[existing_server_name]) and not can_replace_managed_runtime_entry(previous_runtime_entries, existing_server_name, servers[existing_server_name], desired_config, 'Copilot CLI'):
                continue
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
        existing_config = servers.get(target_server_name)
        if not can_replace_managed_runtime_entry(previous_runtime_entries, target_server_name, existing_config, converted, 'Copilot CLI'):
            continue
        if target_server_name not in servers or not json_like_equal(existing_config, converted):
            changed = True
            prefix = '[DRY RUN] Would set' if dry_run else '[SET]'
            print(f'  {prefix} Copilot CLI MCP server: {target_server_name}')
            if not dry_run:
                servers[target_server_name] = converted
        if existing_config is None or target_server_name in previous_runtime_entries:
            managed_runtime_entries[target_server_name] = converted
    if changed and not dry_run:
        write_json(copilot_cli_mcp, data)
        print(f'  [OK] {copilot_cli_mcp}')
    return managed_runtime_entries


def update_antigravity(manifest):
    # --- Write MCP config to the AGY plugin root ---
    plugin_install_target = Path(os.environ.get('HOME', Path.home())) / '.gemini' / 'antigravity-cli' / 'plugins' / 'gal'
    plugin_mcp_file = plugin_install_target / 'mcp_config.json'

    agy_servers = {}
    for server_name, server_config in manifest['servers'].items():
        agy_servers[server_name] = convert_agy_config(server_config)
    plugin_mcp_config = {'mcpServers': agy_servers}
    if 'inputs' in manifest:
        plugin_mcp_config['inputs'] = manifest['inputs']

    if not dry_run:
        plugin_install_target.mkdir(parents=True, exist_ok=True)
        write_json(plugin_mcp_file, plugin_mcp_config)
        print(f'  [SET] AGY plugin MCP config: {plugin_mcp_file}')
    else:
        print(f'  [DRY RUN] Would write AGY plugin MCP config: {plugin_mcp_file}')

    # --- Clean up GAL-managed entries from the global AGY MCP config ---
    data = read_json(agy_mcp)
    if not data:
        return
    servers = data.get('mcpServers')
    if not isinstance(servers, dict):
        return

    changed = False
    for server_name in manifest['servers']:
        # Remove the canonical name
        if server_name in servers:
            changed = True
            prefix = '[DRY RUN] Would remove' if dry_run else '[CLEANUP]'
            print(f'  {prefix} global AGY MCP entry: {server_name}')
            if not dry_run:
                del servers[server_name]
        # Remove legacy aliases
        for alias in legacy_aliases('antigravity', server_name):
            if alias in servers:
                changed = True
                prefix = '[DRY RUN] Would remove' if dry_run else '[CLEANUP]'
                print(f'  {prefix} global AGY MCP alias: {alias}')
                if not dry_run:
                    del servers[alias]

    # Remove managed inputs from global config
    if remove_managed_inputs(data, manifest):
        changed = True
        prefix = '[DRY RUN] Would remove' if dry_run else '[CLEANUP]'
        print(f'  {prefix} global AGY MCP inputs')

    # Clean up empty mcpServers block
    if isinstance(servers, dict) and not servers:
        data.pop('mcpServers', None)
        changed = True

    if changed and not dry_run:
        write_json(agy_mcp, data)
        print(f'  [OK] {agy_mcp}')


def cleanup_legacy_gemini_mcp(manifest):
    data = read_json(gemini_settings)
    if not data:
        return

    servers = data.get('mcpServers')
    changed = False
    if isinstance(servers, dict):
        for server_name in manifest['servers']:
            for managed_name in [server_name, *legacy_aliases('gemini', server_name)]:
                if managed_name in servers:
                    changed = True
                    prefix = '[DRY RUN] Would remove' if dry_run else '[CLEANUP]'
                    print(f'  {prefix} legacy Gemini MCP entry: {managed_name}')
                    if not dry_run:
                        del servers[managed_name]

        if not servers:
            changed = True
            prefix = '[DRY RUN] Would remove' if dry_run else '[CLEANUP]'
            print(f'  {prefix} empty legacy Gemini mcpServers block')
            if not dry_run:
                data.pop('mcpServers', None)

    if remove_managed_inputs(data, manifest):
        changed = True
        prefix = '[DRY RUN] Would remove' if dry_run else '[CLEANUP]'
        print(f'  {prefix} legacy Gemini MCP inputs')

    if changed and not dry_run:
        write_json(gemini_settings, data)
        print(f'  [OK] {gemini_settings}')


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

        for legacy_alias in legacy_aliases('claude', server_name):
            invoke_claude(['mcp', 'remove', legacy_alias], quiet=True)
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

resolved_state = resolved_manifest()
if resolved_state is None:
    raise SystemExit(0)

previous_projection = read_previous_projection()
manifest = resolved_state['runtime_manifest']
values = resolved_state['values']
runtime_entries = {}

if install_copilot:
    runtime_entries['vscode'] = update_vscode(manifest, previous_projection)
    runtime_entries['copilot-cli'] = update_copilot_cli(manifest, previous_projection)
if install_gemini or install_antigravity:
    update_antigravity(manifest)
    cleanup_legacy_gemini_mcp(manifest)
if install_codex:
    update_codex(manifest)
if install_opencode:
    update_opencode(manifest)
if install_claude:
    update_claude(manifest)

write_generated_projections(manifest, values, runtime_entries)
PY
}

parse_setup_args "$@"
initialize_setup_session
invoke_update_mcp