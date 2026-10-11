#!/usr/bin/env bash
set -euo pipefail
REMOTE=github BRANCH=main EXECUTE=0 TAG= KEEP_SNAPSHOT=
usage() { cat <<'EOF'
Usage: packaging/publish-public.sh [--remote NAME] [--branch main] [--tag vX.Y.Z[-suffix]]
       packaging/publish-public.sh --execute --tag vX.Y.Z[-suffix]
Without --execute, build and inspect a temporary snapshot without pushing.
--keep-snapshot DIR retains that dry-run snapshot in a new empty directory.
EOF
}
while (($#)); do
  case "$1" in
    --execute) EXECUTE=1 ;;
    --remote|--branch|--tag|--keep-snapshot)
      (($# >= 2)) || { echo "publish-public: $1 needs a value" >&2; exit 2; }
      case "$1" in --remote) REMOTE=$2 ;; --branch) BRANCH=$2 ;; --tag) TAG=$2 ;; --keep-snapshot) KEEP_SNAPSHOT=$2 ;; esac
      shift ;;
    -h|--help) usage; exit 0 ;;
    --include-project-md|--force-tag) echo "publish-public: removed option '$1'" >&2; exit 2 ;;
    *) echo "publish-public: unknown argument '$1'" >&2; exit 2 ;;
  esac
  shift
done
[[ "$BRANCH" == main ]] || { echo 'publish-public: only the main branch can be published' >&2; exit 1; }
[[ "$EXECUTE" -eq 0 || -n "$TAG" ]] || { echo 'publish-public: --execute requires --tag.' >&2; exit 1; }
[[ -z "$TAG" || "$TAG" =~ ^v[0-9]+\.[0-9]+\.[0-9]+(-[A-Za-z0-9][A-Za-z0-9.-]*)?$ ]] || { echo 'publish-public: invalid release tag.' >&2; exit 1; }
if [[ -n "$TAG" && "$EXECUTE" -ne 1 ]]; then echo 'publish-public: --tag requires --execute.' >&2; exit 1; fi
if [[ -n "$KEEP_SNAPSHOT" ]]; then
  if [[ "$EXECUTE" -eq 1 || -L "$KEEP_SNAPSHOT" || -e "$KEEP_SNAPSHOT" && ( ! -d "$KEEP_SNAPSHOT" || -n "$(find "$KEEP_SNAPSHOT" -mindepth 1 -print -quit 2>/dev/null)" ) ]]; then
    echo 'publish-public: --keep-snapshot requires a dry run and a new or empty non-link directory.' >&2; exit 1
  fi
fi
ROOT=$(git rev-parse --show-toplevel); cd "$ROOT"
if ! git diff --quiet || ! git diff --cached --quiet; then echo 'publish-public: working tree is dirty — commit or stash first.' >&2; exit 1; fi
valid_url() {
  local url=${1,,}
  case "$url" in
    https://github.com/monkey1wizard/golem-agents-legion|https://github.com/monkey1wizard/golem-agents-legion.git|git@github.com:monkey1wizard/golem-agents-legion|git@github.com:monkey1wizard/golem-agents-legion.git) return 0 ;;
    *) return 1 ;;
  esac
}
# Reject rewrite rules because they can make an allowed literal URL resolve elsewhere.
if git config --get-regexp '^url\..*\.(insteadof|pushinsteadof)$' >/dev/null 2>&1; then
  echo 'publish-public: URL rewrite rules are not allowed for publication.' >&2; exit 1
fi
CONFIGURED_URLS=$(git config --get-all "remote.$REMOTE.url" 2>/dev/null) || { echo "publish-public: remote '$REMOTE' is not configured." >&2; exit 1; }
while IFS= read -r configured_url; do
  [[ -n "$configured_url" ]] || continue
  valid_url "$configured_url" || { echo "publish-public: configured fetch destination rejected for '$REMOTE'." >&2; exit 1; }
done <<< "$CONFIGURED_URLS"
CONFIGURED_PUSHURLS=$(git config --get-all "remote.$REMOTE.pushurl" 2>/dev/null || true)
if [[ -n "$CONFIGURED_PUSHURLS" ]]; then
  while IFS= read -r configured_url; do
    [[ -n "$configured_url" ]] || continue
    valid_url "$configured_url" || { echo "publish-public: configured push destination rejected for '$REMOTE'." >&2; exit 1; }
  done <<< "$CONFIGURED_PUSHURLS"
fi
FETCH_URL=$(git remote get-url "$REMOTE" 2>/dev/null) || { echo "publish-public: remote '$REMOTE' fetch URL lookup failed." >&2; exit 1; }
PUSH_URL=$(git remote get-url --push "$REMOTE" 2>/dev/null) || { echo "publish-public: remote '$REMOTE' push URL lookup failed." >&2; exit 1; }
if ! valid_url "$FETCH_URL" || ! valid_url "$PUSH_URL"; then
  echo "publish-public: remote '$REMOTE' destination rejected." >&2; exit 1
fi
if [[ -n "$TAG" ]]; then
  if git ls-remote --exit-code --tags "$PUSH_URL" "refs/tags/$TAG" "refs/tags/$TAG^{}" >/dev/null 2>&1; then
    echo "publish-public: tag '$TAG' already exists at destination." >&2; exit 1
  else
    status=$?
    [[ "$status" -eq 2 ]] || { echo "publish-public: tag-state lookup failed for '$TAG'." >&2; exit 1; }
  fi
fi
EXCLUDE=(.dev .gitlab-ci.yml .github/workflows/publish-public.yml)
if [[ -n "$KEEP_SNAPSHOT" && -d "$KEEP_SNAPSHOT" && ! -L "$KEEP_SNAPSHOT" ]]; then SNAP=$(mktemp -d); else SNAP=$(mktemp -d); fi
cleanup() { if [[ -n "${SNAP:-}" && -d "$SNAP" ]]; then rm -rf -- "$SNAP"; fi; }
trap cleanup EXIT
# Curated snapshot: archive tracked source, then remove private planning and controller paths.
git -c filter.gal-config.smudge=cat -c filter.gal-config.clean=cat archive --format=tar HEAD | tar -x -C "$SNAP"
for path in "${EXCLUDE[@]}"; do rm -rf -- "$SNAP/$path"; done
PRIVATE_HITS=$(find "$SNAP" -type f -name '*.private.md' -print)
find "$SNAP" -type f \( -name '*.private.md' -o -name '*.local.*' -o -name config.json \) -delete
LEAKS=0
if [[ -e "$SNAP/.dev/state.md" || -d "$SNAP/.dev/plans" ]]; then echo 'LEAK: .dev planning/state present in snapshot' >&2; LEAKS=1; fi
if [[ -n "$PRIVATE_HITS" ]]; then echo 'LEAK: private notes present in snapshot.' >&2; LEAKS=1; fi
for path in "${EXCLUDE[@]}"; do [[ ! -e "$SNAP/$path" ]] || { echo "LEAK: excluded path present: $path" >&2; LEAKS=1; }; done
PATTERN='BEGIN (RSA |OPENSSH |EC )?PRIVATE KEY|glpat-[A-Za-z0-9_-]{20}|gho_[A-Za-z0-9]{36}|xox[baprs]-[A-Za-z0-9-]+'
set +e
SCAN_RESULTS=$(grep -rIlE "$PATTERN" "$SNAP" 2>&1); SCAN_STATUS=$?
set -e
if [[ "$SCAN_STATUS" -eq 0 ]]; then echo 'LEAK: possible secret material detected:' >&2; printf '%s\n' "$SCAN_RESULTS" >&2; LEAKS=1
elif [[ "$SCAN_STATUS" -ne 1 ]]; then echo 'publish-public: secret scan failed.' >&2; LEAKS=1; fi
[[ "$LEAKS" -eq 0 ]] || { echo 'publish-public: ABORT — snapshot safety checks failed; nothing pushed.' >&2; exit 1; }
FILE_COUNT=$(find "$SNAP" -type f | wc -l | tr -d ' ')
git -C "$SNAP" init -q -b main
git -C "$SNAP" config filter.gal-config.clean cat
git -C "$SNAP" config filter.gal-config.smudge cat
git -C "$SNAP" -c user.name='GAL Publish' -c user.email=noreply@golem-agents-legion add -A
SOURCE_SHA=$(git rev-parse --short HEAD); STAMP=$(date -u +%Y-%m-%dT%H:%M:%SZ)
git -C "$SNAP" -c user.name='GAL Publish' -c user.email=noreply@golem-agents-legion commit -q -m "Public snapshot from $SOURCE_SHA ($STAMP)" -m 'Curated export; planning (.dev) and private controller stripped.'
SNAP_SHA=$(git -C "$SNAP" rev-parse HEAD)
[[ -z "$TAG" ]] || git -C "$SNAP" tag "$TAG" HEAD
git -C "$SNAP" remote add "$REMOTE" "$PUSH_URL"
echo "publish-public: source=$SOURCE_SHA snapshot=$SNAP_SHA remote=$REMOTE branch=main files=$FILE_COUNT"
echo 'publish-public: heuristic scan found no configured secret patterns.'
if [[ "$EXECUTE" -ne 1 ]]; then
  if [[ -n "$KEEP_SNAPSHOT" ]]; then
    if [[ -d "$KEEP_SNAPSHOT" ]]; then rmdir -- "$KEEP_SNAPSHOT"; fi
    mkdir -p -- "$(dirname "$KEEP_SNAPSHOT")"
    mv -- "$SNAP" "$KEEP_SNAPSHOT"; SNAP=
    echo "publish-public: snapshot retained at $KEEP_SNAPSHOT"
  fi
  echo 'DRY RUN — nothing pushed.'; exit 0
fi
REFS=(+refs/heads/main:refs/heads/main "refs/tags/$TAG:refs/tags/$TAG")
echo 'publish-public: pushing main and requested tag in one atomic transaction.'
git -C "$SNAP" push --atomic "$REMOTE" "${REFS[@]}"
echo "publish-public: published snapshot $SNAP_SHA to $REMOTE/main."