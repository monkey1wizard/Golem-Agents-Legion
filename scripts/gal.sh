#!/usr/bin/env bash

set -euo pipefail

show_usage() {
  cat <<'EOF'
Usage: gal <command> [args]

Commands:
  init [targetPath] [projectName]    Initialize .dev/ and docs/plans/
  install|update|sync|doctor|uninstall|commit-msg|release|mcp|setup|clean|smudge|init-repo|resolve-catalog|translation-freshness
                                     Forward to the Rust gal binary
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

source "$script_dir/common/common.sh"

invoke_gal_binary_command() {
  if command -v gal >/dev/null 2>&1; then
    gal "$command" "$@"
    exit $?
  fi

  for candidate in \
    "$repo_root/target/debug/gal" \
    "$repo_root/target/release/gal" \
    "$repo_root/target/debug/gal.exe" \
    "$repo_root/target/release/gal.exe"
  do
    if [[ -f "$candidate" ]]; then
      "$candidate" "$command" "$@"
      exit $?
    fi
  done

  echo 'gal binary not found on PATH.' >&2
  exit 1
}

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
  local config_path
  config_path="$(resolve_xmachine_config_path "$repo_root")"
  [[ -f "$config_path" ]] || return 0

  jq -r '(.xmachineNodeAliases // .nodes // {}) | keys[]?' "$config_path"
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

is_explicit_plan_token() {
  local token candidate
  token="${1:-}"
  [[ -n "$token" ]] || return 1

  candidate="$token"
  if [[ "$candidate" == \#file:* || "$candidate" == \#FILE:* ]]; then
    candidate="${candidate:6}"
  fi

  [[ "$candidate" == *.prompt.md || "$candidate" == *.md ]]
}

get_pipeline_dispatch_context() {
  PIPELINE_REQUESTED=0
  PIPELINE_PHASE=""
  PIPELINE_TASK_SCOPE=""
  PIPELINE_FROM=""
  PIPELINE_STOP_AT=""
  PIPELINE_FIX_MODE=0
  PIPELINE_ERROR=""
  PIPELINE_REMAINING_TOKENS=()

  local token
  while (($#)); do
    token="$1"
    shift

    [[ -n "$token" ]] || continue

    case "$token" in
      --pipeline-phase)
        PIPELINE_REQUESTED=1
        if (($# == 0)) ; then
          PIPELINE_ERROR="Missing phase after --pipeline-phase. Expected one of: implement, test, review, verify, security."
          return 0
        fi
        PIPELINE_PHASE="${1,,}"
        shift
        ;;
      --task-scope)
        if (($# == 0)) ; then
          PIPELINE_ERROR="Missing task reference after --task-scope."
          return 0
        fi
        PIPELINE_TASK_SCOPE="$1"
        shift
        ;;
      --fix-mode)
        PIPELINE_FIX_MODE=1
        ;;
      from)
        if (($# == 0)) ; then
          PIPELINE_ERROR="Missing task reference after from."
          return 0
        fi
        PIPELINE_FROM="$1"
        shift
        ;;
      stop-at)
        if (($# == 0)) ; then
          PIPELINE_ERROR="Missing task reference after stop-at."
          return 0
        fi
        PIPELINE_STOP_AT="$1"
        shift
        ;;
      *)
        if is_explicit_plan_token "$token"; then
          continue
        fi
        PIPELINE_REMAINING_TOKENS+=("$token")
        ;;
    esac
  done

  if [[ "$PIPELINE_REQUESTED" -eq 1 ]]; then
    case "$PIPELINE_PHASE" in
      implement|test|review|verify|security)
        ;;
      "")
        PIPELINE_ERROR="Pipeline-bound dispatch requires --pipeline-phase <implement|test|review|verify|security>."
        ;;
      *)
        PIPELINE_ERROR="Unsupported pipeline phase '$PIPELINE_PHASE'. Expected one of: implement, test, review, verify, security."
        ;;
    esac
  fi
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

get_source_plan_path() {
  local execution_plan_path="$1"
  [[ -n "$execution_plan_path" ]] || return 0

  local repo_context_root dev_plans_root relative_prompt_path relative_source_path
  repo_context_root="$(get_repo_context_root)"
  dev_plans_root="$repo_context_root/.dev/plans"

  case "$execution_plan_path" in
    "$dev_plans_root"/*) ;;
    *) return 0 ;;
  esac

  relative_prompt_path="${execution_plan_path#"$dev_plans_root"/}"
  [[ "$relative_prompt_path" == *.prompt.md ]] || return 0
  relative_source_path="${relative_prompt_path%.prompt.md}.md"
  printf '%s\n' "$repo_context_root/docs/plans/$relative_source_path"
}

get_language_convention_file_names() {
  local signal_text="$1"
  [[ -n "$signal_text" ]] || return 0

  local normalized_signal_text
  normalized_signal_text="$(printf '%s' "$signal_text" | tr '[:upper:]' '[:lower:]')"

  [[ "$normalized_signal_text" == *"c#"* || "$normalized_signal_text" == *".net"* ]] && printf '%s\n' csharp.md
  [[ "$normalized_signal_text" == *"typescript"* || "$normalized_signal_text" == *"javascript"* ]] && printf '%s\n' typescript.md
  [[ "$normalized_signal_text" == *"golang"* || "$normalized_signal_text" == *"go"* ]] && printf '%s\n' go.md
  [[ "$normalized_signal_text" == *"rust"* ]] && printf '%s\n' rust.md
}

get_task_scope_language_signal_text() {
  local task_scope="$1"
  shift
  [[ -n "$task_scope" ]] || return 0

  local plan_path line
  for plan_path in "$@"; do
    [[ -n "$plan_path" && -f "$plan_path" ]] || continue
    while IFS= read -r line; do
      if [[ "$line" == *"$task_scope"* ]]; then
        printf '%s\n' "$(printf '%s' "$line" | trim)"
        return 0
      fi
    done < "$plan_path"
  done
}

get_project_language_convention_paths() {
  local task_scope="$1"
  shift
  local repo_context_root project_path conventions_root language_line language_text
  repo_context_root="$(get_repo_context_root)"
  project_path="$repo_context_root/.dev/project.md"
  conventions_root="$repo_context_root/conventions"

  local paths=()
  [[ -f "$conventions_root/token-budget.md" ]] && paths+=("$conventions_root/token-budget.md")
  [[ -f "$conventions_root/working-hours.md" ]] && paths+=("$conventions_root/working-hours.md")

  local task_signal_text convention_file_name
  task_signal_text="$(get_task_scope_language_signal_text "$task_scope" "$@")"
  while IFS= read -r convention_file_name; do
    [[ -n "$convention_file_name" ]] || continue
    paths+=("$conventions_root/$convention_file_name")
  done < <(get_language_convention_file_names "$task_signal_text")

  if (( ${#paths[@]} > 2 )); then
    printf '%s\n' "${paths[@]}" | awk 'NF && !seen[$0]++'
    return 0
  fi

  if [[ ! -f "$project_path" ]]; then
    printf '%s\n' "${paths[@]}"
    return 0
  fi

  language_line="$(grep -E '^\|[[:space:]]*Language[[:space:]]*\|' "$project_path" | head -n 1 || true)"
  if [[ -z "$language_line" ]]; then
    printf '%s\n' "${paths[@]}"
    return 0
  fi

  while IFS= read -r convention_file_name; do
    [[ -n "$convention_file_name" ]] || continue
    paths+=("$conventions_root/$convention_file_name")
  done < <(get_language_convention_file_names "$language_line")

  printf '%s\n' "${paths[@]}" | awk 'NF && !seen[$0]++'
}

add_pipeline_dispatch_metadata() {
  local phase="$1" context_carry_supported="$2" preferred_plan_path="${3:-}" task_scope="${4:-}"
  get_state_context

  local active_plan_path source_plan_path status_step status_last_activity status_next_step context_mode repo_context_root
  repo_context_root="$(get_repo_context_root)"
  active_plan_path="$STATE_ACTIVE_PLAN"
  source_plan_path="$(get_source_plan_path "$active_plan_path")"

  if [[ -n "$preferred_plan_path" ]]; then
    local resolved_plan_path
    resolved_plan_path="$(resolve_plan_path "$preferred_plan_path")"
    if [[ "$resolved_plan_path" == *.prompt.md ]]; then
      active_plan_path="$resolved_plan_path"
      source_plan_path="$(get_source_plan_path "$resolved_plan_path")"
    elif [[ "$resolved_plan_path" == *.md ]]; then
      active_plan_path=""
      source_plan_path="$resolved_plan_path"
    fi
  fi

  status_step="$(get_plan_status_field "$active_plan_path" Step)"
  status_last_activity="$(get_plan_status_field "$active_plan_path" 'Last activity')"
  status_next_step="$(get_plan_status_field "$active_plan_path" 'Next step')"

  if [[ "$context_carry_supported" == true && "$phase" != implement ]]; then
    context_mode='delta'
  else
    context_mode='full'
  fi

  dispatch_extra+=(CURRENT_TASK "$(get_plan_status_field "$active_plan_path" 'Current Task')")
  dispatch_extra+=(TASK_BASE_COMMIT "$(get_plan_status_field "$active_plan_path" 'Task Base Commit')")
  dispatch_extra+=(TASK_FINAL_COMMIT "$(get_plan_status_field "$active_plan_path" 'Task Final Commit')")
  dispatch_extra+=(TEST_RETRY_COUNT "$(get_plan_status_field "$active_plan_path" 'Test Retry Count')")
  dispatch_extra+=(REVIEW_RETRY_COUNT "$(get_plan_status_field "$active_plan_path" 'Review Retry Count')")
  dispatch_extra+=(CONTEXT_CARRY "$context_carry_supported")
  dispatch_extra+=(PIPELINE_CONTEXT_MODE "$context_mode")

  if [[ "$context_mode" == full ]]; then
    dispatch_extra+=(ACTIVE_EXECUTION_PROMPT "$active_plan_path")
    dispatch_extra+=(SOURCE_PLAN "$source_plan_path")
    dispatch_extra+=(WORKFLOW_STATE "$STATE_WORKFLOW")
    dispatch_extra+=(STATUS_STEP "$status_step")
    dispatch_extra+=(STATUS_LAST_ACTIVITY "$status_last_activity")
    dispatch_extra+=(STATUS_NEXT_STEP "$status_next_step")
    dispatch_extra+=(PIPELINE_CONTEXT_FILES "$(printf '%s\n' "$repo_context_root/.dev/project.md" "$repo_context_root/.dev/state.md" "$active_plan_path" "$source_plan_path" | awk 'NF && !seen[$0]++' | paste -sd '; ' -)")
    dispatch_extra+=(CONVENTION_HINTS "$(get_project_language_convention_paths "$task_scope" "$active_plan_path" "$source_plan_path" | paste -sd '; ' -)")
  fi
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
    golem-notewriter|golem-designer|golem-researcher|\
    golem-security|golem-releaser) echo "$full" ;;
    *) echo "" ;;
  esac
}

golem_class() {
  case "$1" in
    golem-debugger|golem-notewriter) echo utility ;;
    golem-architect|golem-analyst|golem-designer|golem-researcher|golem-security|golem-releaser) echo domain ;;
    *) echo pipeline ;;
  esac
}

case "$command" in
  init)
    gal init-repo "$@"
    ;;
  install|update|sync|doctor|uninstall|commit-msg|release|mcp|setup|clean|smudge|init-repo|resolve-catalog|translation-freshness)
    invoke_gal_binary_command "$@"
    ;;
  xmachine)
    parse_xmachine_task_shorthand "$@"
    if [[ -n "$XMACHINE_SHORTHAND_ERROR" ]]; then
      write_dispatch COMMAND error ACTION "$XMACHINE_SHORTHAND_ERROR"
      exit 0
    fi

    if [[ -n "$XMACHINE_SHORTHAND_PLAN" ]]; then
      write_dispatch COMMAND pipeline ACTION "Follow the /gal-pipeline procedure only to resolve task '$XMACHINE_SHORTHAND_TASK_REF' and prepare the bounded task spec, then offload with scripts/Invoke-XmachineTask.sh --work-node '$XMACHINE_SHORTHAND_NODE' --task-spec <phase-task-spec> --wait. Do not use Invoke-XmachinePipeline.* for this shorthand. Do not pass --work-repo-path unless this target repo has an intentional persistent checkout on the work node." ON_COMPLETE "Report the single-task verdict and whether local convergence is complete." READ "$xmachine_doc_path" EXECUTION xmachine OFFLOAD direct-task XMACHINE_MODE execute WORK_NODE "$XMACHINE_SHORTHAND_NODE" TASK_REF "$XMACHINE_SHORTHAND_TASK_REF" FROM "$XMACHINE_SHORTHAND_TASK_REF" STOP_AT "$XMACHINE_SHORTHAND_TASK_REF" PLAN "$XMACHINE_SHORTHAND_PLAN"
    else
      write_dispatch COMMAND pipeline ACTION "Follow the /gal-pipeline procedure only to resolve task '$XMACHINE_SHORTHAND_TASK_REF' and prepare the bounded task spec, then offload with scripts/Invoke-XmachineTask.sh --work-node '$XMACHINE_SHORTHAND_NODE' --task-spec <phase-task-spec> --wait. Do not use Invoke-XmachinePipeline.* for this shorthand. Do not pass --work-repo-path unless this target repo has an intentional persistent checkout on the work node." ON_COMPLETE "Report the single-task verdict and whether local convergence is complete." READ "$xmachine_doc_path" EXECUTION xmachine OFFLOAD direct-task XMACHINE_MODE execute WORK_NODE "$XMACHINE_SHORTHAND_NODE" TASK_REF "$XMACHINE_SHORTHAND_TASK_REF" FROM "$XMACHINE_SHORTHAND_TASK_REF" STOP_AT "$XMACHINE_SHORTHAND_TASK_REF"
    fi
    ;;
  dispatch)
    intent="${1:-}"
    dispatch_tokens=("${@:2}")
    get_xmachine_dispatch_context "${dispatch_tokens[@]}"
    get_pipeline_dispatch_context "${dispatch_tokens[@]}"
    explicit_plan="$(get_explicit_plan_argument "${dispatch_tokens[@]}")"

    if [[ "$XMACHINE_REQUESTED" -eq 1 && -z "$XMACHINE_WORK_NODE" ]]; then
      available_nodes="<none configured>"
      if (( ${#XMACHINE_AVAILABLE_NODES[@]} > 0 )); then
        available_nodes="$(printf '%s, ' "${XMACHINE_AVAILABLE_NODES[@]}")"
        available_nodes="${available_nodes%, }"
      fi
      write_dispatch COMMAND error ACTION "xmachine execution requires both the literal keyword 'xmachine' and a valid work-node alias from ~/.gal/config/xmachine.json (legacy repo-root fallback supported). Available aliases: $available_nodes"
      exit 0
    fi

    if [[ -n "$PIPELINE_ERROR" ]]; then
      write_dispatch COMMAND error ACTION "$PIPELINE_ERROR"
      exit 0
    fi

    case "$intent" in
      init|research|deep-research|pipeline)
        action="Execute the $intent workflow step."
        on_complete="Report result to user."
        dispatch_args=()
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
            action="Follow the /gal-pipeline procedure to chain implement → test → review using executor-routing.json for multi-vendor AI assignment."
            on_complete="Report combined verdict: implement/test/review status and whether the branch is ready for /ship."
            ;;
        esac

        if [[ "$XMACHINE_REQUESTED" -eq 1 ]]; then
          action="$action Use xmachine work node '$XMACHINE_WORK_NODE' for bounded execution where supported, and keep control-plane state convergence local."
          dispatch_args=(COMMAND "$intent" ACTION "$action" ON_COMPLETE "$on_complete")
          [[ -n "$explicit_plan" ]] && dispatch_args+=(PLAN "$explicit_plan")
          [[ -n "$PIPELINE_FROM" ]] && dispatch_args+=(FROM "$PIPELINE_FROM")
          [[ -n "$PIPELINE_STOP_AT" ]] && dispatch_args+=(STOP_AT "$PIPELINE_STOP_AT")
          dispatch_args+=(READ "$xmachine_doc_path" EXECUTION xmachine WORK_NODE "$XMACHINE_WORK_NODE")
          write_dispatch "${dispatch_args[@]}"
        else
          dispatch_args=(COMMAND "$intent" ACTION "$action" ON_COMPLETE "$on_complete")
          [[ -n "$explicit_plan" ]] && dispatch_args+=(PLAN "$explicit_plan")
          [[ -n "$PIPELINE_FROM" ]] && dispatch_args+=(FROM "$PIPELINE_FROM")
          [[ -n "$PIPELINE_STOP_AT" ]] && dispatch_args+=(STOP_AT "$PIPELINE_STOP_AT")
          write_dispatch "${dispatch_args[@]}"
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
          elif [[ "$PIPELINE_REQUESTED" -eq 1 && "$resolved" =~ ^golem-(implementer|tester|reviewer|verifier|security)$ ]]; then
            mode=bound
          else
            mode=consult
          fi

          if (( ${#PIPELINE_REMAINING_TOKENS[@]} > 0 )); then
            action="${PIPELINE_REMAINING_TOKENS[*]}"
          elif [[ "$PIPELINE_REQUESTED" -eq 1 ]]; then
            action="Invoke $resolved for pipeline phase '$PIPELINE_PHASE'."
          elif (( ${#dispatch_tokens[@]} > 0 )); then
            action="${dispatch_tokens[*]}"
          else
            action="Invoke $resolved — awaiting user instruction."
          fi

          dispatch_extra=()
          if [[ "$PIPELINE_REQUESTED" -eq 1 && "$resolved" =~ ^golem-(implementer|tester|reviewer|verifier|security)$ ]]; then
            dispatch_extra+=(DISPATCH_KIND pipeline-phase PIPELINE_PHASE "$PIPELINE_PHASE")
            if [[ -n "$PIPELINE_TASK_SCOPE" ]]; then
              dispatch_extra+=(TASK_SCOPE "$PIPELINE_TASK_SCOPE")
            fi
            if [[ "$PIPELINE_FIX_MODE" -eq 1 ]]; then
              dispatch_extra+=(FIX_MODE true)
            fi
            if [[ "$XMACHINE_REQUESTED" -eq 1 ]]; then
              add_pipeline_dispatch_metadata "$PIPELINE_PHASE" false "$explicit_plan" "$PIPELINE_TASK_SCOPE"
            else
              add_pipeline_dispatch_metadata "$PIPELINE_PHASE" true "$explicit_plan" "$PIPELINE_TASK_SCOPE"
            fi
          fi

          if [[ "$XMACHINE_REQUESTED" -eq 1 ]]; then
            write_dispatch ROLE "$resolved" MODE "$mode" "${dispatch_extra[@]}" ACTION "$action" ON_COMPLETE "Report result to user." READ "$xmachine_doc_path" EXECUTION xmachine WORK_NODE "$XMACHINE_WORK_NODE"
          else
            write_dispatch ROLE "$resolved" MODE "$mode" "${dispatch_extra[@]}" ACTION "$action" ON_COMPLETE "Report result to user."
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
