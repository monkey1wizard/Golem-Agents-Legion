#!/usr/bin/env bash
# setup-machine.sh — Create symlinks from golem-agents-legion to ~/.copilot/ and ~/.gemini/
#
# Links:
#   agent/*.agent.md  -> ~/.copilot/agents/*.agent.md
#   skills/*/         -> ~/.copilot/skills/*/
#   skills/*/         -> ~/.gemini/skills/*/ (Gemini CLI)
#   commands/gal/     -> ~/.copilot/skills/gal/ + ~/.gemini/skills/gal/ (baked dispatcher skill)
#   <repo root>       -> ~/.copilot/gal/ + ~/.gemini/gal/ (GAL_ROOT dir symlinks)
#   Generates commands/gal/SKILL.md from SKILL.template.md (baked absolute paths)
#   Generates ~/.gemini/gal-context.md (@file skill imports)
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

GAL_SOURCE="$REPO_ROOT/commands/gal"
GAL_ROOT_COPILOT="$COPILOT_ROOT/gal"
GAL_ROOT_GEMINI="$GEMINI_ROOT/gal"
GAL_SKILL_COPILOT="$SKILLS_TARGET/gal"
GAL_SKILL_GEMINI="$GEMINI_SKILLS_TARGET/gal"
SKILL_TEMPLATE="$GAL_SOURCE/SKILL.template.md"
COMMAND_ALIAS_NAMES=(gal-init gal-plan gal-status gal-next gal-pause)
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
    for dir in "$COPILOT_ROOT" "$AGENTS_TARGET" "$SKILLS_TARGET" "$GEMINI_ROOT" "$GEMINI_SKILLS_TARGET"; do
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

# --- Gemini Skill symlinks ---

gemini_skill_ok=0
gemini_skill_fail=0

echo ""
echo "=== Gemini Skills ($skill_count directories) ==="

for d in "${skill_dirs[@]}"; do
    name="$(basename "$d")"
    link_path="$GEMINI_SKILLS_TARGET/$name"

    if $UNINSTALL; then
        safe_unlink "$link_path"
    else
        if safe_link "$link_path" "$d"; then
            ((gemini_skill_ok++)) || true
        else
            ((gemini_skill_fail++)) || true
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
        context_lines+="@$GEMINI_SKILLS_TARGET/$command_skill_name/SKILL.md"$'\n'
    done
    while IFS= read -r -d '' sd; do
        sname="$(basename "$sd")"
        context_lines+="@$GEMINI_SKILLS_TARGET/$sname/SKILL.md"$'\n'
    done < <(find "$REPO_ROOT/skills" -mindepth 1 -maxdepth 1 -type d -print0 | sort -z)

    if $DRY_RUN; then
        echo "  [DRY RUN] Would write: $GEMINI_CONTEXT_FILE ($((skill_count + ${#COMMAND_SKILL_NAMES[@]})) skill imports)"
    else
        printf '%s' "$context_lines" > "$GEMINI_CONTEXT_FILE"
        echo "  [OK] $GEMINI_CONTEXT_FILE ($((skill_count + ${#COMMAND_SKILL_NAMES[@]})) skill imports)"
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

# --- GAL command skill symlinks (commands/gal*/ -> ~/.copilot/skills/gal*/ + ~/.gemini/skills/gal*/) ---

echo ""
echo "=== GAL command skill symlinks ==="

if $UNINSTALL; then
    for command_skill_name in "${COMMAND_SKILL_NAMES[@]}"; do
        safe_unlink "$SKILLS_TARGET/$command_skill_name"
        safe_unlink "$GEMINI_SKILLS_TARGET/$command_skill_name"
    done
else
    for command_skill_name in "${COMMAND_SKILL_NAMES[@]}"; do
        command_skill_source="$REPO_ROOT/commands/$command_skill_name"
        safe_link "$SKILLS_TARGET/$command_skill_name" "$command_skill_source"
        safe_link "$GEMINI_SKILLS_TARGET/$command_skill_name" "$command_skill_source"
    done
fi

# --- Migration: remove legacy gal-* dirs from installed locations ---

echo ""
echo "=== Migration: gal-* cleanup ==="

for skills_dir in "$SKILLS_TARGET" "$GEMINI_SKILLS_TARGET"; do
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
    echo "Setup complete: agents=$agent_ok/$agent_count, skills(copilot)=$skill_ok/$skill_count, skills(gemini)=$gemini_skill_ok/$skill_count, gal-root=$gal_root_ok/2"
    echo "Note: If SKILL.template.md changes, re-run setup-machine.sh --replace to regenerate."
    if [ "$agent_fail" -gt 0 ] || [ "$skill_fail" -gt 0 ]; then
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
