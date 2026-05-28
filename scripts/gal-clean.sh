#!/usr/bin/env bash
# gal-clean.sh — Git clean filter: replaces real paths with <PLACEHOLDER>
# Called by git on add/diff. Reads stdin, writes stdout.
# Uses ~/.gal/config/config.local.env as the primary source, with an explicit
# repo-root legacy fallback for transitional setups.
#
# SECURITY: This is the critical path — if this fails, personal paths leak.
# The filter is registered with required=true so git will abort on failure.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
PRIMARY_CONFIG="$HOME/.gal/config/config.local.env"
LEGACY_CONFIG="$REPO_ROOT/config.local.env"
CONFIG="$PRIMARY_CONFIG"

if [ ! -f "$CONFIG" ] && [ -f "$LEGACY_CONFIG" ]; then
    CONFIG="$LEGACY_CONFIG"
fi

raw="$(cat; echo x)"
raw="${raw%x}"

if [[ "$raw" == *$'\n' ]]; then
    has_trailing_nl=true
    content="${raw%$'\n'}"
else
    has_trailing_nl=false
    content="$raw"
fi

if [ ! -f "$CONFIG" ]; then
    suspicious_path_regex='([A-Za-z]:[\\/]|/Users/|/home/|/Volumes/|\\\\)'
    suspicious_secret_regex='(API_KEY|TOKEN|SECRET|PASSWORD)[[:space:]]*[:=][[:space:]]*[^<[:space:]]+'

    if printf '%s' "$content" | grep -Eq "$suspicious_path_regex|$suspicious_secret_regex"; then
        echo "gal-clean: ERROR — no config source found at $PRIMARY_CONFIG or legacy fallback $LEGACY_CONFIG." >&2
        echo "gal-clean: Refusing to clean content that appears to contain machine-local paths or secret-like values." >&2
        exit 1
    fi

    echo "gal-clean: WARN — no config source found; passing through placeholder-only content unchanged." >&2
    printf '%s' "$content"
    if $has_trailing_nl; then
        echo
    fi
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
    [[ "$line" =~ ^[[:space:]]*# ]] && continue
    [[ -z "${line// /}" ]] && continue

    key="${line%%=*}"
    val="${line#*=}"

    should_filter_key "$key" || continue
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

# Replace real values with placeholders (reverse of smudge)
for ((i = 0; i < count; i++)); do
    placeholder="<${KEYS[$i]}>"
    content="${content//"${VALS[$i]}"/"$placeholder"}"
done

printf '%s' "$content"
if $has_trailing_nl; then
    echo
fi
