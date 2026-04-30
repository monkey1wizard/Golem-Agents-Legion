#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
. "$SCRIPT_DIR/common/common.sh"

invoke_update_skills() {
    ensure_setup_directories \
        "$COPILOT_ROOT" \
        "$AGENTS_TARGET" \
        "$SKILLS_TARGET" \
        "$SHARED_AGENTS_ROOT" \
        "$SHARED_SKILLS_TARGET" \
        "$GEMINI_ROOT" \
        "$GEMINI_SKILLS_TARGET" \
        "$CODEX_ROOT" \
        "$CODEX_SKILLS_TARGET" \
        "$CLAUDE_ROOT" \
        "$CLAUDE_SKILLS_TARGET"

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
        if $UNINSTALL || ! $INSTALL_COPILOT; then
            safe_unlink "$link_path"
        else
            safe_link "$link_path" "$agent_file"
        fi
    done

    local skill_dirs=()
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
        if $UNINSTALL || ! $INSTALL_COPILOT; then
            safe_unlink "$link_path"
        else
            safe_link "$link_path" "$skill_dir"
        fi
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

    echo ''
    echo "=== Shared Skills - Gemini + Codex (${#skill_dirs[@]} reusable directories via .agents) ==="
    for skill_dir in "${skill_dirs[@]}"; do
        skill_name="$(basename "$skill_dir")"
        link_path="$SHARED_SKILLS_TARGET/$skill_name"
        if $UNINSTALL || ! $INSTALL_SHARED_SKILLS; then
            safe_unlink "$link_path"
        else
            safe_link "$link_path" "$skill_dir"
        fi
    done

    echo ''
    echo "=== Claude Skills (${#skill_dirs[@]} reusable directories) ==="
    for skill_dir in "${skill_dirs[@]}"; do
        skill_name="$(basename "$skill_dir")"
        link_path="$CLAUDE_SKILLS_TARGET/$skill_name"
        if $UNINSTALL || ! $INSTALL_CLAUDE; then
            safe_unlink "$link_path"
        else
            safe_link "$link_path" "$skill_dir"
        fi
    done

    echo ''
    echo '=== GAL_ROOT symlinks ==='
    if $UNINSTALL || ! $INSTALL_COPILOT; then
        safe_unlink "$GAL_ROOT_COPILOT"
    else
        safe_link "$GAL_ROOT_COPILOT" "$REPO_ROOT"
    fi

    if $UNINSTALL || ! $INSTALL_GEMINI; then
        safe_unlink "$GAL_ROOT_GEMINI"
    else
        safe_link "$GAL_ROOT_GEMINI" "$REPO_ROOT"
    fi
}

parse_setup_args "$@"
initialize_setup_session
invoke_update_skills