#!/usr/bin/env bash

set -euo pipefail

blank=false
target_path="$PWD"
project_name=""
graphify_version_file_name="GAL_GRAPHIFY_VERSION.txt"

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

command_exists() {
  command -v "$1" >/dev/null 2>&1
}

get_graphify_version() {
  graphify --version 2>/dev/null | head -n 1 | tr -d '\r'
}

write_graphify_version_stamp() {
  local repo_path="$1"
  local version="$2"
  local graph_version_path="$repo_path/graphify-out/$graphify_version_file_name"

  printf '%s\n' "$version" > "$graph_version_path"
}

run_graphify_auto_init() {
  local repo_path="$1"
  local graph_report_path="$repo_path/graphify-out/GRAPH_REPORT.md"
  local graph_version_path="$repo_path/graphify-out/$graphify_version_file_name"
  local current_version=""
  local stamped_version=""

  if [[ ! -f "$graph_report_path" ]]; then
    echo "- Skipped: graphify artifact inspection (no graphify-out/GRAPH_REPORT.md)"
    return
  fi

  echo "- Detected: existing graphify-out/GRAPH_REPORT.md"

  if [[ -f "$graph_version_path" ]] && command_exists graphify; then
    current_version="$(get_graphify_version || true)"
    stamped_version="$(tr -d '\r' < "$graph_version_path")"
    if [[ -n "$current_version" && -n "$stamped_version" && "$current_version" != "$stamped_version" && ! "$graph_report_path" -nt "$graph_version_path" ]]; then
      echo "- Warning: graphify version changed (report: $stamped_version, installed: $current_version)"
      echo "- Next: refresh graphify artifacts manually if you want updated graph context"
    fi
  fi

  if [[ ! -f "$graph_version_path" ]] && command_exists graphify; then
    current_version="$(get_graphify_version || true)"
    if [[ -n "$current_version" ]]; then
      write_graphify_version_stamp "$repo_path" "$current_version"
      echo "- Stamped: graphify-out/$graphify_version_file_name ($current_version)"
    fi
  fi

  echo "- Skipped: graphify auto-run"
}

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
  if [[ ${#tech_hints[@]} -gt 0 ]]; then
    deduped_tech_hints=()
    while IFS= read -r tech; do
      [[ -n "$tech" ]] && deduped_tech_hints+=("$tech")
    done < <(printf '%s\n' "${tech_hints[@]}" | sort -u)
    tech_hints=("${deduped_tech_hints[@]}")
  fi
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
run_graphify_auto_init "$target_path"

echo "Initialized repo context in: $target_path"
echo "- Created: .dev/project.md"
echo "- Created: .dev/state.md"
echo "- Ensured: docs/plans/"
echo "- Generated: .github/copilot-instructions.md"
echo "- Generated: GEMINI.md"
echo "- Generated: CLAUDE.md"
echo "- Generated: AGENTS.md"
echo "- Generated: .agents/rules/gal.md"
echo "- Next: review .dev/project.md, fill in summary fields, then run /gal status"

if [[ ${#source_docs[@]} -gt 0 ]]; then
  echo ""
  echo "Adopt-existing: found ${#source_docs[@]} source document(s)."
  echo "Review .dev/project.md and fill in summaries from discovered docs."
fi

if [[ ${#tech_hints[@]} -gt 0 ]]; then
  echo "Detected tech stack: $(IFS=', '; echo "${tech_hints[*]}")"
fi