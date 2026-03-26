#!/usr/bin/env bash

set -euo pipefail

show_usage() {
  cat <<'EOF'
Usage: gal <command> [args]

Commands:
  init [targetPath] [projectName]   Initialize .dev/ and docs/plans/
  plan [-t <type>] <name>           Create a draft plan file from template
  status                            Show current workflow status from .dev/state.md
  next                              Show the next step from .dev/state.md
  pause                             Commit .dev/ context for worktree handoff
  sync                              Run sync-dev-context.sh if available
  ask <agent>                       Describe adapter-level direct consult command
  run <agent>                       Describe adapter-level utility invocation
EOF
}

to_slug() {
  local value
  value="$(echo "$1" | tr '[:upper:]' '[:lower:]' | sed -E 's/[^a-z0-9]+/-/g; s/^-+//; s/-+$//')"
  if [[ -z "$value" ]]; then
    echo "Could not derive slug from input: $1" >&2
    exit 1
  fi
  echo "$value"
}

require_state() {
  if [[ ! -f .dev/state.md ]]; then
    echo "Missing .dev/state.md in current directory. Run gal init first." >&2
    exit 1
  fi
}

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"

command="${1:-}"
shift || true

case "$command" in
  init)
    "$script_dir/init-repo.sh" "$@"
    ;;
  plan)
    plan_type="feat"
    name_args=()
    while [[ $# -gt 0 ]]; do
      case "$1" in
        -t)
          plan_type="$2"
          shift 2
          ;;
        *)
          name_args+=("$1")
          shift
          ;;
      esac
    done
    if [[ ${#name_args[@]} -eq 0 ]]; then
      echo "Usage: gal plan [-t <type>] <name>" >&2
      exit 1
    fi
    plan_name="${name_args[*]}"
    slug="$(to_slug "$plan_name")"
    mkdir -p docs/plans
    template="$repo_root/templates/plan.md"
    target="docs/plans/${plan_type}-${slug}.prompt.md"
    if [[ -f "$target" ]]; then
      echo "Plan already exists: $target" >&2
      exit 1
    fi
    sed "s/\[Feature Name\]/$plan_name/g" "$template" > "$target"
    echo "Created plan scaffold: $target"
    echo "Next: classify T0/T1/T2 and fill Review Pack before implementation."
    ;;
  status)
    require_state
    echo "State file: .dev/state.md"
    grep -E '^(Workflow:|Plan:|Step:|Last activity:|Last session:|Next step:)' .dev/state.md || true
    ;;
  next)
    require_state
    grep -E '^Next step:' .dev/state.md | head -n 1
    ;;
  pause)
    require_state
    echo "=== gal pause: Context Handoff ==="
    echo ""
    echo "Before running this command, ask your AI session to:"
    echo "  1. Write key context to plan's ## Status > ### Handoff Notes"
    echo "  2. Update .dev/state.md Session Continuity"
    echo ""
    if git status --porcelain -- .dev/ docs/plans/ 2>/dev/null | grep -q .; then
      git add .dev/ docs/plans/
      git commit -m "chore: gal pause — context handoff"
      echo ""
      echo "Committed .dev/ and docs/plans/ changes. Safe to switch worktree."
    else
      echo "No uncommitted changes in .dev/ or docs/plans/. Nothing to commit."
    fi
    ;;
  sync)
    if [[ ! -x "$script_dir/sync-dev-context.sh" ]]; then
      echo "sync-dev-context.sh is not implemented yet. Command surface reserved; adapter generation still pending." >&2
      exit 1
    fi
    "$script_dir/sync-dev-context.sh" "$@"
    ;;
  ask)
    if [[ $# -eq 0 ]]; then
      echo "Usage: gal ask <agent>" >&2
      exit 1
    fi
    echo "Adapter-level consult command: /gal ask $*"
    ;;
  run)
    if [[ $# -eq 0 ]]; then
      echo "Usage: gal run <agent>" >&2
      exit 1
    fi
    echo "Adapter-level utility command: /gal run $*"
    ;;
  "")
    show_usage
    ;;
  *)
    show_usage
    exit 1
    ;;
esac