#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
. "$SCRIPT_DIR/common/common.sh"

get_agent_frontmatter_value() {
    local agent_path="$1"
    local field_name="$2"
    local value

    value="$(awk -v field="$field_name" '
        BEGIN { in_frontmatter = 0 }
        NR == 1 {
            if ($0 ~ /^---[[:space:]]*$/) {
                in_frontmatter = 1
                next
            }
        }
        in_frontmatter {
            if ($0 ~ /^---[[:space:]]*$/) {
                exit
            }
            if ($0 ~ ("^" field ":[[:space:]]*")) {
                sub("^" field ":[[:space:]]*", "", $0)
                print $0
                exit
            }
        }
    ' "$agent_path")"

    value="${value#\"}"
    value="${value%\"}"
    printf '%s' "$value"
}

get_agent_markdown_body() {
    local agent_path="$1"
    awk '
        BEGIN { in_frontmatter = 0 }
        NR == 1 {
            if ($0 ~ /^---[[:space:]]*$/) {
                in_frontmatter = 1
                next
            }
        }
        in_frontmatter {
            if ($0 ~ /^---[[:space:]]*$/) {
                in_frontmatter = 0
                next
            }
            next
        }
        { print }
    ' "$agent_path"
}

get_agent_tools() {
    local agent_path="$1"
    local raw_tools
    raw_tools="$(get_agent_frontmatter_value "$agent_path" tools)"
    raw_tools="${raw_tools#[}"
    raw_tools="${raw_tools%]}"
    printf '%s' "$raw_tools" | tr ',' '\n' | sed "s/^[[:space:]]*//; s/[[:space:]]*$//; s/^'//; s/'$//; s/^\"//; s/\"$//" | awk 'NF'
}

emit_opencode_permission_lines() {
    local agent_path="$1"
    local line
    local allow_read=false
    local allow_list=false
    local allow_grep=false
    local allow_glob=false
    local allow_edit=false
    local allow_bash=false

    while IFS= read -r line; do
        case "$line" in
            read)
                allow_read=true
                allow_list=true
                ;;
            search)
                allow_read=true
                allow_list=true
                allow_grep=true
                allow_glob=true
                ;;
            edit)
                allow_edit=true
                ;;
            execute)
                allow_bash=true
                ;;
        esac
    done < <(get_agent_tools "$agent_path")

    if ! $allow_read && ! $allow_list && ! $allow_grep && ! $allow_glob && ! $allow_edit && ! $allow_bash; then
        allow_read=true
        allow_list=true
    fi

    $allow_read && printf '  read: allow\n'
    $allow_list && printf '  list: allow\n'
    $allow_grep && printf '  grep: allow\n'
    $allow_glob && printf '  glob: allow\n'
    $allow_edit && printf '  edit: allow\n'
    $allow_bash && printf '  bash: allow\n'
}

new_opencode_agent_file_content() {
    local agent_path="$1"
    local description color body

    description="$(get_agent_frontmatter_value "$agent_path" description)"
    [ -n "$description" ] || description='GAL golem agent'
    color="$(get_agent_frontmatter_value "$agent_path" color)"
    body="$(get_agent_markdown_body "$agent_path")"

    {
        printf '%s\n' "$GAL_MANAGED_FILE_HEADER"
        printf '%s\n' '---'
        printf '%s\n' 'description: |'
        while IFS= read -r line; do
            printf '  %s\n' "$line"
        done <<< "$description"
        printf '%s\n' 'mode: subagent'
        if [ -n "$color" ]; then
            printf 'color: %s\n' "$color"
        fi
        printf '%s\n' 'permission:'
        emit_opencode_permission_lines "$agent_path"
        printf '%s\n\n' '---'
        printf '%s\n' "$body"
    }
}

invoke_update_skills() {
    ensure_setup_directories \
        "$COPILOT_ROOT" \
        "$AGENTS_TARGET" \
        "$SKILLS_TARGET" \
        "$SHARED_AGENTS_ROOT" \
        "$SHARED_SKILLS_TARGET" \
        "$GEMINI_ROOT" \
        "$GEMINI_SKILLS_TARGET" \
        "$ANTIGRAVITY_ROOT" \
        "$ANTIGRAVITY_SKILLS_TARGET" \
        "$CODEX_ROOT" \
        "$CODEX_SKILLS_TARGET" \
        "$OPENCODE_ROOT" \
        "$OPENCODE_AGENTS_TARGET" \
        "$GAL_GENERATED_PROVIDERS_ROOT"

    local agent_files=()
    while IFS= read -r agent_file; do
        [ -n "$agent_file" ] || continue
        agent_files+=("$agent_file")
    done < <(find "$REPO_ROOT/agent" -maxdepth 1 -type f -name '*.agent.md' -print | LC_ALL=C sort)

    echo ''
    echo "=== Agents (${#agent_files[@]} files) ==="
    local agent_file link_path
    for agent_file in "${agent_files[@]}"; do
        link_path="$AGENTS_TARGET/$(basename "$agent_file")"
        safe_unlink "$link_path"
    done
    local active_agent_names=()
    for agent_file in "${agent_files[@]}"; do
        active_agent_names+=("$(basename "$agent_file")")
    done
    if [ -d "$AGENTS_TARGET" ]; then
        while IFS= read -r existing_agent_link; do
            [ -n "$existing_agent_link" ] || continue
            agent_name="$(basename "$existing_agent_link")"
            contains_value "$agent_name" "${active_agent_names[@]}" && continue
            is_gal_repo_link "$existing_agent_link" || continue
            safe_unlink "$existing_agent_link"
        done < <(find "$AGENTS_TARGET" -maxdepth 1 -type l -name '*.agent.md' -print | LC_ALL=C sort)
    fi

    echo ''
    echo "=== OpenCode Agents (${#agent_files[@]} generated subagents) ==="
    local agent_name target_path agent_content
    for agent_file in "${agent_files[@]}"; do
        agent_name="$(basename "$agent_file" .agent.md)"
        target_path="$OPENCODE_AGENTS_TARGET/$agent_name.md"
        if $UNINSTALL || ! $INSTALL_OPENCODE; then
            if [ ! -f "$target_path" ]; then
                continue
            fi
            if ! is_gal_managed_file "$target_path"; then
                echo "  [SKIP] User-owned OpenCode agent preserved: $target_path"
                continue
            fi
            if $DRY_RUN; then
                echo "  [DRY RUN] Would remove: $target_path"
            else
                rm -f "$target_path"
                echo "  [REMOVED] $target_path"
            fi
        else
            agent_content="$(new_opencode_agent_file_content "$agent_file")"
            if $DRY_RUN; then
                echo "  [DRY RUN] Would write: $target_path"
            else
                printf '%s\n' "$agent_content" > "$target_path"
                echo "  [OK] $target_path"
            fi
        fi
    done

    local skill_dirs=()
    local install_mode
    install_mode="$(get_configured_install_mode)"
    while IFS= read -r skill_dir; do
        [ -n "$skill_dir" ] || continue
        skill_dirs+=("$skill_dir")
    done < <(find "$REPO_ROOT/skills" -mindepth 1 -maxdepth 1 -type d -print | LC_ALL=C sort)

    echo ''
    echo "=== Skills (${#skill_dirs[@]} directories) ==="
    local skill_dir skill_name
    for skill_dir in "${skill_dirs[@]}"; do
        skill_name="$(basename "$skill_dir")"
        link_path="$SKILLS_TARGET/$skill_name"
        safe_unlink "$link_path"
    done
    local active_copilot_skill_names=()
    for skill_dir in "${skill_dirs[@]}"; do
        active_copilot_skill_names+=("$(basename "$skill_dir")")
    done
    for command_skill_name in "${COMMAND_SKILL_NAMES[@]}"; do
        active_copilot_skill_names+=("$command_skill_name")
    done
    if [ -d "$SKILLS_TARGET" ]; then
        while IFS= read -r existing_skill_link; do
            [ -n "$existing_skill_link" ] || continue
            skill_name="$(basename "$existing_skill_link")"
            contains_value "$skill_name" "${active_copilot_skill_names[@]}" && continue
            is_gal_repo_link "$existing_skill_link" || continue
            safe_unlink "$existing_skill_link"
        done < <(find "$SKILLS_TARGET" -maxdepth 1 -type l -print | LC_ALL=C sort)
    fi

    echo ''
    echo '=== AGY Plugin (skills + command skills + agents) ==='
    if [ "$install_mode" = 'install' ]; then
        echo '  [SKIP] Install mode delegates AGY plugin lifecycle to install-gal-plugins.sh.'
    elif $UNINSTALL || ! $INSTALL_ANTIGRAVITY; then
        # Legacy cleanup: remove old AGY skill symlinks under antigravity-cli/skills/
        for skill_dir in "${skill_dirs[@]}"; do
            skill_name="$(basename "$skill_dir")"
            legacy_link="$ANTIGRAVITY_SKILLS_TARGET/$skill_name"
            safe_unlink "$legacy_link"
        done
        # Also clean up legacy command skill symlinks
        for cmd_name in "${COMMAND_SKILL_NAMES[@]}"; do
            legacy_link="$ANTIGRAVITY_SKILLS_TARGET/$cmd_name"
            safe_unlink "$legacy_link"
        done
        # Remove the installed plugin directory
        if [ -e "$AGY_PLUGIN_INSTALL_TARGET" ]; then
            if $DRY_RUN; then
                echo "  [DRY RUN] Would remove: $AGY_PLUGIN_INSTALL_TARGET"
            else
                rm -rf "$AGY_PLUGIN_INSTALL_TARGET"
                echo "  [REMOVED] $AGY_PLUGIN_INSTALL_TARGET"
            fi
        fi
    else
        # Legacy cleanup: remove old AGY skill symlinks under antigravity-cli/skills/
        for skill_dir in "${skill_dirs[@]}"; do
            skill_name="$(basename "$skill_dir")"
            legacy_link="$ANTIGRAVITY_SKILLS_TARGET/$skill_name"
            safe_unlink "$legacy_link"
        done
        for cmd_name in "${COMMAND_SKILL_NAMES[@]}"; do
            legacy_link="$ANTIGRAVITY_SKILLS_TARGET/$cmd_name"
            safe_unlink "$legacy_link"
        done

        # Build and install the AGY plugin (includes reusable skills, command skills, agents)
        local build_script="$SCRIPT_DIR/build-core-plugin.sh"
        if [ -f "$build_script" ]; then
            if $DRY_RUN; then
                echo '  [DRY RUN] Would run: build-core-plugin.sh --force --install'
            else
                bash "$build_script" --force --install
                echo '  [OK] Core plugin built and installed'
            fi
        else
            echo "  [WARN] build-core-plugin.sh not found at: $build_script"
        fi
    fi

    echo ''
    echo '=== Migration: repo .agents skills cleanup ==='
    for skill_dir in "${skill_dirs[@]}"; do
        skill_name="$(basename "$skill_dir")"
        link_path="$WORKSPACE_SKILLS_TARGET/$skill_name"
        safe_unlink "$link_path"
    done

    echo ''
    echo '=== Migration: .gemini/skills cleanup ==='
    local all_gal_skill_names=("${COMMAND_SKILL_NAMES[@]}" 'gal.bak')
    for skill_dir in "${skill_dirs[@]}"; do
        all_gal_skill_names+=("$(basename "$skill_dir")")
    done

    local name target
    for name in "${all_gal_skill_names[@]}"; do
        link_path="$GEMINI_SKILLS_TARGET/$name"
        [ -e "$link_path" ] || continue
        if is_symlink "$link_path"; then
            target="$(readlink "$link_path" 2>/dev/null || true)"
            if [[ "$target" == *"$REPO_ROOT"* ]]; then
                if $DRY_RUN; then
                    echo "  [DRY RUN] Would remove: $link_path"
                else
                    rm -rf "$link_path"
                    echo "  [REMOVED] $link_path"
                fi
            fi
        elif [ "$name" = 'gal.bak' ]; then
            if $DRY_RUN; then
                echo "  [DRY RUN] Would remove: $link_path"
            else
                rm -rf "$link_path"
                echo "  [REMOVED] $link_path"
            fi
        fi
    done

    echo "=== Shared Skills - Codex + OpenCode (${#skill_dirs[@]} reusable directories via ~/.agents) ==="
    for skill_dir in "${skill_dirs[@]}"; do
        skill_name="$(basename "$skill_dir")"
        link_path="$SHARED_SKILLS_TARGET/$skill_name"
        if $UNINSTALL || { ! $INSTALL_CODEX && ! $INSTALL_OPENCODE; }; then
            safe_unlink "$link_path"
        else
            safe_link "$link_path" "$skill_dir"
        fi
    done

    echo ''
    echo "=== Claude legacy skills cleanup (${#skill_dirs[@]} reusable directories) ==="
    local claude_skills_target="$HOME/.claude/skills"
    for skill_dir in "${skill_dirs[@]}"; do
        skill_name="$(basename "$skill_dir")"
        link_path="$claude_skills_target/$skill_name"
        safe_unlink "$link_path"
    done

    echo ''
    echo '=== GAL_ROOT symlinks ==='
    local should_keep_gal_source_link=false
    if ! $UNINSTALL && { $INSTALL_GEMINI || $INSTALL_ANTIGRAVITY; }; then
        should_keep_gal_source_link=true
    fi

    if $should_keep_gal_source_link; then
        safe_link "$GAL_SOURCE_ROOT" "$REPO_ROOT"
    else
        safe_unlink "$GAL_SOURCE_ROOT"
    fi

    if $UNINSTALL || ! $INSTALL_COPILOT; then
        safe_unlink "$GAL_ROOT_COPILOT"
    else
        safe_link "$GAL_ROOT_COPILOT" "$GAL_STATE_ROOT"
    fi

    if $UNINSTALL || ! $INSTALL_GEMINI; then
        safe_unlink "$GAL_ROOT_GEMINI"
    else
        safe_link "$GAL_ROOT_GEMINI" "$GAL_SOURCE_ROOT"
    fi

    if $UNINSTALL || ! $INSTALL_ANTIGRAVITY; then
        safe_unlink "$GAL_ROOT_ANTIGRAVITY"
    else
        safe_link "$GAL_ROOT_ANTIGRAVITY" "$GAL_SOURCE_ROOT"
    fi
}

parse_setup_args "$@"
initialize_setup_session
invoke_update_skills