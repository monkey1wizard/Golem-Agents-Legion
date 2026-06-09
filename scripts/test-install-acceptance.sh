#!/usr/bin/env bash
# GAL end-to-end install acceptance script (T-009 / TP-11 / TP-15)
#
# Verifies that a clean-environment `gal install` produced a correct and complete
# installation: skill surface aligned, canonical root complete, gal doctor green.
#
# Usage (on the target machine after `gal install`):
#   bash scripts/test-install-acceptance.sh
#
# Usage (from Windows side via SSH to mac-mini):
#   ssh mac-mini 'bash -s' < scripts/test-install-acceptance.sh
#
# Exit codes:
#   0 — all checks passed
#   1 — one or more checks failed

set -uo pipefail

PASS=0
FAIL=0

pass() { echo "  PASS  $1"; PASS=$((PASS + 1)); }
fail() { echo "  FAIL  $1"; FAIL=$((FAIL + 1)); }
header() { echo ""; echo "=== $1 ==="; }

CANONICAL_ROOT="${HOME}/.gal/plugins/gal"
SKILL_SURFACE="${HOME}/.claude/skills/gal"
PLUGINS_PARENT="${HOME}/.gal/plugins"

header "GAL install acceptance — $(uname -s) $(uname -m)"
echo "  Canonical root : ${CANONICAL_ROOT}"
echo "  Skill surface  : ${SKILL_SURFACE}"

header "1. gal binary"
if command -v gal > /dev/null 2>&1; then
    pass "gal is on PATH"
else
    fail "gal is NOT on PATH"
fi

header "2. gal doctor"
if gal doctor > /dev/null 2>&1; then
    pass "gal doctor exits 0"
else
    fail "gal doctor exits non-zero"
    echo "       (run 'gal doctor' manually to see findings)"
fi

header "3. Canonical root"
if [ -d "${CANONICAL_ROOT}" ]; then
    pass "canonical root exists: ${CANONICAL_ROOT}"
else
    fail "canonical root missing: ${CANONICAL_ROOT}"
fi

header "4. Agent completeness (golem-dockeeper)"
if [ -f "${CANONICAL_ROOT}/agents/golem-dockeeper.agent.md" ]; then
    pass "golem-dockeeper.agent.md present in canonical root agents/"
else
    fail "golem-dockeeper.agent.md missing from ${CANONICAL_ROOT}/agents/"
fi

header "5. Skill completeness (doc-sync)"
if [ -d "${CANONICAL_ROOT}/skills/doc-sync" ]; then
    pass "skills/doc-sync/ present in canonical root"
else
    fail "skills/doc-sync/ missing from ${CANONICAL_ROOT}/skills/"
fi

header "6. Claude skill surface"
if [ -L "${SKILL_SURFACE}" ]; then
    pass "skill surface is a symlink: ${SKILL_SURFACE}"
    TARGET="$(readlink -f "${SKILL_SURFACE}" 2>/dev/null || true)"
    if [ "${TARGET}" = "$(cd "${CANONICAL_ROOT}" 2>/dev/null && pwd -P)" ]; then
        pass "skill surface points to canonical root"
    else
        fail "skill surface points to unexpected target: ${TARGET}"
    fi
elif [ -d "${SKILL_SURFACE}" ]; then
    # macOS may use a junction-equivalent; accept directory too
    pass "skill surface exists (directory): ${SKILL_SURFACE}"
else
    fail "skill surface missing: ${SKILL_SURFACE} — run 'gal install' to create it"
fi

header "7. No orphan render temp dirs"
ORPHAN_COUNT=0
if [ -d "${PLUGINS_PARENT}" ]; then
    while IFS= read -r -d '' orphan; do
        fail "orphan render dir found: ${orphan}"
        ORPHAN_COUNT=$((ORPHAN_COUNT + 1))
    done < <(find "${PLUGINS_PARENT}" -maxdepth 1 -type d -name '.gal-render-*' -print0 2>/dev/null)
fi
if [ "${ORPHAN_COUNT}" -eq 0 ]; then
    pass "no orphan .gal-render-* dirs"
fi

header "8. Plugin bin exposure"
BIN_NAME="gal"
BIN_PATH="${CANONICAL_ROOT}/bin/${BIN_NAME}"
if [ -f "${BIN_PATH}" ]; then
    if [ -x "${BIN_PATH}" ]; then
        pass "plugin bin/${BIN_NAME} exists and is executable"
    else
        fail "plugin bin/${BIN_NAME} exists but is NOT executable (+x missing)"
    fi
else
    fail "plugin bin/${BIN_NAME} missing from canonical root"
fi

echo ""
echo "================================================================="
echo "  Result: ${PASS} passed, ${FAIL} failed"
echo "================================================================="

if [ "${FAIL}" -gt 0 ]; then
    exit 1
fi
exit 0
