#!/usr/bin/env bash

set -euo pipefail

if ! repo_root="$(git rev-parse --show-toplevel 2>/dev/null)"; then
  printf 'Not a git repository.\n'
  exit 0
fi

cd "$repo_root"

mapfile -t name_status < <(git diff --cached --name-status --find-renames)
if [[ ${#name_status[@]} -eq 0 ]]; then
  printf 'No changes staged for commit.\n'
  exit 0
fi

declare -a paths=()
declare -a statuses=()

for line in "${name_status[@]}"; do
  [[ -z "$line" ]] && continue
  IFS=$'\t' read -r status from_path to_path <<<"$line"
  if [[ "$status" == R* && -n "${to_path:-}" ]]; then
    path="$to_path"
  else
    path="$from_path"
  fi

  statuses+=("$status")
  paths+=("$path")
done

diff_text="$(git diff --cached --no-ext-diff || true)"
lower_diff="$(printf '%s' "$diff_text" | tr '[:upper:]' '[:lower:]')"
file_count="$(printf '%s\n' "${paths[@]}" | sort -u | wc -l | tr -d ' ')"

is_rename=false
for status in "${statuses[@]}"; do
  if [[ "$status" == R* ]]; then
    is_rename=true
    break
  fi
done

all_docs=true
for path in "${paths[@]}"; do
  case "$path" in
    *.md|*.mdx|*.txt|*.rst) ;;
    *) all_docs=false ; break ;;
  esac
done

type="refactor"
if [[ "$all_docs" == true ]]; then
  type="docs"
elif printf '%s\n' "${paths[@]}" | grep -Eq '^\.github/(workflows|actions)/'; then
  type="ci"
elif printf '%s\n' "${paths[@]}" | grep -Eq '(^|/)(test|tests)/'; then
  type="test"
elif [[ "$is_rename" == true ]]; then
  type="refactor"
elif printf '%s' "$lower_diff" | grep -Eq 'broken|stale|invalid|repair|fix|reasoning|<think>|code fence|plain text|corrected commit|local model|stabil'; then
  type="fix"
elif paste <(printf '%s\n' "${statuses[@]}") <(printf '%s\n' "${paths[@]}") | grep -Eq '^A[^[:space:]]*[[:space:]]+(commands|agent|skills|workflows|templates)/'; then
  type="feat"
elif printf '%s\n' "${paths[@]}" | grep -Eq '^(scripts/|opencode\.json$|mcp\.json$)'; then
  type="chore"
fi

scope=''
if printf '%s\n' "${paths[@]}" | grep -Eq '^opencode\.json$'; then
  scope='opencode'

else
  command_names="$(printf '%s\n' "${paths[@]}" | sed -n 's#^commands/\([^/]*\)/.*#\1#p' | sort -u)"
  skill_names="$(printf '%s\n' "${paths[@]}" | sed -n 's#^skills/\([^/]*\)/.*#\1#p' | sort -u)"

  if [[ -n "$command_names" && "$(printf '%s\n' "$command_names" | wc -l)" -eq 1 ]]; then
    scope="$command_names"
  elif [[ -n "$skill_names" && "$(printf '%s\n' "$skill_names" | wc -l)" -eq 1 ]]; then
    scope="$skill_names"
  else
    top_counts="$(printf '%s\n' "${paths[@]}" | awk -F/ '{print tolower($1)}' | sort | uniq -c | sort -nr | head -n1 | awk '{print $2}')"
    case "$top_counts" in
      agent) scope='agents' ;;
      '') scope='' ;;
      *) scope="$top_counts" ;;
    esac
  fi
fi

subject='update repo workflow'
has_git_commit=false
if printf '%s' "$lower_diff" | grep -Eq 'git-commit-msg|git-commit|git-commits' || printf '%s\n' "${paths[@]}" | grep -Eq 'git-commit-msg|git-commit|git-commits'; then
  has_git_commit=true
fi

mentions_local=false
if printf '%s' "$lower_diff" | grep -Eq 'local model|<think>|code fence|reasoning'; then
  mentions_local=true
fi

if [[ "$is_rename" == true && -n "$scope" ]]; then
  subject="rename $scope"
elif [[ "$has_git_commit" == true ]]; then
  case "$type" in
    feat)
      if [[ "$mentions_local" == true ]]; then
        subject='add dynamic git-commit-msg flow for local models'
      else
        subject='add dynamic git-commit-msg command flow'
      fi
      ;;
    fix) subject='stabilize git-commit-msg output for local models' ;;
    docs) subject='document git-commit-msg trigger handling' ;;
    refactor) subject='refactor git-commit-msg generation flow' ;;
    chore) subject='update git-commit-msg command wiring' ;;
    *) subject='update git-commit-msg generation' ;;
  esac
else
  case "$type" in
    docs) subject="document ${scope:-repo workflows}" ;;
    feat) subject="add ${scope:-repo workflow} support" ;;
    fix) subject="stabilize ${scope:-repo workflow}" ;;
    refactor) subject="refactor ${scope:-repo workflow}" ;;
    test) subject="cover ${scope:-repo workflow}" ;;
    ci) subject="adjust ${scope:-ci workflow}" ;;
    *) subject="update ${scope:-repo workflow}" ;;
  esac
fi

if [[ -n "$scope" ]]; then
  printf '%s(%s): %s\n' "$type" "$scope" "$subject"
else
  printf '%s: %s\n' "$type" "$subject"
fi

declare -a bullets=()
if printf '%s\n' "${paths[@]}" | grep -Eq '^commands/git-commit-msg/'; then
  bullets+=('add a source-of-truth git-commit-msg command under commands/')
elif printf '%s\n' "${paths[@]}" | grep -Eq '^opencode\.json$'; then
  bullets+=('route the repo OpenCode git-commit-msg command through staged helper output')
elif printf '%s' "$lower_diff" | grep -Eq '<think>|code fence|plain text|reasoning'; then
  bullets+=('block reasoning-tag and fenced-output regressions in commit responses')
fi

if [[ "$file_count" -gt 3 && ${#bullets[@]} -gt 0 ]]; then
  printf '\n'
  printf -- '- %s\n' "${bullets[@]:0:3}"
fi