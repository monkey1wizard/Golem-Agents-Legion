#!/usr/bin/env bash
# T-022: macOS / Linux cross-platform oracle parity test runner
#
# Run this script on a Mac Mini (or Linux host) via SSH to validate TP-029/030.
#
# Usage (from the Windows side):
#   ssh mac-mini 'bash -s -- /path/to/repo' < scripts/test-t022-ssh.sh
#
# Usage (directly on the target machine inside the repo):
#   bash scripts/test-t022-ssh.sh
#
# Exit codes:
#   0 — all tests passed
#   1 — one or more test failures
#   2 — prerequisite missing (Rust toolchain not found and could not be installed)

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PLATFORM="$(uname -s)"
PASS_COUNT=0
FAIL_COUNT=0
SKIP_COUNT=0

print_header() {
    echo ""
    echo "================================================================="
    echo "  T-022 Cross-Platform Oracle Parity — $PLATFORM"
    echo "================================================================="
    echo "  Repo: $REPO_ROOT"
    echo "  Rust: $(rustc --version 2>/dev/null || echo 'not found')"
    echo "================================================================="
    echo ""
}

check_prerequisites() {
    if ! command -v rustc >/dev/null 2>&1 || ! command -v cargo >/dev/null 2>&1; then
        echo "[WARN] Rust toolchain not found."
        echo "       Install via: curl https://sh.rustup.rs -sSf | sh"
        echo "       Then re-run: bash scripts/test-t022-ssh.sh"
        exit 2
    fi

    if ! command -v jq >/dev/null 2>&1; then
        echo "[WARN] jq not found — oracle comparison checks that use jq will be skipped."
    fi
}

run_cargo_tests() {
    echo "--- [1/4] cargo test (all unit + integration tests) ---"
    echo ""

    cd "$REPO_ROOT"

    # Run the full test suite first — catches any platform-specific regressions
    if cargo test --workspace 2>&1 | tee /tmp/gal-t022-cargo-test.log; then
        PASS_COUNT=$((PASS_COUNT + 1))
        echo ""
        echo "[PASS] cargo test --workspace"
    else
        FAIL_COUNT=$((FAIL_COUNT + 1))
        echo ""
        echo "[FAIL] cargo test --workspace — see /tmp/gal-t022-cargo-test.log"
        echo "       Key lines:"
        grep -E '^(FAILED|error\[|thread .* panicked)' /tmp/gal-t022-cargo-test.log | head -20 || true
    fi
}

run_cross_platform_tests() {
    echo ""
    echo "--- [2/4] cargo test cross_platform_oracle_parity (unit, non-ignored) ---"
    echo ""

    cd "$REPO_ROOT"

    if cargo test --test cross_platform_oracle_parity 2>&1 | tee /tmp/gal-t022-cross.log; then
        PASS_COUNT=$((PASS_COUNT + 1))
        echo ""
        echo "[PASS] cross_platform_oracle_parity unit tests"
    else
        FAIL_COUNT=$((FAIL_COUNT + 1))
        echo ""
        echo "[FAIL] cross_platform_oracle_parity unit tests"
        grep -E '^(FAILED|thread .* panicked)' /tmp/gal-t022-cross.log | head -20 || true
    fi
}

run_ignored_integration_tests() {
    echo ""
    echo "--- [3/4] cargo test cross_platform_oracle_parity --include-ignored ---"
    echo ""

    cd "$REPO_ROOT"

    # These tests mutate HOME — run single-threaded to avoid races
    if cargo test --test cross_platform_oracle_parity -- --include-ignored --test-threads=1 \
        2>&1 | tee /tmp/gal-t022-cross-ignored.log; then
        PASS_COUNT=$((PASS_COUNT + 1))
        echo ""
        echo "[PASS] cross_platform_oracle_parity integration tests (--include-ignored)"
    else
        FAIL_COUNT=$((FAIL_COUNT + 1))
        echo ""
        echo "[FAIL] cross_platform_oracle_parity integration tests"
        grep -E '^(FAILED|thread .* panicked)' /tmp/gal-t022-cross-ignored.log | head -20 || true
    fi
}

run_bash_oracle_smoke() {
    echo ""
    echo "--- [4/4] Bash oracle smoke (build-core-plugin.sh --install-mode source) ---"
    echo ""

    if ! command -v jq >/dev/null 2>&1; then
        echo "[SKIP] jq not found — skipping Bash oracle smoke"
        SKIP_COUNT=$((SKIP_COUNT + 1))
        return 0
    fi

    # Create an isolated home for the oracle run
    ORACLE_HOME="$(mktemp -d /tmp/gal-t022-oracle-home.XXXXXX)"
    trap 'rm -rf "$ORACLE_HOME"' EXIT

    export HOME="$ORACLE_HOME"

    if bash "$REPO_ROOT/scripts/build-core-plugin.sh" \
        --install-mode source 2>&1 | tee /tmp/gal-t022-oracle.log; then

        CANONICAL_ROOT="$ORACLE_HOME/.gal/plugins/gal"

        # Structural checks matching the Rust render output
        ORACLE_OK=true

        check_path() {
            local path="$1"
            local label="$2"
            if [[ -e "$path" ]]; then
                echo "  [OK] $label"
            else
                echo "  [MISS] $label — expected: $path"
                ORACLE_OK=false
            fi
        }

        echo "  Checking canonical root structure at: $CANONICAL_ROOT"
        check_path "$CANONICAL_ROOT/agents"                       "agents/ directory"
        check_path "$CANONICAL_ROOT/agy-agents"                   "agy-agents/ directory"
        check_path "$CANONICAL_ROOT/skills"                       "skills/ directory"
        check_path "$CANONICAL_ROOT/commands"                     "commands/ directory"
        check_path "$CANONICAL_ROOT/.claude-plugin/plugin.json"   ".claude-plugin/plugin.json"
        check_path "$CANONICAL_ROOT/copilot-manifest.json"        "copilot-manifest.json"
        check_path "$CANONICAL_ROOT/plugin.json"                  "plugin.json (AGY)"

        # Intent assertion: golem-dockeeper must be present
        if ls "$CANONICAL_ROOT/agents/"*"dockeeper"* >/dev/null 2>&1; then
            echo "  [OK] golem-dockeeper in agents/ (intent assertion TP-007)"
        else
            echo "  [MISS] golem-dockeeper NOT found in agents/ — TP-007 FAIL"
            ORACLE_OK=false
        fi

        if ls "$CANONICAL_ROOT/skills/doc-sync" >/dev/null 2>&1; then
            echo "  [OK] doc-sync in skills/ (intent assertion TP-007)"
        else
            echo "  [MISS] doc-sync NOT found in skills/ — TP-007 FAIL"
            ORACLE_OK=false
        fi

        if [[ "$ORACLE_OK" == 'true' ]]; then
            PASS_COUNT=$((PASS_COUNT + 1))
            echo ""
            echo "[PASS] Bash oracle smoke — structure matches expected"
        else
            FAIL_COUNT=$((FAIL_COUNT + 1))
            echo ""
            echo "[FAIL] Bash oracle smoke — missing files (see above)"
        fi
    else
        FAIL_COUNT=$((FAIL_COUNT + 1))
        echo "[FAIL] build-core-plugin.sh failed — see /tmp/gal-t022-oracle.log"
        tail -20 /tmp/gal-t022-oracle.log || true
    fi
}

print_summary() {
    echo ""
    echo "================================================================="
    echo "  T-022 Summary — $PLATFORM"
    echo "================================================================="
    echo "  PASS:  $PASS_COUNT"
    echo "  FAIL:  $FAIL_COUNT"
    echo "  SKIP:  $SKIP_COUNT"
    echo "================================================================="

    if [[ "$FAIL_COUNT" -gt 0 ]]; then
        echo ""
        echo "  Result: FAIL — see log files in /tmp/gal-t022-*.log"
        echo ""
        exit 1
    else
        echo ""
        echo "  Result: PASS"
        echo ""
        # TP-029 (macOS) or TP-030 (Linux) evidence
        if [[ "$PLATFORM" == 'Darwin' ]]; then
            echo "  TP-029 PASS — macOS cross-platform parity confirmed"
        elif [[ "$PLATFORM" == 'Linux' ]]; then
            echo "  TP-030 PASS — Linux cross-platform parity confirmed"
        fi
        echo ""
    fi
}

print_header
check_prerequisites
run_cargo_tests
run_cross_platform_tests
run_ignored_integration_tests
run_bash_oracle_smoke
print_summary
