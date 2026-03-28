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
  dispatch [subcommand|golem] [text] Auto-detect state or invoke named golem
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

# --- Dispatch helpers ---

write_dispatch() {
  echo "--- GAL DISPATCH ---"
  local key val
  while [[ $# -ge 2 ]]; do
    key="$1"; val="$2"; shift 2
    [[ -n "$val" ]] && echo "${key}: ${val}"
  done
  echo "--- END DISPATCH ---"
}

get_wf_state() {
  [[ -f .dev/state.md ]] || { echo ""; return; }
  grep -E '^Workflow:' .dev/state.md | head -n1 | sed 's/^Workflow:[[:space:]]*//' || true
}

resolve_golem() {
  local name="$1"
  local full="$name"
  [[ "$name" != golem-* ]] && full="golem-$name"
  case "$full" in
    golem-planner|golem-architect|golem-analyst|golem-implementer|\
    golem-tester|golem-reviewer|golem-verifier|golem-debugger|\
    golem-scribe|golem-librarian|golem-designer|golem-researcher) echo "$full" ;;
    *) echo "" ;;
  esac
}

golem_class() {
  case "$1" in
    golem-debugger|golem-scribe) echo utility ;;
    golem-architect|golem-analyst|golem-librarian|golem-designer|golem-researcher) echo domain ;;
    *) echo workflow ;;
  esac
}

golem_bound_state() {
  case "$1" in
    golem-planner)     echo PLAN ;;
    golem-implementer) echo IMPLEMENT ;;
    golem-tester)      echo TEST ;;
    golem-reviewer)    echo REVIEW ;;
    golem-verifier)    echo VERIFY ;;
    golem-architect)   echo DISCUSS ;;
    *) echo "" ;;
  esac
}

dispatch_for_state() {
  local state="$1"
  case "$state" in
    PLAN)         echo golem-planner ;;
    DISCUSS)      echo golem-architect ;;
    IMPLEMENT)    echo golem-implementer ;;
    TEST)         echo golem-tester ;;
    REVIEW)       echo golem-reviewer ;;
    CROSS_REVIEW) echo golem-reviewer ;;
    VERIFY)       echo golem-verifier ;;
    *) echo "" ;;
  esac
}

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
    if [[ ! -f "$script_dir/sync-dev-context.sh" ]]; then
      echo "sync-dev-context.sh is not implemented yet. Command surface reserved; adapter generation still pending." >&2
      exit 1
    fi
    bash "$script_dir/sync-dev-context.sh" "$@"
    ;;
  dispatch)
    intent="${1:-}"
    sub_text="${*:2}"
    case "$intent" in
      init|plan|status|next|pause|sync)
        action="Execute the $intent workflow step."
        on_complete="Report result to user."
        case "$intent" in
          init)
            action="Initialize .dev/ for the target repo, then surface the manual next step."
            on_complete="Tell the user to review .dev/project.md, curate ## Active Skills, then run /gal sync."
            ;;
          sync)
            action="Generate .github/copilot-instructions.md and GEMINI.md from .dev/project.md."
            on_complete="Report which adapter files were generated and whether Active Skills validation passed."
            ;;
        esac
        write_dispatch COMMAND "$intent" ACTION "$action" ON_COMPLETE "$on_complete"
        ;;
      "")
        state="$(get_wf_state)"
        if [[ -z "$state" ]]; then
          write_dispatch COMMAND suggest ACTION "No .dev/state.md found. Run /gal init to initialize this repository." ON_COMPLETE "Run /gal init"
        elif [[ "$state" == "IDLE" ]]; then
          write_dispatch COMMAND suggest ACTION "Workflow is IDLE. Run '/gal plan' to create a plan or '/gal status' for details." ON_COMPLETE "Run /gal plan"
        else
          golem="$(dispatch_for_state "$state")"
          if [[ -n "$golem" ]]; then
            agent_path="$repo_root/agent/${golem}.agent.md"
            write_dispatch ROLE "$golem" MODE bound READ "$agent_path" ACTION "Workflow state is $state. Activate $golem in bound mode." ON_COMPLETE "Update .dev/state.md and suggest next step."
          else
            write_dispatch COMMAND suggest ACTION "Workflow state '$state' has no default golem. Use '/gal <golem-name>' to invoke directly."
          fi
        fi
        ;;
      *)
        resolved="$(resolve_golem "$intent")"
        if [[ -n "$resolved" ]]; then
          cls="$(golem_class "$resolved")"
          if [[ "$cls" == utility ]]; then
            mode=utility
          elif [[ "$cls" == domain ]]; then
            mode=consult
          else
            state="$(get_wf_state)"
            bound="$(golem_bound_state "$resolved")"
            mode=consult
            if [[ "$resolved" == "golem-reviewer" ]]; then
              [[ "$state" == "REVIEW" || "$state" == "CROSS_REVIEW" ]] && mode=bound
            elif [[ -n "$bound" && "$state" == "$bound" ]]; then
              mode=bound
            fi
          fi
          action="${sub_text:-Invoke $resolved — awaiting user instruction.}"
          write_dispatch ROLE "$resolved" MODE "$mode" ACTION "$action" ON_COMPLETE "Report result to user."
        else
          write_dispatch COMMAND error ACTION "Unknown argument: '$intent'. Use a subcommand (init/plan/status/next/pause/sync) or a golem name."
        fi
        ;;
    esac
    ;;
  "")
    show_usage
    ;;
  *)
    show_usage
    exit 1
    ;;
esac