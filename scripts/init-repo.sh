#!/usr/bin/env bash

set -euo pipefail

target_path="${1:-$PWD}"
project_name="${2:-$(basename "$target_path")}"

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"

if [[ ! -d "$target_path" ]]; then
  echo "Target path does not exist: $target_path" >&2
  exit 1
fi

mkdir -p "$target_path/.dev"
mkdir -p "$target_path/docs/plans"

project_template="$repo_root/templates/project.md"
state_template="$repo_root/templates/state.md"

if [[ ! -f "$project_template" || ! -f "$state_template" ]]; then
  echo "Missing project or state template under $repo_root/templates" >&2
  exit 1
fi

project_target="$target_path/.dev/project.md"
state_target="$target_path/.dev/state.md"

if [[ -f "$project_target" || -f "$state_target" ]]; then
  echo "Target repo already contains .dev files. Remove them first or extend this script with force handling." >&2
  exit 1
fi

sed "s/^# \[Project Name\]/# $project_name/" "$project_template" > "$project_target"
cp "$state_template" "$state_target"

echo "Initialized repo context in: $target_path"
echo "- Created: .dev/project.md"
echo "- Created: .dev/state.md"
echo "- Ensured: docs/plans/"