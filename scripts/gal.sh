#!/usr/bin/env bash

set -euo pipefail

show_usage() {
  cat <<'EOF'
Usage: gal <command> [args]

Commands:
  init [targetPath] [projectName]    Initialize .dev/ and docs/plans/
  dispatch [subcommand|golem] [text] Route to subcommand or golem via /gal skill

Script-dispatched subcommands: init, research
Control-plane skills (use in chat): /gal status, /gal whats-next, /gal wrap-up
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

get_repo_context_root() {
  local current parent
  current="$(pwd)"

  while true; do
    if [[ -f "$current/.dev/state.md" ]]; then
      printf '%s\n' "$current"
      return 0
    fi

    parent="$(dirname "$current")"
    if [[ -z "$parent" || "$parent" == "$current" ]]; then
      printf '%s\n' "$(pwd)"
      return 0
    fi

    current="$parent"
  done
}

get_state_path() {
  local repo_root
  repo_root="$(get_repo_context_root)"
  printf '%s/.dev/state.md\n' "$repo_root"
}

require_state() {
  local state_path
  state_path="$(get_state_path)"
  if [[ ! -f "$state_path" ]]; then
    echo "Missing .dev/state.md in current directory. Run gal init first." >&2
    exit 1
  fi
}

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"
command="${1:-}"
if [[ $# -gt 0 ]]; then
  shift
fi

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
  local workflow
  get_state_context
  workflow="$STATE_WORKFLOW"
  echo "$workflow"
}

trim() {
  sed -E 's/^[[:space:]]+//; s/[[:space:]]+$//'
}

unwrap_markdown_code() {
  local value
  value="$(printf '%s' "$1" | trim)"
  value="${value#\`}"
  value="${value%\`}"
  printf '%s' "$value"
}

resolve_plan_path() {
  local candidate absolute prompt repo_root
  candidate="$(unwrap_markdown_code "$1")"
  [[ -n "$candidate" ]] || return 1
  candidate="${candidate//\\//}"
  repo_root="$(get_repo_context_root)"

  if [[ "$candidate" = /* || "$candidate" =~ ^[A-Za-z]:/ ]]; then
    absolute="$candidate"
  else
    absolute="$repo_root/$candidate"
  fi

  if [[ "$absolute" == *.prompt.md ]]; then
    printf '%s\n' "$absolute"
    return 0
  fi

  if [[ "$absolute" == *.md ]]; then
    prompt="${absolute%.md}.prompt.md"
    if [[ -f "$prompt" ]]; then
      printf '%s\n' "$prompt"
      return 0
    fi
  fi

  printf '%s\n' "$absolute"
}

get_active_plan_path() {
  local state_path
  state_path="$(get_state_path)"
  [[ -f "$state_path" ]] || return 0

  local in_active_plans=0 header_seen=0 line cell resolved
  while IFS= read -r line; do
    if [[ "$line" =~ ^##[[:space:]]+Active[[:space:]]+Plans ]]; then
      in_active_plans=1
      continue
    fi

    if (( in_active_plans )) && [[ "$line" =~ ^##[[:space:]]+ ]]; then
      break
    fi

    (( in_active_plans )) || continue
    [[ "$line" == \|* ]] || continue
    [[ "$line" =~ ^\|[[:space:]]*--- ]] && continue

    IFS='|' read -r -a raw_cells <<<"$line"
    local cells=()
    for cell in "${raw_cells[@]}"; do
      cell="$(printf '%s' "$cell" | trim)"
      [[ -n "$cell" ]] && cells+=("$cell")
    done

    (( ${#cells[@]} >= 2 )) || continue

    if (( ! header_seen )); then
      if printf '%s\n' "${cells[@]}" | grep -Fxq 'Plan' && printf '%s\n' "${cells[@]}" | grep -Fxq 'Workflow State'; then
        header_seen=1
      fi
      continue
    fi

    for cell in "${cells[@]}"; do
      resolved="$(resolve_plan_path "$cell")" || continue
      if [[ "$resolved" == *.prompt.md || "$resolved" == *.md ]]; then
        printf '%s\n' "$resolved"
        return 0
      fi
    done
  done < "$state_path"
}

get_plan_status_field() {
  local plan_path="$1" field_name="$2"
  [[ -n "$plan_path" && -f "$plan_path" ]] || return 0

  awk -v field="$field_name" '
    /^##[[:space:]]+Status/ { in_status=1; next }
    in_status && /^##[[:space:]]+/ { exit }
    in_status && $0 ~ ("^" field ":[[:space:]]*") {
      sub("^" field ":[[:space:]]*", "", $0)
      print
      exit
    }
  ' "$plan_path"
}

normalize_workflow_state() {
  local value upper
  value="$(printf '%s' "$1" | trim)"
  [[ -n "$value" ]] || return 0
  upper="$(printf '%s' "$value" | tr '[:lower:]' '[:upper:]')"
  case "$upper" in
    ENG-REVIEWED|APPROVED) echo IMPLEMENT ;;
    *) echo "$upper" ;;
  esac
}

get_state_context() {
  STATE_KIND=""
  STATE_WORKFLOW=""
  STATE_WORKFLOW_RAW=""
  STATE_ACTIVE_PLAN=""
  STATE_ERROR=""

  local state_path
  state_path="$(get_state_path)"

  if [[ ! -f "$state_path" ]]; then
    STATE_KIND="uninitialized"
    return 0
  fi

  STATE_ACTIVE_PLAN="$(get_active_plan_path)"
  if [[ -z "$STATE_ACTIVE_PLAN" ]]; then
    STATE_KIND="idle"
    STATE_WORKFLOW="IDLE"
    STATE_WORKFLOW_RAW="IDLE"
    return 0
  fi

  if [[ ! -f "$STATE_ACTIVE_PLAN" ]]; then
    STATE_KIND="state-error"
    STATE_ERROR="Active plan file not found: $STATE_ACTIVE_PLAN"
    return 0
  fi

  STATE_WORKFLOW_RAW="$(get_plan_status_field "$STATE_ACTIVE_PLAN" Workflow)"
  STATE_WORKFLOW="$(normalize_workflow_state "$STATE_WORKFLOW_RAW")"
  if [[ -z "$STATE_WORKFLOW" ]]; then
    STATE_KIND="state-error"
    STATE_ERROR="Could not resolve workflow state from $STATE_ACTIVE_PLAN ## Status."
    return 0
  fi

  if [[ "$STATE_WORKFLOW" == IDLE ]]; then
    STATE_KIND="idle"
  else
    STATE_KIND="active"
  fi
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
  dispatch)
    intent="${1:-}"
    sub_text="${*:2}"
    case "$intent" in
      init|research)
        action="Execute the $intent workflow step."
        on_complete="Report result to user."
        case "$intent" in
          init)
            action="Initialize .dev/ for the target repo, then surface the manual next step."
            on_complete="Tell the user to review .dev/project.md, then invoke /gal status for next steps."
            ;;
          research)
            action="Activate the /gal research skill for structured investigation."
            on_complete="Synthesize findings and surface RESEARCH_COMPLETE to the user."
            ;;
        esac
        write_dispatch COMMAND "$intent" ACTION "$action" ON_COMPLETE "$on_complete"
        ;;
      "")
        get_state_context
        state="$STATE_WORKFLOW"
        if [[ "$STATE_KIND" == uninitialized ]]; then
          write_dispatch COMMAND suggest ACTION "No .dev/state.md found. Run /gal init to initialize this repository." ON_COMPLETE "Run /gal init"
        elif [[ "$STATE_KIND" == idle ]]; then
          write_dispatch COMMAND suggest ACTION "Repo is initialized but no active workflow is recorded. Use /office-hours or /autoplan to create a plan, or /gal status for details." ON_COMPLETE "Run /gal status or /office-hours"
        elif [[ "$STATE_KIND" == state-error ]]; then
          write_dispatch COMMAND suggest ACTION "Repo is initialized, but GAL could not resolve workflow state from the active plan. Inspect .dev/state.md Active Plans and $STATE_ACTIVE_PLAN ## Status." ON_COMPLETE "Fix repo state, then run /gal status"
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
            get_state_context
            state="$STATE_WORKFLOW"
            bound="$(golem_bound_state "$resolved")"
            mode=consult
            if [[ "$STATE_KIND" != active ]]; then
              mode=consult
            elif [[ "$resolved" == "golem-reviewer" ]]; then
              [[ "$state" == "REVIEW" || "$state" == "CROSS_REVIEW" ]] && mode=bound
            elif [[ -n "$bound" && "$state" == "$bound" ]]; then
              mode=bound
            fi
          fi
          action="${sub_text:-Invoke $resolved — awaiting user instruction.}"
          write_dispatch ROLE "$resolved" MODE "$mode" ACTION "$action" ON_COMPLETE "Report result to user."
        else
          write_dispatch COMMAND error ACTION "Unknown argument: '$intent'. Use a subcommand (init/research) or a golem name."
        fi
        ;;
    esac
    ;;
  "")
    show_usage
    ;;
  *)
    echo "ERROR: Unsupported top-level command '$command'. Use 'gal init' or 'gal dispatch'." >&2
    echo "Control-plane actions like /gal status, /gal whats-next, and /gal wrap-up run in chat." >&2
    show_usage
    exit 1
    ;;
esac
