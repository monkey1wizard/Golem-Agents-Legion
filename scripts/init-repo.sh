#!/usr/bin/env bash

set -euo pipefail

blank=false
target_path="$PWD"
project_name=""

# Parse arguments
while [[ $# -gt 0 ]]; do
  case "$1" in
    --blank)
      blank=true
      shift
      ;;
    *)
      if [[ -z "$target_path" || "$target_path" == "$PWD" ]] && [[ -d "$1" ]]; then
        target_path="$1"
      elif [[ -z "$project_name" ]]; then
        project_name="$1"
      fi
      shift
      ;;
  esac
done

if [[ -z "$project_name" ]]; then
  project_name="$(basename "$target_path")"
fi

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
  echo "Target repo already contains .dev files. Remove them first or use --force." >&2
  exit 1
fi

# --- Adopt-existing: scan for existing documentation ---
source_docs=()
tech_hints=()

if [[ "$blank" != "true" ]]; then
  # Scan for README variants
  for name in README.md README README.rst README.txt; do
    if [[ -f "$target_path/$name" ]]; then
      source_docs+=("$name|README")
    fi
  done

  # Scan docs/ directory
  if [[ -d "$target_path/docs" ]]; then
    while IFS= read -r -d '' f; do
      rel="${f#"$target_path/"}"
      source_docs+=("$rel|docs")
    done < <(find "$target_path/docs" -type f \( -name '*.md' -o -name '*.rst' -o -name '*.txt' \) -print0 2>/dev/null)
  fi

  # Detect tech stack
  detect_stack() {
    local pattern="$1" tech="$2"
    if find "$target_path" -maxdepth 3 -name "$pattern" -print -quit 2>/dev/null | grep -q .; then
      tech_hints+=("$tech")
    fi
  }
  detect_stack "*.csproj"        ".NET"
  detect_stack "*.sln"           ".NET"
  detect_stack "package.json"    "Node.js"
  detect_stack "go.mod"          "Go"
  detect_stack "Cargo.toml"      "Rust"
  detect_stack "pyproject.toml"  "Python"
  detect_stack "requirements.txt" "Python"
  detect_stack "Gemfile"         "Ruby"
  detect_stack "pom.xml"         "Java/Maven"
  # Deduplicate
  mapfile -t tech_hints < <(printf '%s\n' "${tech_hints[@]}" | sort -u)
fi

# --- Generate project.md ---
project_content="$(cat "$project_template")"
project_content="${project_content//\# \[Project Name\]/# $project_name}"

# Inject discovered source documents
if [[ ${#source_docs[@]} -gt 0 ]]; then
  doc_table="| Path | Type | Notes |\n| --- | --- | --- |"
  for entry in "${source_docs[@]}"; do
    IFS='|' read -r path type <<< "$entry"
    doc_table="$doc_table\n| \`$path\` | $type | |"
  done
  project_content="${project_content//\[Index of source docs...\]/$doc_table}"
fi

# Inject detected tech stack
if [[ ${#tech_hints[@]} -gt 0 ]]; then
  stack_line="$(IFS=', '; echo "${tech_hints[*]}")"
  project_content="${project_content//\[Language \/ framework \/ major libs \/ infrastructure\]/$stack_line}"
fi

echo -e "$project_content" > "$project_target"
cp "$state_template" "$state_target"

"$script_dir/sync-dev-context.sh" "$target_path"

echo "Initialized repo context in: $target_path"
echo "- Created: .dev/project.md"
echo "- Created: .dev/state.md"
echo "- Ensured: docs/plans/"
echo "- Generated: .github/copilot-instructions.md"
echo "- Generated: GEMINI.md"
echo "- Generated: AGENTS.md"
echo "- Next: review .dev/project.md, fill in summary fields, then run /gal status"

if [[ ${#source_docs[@]} -gt 0 ]]; then
  echo ""
  echo "Adopt-existing: found ${#source_docs[@]} source document(s)."
  echo "Review .dev/project.md and fill in summaries from discovered docs."
fi

if [[ ${#tech_hints[@]} -gt 0 ]]; then
  echo "Detected tech stack: $(IFS=', '; echo "${tech_hints[*]}")"
fi