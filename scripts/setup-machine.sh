#!/usr/bin/env bash
# setup-machine.sh — Create symlinks from golem-agents-legion to ~/.copilot/
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
    for dir in "$COPILOT_ROOT" "$AGENTS_TARGET" "$SKILLS_TARGET"; do
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

# --- Summary ---

echo ""
if $UNINSTALL; then
    echo "Uninstall complete."
elif $DRY_RUN; then
    echo "Dry run complete. No changes made."
else
    echo "Setup complete: agents=$agent_ok/$agent_count, skills=$skill_ok/$skill_count"
    if [ "$agent_fail" -gt 0 ] || [ "$skill_fail" -gt 0 ]; then
        echo "Some links failed. Check warnings above."
    fi
fi
