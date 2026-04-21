#!/usr/bin/env bash
# setup-tools.sh — interactive installer for optional GAL collaborative tools.

if [ -n "${BASH_VERSINFO:-}" ] && [ "${BASH_VERSINFO[0]}" -lt 4 ]; then
  for candidate in /opt/homebrew/bin/bash /usr/local/bin/bash; do
    if [ -x "$candidate" ]; then
      exec "$candidate" "$0" "$@"
    fi
  done

  echo "setup-tools.sh requires Bash 4+." >&2
  echo "Install newer Bash with Homebrew, then rerun this script." >&2
  exit 1
fi

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

CHECK_ONLY=false
TOOLS=()
SUPPORTED_TOOLS=(gstack graphify opencli)

GSTACK_ROOT="$HOME/gstack"
GSTACK_STATE_DIR="$HOME/.gstack"
GSTACK_CONFIG_FILE="$HOME/.gstack/config.yaml"
GSTACK_PROJECTS_DIR="$GSTACK_STATE_DIR/projects"
GSTACK_SETUP_VERSION_FILE="$GSTACK_STATE_DIR/.last-setup-version"
GSTACK_WELCOME_FILE="$GSTACK_STATE_DIR/.welcome-seen"
GRAPHIFY_OUT_DIR="$REPO_ROOT/graphify-out"
GRAPHIFY_REPORT_FILE="$GRAPHIFY_OUT_DIR/GRAPH_REPORT.md"
OPENCLI_REPO_URL="https://github.com/jackwener/OpenCLI"
OPENCLI_RELEASES_URL="https://github.com/jackwener/OpenCLI/releases"
OPENCLI_LATEST_API_URL="https://api.github.com/repos/jackwener/OpenCLI/releases/latest"
OPENCLI_DOWNLOAD_ROOT="${HOME}/Downloads"
UV_TOOL_BIN_DIR="${HOME}/.local/bin"
BUN_BIN_DIR="${HOME}/.bun/bin"

print_section() {
  echo ""
  echo "=== $1 ==="
}

command_exists() {
  command -v "$1" >/dev/null 2>&1
}

ensure_path_dir() {
  local dir="$1"
  [ -d "$dir" ] || return 0
  case ":$PATH:" in
    *":$dir:"*) ;;
    *) export PATH="$dir:$PATH" ;;
  esac
}

normalize_tools() {
  if [ "$#" -eq 0 ]; then
    TOOLS=("${SUPPORTED_TOOLS[@]}")
    return
  fi

  TOOLS=()
  local seen=""
  local arg item normalized
  for arg in "$@"; do
    IFS=',' read -r -a parts <<< "$arg"
    for item in "${parts[@]}"; do
      normalized="$(printf '%s' "$item" | tr '[:upper:]' '[:lower:]' | xargs)"
      [ -z "$normalized" ] && continue
      case " $seen " in
        *" $normalized "*) ;;
        *)
          case "$normalized" in
            gstack|graphify|opencli)
              TOOLS+=("$normalized")
              seen="$seen $normalized"
              ;;
            *)
              echo "Unsupported tool '$normalized'. Valid values: ${SUPPORTED_TOOLS[*]}" >&2
              exit 1
              ;;
          esac
          ;;
      esac
    done
  done

  if [ "${#TOOLS[@]}" -eq 0 ]; then
    TOOLS=("${SUPPORTED_TOOLS[@]}")
  fi
}

get_python_cmd() {
  local candidate

  if command_exists uv; then
    for candidate in 3.14 3.13 3.12 3.11 3.10; do
      candidate="$(uv python find "$candidate" 2>/dev/null | head -n 1 || true)"
      if [ -n "$candidate" ] && [ -x "$candidate" ]; then
        printf '%s\n' "$candidate"
        return 0
      fi
    done
  fi

  for candidate in python3.14 python3.13 python3.12 python3.11 python3.10 python3 python; do
    if command_exists "$candidate"; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done

  return 1
}

get_python_version_request() {
  local python_cmd="$1"
  "$python_cmd" - <<'PY' 2>/dev/null
import sys
print(f"{sys.version_info[0]}.{sys.version_info[1]}")
PY
}

python_at_least_310() {
  local python_cmd="$1"
  "$python_cmd" - <<'PY' >/dev/null 2>&1
import sys
raise SystemExit(0 if sys.version_info >= (3, 10) else 1)
PY
}

graphify_module_exists() {
  local python_cmd="$1"
  "$python_cmd" -m graphify --version >/dev/null 2>&1
}

can_install_bun() {
  if [[ "${OSTYPE:-}" == darwin* ]] && command_exists brew; then
    return 0
  fi
  command_exists curl
}

install_bun() {
  if command_exists bun; then
    return 0
  fi

  print_section "Install Bun"

  if [[ "${OSTYPE:-}" == darwin* ]] && command_exists brew; then
    echo "  [INFO] Installing Bun via Homebrew (official tap)."
    brew tap oven-sh/bun
    brew install bun
  elif command_exists curl; then
    echo "  [INFO] Installing Bun via the official install script."
    curl -fsSL https://bun.com/install | bash
    ensure_path_dir "$BUN_BIN_DIR"
  else
    echo "Bun installation requires Homebrew on macOS or curl for the official installer." >&2
    return 1
  fi

  ensure_path_dir "$BUN_BIN_DIR"
  command_exists bun || { echo "Bun install completed, but the bun command is still not available in this shell." >&2; return 1; }
  echo "  [OK] Bun is installed and available."
}

get_node_major() {
  if ! command_exists node; then
    return 1
  fi
  node -p "process.versions.node.split('.')[0]" 2>/dev/null
}

get_collaboration_label() {
  case "$1" in
    ready) echo yes ;;
    available-but-needs-init|available-but-not-ready) echo partially ;;
    *) echo no ;;
  esac
}

declare -A tool_status_name=()
declare -A tool_status_state=()
declare -A tool_status_reason=()
declare -A tool_status_can_install=()
declare -A tool_status_next=()

set_status() {
  local name="$1"
  local state="$2"
  local reason="$3"
  local can_install="$4"
  local next_step="$5"
  tool_status_name["$name"]="$name"
  tool_status_state["$name"]="$state"
  tool_status_reason["$name"]="$reason"
  tool_status_can_install["$name"]="$can_install"
  tool_status_next["$name"]="$next_step"
}

check_gstack() {
  local missing=()
  local bun_missing=false
  command_exists git || missing+=("Git is missing")
  command_exists bun || { missing+=("Bun v1.0+ is missing"); bun_missing=true; }
  if [[ "${OSTYPE:-}" == msys* || "${OSTYPE:-}" == cygwin* ]]; then
    command_exists node || missing+=("Node.js is required on Windows")
  fi

  if [ "${#missing[@]}" -gt 0 ]; then
    local can_install=false
    local next_step="Install the missing prerequisites, then rerun this installer."
    if $bun_missing && [ "${#missing[@]}" -eq 1 ] && can_install_bun; then
      can_install=true
      next_step="This installer can install Bun, then clone gstack and run ./setup."
    fi
    set_status gstack unavailable "${missing[*]}" "$can_install" "$next_step"
    return
  fi

  if [ ! -d "$GSTACK_ROOT" ]; then
    set_status gstack unavailable "Official checkout was not found at $GSTACK_ROOT" true "This installer can clone gstack and run ./setup."
    return
  fi

  if [ ! -f "$GSTACK_CONFIG_FILE" ] \
    && [ ! -d "$GSTACK_PROJECTS_DIR" ] \
    && [ ! -f "$GSTACK_SETUP_VERSION_FILE" ] \
    && [ ! -f "$GSTACK_WELCOME_FILE" ]; then
    set_status gstack available-but-needs-init "gstack checkout exists at $GSTACK_ROOT, but machine bootstrap markers are missing under $GSTACK_STATE_DIR" true "This installer can run: cd $GSTACK_ROOT && ./setup"
    return
  fi

  set_status gstack ready "Official checkout and gstack machine bootstrap markers are present under $GSTACK_STATE_DIR." false "No action required."
}

check_graphify() {
  local python_cmd
  if ! python_cmd="$(get_python_cmd)"; then
    set_status graphify unavailable "Python 3.10+ is required but no supported Python launcher was found." false "Install Python 3.10+ and rerun this installer."
    return
  fi

  if ! python_at_least_310 "$python_cmd"; then
    local version
    version="$($python_cmd -c 'import sys; print(f"{sys.version_info[0]}.{sys.version_info[1]}")' 2>/dev/null || true)"
    set_status graphify unavailable "Python ${version:-unknown} detected; graphify requires Python 3.10+." false "Upgrade Python to 3.10+ and rerun this installer."
    return
  fi

  local cli_exists=false
  command_exists graphify && cli_exists=true
  local module_exists=false
  graphify_module_exists "$python_cmd" && module_exists=true

  if ! $cli_exists && ! $module_exists; then
    set_status graphify unavailable "graphify CLI is not installed." true "This installer can run the official graphify package install and graphify install flow."
    return
  fi

  if ! $cli_exists && $module_exists; then
    set_status graphify available-but-needs-init "graphify is installed as a Python module, but the graphify command is not on PATH." false "Open a new terminal or add your Python Scripts directory to PATH."
    return
  fi

  if [ ! -d "$GRAPHIFY_OUT_DIR" ]; then
    set_status graphify available-but-needs-init "graphify is installed, but graphify-out/ has not been generated for this repo yet." false "Run /graphify . to generate graphify-out/ for this repo."
    return
  fi

  if [ ! -f "$GRAPHIFY_REPORT_FILE" ]; then
    set_status graphify available-but-not-ready "graphify-out/ exists, but GRAPH_REPORT.md is missing." false "Regenerate graphify outputs so graphify-out/GRAPH_REPORT.md exists."
    return
  fi

  set_status graphify ready "graphify CLI and graphify-out/GRAPH_REPORT.md are both present." false "No action required."
}

check_opencli() {
  if ! command_exists opencli; then
    local node_major
    if ! node_major="$(get_node_major)"; then
      set_status opencli unavailable "Node.js 21+ and npm are required for the official OpenCLI install path." false "Install Node.js 21+ and npm, then rerun this installer."
      return
    fi
    if [ "$node_major" -lt 21 ]; then
      set_status opencli unavailable "Node.js ${node_major} detected; OpenCLI requires Node.js 21+." false "Upgrade Node.js to 21+ and rerun this installer."
      return
    fi
    if ! command_exists npm; then
      set_status opencli unavailable "npm is required for the official OpenCLI install path." false "Install npm and rerun this installer."
      return
    fi
    set_status opencli unavailable "opencli CLI is not installed." true "This installer can run npm install -g @jackwener/opencli."
    return
  fi

  local doctor_output
  doctor_output="$(opencli doctor 2>&1 || true)"
  if opencli doctor >/dev/null 2>&1; then
    set_status opencli ready "opencli doctor succeeded." false "No action required."
    return
  fi

  doctor_output="$(printf '%s' "$doctor_output" | tr '\n' ' ' | sed 's/[[:space:]]\+/ /g' | xargs || true)"
  set_status opencli available-but-needs-init "OpenCLI CLI exists, but opencli doctor reported incomplete browser bridge or local session wiring. Last doctor output: ${doctor_output:-unknown}" true "Complete the Browser Bridge installation in Chrome or Chromium, then rerun opencli doctor."
}

check_tool() {
  case "$1" in
    gstack) check_gstack ;;
    graphify) check_graphify ;;
    opencli) check_opencli ;;
  esac
}

print_statuses() {
  print_section "Collaborative Tool Status"
  local tool
  for tool in "${TOOLS[@]}"; do
    echo "  - $tool: ${tool_status_state[$tool]}"
    echo "    ${tool_status_reason[$tool]}"
  done
}

parse_selection() {
  local answer="$1"
  local max_index="$2"
  local selected=()
  [ -z "$(printf '%s' "$answer" | xargs || true)" ] && { printf '%s\n' ""; return 0; }

  local item number
  IFS=',' read -r -a parts <<< "$answer"
  for item in "${parts[@]}"; do
    item="$(printf '%s' "$item" | xargs)"
    [ -z "$item" ] && continue
    case "$item" in
      *[!0-9]*)
        echo "Invalid selection '$item'." >&2
        return 1
        ;;
    esac
    number="$item"
    if [ "$number" -lt 1 ] || [ "$number" -gt "$max_index" ]; then
      echo "Selection '$item' is out of range." >&2
      return 1
    fi
    case " ${selected[*]-} " in
      *" $number "*) ;;
      *) selected+=("$number") ;;
    esac
  done

  printf '%s\n' "${selected[*]-}"
}

prompt_for_install_selection() {
  local candidates=()
  local tool
  for tool in "${TOOLS[@]}"; do
    if [ "${tool_status_can_install[$tool]:-false}" = true ]; then
      case "${tool_status_state[$tool]}" in
        unavailable|available-but-needs-init) candidates+=("$tool") ;;
      esac
    fi
  done

  if [ "${#candidates[@]}" -eq 0 ]; then
    echo "  [OK] No collaborative tools need installation or installer-assisted setup." >&2
    printf '%s\n' ""
    return 0
  fi

  if [ "${#candidates[@]}" -eq 1 ]; then
    local tool_name="${candidates[0]}"
    local answer
    read -r -p "  [PROMPT] ${tool_name} is not currently ready. Install ${tool_name} now? [Y/n] " answer
    case "$(printf '%s' "$answer" | tr '[:upper:]' '[:lower:]')" in
      n|no)
        echo "  [SKIP] ${tool_name} installation skipped by user." >&2
        printf '%s\n' ""
        ;;
      *) printf '%s\n' "$tool_name" ;;
    esac
    return 0
  fi

  echo "  [PROMPT] The following tools are missing or still need install-time setup. Enter one or more numbers separated by commas, or press Enter to skip all." >&2
  local index=1
  for tool in "${candidates[@]}"; do
    echo "    ${index}. ${tool} - ${tool_status_reason[$tool]}" >&2
    index=$((index + 1))
  done

  while true; do
    local answer parsed result=()
    read -r -p "  [PROMPT] Enter selection numbers (for example: 1,3): " answer
    if parsed="$(parse_selection "$answer" "${#candidates[@]}")"; then
      [ -z "$parsed" ] && { echo "  [SKIP] No collaborative tools selected for installation." >&2; printf '%s\n' ""; return 0; }
      for index in $parsed; do
        result+=("${candidates[$((index - 1))]}")
      done
      printf '%s\n' "${result[*]-}"
      return 0
    fi
    echo "  [WARN] Invalid selection. Please try again." >&2
  done
}

install_gstack() {
  print_section "Install gstack"
  install_bun
  echo "  [INFO] Running the official gstack clone + setup flow."
  if [ ! -d "$GSTACK_ROOT/.git" ]; then
    git clone --single-branch --depth 1 https://github.com/garrytan/gstack.git "$GSTACK_ROOT"
  fi
  (
    cd "$GSTACK_ROOT"
    ./setup
  )
  echo "  [OK] gstack install flow completed."
}

install_graphify() {
  print_section "Install graphify"
  local python_cmd
  python_cmd="$(get_python_cmd)"
  if ! python_at_least_310 "$python_cmd"; then
    echo "Python 3.10+ is required before graphify can be installed." >&2
    return 1
  fi

  local python_request
  python_request="$(get_python_version_request "$python_cmd")"

  if command_exists uv; then
    echo "  [INFO] Installing the official graphifyy package via uv tool install."
    uv tool install --force --python "$python_request" graphifyy
    ensure_path_dir "$UV_TOOL_BIN_DIR"
  else
    echo "  [INFO] Installing the official graphifyy package via pip."
    "$python_cmd" -m pip install graphifyy
  fi

  echo "  [INFO] Running the official graphify install command."
  if command_exists graphify; then
    graphify install
  elif command_exists uvx; then
    uvx --python "$python_request" --from graphifyy graphify install
  else
    "$python_cmd" -m graphify install
  fi
  echo "  [OK] graphify install flow completed."
}

resolve_opencli_asset() {
  local json
  json="$(curl -fsSL -H 'User-Agent: GAL-CollaborativeTools-Installer' "$OPENCLI_LATEST_API_URL")"
  local python_cmd
  if python_cmd="$(get_python_cmd)"; then
    RELEASE_JSON="$json" "$python_cmd" - <<'PY'
import json
import os

data = json.loads(os.environ['RELEASE_JSON'])
for asset in data.get('assets', []):
    name = asset.get('name', '')
    if name.startswith('opencli-extension-') and name.endswith('.zip'):
        print(name)
        print(asset.get('browser_download_url', ''))
        raise SystemExit(0)
raise SystemExit(1)
PY
    return 0
  fi

  echo "python3 or python is required to parse the latest OpenCLI release metadata." >&2
  return 1
}

ensure_opencli_extension_download() {
  local asset_info asset_name asset_url target_path
  asset_info="$(resolve_opencli_asset)"
  asset_name="$(printf '%s\n' "$asset_info" | sed -n '1p')"
  asset_url="$(printf '%s\n' "$asset_info" | sed -n '2p')"
  [ -n "$asset_name" ] || { echo "Could not resolve the latest OpenCLI Browser Bridge asset." >&2; return 1; }

  mkdir -p "$OPENCLI_DOWNLOAD_ROOT"
  target_path="$OPENCLI_DOWNLOAD_ROOT/$asset_name"
  if [ -f "$target_path" ]; then
    echo "  [SKIP] OpenCLI Browser Bridge zip already exists: $target_path" >&2
    printf '%s\n' "$target_path"
    return 0
  fi

  echo "  [INFO] Downloading $asset_url" >&2
  curl -fsSL "$asset_url" -o "$target_path"
  echo "  [OK] Downloaded OpenCLI Browser Bridge zip to $target_path" >&2
  printf '%s\n' "$target_path"
}

print_opencli_guide() {
  local downloaded_path="$1"
  echo ""
  echo "  [INFO] OpenCLI Browser Bridge install guide (official upstream):"
  echo "    1. Download the latest opencli-extension-v{version}.zip from the GitHub Releases page."
  echo "    2. Unzip it, open chrome://extensions, and enable Developer mode."
  echo "    3. Click Load unpacked and select the unzipped folder."
  echo "  [INFO] Downloaded file path: $downloaded_path"
  echo "  [INFO] GitHub repo: $OPENCLI_REPO_URL"
  echo "  [INFO] Releases page: $OPENCLI_RELEASES_URL"
  echo "  [INFO] After completing those steps, rerun opencli doctor."
}

install_opencli() {
  print_section "Install OpenCLI"
  local node_major
  node_major="$(get_node_major)"
  [ -n "$node_major" ] || { echo "Node.js 21+ is required before OpenCLI can be installed." >&2; return 1; }
  [ "$node_major" -ge 21 ] || { echo "Node.js 21+ is required before OpenCLI can be installed." >&2; return 1; }
  command_exists npm || { echo "npm is required before OpenCLI can be installed." >&2; return 1; }

  if ! command_exists opencli; then
    echo "  [INFO] Running the official OpenCLI npm install command."
    npm install -g @jackwener/opencli
  else
    echo "  [OK] opencli CLI already exists; skipping npm install."
  fi

  command_exists opencli || { echo "OpenCLI install completed, but the opencli command is still not available in this shell." >&2; return 1; }

  local downloaded_path
  downloaded_path="$(ensure_opencli_extension_download)"
  print_opencli_guide "$downloaded_path"
  printf '%s\n' "$downloaded_path"
}

declare -A BEFORE_STATUS=()
declare -A ACTION_TAKEN=()
declare -A AFTER_STATUS=()
declare -A DOWNLOADED_PATH=()

while [ "$#" -gt 0 ]; do
  case "$1" in
    --check)
      CHECK_ONLY=true
      shift
      ;;
    --tool=*)
      TOOLS+=("${1#--tool=}")
      shift
      ;;
    --tool)
      if [ "$#" -lt 2 ]; then
        echo "Missing value after --tool" >&2
        exit 1
      fi
      TOOLS+=("$2")
      shift 2
      ;;
    *)
      echo "Unknown argument: $1" >&2
      exit 1
      ;;
  esac
done

ensure_path_dir "$UV_TOOL_BIN_DIR"
ensure_path_dir "$BUN_BIN_DIR"

normalize_tools "${TOOLS[@]}"

for tool in "${TOOLS[@]}"; do
  check_tool "$tool"
done

print_statuses

if $CHECK_ONLY; then
  exit 0
fi

selection_line="$(prompt_for_install_selection)"
read -r -a SELECTED_TOOLS <<< "$selection_line"

selected_lookup=" ${SELECTED_TOOLS[*]-} "

for tool in "${TOOLS[@]}"; do
  BEFORE_STATUS["$tool"]="${tool_status_state[$tool]}"
  ACTION_TAKEN["$tool"]="no-op"
  DOWNLOADED_PATH["$tool"]=""

  if [[ "$selected_lookup" == *" $tool "* ]]; then
    check_tool "$tool"
    if [ "${tool_status_state[$tool]}" = ready ]; then
      ACTION_TAKEN["$tool"]="skipped - already ready before install step"
    elif [ "${tool_status_can_install[$tool]}" != true ]; then
      ACTION_TAKEN["$tool"]="skipped - install not applicable for this status"
    else
      if install_output="$({
          case "$tool" in
            gstack) install_gstack ;;
            graphify) install_graphify ;;
            opencli) install_opencli ;;
          esac
        } 2>&1)"; then
        printf '%s\n' "$install_output"
        case "$tool" in
          gstack) ACTION_TAKEN["$tool"]="installed via official clone + setup" ;;
          graphify) ACTION_TAKEN["$tool"]="installed via official graphifyy + graphify install" ;;
          opencli)
            ACTION_TAKEN["$tool"]="installed CLI and downloaded Browser Bridge zip"
            DOWNLOADED_PATH["$tool"]="$(printf '%s\n' "$install_output" | tail -n 1)"
            ;;
        esac
      else
        printf '%s\n' "$install_output"
        echo "  [ERROR] $tool install failed." >&2
        ACTION_TAKEN["$tool"]="install failed"
      fi
    fi
  elif [ "${tool_status_can_install[$tool]:-false}" = true ] && { [ "${tool_status_state[$tool]}" = unavailable ] || [ "${tool_status_state[$tool]}" = available-but-needs-init ]; }; then
    ACTION_TAKEN["$tool"]="skipped by user"
  fi

  check_tool "$tool"
  AFTER_STATUS["$tool"]="${tool_status_state[$tool]}"
done

print_section "Final Summary"
for tool in "${TOOLS[@]}"; do
  echo "  - $tool"
  echo "    before: ${BEFORE_STATUS[$tool]}"
  echo "    action: ${ACTION_TAKEN[$tool]}"
  echo "    after:  ${AFTER_STATUS[$tool]}"
  echo "    GAL collaboration: $(get_collaboration_label "${AFTER_STATUS[$tool]}")"
  echo "    reason: ${tool_status_reason[$tool]}"
  if [ "${AFTER_STATUS[$tool]}" != ready ]; then
    echo "    next:   ${tool_status_next[$tool]}"
  fi
  if [ "$tool" = opencli ] && [ -n "${DOWNLOADED_PATH[$tool]}" ]; then
    echo "    downloaded extension zip: ${DOWNLOADED_PATH[$tool]}"
    echo "    github: $OPENCLI_REPO_URL"
  fi
done