#!/usr/bin/env bash
set -euo pipefail
REMOTE=github BRANCH=main EXECUTE=0 CANDIDATE=0 TAG=
PUBLIC_REPO=monkey1wizard/Golem-Agents-Legion
CANDIDATE_REF=refs/heads/release-candidate
usage() { cat <<'EOF'
Usage: packaging/publish-public.sh [--remote NAME] [--branch main]
       packaging/publish-public.sh --candidate --tag vX.Y.Z[-suffix]
       packaging/publish-public.sh --execute --tag vX.Y.Z[-suffix]
Without a mode, build and inspect a temporary snapshot without pushing.
--candidate pushes the snapshot to the public release-candidate branch only.
The release.yml rehearsal runs there and publishes nothing. No tag and no
main update happen.
--execute promotes the rehearsed candidate: it requires the candidate to match
HEAD and the tag, and its latest release.yml rehearsal to have succeeded. It
then pushes main to that commit and starts the release.yml publish run, which
creates a pre-release and promotes it to the full release after every check.
EOF
}
while (($#)); do
  case "$1" in
    --execute) EXECUTE=1 ;;
    --candidate) CANDIDATE=1 ;;
    --remote|--branch|--tag)
      (($# >= 2)) || { echo "publish-public: $1 needs a value" >&2; exit 2; }
      case "$1" in --remote) REMOTE=$2 ;; --branch) BRANCH=$2 ;; --tag) TAG=$2 ;; esac
      shift ;;
    -h|--help) usage; exit 0 ;;
    --include-project-md|--force-tag|--keep-snapshot) echo "publish-public: removed option '$1'" >&2; exit 2 ;;
    *) echo "publish-public: unknown argument '$1'" >&2; exit 2 ;;
  esac
  shift
done
[[ "$BRANCH" == main ]] || { echo 'publish-public: only the main branch can be published' >&2; exit 1; }
[[ "$EXECUTE" -eq 0 || "$CANDIDATE" -eq 0 ]] || { echo 'publish-public: choose --candidate or --execute, not both.' >&2; exit 1; }
PUSHING=$(( EXECUTE + CANDIDATE ))
[[ "$EXECUTE" -eq 0 || -n "$TAG" ]] || { echo 'publish-public: --execute requires --tag.' >&2; exit 1; }
[[ "$CANDIDATE" -eq 0 || -n "$TAG" ]] || { echo 'publish-public: --candidate requires --tag.' >&2; exit 1; }
[[ -z "$TAG" || "$TAG" =~ ^v[0-9]+\.[0-9]+\.[0-9]+(-[A-Za-z0-9][A-Za-z0-9.-]*)?$ ]] || { echo 'publish-public: invalid release tag.' >&2; exit 1; }
if [[ -n "$TAG" && "$PUSHING" -ne 1 ]]; then echo 'publish-public: --tag requires --candidate or --execute.' >&2; exit 1; fi
ROOT=$(git rev-parse --show-toplevel); cd "$ROOT"
if ! git diff --quiet || ! git diff --cached --quiet; then echo 'publish-public: working tree is dirty — commit or stash first.' >&2; exit 1; fi
if [[ "$PUSHING" -eq 1 ]]; then
  # Releases run from the dedicated release worktree, detached at the main tip,
  # so other work in the primary checkout never blocks or leaks into them.
  # Checking out main itself also works.
  CURRENT_BRANCH=$(git symbolic-ref -q --short HEAD || true)
  MAIN_SHA=$(git rev-parse -q --verify refs/heads/main || true)
  if [[ "$CURRENT_BRANCH" != main ]]; then
    [[ -z "$CURRENT_BRANCH" && -n "$MAIN_SHA" && "$(git rev-parse HEAD)" == "$MAIN_SHA" ]] || { echo "publish-public: current branch is ${CURRENT_BRANCH:-detached HEAD}; main or a detached HEAD at the main tip required." >&2; exit 1; }
  fi
fi
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
if [[ "$PUSHING" -eq 1 ]]; then
  VERSION=$(git show HEAD:crates/cli/Cargo.toml 2>/dev/null | grep -m1 '^version' | sed -nE 's/^version[[:space:]]*=[[:space:]]*"([^"]+)"[[:space:]]*$/\1/p' || true)
  RELEASE_VERSION=${TAG#v}; RELEASE_VERSION=${RELEASE_VERSION%%-*}
  [[ -n "$VERSION" && "$VERSION" == "$RELEASE_VERSION" ]] || { echo "publish-public: version mismatch: tag=$TAG manifest=${VERSION:-missing or unparsable}." >&2; exit 1; }
fi
if [[ -n "$TAG" ]]; then
  if git ls-remote --exit-code --tags "$PUSH_URL" "refs/tags/$TAG" "refs/tags/$TAG^{}" >/dev/null 2>&1; then
    # A rehearsal publishes nothing, so it may run while the tag still exists.
    # Publishing never moves an existing tag.
    [[ "$CANDIDATE" -eq 1 ]] || { echo "publish-public: tag '$TAG' already exists at destination." >&2; exit 1; }
    echo "publish-public: warning: tag '$TAG' already exists at destination. --execute stays blocked until the owner removes it." >&2
  else
    status=$?
    [[ "$status" -eq 2 ]] || { echo "publish-public: tag-state lookup failed for '$TAG'." >&2; exit 1; }
  fi
fi
EXCLUDE=(.dev .gitlab-ci.yml .github/workflows/publish-public.yml)
SNAP=$(mktemp -d)
cleanup() { rm -rf -- "$SNAP"; }
trap cleanup EXIT
export GIT_INDEX_FILE="$SNAP/index"
git read-tree HEAD
git ls-files -z -- "${EXCLUDE[@]}" ':(glob)**/*.local.*' ':(glob)**/config.json' | git update-index -z --force-remove --stdin
PRIVATE_HITS=$(git ls-files ':(glob)**/*.private.md')
LEAKS=0
if [[ -n "$PRIVATE_HITS" ]]; then echo 'LEAK: private notes present in snapshot.' >&2; LEAKS=1; fi
TREE=$(git write-tree)
TREE_PATHS=$(git ls-tree -r --name-only "$TREE")
for path in "${EXCLUDE[@]}"; do
  if printf '%s\n' "$TREE_PATHS" | grep -Fx -e "$path" -e "$path/" >/dev/null || printf '%s\n' "$TREE_PATHS" | grep -F "${path}/" | grep -q "^${path}/"; then
    echo "LEAK: excluded path present: $path" >&2; LEAKS=1
  fi
done
PATTERN='BEGIN (RSA |OPENSSH |EC )?PRIVATE KEY|glpat-[A-Za-z0-9_-]{20}|gho_[A-Za-z0-9]{36}|xox[baprs]-[A-Za-z0-9-]+'
set +e
SCAN_RESULTS=$(git grep -I -l -E "$PATTERN" "$TREE" 2>&1); SCAN_STATUS=$?
set -e
if [[ "$SCAN_STATUS" -eq 0 ]]; then echo 'LEAK: possible secret material detected:' >&2; printf '%s\n' "$SCAN_RESULTS" >&2; LEAKS=1
elif [[ "$SCAN_STATUS" -ne 1 ]]; then echo 'publish-public: secret scan failed.' >&2; LEAKS=1; fi
[[ "$LEAKS" -eq 0 ]] || { echo 'publish-public: ABORT — snapshot safety checks failed; nothing pushed.' >&2; exit 1; }
FILE_COUNT=$(git ls-tree -r --name-only "$TREE" | wc -l | tr -d ' ')
SOURCE_SHA=$(git rev-parse HEAD); SOURCE_SHORT=$(git rev-parse --short HEAD); STAMP=$(date -u +%Y-%m-%dT%H:%M:%SZ)

if [[ "$EXECUTE" -eq 1 ]]; then
  # Promote the tested candidate. Never rebuild: a rebuilt snapshot is a new,
  # untested commit because the commit message carries a timestamp.
  CANDIDATE_LINE=$(git ls-remote "$FETCH_URL" "$CANDIDATE_REF") || { echo 'publish-public: candidate lookup failed.' >&2; exit 1; }
  CANDIDATE_SHA=${CANDIDATE_LINE%%[[:space:]]*}
  [[ "$CANDIDATE_SHA" =~ ^[0-9a-f]{40}$ ]] || { echo 'publish-public: no release candidate found. Run --candidate first.' >&2; exit 1; }
  git fetch -q --no-tags "$FETCH_URL" "$CANDIDATE_REF" || { echo 'publish-public: candidate fetch failed.' >&2; exit 1; }
  [[ "$(git rev-parse FETCH_HEAD)" == "$CANDIDATE_SHA" ]] || { echo 'publish-public: candidate moved during fetch.' >&2; exit 1; }
  git log -1 --format=%B "$CANDIDATE_SHA" | grep -Fxq "Source-Commit: $SOURCE_SHA" || { echo "publish-public: candidate $CANDIDATE_SHA was not built from HEAD $SOURCE_SHA. Run --candidate again." >&2; exit 1; }
  [[ "$(git rev-parse "$CANDIDATE_SHA^{tree}")" == "$TREE" ]] || { echo 'publish-public: candidate tree differs from the snapshot of HEAD.' >&2; exit 1; }
  git log -1 --format=%B "$CANDIDATE_SHA" | grep -Fxq "Release-Tag: $TAG" || { echo "publish-public: candidate $CANDIDATE_SHA was not built for $TAG. Run --candidate --tag $TAG again." >&2; exit 1; }
  command -v gh >/dev/null 2>&1 || { echo 'publish-public: gh is required to check the release rehearsal.' >&2; exit 1; }
  REHEARSAL=$(gh run list --repo "$PUBLIC_REPO" --workflow release.yml --event push --commit "$CANDIDATE_SHA" --limit 1 --json databaseId,status,conclusion --jq '.[0] | "\(.databaseId) \(.status) \(.conclusion)"' 2>/dev/null) || { echo 'publish-public: release rehearsal lookup failed.' >&2; exit 1; }
  REHEARSAL_ID=${REHEARSAL%% *}; REHEARSAL_STATE=${REHEARSAL#* }
  [[ "$REHEARSAL_ID" =~ ^[0-9]+$ && "$REHEARSAL_STATE" == "completed success" ]] || { echo "publish-public: release rehearsal for candidate $CANDIDATE_SHA is '${REHEARSAL:-missing}', not 'completed success'. Nothing pushed." >&2; exit 1; }
  echo "publish-public: source=$SOURCE_SHA candidate=$CANDIDATE_SHA tree=$TREE rehearsal=$REHEARSAL_ID remote=$REMOTE files=$FILE_COUNT"
  echo 'publish-public: pushing main to the rehearsed candidate.'
  git push "$PUSH_URL" "+$CANDIDATE_SHA:refs/heads/main"
  # The publish run creates the tag and a GitHub pre-release itself, publishes
  # Homebrew and WinGet, runs every smoke check, and only then turns the
  # pre-release into the full release and Latest.
  gh workflow run release.yml --repo "$PUBLIC_REPO" --ref main -f "rehearsal_run_id=$REHEARSAL_ID" || { echo "publish-public: main is at $CANDIDATE_SHA, but starting the publish run failed. Start it with: gh workflow run release.yml --repo $PUBLIC_REPO --ref main -f rehearsal_run_id=$REHEARSAL_ID" >&2; exit 1; }
  echo "publish-public: main is at $CANDIDATE_SHA. Publish run for $TAG started from rehearsal $REHEARSAL_ID."
  exit 0
fi

TRAILERS="Source-Commit: $SOURCE_SHA"
[[ -z "$TAG" ]] || TRAILERS+=$'\n'"Release-Tag: $TAG"
SNAP_SHA=$(git -c user.name='GAL Publish' -c user.email=noreply@golem-agents-legion commit-tree "$TREE" -m "Public snapshot from $SOURCE_SHORT ($STAMP)" -m 'Curated export; planning (.dev) and excluded private paths stripped.' -m "$TRAILERS")
echo "publish-public: source=$SOURCE_SHA snapshot=$SNAP_SHA tree=$TREE remote=$REMOTE branch=main files=$FILE_COUNT"
echo 'publish-public: heuristic scan found no configured secret patterns.'
if [[ "$CANDIDATE" -ne 1 ]]; then echo 'DRY RUN — nothing pushed.'; exit 0; fi
echo 'publish-public: pushing the snapshot to the release-candidate branch only.'
git push "$PUSH_URL" "+$SNAP_SHA:$CANDIDATE_REF"
echo "publish-public: candidate $SNAP_SHA pushed for $TAG. Wait for the release.yml rehearsal on release-candidate, then run --execute --tag $TAG after the owner says pre-release."
