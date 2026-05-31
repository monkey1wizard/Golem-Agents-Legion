#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
. "$SCRIPT_DIR/common/common.sh"

get_baked_command_skill_content() {
    local template_path="$1"
    local local_override_path="$2"
    local baked

    baked="$(sed "s|{{GAL_ROOT}}|$REPO_ROOT|g" "$template_path")"
    if [ ! -f "$local_override_path" ] || ! [ -s "$local_override_path" ]; then
        printf '%s\n' "$baked"
        return 0
    fi

    printf '%s\n\n' "$baked"
    printf '%s\n' '<!-- GAL LOCAL OVERRIDE START -->'
    printf '%s\n' '<!-- Source: SKILL.local.md (gitignored machine-local overlay) -->'
    cat "$local_override_path"
    printf '\n%s\n' '<!-- GAL LOCAL OVERRIDE END -->'
}

get_skill_frontmatter_description() {
    local skill_path="$1"
    local description

    description="$(awk '
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
            if ($0 ~ /^description:[[:space:]]*/) {
                sub(/^description:[[:space:]]*/, "", $0)
                print $0
                exit
            }
            next
        }
    ' "$skill_path")"

    description="${description#\"}"
    description="${description%\"}"
    printf '%s' "$description"
}

get_skill_markdown_body() {
    local skill_path="$1"
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
    ' "$skill_path"
}

escape_toml_basic_string() {
    printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g'
}

toml_multiline_literal_string() {
    printf "'''\n"
    printf '%s' "$1"
    printf "\n'''"
}

new_gemini_command_file_content() {
    local skill_path="$1"
    local description body prompt

    description="$(get_skill_frontmatter_description "$skill_path")"
    [ -n "$description" ] || description='GAL command'

    body="$(get_skill_markdown_body "$skill_path")"
    prompt="$(printf 'User command arguments, if any: {{args}}\n\n%s' "$body")"

    {
        printf '%s\n' "$GAL_MANAGED_FILE_HEADER"
        printf 'description = "%s"\n' "$(escape_toml_basic_string "$description")"
        printf 'prompt = '
        toml_multiline_literal_string "$prompt"
        printf '\n'
    }
}

new_claude_command_file_content() {
    local skill_path="$1"
    local command_name="$2"
    local description body

    description="$(get_skill_frontmatter_description "$skill_path")"
    [ -n "$description" ] || description='GAL command'
    body="$(get_skill_markdown_body "$skill_path")"

    {
        printf '%s\n' "$GAL_MANAGED_FILE_HEADER"
        printf '%s\n' '---'
        printf '%s\n' 'description: |'
        while IFS= read -r line; do
            printf '  %s\n' "$line"
        done <<< "$description"
        printf '%s\n\n' '---'
        printf '# %s\n\n' "$command_name"
        printf '%s\n\n' 'User command arguments, if any: {{args}}'
        printf '%s\n' "$body"
    }
}

new_opencode_command_file_content() {
    local skill_path="$1"
    local command_name="$2"
    local description body

    description="$(get_skill_frontmatter_description "$skill_path")"
    [ -n "$description" ] || description='GAL command'
    if [ "$command_name" = 'git-commit-msg' ]; then
        body="Run the repo helper below and return its output exactly. The helper decides whether the output is header-only or includes a body, so do not invent bullets or rewrite the summary. Do not add explanations, markdown fences, reasoning tags, JSON, or any extra prose. If the helper reports No changes staged for commit. or Not a git repository., return that text exactly. Apply extra instructions if provided: \$ARGUMENTS

!\`pwsh -NoProfile -File ./scripts/Get-StagedCommitMessage.ps1\`"
    else
        body="$(get_skill_markdown_body "$skill_path")"
    fi

    {
        printf '%s\n' "$GAL_MANAGED_FILE_HEADER"
        printf '%s\n' '---'
        printf '%s\n' 'description: |'
        while IFS= read -r line; do
            printf '  %s\n' "$line"
        done <<< "$description"
        printf '%s\n\n' '---'
        printf '%s\n\n' 'User command arguments, if any: $ARGUMENTS'
        printf '%s\n' "$body"
    }
}

invoke_update_commands() {
    ensure_setup_directories \
        "$SKILLS_TARGET" \
        "$CODEX_SKILLS_TARGET" \
        "$GEMINI_COMMANDS_TARGET" \
        "$ANTIGRAVITY_SKILLS_TARGET" \
        "$OPENCODE_COMMANDS_TARGET" \
        "$SHARED_SKILLS_TARGET"

    echo ''
    echo '=== Generated GAL command skills ==='
    local command_skill_name command_skill_source command_skill_template local_override_path baked_skill baked
    if $UNINSTALL || ! $NEEDS_BAKED_COMMAND_SKILLS; then
        for command_skill_name in "${COMMAND_SKILL_NAMES[@]}"; do
            baked_skill="$REPO_ROOT/commands/$command_skill_name/SKILL.md"
            [ -f "$baked_skill" ] || continue
            if $DRY_RUN; then
                echo "  [DRY RUN] Would remove baked: $baked_skill"
            else
                rm "$baked_skill"
                echo "  [REMOVED] $baked_skill"
            fi
        done
    else
        for command_skill_name in "${COMMAND_SKILL_NAMES[@]}"; do
            command_skill_source="$REPO_ROOT/commands/$command_skill_name"
            command_skill_template="$command_skill_source/SKILL.template.md"
            local_override_path="$command_skill_source/SKILL.local.md"
            if [ ! -f "$command_skill_template" ]; then
                echo "  [WARN] Template not found: $command_skill_template"
                continue
            fi

            baked_skill="$command_skill_source/SKILL.md"
            if $DRY_RUN; then
                echo "  [DRY RUN] Would write baked: $baked_skill"
            else
                baked="$(get_baked_command_skill_content "$command_skill_template" "$local_override_path")"
                printf '%s\n' "$baked" > "$baked_skill"
                echo "  [OK] $baked_skill"
            fi
        done
    fi

    echo ''
    echo '=== GAL command skill symlinks (Copilot + Codex) ==='
    local copilot_target codex_target
    for command_skill_name in "${COMMAND_SKILL_NAMES[@]}"; do
        command_skill_source="$REPO_ROOT/commands/$command_skill_name"
        copilot_target="$SKILLS_TARGET/$command_skill_name"
        codex_target="$CODEX_SKILLS_TARGET/$command_skill_name"

        if $UNINSTALL || ! $INSTALL_COPILOT; then
            safe_unlink "$copilot_target"
        else
            safe_link "$copilot_target" "$command_skill_source"
        fi

        if $UNINSTALL || ! $INSTALL_CODEX; then
            safe_unlink "$codex_target"
        else
            safe_link "$codex_target" "$command_skill_source"
        fi
    done

    echo ''
    echo '=== AGY command skill legacy cleanup ==='
    # AGY command skills are now rendered by build-core-plugin.sh (called from update-skills).
    # This section only cleans up legacy symlinks that predate the plugin model.
    local antigravity_target
    for command_skill_name in "${COMMAND_SKILL_NAMES[@]}"; do
        antigravity_target="$ANTIGRAVITY_SKILLS_TARGET/$command_skill_name"
        safe_unlink "$antigravity_target"
    done

    echo ''
    echo '=== Migration: repo .agents command cleanup ==='
    local workspace_target
    for command_skill_name in "${COMMAND_SKILL_NAMES[@]}"; do
        workspace_target="$WORKSPACE_SKILLS_TARGET/$command_skill_name"
        safe_unlink "$workspace_target"
    done

    echo ''
    echo '=== Migration: ~/.agents command cleanup ==='
    local shared_command_path
    for command_skill_name in "${COMMAND_SKILL_NAMES[@]}"; do
        shared_command_path="$SHARED_SKILLS_TARGET/$command_skill_name"
        [ -e "$shared_command_path" ] || continue
        if ! is_gal_repo_link "$shared_command_path"; then
            echo "  [SKIP] User-owned shared skill preserved: $shared_command_path"
            continue
        fi
        if $DRY_RUN; then
            echo "  [DRY RUN] Would remove: $shared_command_path"
        else
            rm "$shared_command_path"
            echo "  [REMOVED] $shared_command_path"
        fi
    done

    echo ''
    echo '=== Gemini custom commands ==='
    local command_file skill_path command_content
    for command_skill_name in "${COMMAND_SKILL_NAMES[@]}"; do
        command_file="$GEMINI_COMMANDS_TARGET/$command_skill_name.toml"
        if $UNINSTALL || ! $INSTALL_GEMINI; then
            [ -f "$command_file" ] || continue
            if ! is_gal_managed_file "$command_file"; then
                echo "  [SKIP] User-owned Gemini command preserved: $command_file"
                continue
            fi
            if $DRY_RUN; then
                echo "  [DRY RUN] Would remove: $command_file"
            else
                rm "$command_file"
                echo "  [REMOVED] $command_file"
            fi
        else
            skill_path="$REPO_ROOT/commands/$command_skill_name/SKILL.md"
            if $DRY_RUN; then
                echo "  [DRY RUN] Would write: $command_file"
            else
                command_content="$(new_gemini_command_file_content "$skill_path")"
                printf '%s\n' "$command_content" > "$command_file"
                echo "  [OK] $command_file"
            fi
        fi
    done
    if ! $UNINSTALL && $INSTALL_GEMINI; then
        echo '  [NOTE] Reload active Gemini sessions with /commands reload or restart Gemini CLI to pick up updated GAL commands.'
    fi

    echo ''
    echo '=== Claude legacy command cleanup ==='
    local claude_commands_target="$HOME/.claude/commands"
    for command_skill_name in "${COMMAND_SKILL_NAMES[@]}"; do
        command_file="$claude_commands_target/$command_skill_name.md"
        [ -f "$command_file" ] || continue
        if ! is_gal_managed_file "$command_file"; then
            echo "  [SKIP] User-owned Claude command preserved: $command_file"
            continue
        fi
        if $DRY_RUN; then
            echo "  [DRY RUN] Would remove: $command_file"
        else
            rm "$command_file"
            echo "  [REMOVED] $command_file"
        fi
    done

    echo ''
    echo '=== OpenCode custom commands ==='
    for command_skill_name in "${COMMAND_SKILL_NAMES[@]}"; do
        command_file="$OPENCODE_COMMANDS_TARGET/$command_skill_name.md"
        if $UNINSTALL || ! $INSTALL_OPENCODE; then
            [ -f "$command_file" ] || continue
            if ! is_gal_managed_file "$command_file"; then
                echo "  [SKIP] User-owned OpenCode command preserved: $command_file"
                continue
            fi
            if $DRY_RUN; then
                echo "  [DRY RUN] Would remove: $command_file"
            else
                rm "$command_file"
                echo "  [REMOVED] $command_file"
            fi
        else
            skill_path="$REPO_ROOT/commands/$command_skill_name/SKILL.md"
            if $DRY_RUN; then
                echo "  [DRY RUN] Would write: $command_file"
            else
                command_content="$(new_opencode_command_file_content "$skill_path" "$command_skill_name")"
                printf '%s\n' "$command_content" > "$command_file"
                echo "  [OK] $command_file"
            fi
        fi
    done
    if ! $UNINSTALL && $INSTALL_OPENCODE; then
        echo '  [NOTE] Restart OpenCode or reload its command surface to pick up updated GAL commands.'
    fi

    echo ''
    echo '=== Migration: obsolete command cleanup ==='
    local skills_dir dir_name keep_dir keep_file existing_dir existing_file
    for skills_dir in "$SKILLS_TARGET" "$GEMINI_SKILLS_TARGET" "$ANTIGRAVITY_SKILLS_TARGET" "$SHARED_SKILLS_TARGET" "$CODEX_SKILLS_TARGET"; do
        for existing_dir in "$skills_dir"/*; do
            [ -e "$existing_dir" ] || continue
            [ -d "$existing_dir" ] || continue
            dir_name="$(basename "$existing_dir")"
            keep_dir=false
            if contains_value "$dir_name" "${COMMAND_SKILL_NAMES[@]}"; then
                keep_dir=true
            fi
            if $keep_dir || ! is_gal_command_link "$existing_dir"; then
                continue
            fi
            if $DRY_RUN; then
                echo "  [DRY RUN] Would remove obsolete command link: $existing_dir"
            else
                rm -rf "$existing_dir"
                echo "  [REMOVED] Obsolete command link: $existing_dir"
            fi
        done
    done

    for existing_file in "$GEMINI_COMMANDS_TARGET"/*.toml; do
        [ -e "$existing_file" ] || continue
        keep_file=false
        if contains_value "$(basename "$existing_file" .toml)" "${COMMAND_SKILL_NAMES[@]}"; then
            keep_file=true
        fi
        if $keep_file || ! is_gal_managed_file "$existing_file"; then
            continue
        fi
        if $DRY_RUN; then
            echo "  [DRY RUN] Would remove obsolete Gemini command: $existing_file"
        else
            rm "$existing_file"
            echo "  [REMOVED] Obsolete Gemini command: $existing_file"
        fi
    done

    for existing_file in "$claude_commands_target"/*.md; do
        [ -e "$existing_file" ] || continue
        keep_file=false
        if contains_value "$(basename "$existing_file" .md)" "${COMMAND_SKILL_NAMES[@]}"; then
            keep_file=true
        fi
        if $keep_file || ! is_gal_managed_file "$existing_file"; then
            continue
        fi
        if $DRY_RUN; then
            echo "  [DRY RUN] Would remove obsolete Claude command: $existing_file"
        else
            rm "$existing_file"
            echo "  [REMOVED] Obsolete Claude command: $existing_file"
        fi
    done

    for existing_file in "$OPENCODE_COMMANDS_TARGET"/*.md; do
        [ -e "$existing_file" ] || continue
        keep_file=false
        if contains_value "$(basename "$existing_file" .md)" "${COMMAND_SKILL_NAMES[@]}"; then
            keep_file=true
        fi
        if $keep_file || ! is_gal_managed_file "$existing_file"; then
            continue
        fi
        if $DRY_RUN; then
            echo "  [DRY RUN] Would remove obsolete OpenCode command: $existing_file"
        else
            rm "$existing_file"
            echo "  [REMOVED] Obsolete OpenCode command: $existing_file"
        fi
    done

    echo ''
    echo '=== Migration: gal-* cleanup ==='
    for skills_dir in "$SKILLS_TARGET" "$GEMINI_SKILLS_TARGET" "$ANTIGRAVITY_SKILLS_TARGET" "$SHARED_SKILLS_TARGET" "$CODEX_SKILLS_TARGET"; do
        for existing_dir in "$skills_dir"/gal-*/; do
            [ -e "$existing_dir" ] || continue
            dir_name="$(basename "$existing_dir")"
            if contains_value "$dir_name" "${COMMAND_SKILL_NAMES[@]}"; then
                continue
            fi
            if $DRY_RUN; then
                echo "  [DRY RUN] Would remove: $existing_dir"
            else
                rm -rf "$existing_dir"
                echo "  [REMOVED] $existing_dir"
            fi
        done
    done
}

parse_setup_args "$@"
initialize_setup_session
invoke_update_commands