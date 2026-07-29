#!/usr/bin/env bash
#
# publish-public.sh — curated one-way export of the private source-of-truth
# (GitLab `origin`) to the public GitHub remote, WITHOUT planning/state.
#
# Maintainer-only. This is the approved shell exception to the zero-ps1/sh
# end-state and is not pending a Rust port. Operational authority is
# `.dev/release-runbook.md`. It is the ONLY sanctioned way to put code on the
# public remote — never `git push <public> main` directly (that ships .dev
# planning/state).
#
# Model (strategy method 1 — curated snapshot):
#   working tree (everything) ──push──► GitLab origin (private, planning tracked)
#                                          │  this script: strip planning,
#                                          ▼  commit a clean orphan snapshot
#                                       GitHub (curated, no planning/history)
#
# The public repo receives a single clean snapshot commit (force-pushed). It is
# NOT a per-commit mirror — by design, so planning can never leak via history.
#
# Usage:
#   packaging/publish-public.sh                 # DRY RUN (default): build + scan, no push
#   packaging/publish-public.sh --execute       # actually force-push the snapshot
#   packaging/publish-public.sh --remote github --branch main
#   packaging/publish-public.sh --include-project-md   # also publish .dev/project.md
#
# Exit non-zero on any safety violation (dirty tree, wrong remote, residual leak).

set -euo pipefail

REMOTE="github"
BRANCH="main"
EXECUTE=0
INCLUDE_PROJECT_MD=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --execute) EXECUTE=1 ;;
    --include-project-md) INCLUDE_PROJECT_MD=1 ;;
    --remote) REMOTE="${2:?--remote needs a value}"; shift ;;
    --branch) BRANCH="${2:?--branch needs a value}"; shift ;;
    -h|--help)
      sed -n '2,40p' "$0" | sed 's/^# \{0,1\}//'
      exit 0 ;;
    *) echo "publish-public: unknown argument '$1'" >&2; exit 2 ;;
  esac
  shift
done

# Tracked paths that must NEVER reach the public repo. (Gitignored private files —
# *.private.md, *.local.*, config.json, .tmp/ — are absent from `git archive
# HEAD` already, so only TRACKED planning/state needs stripping here.)
EXCLUDE_TRACKED=( ".dev" )

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "$REPO_ROOT"
SOURCE_SHA="$(git rev-parse --short HEAD)"
STAMP="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

# ── Safety preflight ────────────────────────────────────────────────────────────
if ! git diff --quiet || ! git diff --cached --quiet; then
  echo "publish-public: working tree is dirty — commit or stash first." >&2
  exit 1
fi

if ! PUBLIC_URL="$(git remote get-url "$REMOTE" 2>/dev/null)"; then
  echo "publish-public: public remote '$REMOTE' not found. Add it first:" >&2
  echo "  git remote add $REMOTE <github-url>" >&2
  exit 1
fi

# Refuse to publish the curated snapshot anywhere that looks like the private
# source. The public target must be GitHub.
if printf '%s' "$PUBLIC_URL" | grep -qi 'gitlab\.com'; then
  echo "publish-public: remote '$REMOTE' points at GitLab ($PUBLIC_URL)." >&2
  echo "  That is the PRIVATE source-of-truth — never publish the curated snapshot there." >&2
  exit 1
fi
if ! printf '%s' "$PUBLIC_URL" | grep -qi 'github\.com'; then
  echo "publish-public: remote '$REMOTE' ($PUBLIC_URL) is not a github.com URL — refusing." >&2
  exit 1
fi

# ── Build the curated snapshot in an isolated temp dir ──────────────────────────
SNAP="$(mktemp -d)"
cleanup() { rm -rf "$SNAP"; }
trap cleanup EXIT

# `git archive HEAD` emits only TRACKED files at HEAD (gitignored private files
# are not included). Bypass the `gal-config` smudge filter (the `gal` binary may
# not be on PATH, and the public snapshot wants the clean/canonical committed
# blob, not a personalized smudge).
git -c filter.gal-config.smudge=cat -c filter.gal-config.clean=cat \
  archive --format=tar HEAD | tar -x -C "$SNAP"

# Strip tracked planning/state.
for path in "${EXCLUDE_TRACKED[@]}"; do
  rm -rf "${SNAP:?}/${path}"
done
# Optionally re-include only the project summary.
if [[ "$INCLUDE_PROJECT_MD" -eq 1 ]]; then
  mkdir -p "$SNAP/.dev"
  git show "HEAD:.dev/project.md" > "$SNAP/.dev/project.md" 2>/dev/null \
    || echo "publish-public: warning — .dev/project.md not found at HEAD; skipping" >&2
fi
# Scan for private-pattern files BEFORE deleting them so the residual-leak
# tripwire below can actually fire. (Deleting first left the check permanently
# dead — the post-delete find always returned nothing.)
PRIVATE_HITS="$(find "$SNAP" -type f -name '*.private.md' -print)"
# Defensive belt-and-braces: drop any private-pattern files that slipped through.
find "$SNAP" -type f \( -name '*.private.md' -o -name '*.local.*' -o -name 'config.json' \) -delete

# ── Residual-leak tripwire ──────────────────────────────────────────────────────
LEAKS=0
# Plans live under .dev/plans/ (migrated 2026-06-26); the .dev check covers them.
if [[ -e "$SNAP/.dev/state.md" || -d "$SNAP/.dev/plans" ]]; then echo "LEAK: .dev planning/state present in snapshot" >&2; LEAKS=1; fi
if [[ -n "$PRIVATE_HITS" ]]; then echo "LEAK: private notes present:" >&2; echo "$PRIVATE_HITS" >&2; LEAKS=1; fi
# Light secret tripwire — not exhaustive, just a backstop against obvious tokens.
if grep -rIlE 'BEGIN (RSA |OPENSSH |EC )?PRIVATE KEY|glpat-[A-Za-z0-9_-]{20}|gho_[A-Za-z0-9]{36}|xox[baprs]-[A-Za-z0-9-]+' "$SNAP" >/dev/null 2>&1; then
  echo "LEAK: possible secret material detected in snapshot:" >&2
  grep -rIlE 'BEGIN (RSA |OPENSSH |EC )?PRIVATE KEY|glpat-[A-Za-z0-9_-]{20}|gho_[A-Za-z0-9]{36}|xox[baprs]-[A-Za-z0-9-]+' "$SNAP" >&2 || true
  LEAKS=1
fi
if [[ "$LEAKS" -ne 0 ]]; then
  echo "publish-public: ABORT — residual private content in snapshot. Nothing pushed." >&2
  exit 1
fi

# ── Commit the orphan snapshot ──────────────────────────────────────────────────
SNAP_FILES="$(find "$SNAP" -type f | wc -l | tr -d ' ')"
git -C "$SNAP" init -q -b "$BRANCH"
# Neutralize the gal-config clean/smudge filter in the snapshot repo: the files
# are already canonical (from `git archive`), and the public repo must not depend
# on the GAL binary to check out. (.gitattributes is still published as-is.)
git -C "$SNAP" config filter.gal-config.clean cat
git -C "$SNAP" config filter.gal-config.smudge cat
git -C "$SNAP" -c user.name='GAL Publish' -c user.email='noreply@golem-agents-legion' add -A
git -C "$SNAP" -c user.name='GAL Publish' -c user.email='noreply@golem-agents-legion' \
  commit -q -m "Public snapshot from ${SOURCE_SHA} (${STAMP})

Curated export — planning (.dev) stripped. Not a per-commit mirror."
git -C "$SNAP" remote add public "$PUBLIC_URL"

# ── Report / push ───────────────────────────────────────────────────────────────
echo "─────────────────────────────────────────────"
echo "publish-public — curated snapshot"
echo "  source HEAD     : ${SOURCE_SHA}"
echo "  public remote   : ${REMOTE} → ${PUBLIC_URL}"
echo "  public branch   : ${BRANCH}"
echo "  stripped        : ${EXCLUDE_TRACKED[*]}$( [[ $INCLUDE_PROJECT_MD -eq 1 ]] && echo ' (kept .dev/project.md)' )"
echo "  files published : ${SNAP_FILES}"
echo "  residual scan   : PASS (no planning/state/secret residue)"
echo "─────────────────────────────────────────────"

if [[ "$EXECUTE" -ne 1 ]]; then
  echo "DRY RUN — nothing pushed. Re-run with --execute to force-push the snapshot:"
  echo "  git -C <snap> push --force public ${BRANCH}:${BRANCH}"
  exit 0
fi

echo "Force-pushing curated snapshot to ${REMOTE}/${BRANCH} ..."
git -C "$SNAP" push --force public "${BRANCH}:${BRANCH}"
echo "Done. Public ${REMOTE}/${BRANCH} now holds the curated snapshot of ${SOURCE_SHA}."
