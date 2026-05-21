#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
. "$SCRIPT_DIR/common/common.sh"

invoke_update_personalization() {
    local skill_dirs=()
    while IFS= read -r skill_dir; do
        [ -n "$skill_dir" ] || continue
        skill_dirs+=("$skill_dir")
    done < <(find "$REPO_ROOT/skills" -mindepth 1 -maxdepth 1 -type d -print | LC_ALL=C sort)

    ensure_setup_directories "$GAL_STATE_ROOT" "$GEMINI_ROOT" "$ANTIGRAVITY_ROOT"

    echo ''
    echo '=== Gemini gal-context.md ==='
    if $UNINSTALL || ! $INSTALL_GEMINI; then
        if [ -f "$GEMINI_CONTEXT_FILE" ]; then
            if $DRY_RUN; then
                echo "  [DRY RUN] Would remove: $GEMINI_CONTEXT_FILE"
            else
                rm "$GEMINI_CONTEXT_FILE"
                echo "  [REMOVED] $GEMINI_CONTEXT_FILE"
            fi
        fi
    else
        local context_lines=''
        local skill_dir skill_name
        for skill_dir in "${skill_dirs[@]}"; do
            skill_name="$(basename "$skill_dir")"
            context_lines+="@$skill_dir/SKILL.md"$'\n'
        done

        if $DRY_RUN; then
            echo "  [DRY RUN] Would write: $GEMINI_CONTEXT_FILE (${#skill_dirs[@]} skill imports)"
        else
            printf '%s' "$context_lines" > "$GEMINI_CONTEXT_FILE"
            echo "  [OK] $GEMINI_CONTEXT_FILE (${#skill_dirs[@]} skill imports)"
        fi
    fi

    echo ''
    echo '=== Gemini settings.json bridge ==='
    if $UNINSTALL; then
        echo '  [SKIP] settings.json not modified during uninstall (user-owned file)'
    elif ! $INSTALL_GEMINI; then
        echo '  [SKIP] Gemini runtime not selected; settings.json bridge not updated'
    elif $DRY_RUN; then
        echo "  [DRY RUN] Would merge AGENTS.md into context.fileName in: $GEMINI_SETTINGS_FILE"
    else
        if command_exists jq; then
            local merged
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

    echo ''
    echo '=== AGY Plugin (rules/gal.md) ==='
    if $UNINSTALL || ! $INSTALL_ANTIGRAVITY; then
        # Remove the installed plugin directory on uninstall or when AGY is not selected
        if [ -e "$AGY_PLUGIN_INSTALL_TARGET" ]; then
            if $DRY_RUN; then
                echo "  [DRY RUN] Would remove: $AGY_PLUGIN_INSTALL_TARGET"
            else
                rm -rf "$AGY_PLUGIN_INSTALL_TARGET"
                echo "  [REMOVED] $AGY_PLUGIN_INSTALL_TARGET"
            fi
        else
            echo '  [SKIP] No AGY plugin installation to remove'
        fi
    else
        # Build and install the AGY plugin (includes rules/gal.md from instruction corpus)
        local build_script="$SCRIPT_DIR/build-agy-plugin.sh"
        if [ -f "$build_script" ]; then
            if $DRY_RUN; then
                echo '  [DRY RUN] Would run: build-agy-plugin.sh --force --install'
            else
                bash "$build_script" --force --install
                echo '  [OK] AGY plugin built and installed (rules/gal.md from instruction corpus)'
            fi
        else
            echo "  [WARN] build-agy-plugin.sh not found at: $build_script"
        fi
    fi

    echo ''
    echo '=== Migration: repo .agents cleanup ==='
    local workspace_rule_file="$WORKSPACE_RULES_TARGET/gal.md"
    if [ -f "$workspace_rule_file" ]; then
        if ! is_gal_managed_file "$workspace_rule_file"; then
            echo "  [SKIP] User-owned repo rule preserved: $workspace_rule_file"
        elif $DRY_RUN; then
            echo "  [DRY RUN] Would remove: $workspace_rule_file"
        else
            rm "$workspace_rule_file"
            echo "  [REMOVED] $workspace_rule_file"
        fi
    fi

    local repo_agents_path
    for repo_agents_path in "$WORKSPACE_SKILLS_TARGET" "$WORKSPACE_RULES_TARGET" "$WORKSPACE_AGENTS_ROOT"; do
        [ -d "$repo_agents_path" ] || continue
        if find "$repo_agents_path" -mindepth 1 -maxdepth 1 | read -r _; then
            echo "  [SKIP] Non-empty repo path preserved: $repo_agents_path"
            continue
        fi
        if $DRY_RUN; then
            echo "  [DRY RUN] Would remove empty directory: $repo_agents_path"
        else
            rmdir "$repo_agents_path"
            echo "  [REMOVED] Empty directory: $repo_agents_path"
        fi
    fi

    echo ''
    echo '=== VS Code settings bridge ==='
    if $UNINSTALL; then
        echo '  [SKIP] VS Code settings.json not modified during uninstall (user-owned file)'
    elif ! $INSTALL_COPILOT; then
        echo '  [SKIP] Copilot runtime not selected; VS Code settings bridge not updated'
    elif $DRY_RUN; then
        echo "  [DRY RUN] Would set chat.agentSkillsLocations[\"~/.agents/skills\"]=false in: $VSCODE_SETTINGS_FILE"
    else
        local vscode_settings_dir
        vscode_settings_dir="$(dirname "$VSCODE_SETTINGS_FILE")"
        mkdir -p "$vscode_settings_dir"

        if resolve_python_command; then
            if run_python - "$VSCODE_SETTINGS_FILE" <<'PY'
import json
import os
import sys

path = sys.argv[1]
if os.path.exists(path):
    with open(path, 'r', encoding='utf-8') as fh:
        data = json.load(fh)
else:
    data = {}

locations = data.get('chat.agentSkillsLocations')
if locations is None:
    locations = {}
    data['chat.agentSkillsLocations'] = locations
elif not isinstance(locations, dict):
    raise TypeError('chat.agentSkillsLocations is not an object')

locations['~/.agents/skills'] = False

with open(path, 'w', encoding='utf-8') as fh:
    json.dump(data, fh, indent=2)
    fh.write('\n')
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

    if $UNINSTALL || $DRY_RUN; then
        return 0
    fi

    echo ''
    echo '=== Personalization ==='

    local example_env="$REPO_ROOT/config.example.env"
    local local_env="$REPO_ROOT/config.local.env"
    if [ ! -f "$local_env" ]; then
        if [ -f "$example_env" ]; then
            cp "$example_env" "$local_env"
            echo '  [OK] Created config.local.env from config.example.env'
            echo '  [ACTION REQUIRED] Edit config.local.env with your paths'
        else
            echo '  [WARN] config.example.env not found — skipping'
        fi
    else
        echo '  [SKIP] config.local.env already exists'
    fi

    local example_roles="$REPO_ROOT/model-roles.example.md"
    local local_roles="$REPO_ROOT/model-roles.local.md"
    if [ ! -f "$local_roles" ]; then
        if [ -f "$example_roles" ]; then
            cp "$example_roles" "$local_roles"
            echo '  [OK] Created model-roles.local.md from model-roles.example.md'
        fi
    else
        echo '  [SKIP] model-roles.local.md already exists'
    fi

    pushd "$REPO_ROOT" >/dev/null
    git config filter.gal-config.smudge 'bash scripts/gal-smudge.sh'
    git config filter.gal-config.clean 'bash scripts/gal-clean.sh'
    git config filter.gal-config.required true
    echo "  [OK] Registered git filter 'gal-config' (smudge/clean)"

    git config core.hooksPath .githooks
    echo '  [OK] Set core.hooksPath to .githooks'

    if [ -f "$local_env" ]; then
        local has_values
        has_values="$(grep -v '^\s*#' "$local_env" | grep -c '=.' || true)"
        if [ "$has_values" -gt 0 ]; then
            local tracked_filter_files=()
            local filter_file
            for filter_file in config.local.env model-roles.local.md; do
                if git ls-files --error-unmatch "$filter_file" >/dev/null 2>&1; then
                    tracked_filter_files+=("$filter_file")
                fi
            done

            if [ "${#tracked_filter_files[@]}" -gt 0 ]; then
                git checkout -- "${tracked_filter_files[@]}"
                echo '  [OK] Re-checked out tracked filtered files (smudge filter applied)'
            else
                echo '  [INFO] Filtered files are not tracked yet — skipping git checkout'
            fi
        else
            echo '  [INFO] config.local.env has no values yet — fill it in, then run: git checkout -- config.local.env model-roles.local.md'
        fi
    fi
    popd >/dev/null
}

parse_setup_args "$@"
initialize_setup_session
invoke_update_personalization