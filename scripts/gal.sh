#!/usr/bin/env bash

set -euo pipefail

show_usage() {
  cat <<'EOF'
Usage: gal <command> [args]

Commands:
  init [targetPath] [projectName]    Initialize .dev/ and docs/plans/
  dispatch [subcommand|golem] [text] Route to subcommand or golem via /gal skill
  xmachine <node> to do <task-ref>   Run one active-plan task on a readied work node

Script-dispatched subcommands: init, research, deep-research
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
xmachine_doc_path="$repo_root/docs/collaborative-tools/xmachine.md"
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

read_xmachine_node_aliases() {
  local config_path="$repo_root/xmachine.config.json"
  [[ -f "$config_path" ]] || return 0

  jq -r '.nodes | keys[]?' "$config_path"
}

get_xmachine_dispatch_context() {
  XMACHINE_REQUESTED=0
  XMACHINE_WORK_NODE=""
  mapfile -t XMACHINE_AVAILABLE_NODES < <(read_xmachine_node_aliases)

  local token
  for token in "$@"; do
    [[ -n "$token" ]] || continue

    case "$token" in
      xmachine|--xmachine)
        XMACHINE_REQUESTED=1
        ;;
      *)
        if [[ -z "$XMACHINE_WORK_NODE" ]]; then
          local alias
          for alias in "${XMACHINE_AVAILABLE_NODES[@]}"; do
            if [[ "$token" == "$alias" ]]; then
              XMACHINE_WORK_NODE="$token"
              break
            fi
          done
        fi
        ;;
    esac
  done
}

get_explicit_plan_argument() {
  local token candidate
  for token in "$@"; do
    [[ -n "$token" ]] || continue

    candidate="$token"
    if [[ "$candidate" == \#file:* || "$candidate" == \#FILE:* ]]; then
      candidate="${candidate:6}"
    fi

    if [[ "$candidate" == *.prompt.md || "$candidate" == *.md ]]; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done

  return 0
}

parse_xmachine_task_shorthand() {
  XMACHINE_SHORTHAND_ERROR=""
  XMACHINE_SHORTHAND_NODE=""
  XMACHINE_SHORTHAND_TASK_REF=""
  XMACHINE_SHORTHAND_PLAN=""

  mapfile -t XMACHINE_AVAILABLE_NODES < <(read_xmachine_node_aliases)

  if (( $# < 4 )); then
    XMACHINE_SHORTHAND_ERROR="Usage: gal xmachine <node> to do <task-ref> [#file:plan]"
    return 0
  fi

  XMACHINE_SHORTHAND_NODE="$1"
  shift

  local alias found=0
  for alias in "${XMACHINE_AVAILABLE_NODES[@]}"; do
    if [[ "$XMACHINE_SHORTHAND_NODE" == "$alias" ]]; then
      found=1
      break
    fi
  done

  if [[ $found -eq 0 ]]; then
    local available_nodes="<none configured>"
    if (( ${#XMACHINE_AVAILABLE_NODES[@]} > 0 )); then
      available_nodes="$(printf '%s, ' "${XMACHINE_AVAILABLE_NODES[@]}")"
      available_nodes="${available_nodes%, }"
    fi
    XMACHINE_SHORTHAND_ERROR="Unknown xmachine work node '$XMACHINE_SHORTHAND_NODE'. Available aliases: $available_nodes"
    return 0
  fi

  if [[ "${1:-}" != "to" || "${2:-}" != "do" ]]; then
    XMACHINE_SHORTHAND_ERROR="Usage: gal xmachine <node> to do <task-ref> [#file:plan]"
    return 0
  fi

  XMACHINE_SHORTHAND_TASK_REF="${3:-}"
  if [[ -z "$XMACHINE_SHORTHAND_TASK_REF" ]]; then
    XMACHINE_SHORTHAND_ERROR="Missing task reference. Usage: gal xmachine <node> to do <task-ref> [#file:plan]"
    return 0
  fi

  XMACHINE_SHORTHAND_PLAN="$(get_explicit_plan_argument "$XMACHINE_SHORTHAND_NODE" "$@")"
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
    local source_plan_root relative_source_plan_path relative_prompt_path dev_prompt
    source_plan_root="$repo_root/docs/plans"
    if [[ "$absolute" == "$source_plan_root"/* ]]; then
      relative_source_plan_path="${absolute#"$source_plan_root"/}"
      relative_prompt_path="${relative_source_plan_path%.md}.prompt.md"
      dev_prompt="$repo_root/.dev/plans/$relative_prompt_path"
      if [[ -f "$dev_prompt" ]]; then
        printf '%s\n' "$dev_prompt"
        return 0
      fi
    fi

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
      if printf '%s\n' "${cells[@]}" | grep -Fxq 'Plan' && { printf '%s\n' "${cells[@]}" | grep -Fxq 'Workflow State' || printf '%s\n' "${cells[@]}" | grep -Fxq 'Plan Phase'; }; then
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
  STATE_WORKFLOW="$(printf '%s' "$STATE_WORKFLOW_RAW" | trim | tr '[:lower:]' '[:upper:]')"
  STATE_KIND="active"
}

resolve_golem() {
  local name="$1"
  local full="$name"
  [[ "$name" != golem-* ]] && full="golem-$name"
  case "$full" in
    golem-architect|golem-analyst|golem-implementer|\
    golem-tester|golem-reviewer|golem-verifier|golem-debugger|\
    golem-notewriter|golem-designer|golem-researcher) echo "$full" ;;
    *) echo "" ;;
  esac
}

golem_class() {
  case "$1" in
    golem-debugger|golem-notewriter) echo utility ;;
    golem-architect|golem-analyst|golem-designer|golem-researcher) echo domain ;;
    *) echo pipeline ;;
  esac
}

case "$command" in
  init)
    "$script_dir/init-repo.sh" "$@"
    ;;
  xmachine)
    parse_xmachine_task_shorthand "$@"
    if [[ -n "$XMACHINE_SHORTHAND_ERROR" ]]; then
      write_dispatch COMMAND error ACTION "$XMACHINE_SHORTHAND_ERROR"
      exit 0
    fi

    if [[ -n "$XMACHINE_SHORTHAND_PLAN" ]]; then
      write_dispatch COMMAND pipeline ACTION "Follow the /gal-pipeline procedure to execute only task '$XMACHINE_SHORTHAND_TASK_REF' on xmachine work node '$XMACHINE_SHORTHAND_NODE'. Resolve the active plan, scope execution to this single task, and keep control-plane convergence local." ON_COMPLETE "Report the single-task verdict and whether local convergence is complete." READ "$xmachine_doc_path" EXECUTION xmachine WORK_NODE "$XMACHINE_SHORTHAND_NODE" TASK_REF "$XMACHINE_SHORTHAND_TASK_REF" FROM "$XMACHINE_SHORTHAND_TASK_REF" STOP_AT "$XMACHINE_SHORTHAND_TASK_REF" PLAN "$XMACHINE_SHORTHAND_PLAN"
    else
      write_dispatch COMMAND pipeline ACTION "Follow the /gal-pipeline procedure to execute only task '$XMACHINE_SHORTHAND_TASK_REF' on xmachine work node '$XMACHINE_SHORTHAND_NODE'. Resolve the active plan, scope execution to this single task, and keep control-plane convergence local." ON_COMPLETE "Report the single-task verdict and whether local convergence is complete." READ "$xmachine_doc_path" EXECUTION xmachine WORK_NODE "$XMACHINE_SHORTHAND_NODE" TASK_REF "$XMACHINE_SHORTHAND_TASK_REF" FROM "$XMACHINE_SHORTHAND_TASK_REF" STOP_AT "$XMACHINE_SHORTHAND_TASK_REF"
    fi
    ;;
  dispatch)
    intent="${1:-}"
    sub_text="${*:2}"
    dispatch_tokens=("${@:2}")
    get_xmachine_dispatch_context "${dispatch_tokens[@]}"
    explicit_plan=""
    if [[ "$intent" == "pipeline" ]]; then
      explicit_plan="$(get_explicit_plan_argument "${dispatch_tokens[@]}")"
    fi

    if [[ "$XMACHINE_REQUESTED" -eq 1 && -z "$XMACHINE_WORK_NODE" ]]; then
      available_nodes="<none configured>"
      if (( ${#XMACHINE_AVAILABLE_NODES[@]} > 0 )); then
        available_nodes="$(printf '%s, ' "${XMACHINE_AVAILABLE_NODES[@]}")"
        available_nodes="${available_nodes%, }"
      fi
      write_dispatch COMMAND error ACTION "xmachine execution requires both the literal keyword 'xmachine' and a valid work-node alias from xmachine.config.json. Available aliases: $available_nodes"
      exit 0
    fi

    case "$intent" in
      init|research|deep-research|pipeline)
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
          deep-research)
            action="Activate the /gal deep-research skill for multi-source investigation with cross-review and independent reference verification."
            on_complete="Synthesize findings, verify references, and surface RESEARCH_COMPLETE to the user."
            ;;
          pipeline)
            action="Follow the /gal-pipeline procedure to chain implement → test → review using model-roles for multi-vendor AI assignment."
            on_complete="Report combined verdict: implement/test/review status and whether the branch is ready for /ship."
            ;;
        esac

        if [[ "$XMACHINE_REQUESTED" -eq 1 ]]; then
          action="$action Use xmachine work node '$XMACHINE_WORK_NODE' for bounded execution where supported, and keep control-plane state convergence local."
          if [[ -n "$explicit_plan" ]]; then
            write_dispatch COMMAND "$intent" ACTION "$action" ON_COMPLETE "$on_complete" PLAN "$explicit_plan" READ "$xmachine_doc_path" EXECUTION xmachine WORK_NODE "$XMACHINE_WORK_NODE"
          else
            write_dispatch COMMAND "$intent" ACTION "$action" ON_COMPLETE "$on_complete" READ "$xmachine_doc_path" EXECUTION xmachine WORK_NODE "$XMACHINE_WORK_NODE"
          fi
        else
          if [[ -n "$explicit_plan" ]]; then
            write_dispatch COMMAND "$intent" ACTION "$action" ON_COMPLETE "$on_complete" PLAN "$explicit_plan"
          else
            write_dispatch COMMAND "$intent" ACTION "$action" ON_COMPLETE "$on_complete"
          fi
        fi
        ;;
      "")
        get_state_context
        state="$STATE_WORKFLOW"
        if [[ "$STATE_KIND" == uninitialized ]]; then
          write_dispatch COMMAND suggest ACTION "No .dev/state.md found. Run /gal init to initialize this repository." ON_COMPLETE "Run /gal init"
        elif [[ "$STATE_KIND" == idle ]]; then
          write_dispatch COMMAND suggest ACTION "Repo is initialized but no active workflow is recorded. Use /planning to create a source plan, then /refining-plan to lock the implementation contract, then /plan-to-prompt to generate the execution prompt, or /gal status for details." ON_COMPLETE "Run /gal status or /planning"
        elif [[ "$STATE_KIND" == state-error ]]; then
          write_dispatch COMMAND suggest ACTION "Repo is initialized, but the active plan reference is invalid. Inspect .dev/state.md Active Plans and $STATE_ACTIVE_PLAN." ON_COMPLETE "Fix repo state, then run /gal status"
        else
          write_dispatch COMMAND suggest ACTION "Active plan detected at $STATE_ACTIVE_PLAN. Use /gal whats-next to choose the next specialist command from plan files, not dispatcher state." ON_COMPLETE "Run /gal whats-next"
        fi
        ;;
      *)
        resolved="$(resolve_golem "$intent")"
        if [[ -n "$resolved" ]]; then
          cls="$(golem_class "$resolved")"
          if [[ "$cls" == utility ]]; then
            mode=utility
          else
            mode=consult
          fi
          action="${sub_text:-Invoke $resolved — awaiting user instruction.}"
          if [[ "$XMACHINE_REQUESTED" -eq 1 ]]; then
            write_dispatch ROLE "$resolved" MODE "$mode" ACTION "$action" ON_COMPLETE "Report result to user." READ "$xmachine_doc_path" EXECUTION xmachine WORK_NODE "$XMACHINE_WORK_NODE"
          else
            write_dispatch ROLE "$resolved" MODE "$mode" ACTION "$action" ON_COMPLETE "Report result to user."
          fi
        else
          write_dispatch COMMAND error ACTION "Unknown argument: '$intent'. Use a subcommand (init/research/deep-research/pipeline) or a golem name."
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
