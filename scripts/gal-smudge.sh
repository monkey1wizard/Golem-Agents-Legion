#!/usr/bin/env bash
# gal-smudge.sh — Git smudge filter: replaces <PLACEHOLDER> with real paths
# Called by git on checkout/merge. Reads stdin, writes stdout.
# Requires config.local.env in repo root.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CONFIG="$REPO_ROOT/config.local.env"

# No config → passthrough (placeholder stays as-is for new users)
if [ ! -f "$CONFIG" ]; then
    cat
    exit 0
fi

# Read config into parallel arrays, longest value first for safe replacement
declare -a KEYS=()
declare -a VALS=()

should_filter_key() {
    case "$1" in
        OBSIDIAN_VAULT|OBSIDIAN_VAULT_NAME|OBSIDIAN_GUIDE_PATH|OBSIDIAN_PRIVATE_RESEARCH_DIR|OBSIDIAN_DIARY_DIR|OBSIDIAN_SCRATCH_DIR|OBSIDIAN_ARCHIVE_DIR|LOCAL_SEARCH_PROJECT|GAL_SKILLS|TEMP_DIR|MCP_FILESYSTEM_PATHS)
            return 0
            ;;
        *)
            return 1
            ;;
    esac
}

while IFS= read -r line || [ -n "$line" ]; do
    # Skip comments and blank lines
    [[ "$line" =~ ^[[:space:]]*# ]] && continue
    [[ -z "${line// /}" ]] && continue

    key="${line%%=*}"
    val="${line#*=}"

    should_filter_key "$key" || continue
    # Skip empty values
    [ -z "$val" ] && continue

    KEYS+=("$key")
    VALS+=("$val")
done < "$CONFIG"

# Sort by value length descending (longest first to avoid partial matches)
count=${#KEYS[@]}
if [ "$count" -eq 0 ]; then
    cat
    exit 0
fi

for ((i = 0; i < count; i++)); do
    for ((j = i + 1; j < count; j++)); do
        if [ ${#VALS[$j]} -gt ${#VALS[$i]} ]; then
            tmp_k="${KEYS[$i]}"; KEYS[$i]="${KEYS[$j]}"; KEYS[$j]="$tmp_k"
            tmp_v="${VALS[$i]}"; VALS[$i]="${VALS[$j]}"; VALS[$j]="$tmp_v"
        fi
    done
done

# Read entire file content and track trailing newline
raw="$(cat; echo x)"
raw="${raw%x}"
# Detect trailing newline
if [[ "$raw" == *$'\n' ]]; then
    has_trailing_nl=true
    content="${raw%$'\n'}"
else
    has_trailing_nl=false
    content="$raw"
fi

# Replace placeholders with real values
for ((i = 0; i < count; i++)); do
    placeholder="<${KEYS[$i]}>"
    content="${content//"$placeholder"/"${VALS[$i]}"}"
done

printf '%s' "$content"
if $has_trailing_nl; then
    echo
fi
