#!/usr/bin/env bash
# Report translation freshness for docs/i18n/<lang>/<name>.<lang>.md against canonical sources.
#
# For every translation under docs/i18n/, read front-matter (source, lang, source_commit)
# and compare source_commit against the source's latest commit. Reports one row per (doc, lang):
#   current  source_commit hash matches the source's latest commit
#   stale    source_commit missing / not a hash (e.g. PENDING) / differs from source
#   missing  an allowlisted (doc, lang) pair has no translation file
#
# Report-only: exits 0. Policy lives in docs/devguide.md#documentation-conventions;
# on-location guide is docs/i18n/guide.md.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
i18n_root="$repo_root/docs/i18n"
allowlist=("README.md" "docs/manual.md")

[ -d "$i18n_root" ] || { echo "No docs/i18n/ tree found; nothing to check."; exit 0; }

fm_value() { # $1=file $2=key  -> prints value from YAML front-matter
  awk -v key="$2" '
    NR==1 && $0!="---" { exit }
    NR==1 { infm=1; next }
    infm && $0=="---" { exit }
    infm {
      line=$0; sub(/^[ \t]*/,"",line)
      if (line ~ "^" key "[ \t]*:") { sub("^" key "[ \t]*:[ \t]*","",line); print line; exit }
    }' "$1"
}

declare -A seen
printf '%-34s %-8s %-9s %s\n' "DOC" "LANG" "STATUS" "NOTE"
c_current=0; c_stale=0; c_missing=0

while IFS= read -r tx; do
  base="$(basename "$tx")"; [ "$base" = "guide.md" ] && continue
  rel="${tx#$repo_root/}"
  source="$(fm_value "$tx" source)"; lang="$(fm_value "$tx" lang)"; stamp="$(fm_value "$tx" source_commit)"
  if [ -z "$source" ]; then
    printf '%-34s %-8s %-9s %s\n' "(unknown)" "?" "stale" "no front-matter source"; c_stale=$((c_stale+1)); continue
  fi
  seen["$source|$lang"]=1
  if [ ! -f "$repo_root/$source" ]; then
    printf '%-34s %-8s %-9s %s\n' "$source" "$lang" "stale" "source missing"; c_stale=$((c_stale+1)); continue
  fi
  actual="$(git -C "$repo_root" log -1 --format=%H -- "$source" | tr -d '[:space:]')"
  if [[ "$stamp" =~ ^[0-9a-f]{7,40}$ ]] && { [[ "$actual" == "$stamp"* ]] || [[ "$stamp" == "$actual"* ]]; }; then
    printf '%-34s %-8s %-9s %s\n' "$source" "$lang" "current" ""; c_current=$((c_current+1))
  else
    if [[ "$stamp" =~ ^[0-9a-f]{7,40}$ ]]; then note="source advanced to ${actual:0:7}"; else note="unstamped ($stamp)"; fi
    printf '%-34s %-8s %-9s %s\n' "$source" "$lang" "stale" "$note"; c_stale=$((c_stale+1))
  fi
done < <(find "$i18n_root" -type f -name '*.md' | sort)

langs=$(find "$i18n_root" -mindepth 1 -maxdepth 1 -type d -exec basename {} \;)
for src in "${allowlist[@]}"; do
  for lang in $langs; do
    if [ -z "${seen["$src|$lang"]:-}" ]; then
      printf '%-34s %-8s %-9s %s\n' "$src" "$lang" "missing" "allowlisted, no translation"; c_missing=$((c_missing+1))
    fi
  done
done

echo "Summary: current=$c_current  stale=$c_stale  missing=$c_missing"
