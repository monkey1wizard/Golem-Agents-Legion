#!/usr/bin/env bash
# gal-clean.sh — Git clean filter: replaces real paths with <PLACEHOLDER>
# Called by git on add/diff. Reads stdin, writes stdout.
# Requires config.local.env in repo root.
#
# SECURITY: This is the critical path — if this fails, personal paths leak.
# The filter is registered with required=true so git will abort on failure.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CONFIG="$REPO_ROOT/config.local.env"

# No config → abort. Clean without config means we'd commit real paths.
# If smudge ran (working copy has real paths) and clean can't reverse,
# that's a data leak. Fail hard.
if [ ! -f "$CONFIG" ]; then
    echo "gal-clean: ERROR — config.local.env not found. Cannot safely clean." >&2
    echo "Run Setup-Machine to create config.local.env from config.example.env." >&2
    exit 1
fi

# Read config into parallel arrays, longest value first for safe replacement
declare -a KEYS=()
declare -a VALS=()

while IFS= read -r line || [ -n "$line" ]; do
    [[ "$line" =~ ^[[:space:]]*# ]] && continue
    [[ -z "${line// /}" ]] && continue

    key="${line%%=*}"
    val="${line#*=}"

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

# Replace real values with placeholders (reverse of smudge)
for ((i = 0; i < count; i++)); do
    placeholder="<${KEYS[$i]}>"
    content="${content//"${VALS[$i]}"/"$placeholder"}"
done

printf '%s' "$content"
if $has_trailing_nl; then
    echo
fi
