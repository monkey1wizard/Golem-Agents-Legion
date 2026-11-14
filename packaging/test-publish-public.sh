#!/usr/bin/env bash
set -euo pipefail
trap 'status=$?; echo "test-publish-public: failed at line $LINENO (exit $status)" >&2' ERR

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
PUBLISH="$SCRIPT_DIR/publish-public.sh"
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
REAL_GIT=$(command -v git)
mkdir -p "$TMP/home"
export HOME="$TMP/home" XDG_CONFIG_HOME="$TMP/home/.config"
export GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=$TMP/global.gitconfig GIT_TERMINAL_PROMPT=0
"$REAL_GIT" config --global core.autocrlf true
"$REAL_GIT" config --global core.hooksPath /dev/null
mkdir -p "$TMP/bin"
cat >"$TMP/bin/git" <<'WRAPPER'
#!/usr/bin/env bash
args=("$@")
command_name=
for arg in "${args[@]}"; do
  case "$arg" in push|ls-remote|get-url|fetch) command_name=$arg; break ;; esac
done
if [[ "$command_name" == push || "$command_name" == ls-remote || "$command_name" == fetch ]]; then
  for ((i=0; i<${#args[@]}; i++)); do
    if [[ "${args[$i]}" == github || "${args[$i]}" == https://github.com/monkey1wizard/Golem-Agents-Legion.git ]]; then args[$i]="$PUBLISH_TEST_RECEIVER"; fi
  done
fi
if [[ "$command_name" == push && " ${args[*]} " == *":refs/heads/release-candidate "* ]]; then
  [[ " ${args[*]} " =~ [[:space:]]\+[0-9a-f]+:refs/heads/release-candidate[[:space:]] && " ${args[*]} " != *"refs/heads/main"* && " ${args[*]} " != *"refs/tags/"* ]] || {
    echo 'candidate push must update only release-candidate' >&2; exit 98;
  }
elif [[ "$command_name" == push ]]; then
  # Promotion moves main only. The publish run creates the tag later.
  [[ " ${args[*]} " =~ [[:space:]]\+[0-9a-f]+:refs/heads/main[[:space:]] && " ${args[*]} " != *"refs/tags/"* ]] || {
    echo 'promotion push must update only main' >&2; exit 98;
  }
fi
if [[ "$command_name" == ls-remote && "${PUBLISH_TEST_EXISTING_TAG:-0}" == 1 && " ${args[*]} " == *" --tags "* ]]; then exit 0; fi
if [[ "$command_name" == push && "${PUBLISH_TEST_PUSH_REJECT:-0}" == 1 ]]; then exit 1; fi
if [[ "$command_name" == ls-remote && "${PUBLISH_TEST_LOOKUP_FAIL:-0}" == 1 ]]; then exit 128; fi
if [[ "$command_name" == get-url && "${PUBLISH_TEST_EFFECTIVE_LOOKUP_FAIL:-0}" == 1 ]]; then exit 128; fi
exec "$PUBLISH_TEST_REAL_GIT" "${args[@]}"
WRAPPER
chmod +x "$TMP/bin/git"
cat >"$TMP/bin/gh" <<'FAKEGH'
#!/usr/bin/env bash
if [[ "$1 $2" == "workflow run" ]]; then
  printf '%s\n' "$*" >>"$PUBLISH_TEST_GH_LOG"
  [[ "${PUBLISH_TEST_DISPATCH_FAIL:-0}" == 1 ]] && exit 1
  exit 0
fi
[[ "$1 $2" == "run list" ]] || { echo "unexpected gh call: $*" >&2; exit 97; }
[[ " $* " == *" --workflow release.yml "* && " $* " == *" --event push "* ]] || { echo "rehearsal lookup must query release.yml push runs: $*" >&2; exit 97; }
[[ "${PUBLISH_TEST_CI_LOOKUP_FAIL:-0}" == 1 ]] && exit 1
state=${PUBLISH_TEST_CI-completed success}
[[ -z "$state" ]] || printf '4242 %s\n' "$state"
FAKEGH
chmod +x "$TMP/bin/gh"
export PUBLISH_TEST_REAL_GIT=$REAL_GIT PATH="$TMP/bin:$PATH" PUBLISH_TEST_GH_LOG="$TMP/gh.log"
: >"$PUBLISH_TEST_GH_LOG"

init_repo() {
  local name=$1
  mkdir -p "$TMP/$name/source" "$TMP/$name/receiver"
  "$REAL_GIT" init -q -b main "$TMP/$name/source"
  "$REAL_GIT" -C "$TMP/$name/source" config core.filemode false
  "$REAL_GIT" -C "$TMP/$name/source" config user.name Fixture
  "$REAL_GIT" -C "$TMP/$name/source" config user.email fixture@example.invalid
  "$REAL_GIT" -C "$TMP/$name/source" config commit.gpgsign false
  "$REAL_GIT" -C "$TMP/$name/source" config core.hooksPath /dev/null
  "$REAL_GIT" -C "$TMP/$name/source" config filter.gal-config.clean 'sed s/personal/canonical/'
  "$REAL_GIT" -C "$TMP/$name/source" config filter.gal-config.smudge 'sed s/canonical/personal/'
  mkdir -p "$TMP/$name/source/.github/workflows" "$TMP/$name/source/docs/plans" "$TMP/$name/source/.dev/plans" "$TMP/$name/source/crates/cli" "$TMP/$name/source/dir.local.d" "$TMP/$name/source/sub"
  printf 'version = "1.2.3"\n' >"$TMP/$name/source/crates/cli/Cargo.toml"
  printf 'keep\n' >"$TMP/$name/source/dir.local.d/keep.txt"
  printf 'excluded\n' >"$TMP/$name/source/sub/config.json"
  printf 'excluded\n' >"$TMP/$name/source/sub/x.local.toml"
  printf 'excluded\n' >"$TMP/$name/source/config.json"
  printf 'excluded\n' >"$TMP/$name/source/example.local.txt"
  printf '#!/bin/sh\n' >"$TMP/$name/source/executable.sh"
  chmod +x "$TMP/$name/source/executable.sh"
  if [[ "$name" == success ]]; then
    mkdir -p "$TMP/$name/source/packaging"
    cp "$SCRIPT_DIR/test-publish-public.sh" "$TMP/$name/source/packaging/test-publish-public.sh"
  fi
  printf 'private controller\n' >"$TMP/$name/source/.gitlab-ci.yml"
  printf 'canonical\n' >"$TMP/$name/source/config.toml"
  printf 'config.toml filter=gal-config\n' >"$TMP/$name/source/.gitattributes"
  printf 'private\n' >"$TMP/$name/source/.dev/plans/secret.md"
  printf 'private\n' >"$TMP/$name/source/docs/plans/secret.md"
  printf 'controller\n' >"$TMP/$name/source/.github/workflows/publish-public.yml"
  printf 'release\n' >"$TMP/$name/source/.github/workflows/release.yml"
  "$REAL_GIT" -C "$TMP/$name/source" add -A
  "$REAL_GIT" -C "$TMP/$name/source" update-index --chmod=+x executable.sh
  "$REAL_GIT" -C "$TMP/$name/source" commit -qm source
  "$REAL_GIT" init -q --bare -b main "$TMP/$name/receiver"
  "$REAL_GIT" -C "$TMP/$name/source" remote add github https://github.com/monkey1wizard/Golem-Agents-Legion.git
  PUBLISH_TEST_RECEIVER="$TMP/$name/receiver"
  export PUBLISH_TEST_RECEIVER
}
ref() { "$REAL_GIT" --git-dir="$PUBLISH_TEST_RECEIVER" rev-parse --verify "$1" 2>/dev/null || true; }
run_rejected() {
  local before_main before_tag after_main after_tag
  before_main=$(ref refs/heads/main); before_tag=$(ref refs/tags/v1.2.3)
  if (cd "$1" && "$PUBLISH" --execute --tag v1.2.3) >"$TMP/out" 2>&1; then
    echo 'expected exporter rejection' >&2; cat "$TMP/out" >&2; exit 1
  fi
  after_main=$(ref refs/heads/main); after_tag=$(ref refs/tags/v1.2.3)
  printf 'rejection refs: main %s -> %s; tag %s -> %s\n' "$before_main" "$after_main" "$before_tag" "$after_tag"
  [[ "$after_main" == "$before_main" && "$after_tag" == "$before_tag" ]] || { echo "rejected publication changed receiver refs: main $before_main -> $after_main; tag $before_tag -> $after_tag" >&2; exit 1; }
}

# Invalid destinations, removed flags, and non-main targets reject without writes.
init_repo rejected
before=$(ref refs/heads/main)
"$REAL_GIT" -C "$TMP/rejected/source" remote set-url github https://evilgithub.com/monkey1wizard/Golem-Agents-Legion.git
run_rejected "$TMP/rejected/source"
"$REAL_GIT" -C "$TMP/rejected/source" remote set-url github https://github.com/monkey1wizard/Golem-Agents-Legion.git
PUBLISH_TEST_EFFECTIVE_LOOKUP_FAIL=1; export PUBLISH_TEST_EFFECTIVE_LOOKUP_FAIL
run_rejected "$TMP/rejected/source"
unset PUBLISH_TEST_EFFECTIVE_LOOKUP_FAIL
"$REAL_GIT" -C "$TMP/rejected/source" config --add remote.github.url https://example.invalid/extra.git
run_rejected "$TMP/rejected/source"
"$REAL_GIT" -C "$TMP/rejected/source" config --unset-all remote.github.url
"$REAL_GIT" -C "$TMP/rejected/source" config --add remote.github.url https://github.com/monkey1wizard/Golem-Agents-Legion.git
"$REAL_GIT" -C "$TMP/rejected/source" remote set-url github https://github.com/monkey1wizard/Golem-Agents-Legion.git
"$REAL_GIT" -C "$TMP/rejected/source" config remote.github.pushurl https://github.com/monkey1wizard/Other-Repo.git
run_rejected "$TMP/rejected/source"
"$REAL_GIT" -C "$TMP/rejected/source" config --unset remote.github.pushurl
for rewrite in insteadOf pushInsteadOf; do
  "$REAL_GIT" -C "$TMP/rejected/source" config "url.https://evil.example/.${rewrite}" https://github.com/monkey1wizard/Golem-Agents-Legion.git
  run_rejected "$TMP/rejected/source"
  "$REAL_GIT" -C "$TMP/rejected/source" config --unset-all "url.https://evil.example/.${rewrite}"
done
for option in --include-project-md --force-tag --keep-snapshot; do
  if (cd "$TMP/rejected/source" && "$PUBLISH" "$option") >/dev/null 2>&1; then exit 1; fi
done
if (cd "$TMP/rejected/source" && "$PUBLISH" --branch release) >/dev/null 2>&1; then exit 1; fi

# Success publishes an orphan snapshot with matching refs, canonical content, and no excluded paths.
init_repo success
dry_run_before_main=$(ref refs/heads/main); dry_run_before_tag=$(ref refs/tags/v1.2.3)
if ! (cd "$TMP/success/source" && "$PUBLISH") >"$TMP/dry-run.log" 2>&1; then
  cat "$TMP/dry-run.log" >&2; exit 1
fi
grep -q "source=$($REAL_GIT -C "$TMP/success/source" rev-parse HEAD)" "$TMP/dry-run.log"
grep -Eq 'snapshot=[0-9a-f]+ tree=[0-9a-f]+ .* files=[0-9]+' "$TMP/dry-run.log"
grep -q 'DRY RUN — nothing pushed.' "$TMP/dry-run.log"
[[ "$(ref refs/heads/main)" == "$dry_run_before_main" && "$(ref refs/tags/v1.2.3)" == "$dry_run_before_tag" ]]
# Execute without a candidate is refused.
run_rejected "$TMP/success/source"
grep -q 'Run --candidate first' "$TMP/out"
# A candidate updates only release-candidate.
if ! (cd "$TMP/success/source" && "$PUBLISH" --candidate --tag v1.2.3) >"$TMP/candidate.log" 2>&1; then
  cat "$TMP/candidate.log" >&2; exit 1
fi
CANDIDATE=$(ref refs/heads/release-candidate)
[[ -n "$CANDIDATE" && -z "$(ref refs/heads/main)" && -z "$(ref refs/tags/v1.2.3)" ]]
[[ "$("$REAL_GIT" --git-dir="$PUBLISH_TEST_RECEIVER" show -s --format=%B "$CANDIDATE")" == *"Release-Tag: v1.2.3"* ]]
# A candidate for another tag cannot be promoted under this tag.
if (cd "$TMP/success/source" && "$PUBLISH" --execute --tag v1.2.3-rc.1) >"$TMP/out" 2>&1; then exit 1; fi
grep -q 'was not built for v1.2.3-rc.1' "$TMP/out"
# An unfinished, failed, cancelled, or missing rehearsal refuses promotion.
for state in 'in_progress ' 'completed failure' 'completed cancelled' ''; do
  PUBLISH_TEST_CI=$state; export PUBLISH_TEST_CI
  run_rejected "$TMP/success/source"
  grep -q 'Nothing pushed' "$TMP/out"
done
unset PUBLISH_TEST_CI
PUBLISH_TEST_CI_LOOKUP_FAIL=1; export PUBLISH_TEST_CI_LOOKUP_FAIL
run_rejected "$TMP/success/source"
grep -q 'release rehearsal lookup failed' "$TMP/out"
unset PUBLISH_TEST_CI_LOOKUP_FAIL
[[ ! -s "$PUBLISH_TEST_GH_LOG" ]] || { echo 'a refused promotion started a publish run' >&2; exit 1; }
if ! (cd "$TMP/success/source" && "$PUBLISH" --execute --tag v1.2.3) >"$TMP/success.log" 2>&1; then
  cat "$TMP/success.log" >&2; exit 1
fi
P=$(ref refs/heads/main)
[[ -n "$P" && "$P" == "$CANDIDATE" && -z "$(ref refs/tags/v1.2.3)" ]]
grep -Fxq 'workflow run release.yml --repo monkey1wizard/Golem-Agents-Legion --ref main -f rehearsal_run_id=4242' "$PUBLISH_TEST_GH_LOG"
printf 'receiver evidence: main=%s tag=none publish=%s\n' "$P" "$(cat "$PUBLISH_TEST_GH_LOG")"
source_harness=$($REAL_GIT -C "$TMP/success/source" ls-tree HEAD packaging/test-publish-public.sh)
retained_harness=$($REAL_GIT --git-dir="$PUBLISH_TEST_RECEIVER" ls-tree "$P" packaging/test-publish-public.sh)
[[ -n "$source_harness" && "$retained_harness" == "$source_harness" ]]
printf 'retained harness evidence: %s\n' "$retained_harness"
echo 'export tree evidence:'
"$REAL_GIT" --git-dir="$PUBLISH_TEST_RECEIVER" ls-tree -r "$P"
printf 'tree ID: %s\n' "$("$REAL_GIT" --git-dir="$PUBLISH_TEST_RECEIVER" rev-parse "$P^{tree}")"
[[ "$("$REAL_GIT" --git-dir="$PUBLISH_TEST_RECEIVER" ls-tree "$P" executable.sh)" == 100755* ]]
"$REAL_GIT" --git-dir="$PUBLISH_TEST_RECEIVER" cat-file -e "$P:dir.local.d/keep.txt"
for path in config.json example.local.txt sub/config.json sub/x.local.toml; do
  if "$REAL_GIT" --git-dir="$PUBLISH_TEST_RECEIVER" cat-file -e "$P:$path" 2>/dev/null; then exit 1; fi
done
[[ "$("$REAL_GIT" --git-dir="$PUBLISH_TEST_RECEIVER" show -s --format=%B "$P")" == *"Source-Commit: $("$REAL_GIT" -C "$TMP/success/source" rev-parse HEAD)"* ]]
while IFS=$'\t' read -r metadata path; do
  blob=${metadata##* }
  [[ "$blob" == "$("$REAL_GIT" -C "$TMP/success/source" rev-parse "HEAD:$path")" ]]
done < <("$REAL_GIT" --git-dir="$PUBLISH_TEST_RECEIVER" ls-tree -r "$P")
if "$REAL_GIT" --git-dir="$PUBLISH_TEST_RECEIVER" cat-file -e "$("$REAL_GIT" -C "$TMP/success/source" rev-parse HEAD)" 2>/dev/null; then exit 1; fi
[[ "$("$REAL_GIT" --git-dir="$PUBLISH_TEST_RECEIVER" rev-list --parents -n 1 "$P" | wc -w | tr -d ' ')" == 1 ]]
[[ "$("$REAL_GIT" --git-dir="$PUBLISH_TEST_RECEIVER" show "$P:config.toml")" == canonical ]]
for path in .dev/plans/secret.md .gitlab-ci.yml .github/workflows/publish-public.yml; do
  if "$REAL_GIT" --git-dir="$PUBLISH_TEST_RECEIVER" cat-file -e "$P:$path" 2>/dev/null; then echo "excluded path leaked: $path" >&2; exit 1; fi
done
"$REAL_GIT" --git-dir="$PUBLISH_TEST_RECEIVER" cat-file -e "$P:.github/workflows/release.yml"

# A release worktree detached at the main tip can publish a candidate.
init_repo worktree
"$REAL_GIT" -C "$TMP/worktree/source" worktree add -q --detach "$TMP/worktree/release" main
if ! (cd "$TMP/worktree/release" && "$PUBLISH" --candidate --tag v1.2.3) >"$TMP/out" 2>&1; then cat "$TMP/out" >&2; exit 1; fi
[[ "$("$REAL_GIT" --git-dir="$PUBLISH_TEST_RECEIVER" show -s --format=%B "$(ref refs/heads/release-candidate)")" == *"Source-Commit: $("$REAL_GIT" -C "$TMP/worktree/source" rev-parse main)"* ]]

# A candidate built from an older source commit cannot be promoted.
init_repo stale
(cd "$TMP/stale/source" && "$PUBLISH" --candidate --tag v1.2.3) >/dev/null 2>&1
printf 'later\n' >"$TMP/stale/source/later.txt"
"$REAL_GIT" -C "$TMP/stale/source" add later.txt
"$REAL_GIT" -C "$TMP/stale/source" commit -qm later
run_rejected "$TMP/stale/source"
grep -q 'was not built from HEAD' "$TMP/out"

# A prerelease tag matches the stable manifest version and starts a publish run.
init_repo prerelease
: >"$PUBLISH_TEST_GH_LOG"
(cd "$TMP/prerelease/source" && "$PUBLISH" --candidate --tag v1.2.3-rc.1) >/dev/null 2>&1
if ! (cd "$TMP/prerelease/source" && "$PUBLISH" --execute --tag v1.2.3-rc.1) >"$TMP/prerelease.log" 2>&1; then
  cat "$TMP/prerelease.log" >&2; exit 1
fi
PRERELEASE_P=$(ref refs/heads/main)
[[ -n "$PRERELEASE_P" && "$PRERELEASE_P" == "$(ref refs/heads/release-candidate)" && -z "$(ref refs/tags/v1.2.3-rc.1)" ]]
grep -q 'rehearsal_run_id=4242' "$PUBLISH_TEST_GH_LOG"

# An existing tag allows a rehearsal but refuses promotion.
init_repo existing
PUBLISH_TEST_EXISTING_TAG=1; export PUBLISH_TEST_EXISTING_TAG
if ! (cd "$TMP/existing/source" && "$PUBLISH" --candidate --tag v1.2.3) >"$TMP/out" 2>&1; then cat "$TMP/out" >&2; exit 1; fi
grep -q 'already exists at destination. --execute stays blocked' "$TMP/out"
[[ -n "$(ref refs/heads/release-candidate)" ]]
run_rejected "$TMP/existing/source"
grep -q "tag 'v1.2.3' already exists at destination." "$TMP/out"
unset PUBLISH_TEST_EXISTING_TAG
PUBLISH_TEST_LOOKUP_FAIL=1; export PUBLISH_TEST_LOOKUP_FAIL
run_rejected "$TMP/success/source"
unset PUBLISH_TEST_LOOKUP_FAIL

# A rejected main push starts no publish run.
init_repo pushreject
: >"$PUBLISH_TEST_GH_LOG"
(cd "$TMP/pushreject/source" && "$PUBLISH" --candidate --tag v1.2.3) >/dev/null 2>&1
PUBLISH_TEST_PUSH_REJECT=1; export PUBLISH_TEST_PUSH_REJECT
run_rejected "$TMP/pushreject/source"
unset PUBLISH_TEST_PUSH_REJECT
[[ ! -s "$PUBLISH_TEST_GH_LOG" ]]

# A failed dispatch reports main's state and the command to start publishing.
init_repo dispatchfail
(cd "$TMP/dispatchfail/source" && "$PUBLISH" --candidate --tag v1.2.3) >/dev/null 2>&1
PUBLISH_TEST_DISPATCH_FAIL=1; export PUBLISH_TEST_DISPATCH_FAIL
if (cd "$TMP/dispatchfail/source" && "$PUBLISH" --execute --tag v1.2.3) >"$TMP/out" 2>&1; then exit 1; fi
unset PUBLISH_TEST_DISPATCH_FAIL
grep -q 'starting the publish run failed. Start it with: gh workflow run release.yml' "$TMP/out"
[[ "$(ref refs/heads/main)" == "$(ref refs/heads/release-candidate)" ]]

# Removed option returns the documented diagnostic.
status=0
(cd "$TMP/rejected/source" && "$PUBLISH" --keep-snapshot "$TMP/unused") >"$TMP/out" 2>&1 || status=$?
[[ "$status" == 2 ]]
grep -q "removed option '--keep-snapshot'" "$TMP/out"

# Branch, version, private notes, and secret guards preserve receiver refs.
init_repo guards
"$REAL_GIT" -C "$TMP/guards/source" checkout -qb feature
run_rejected "$TMP/guards/source"
grep -q feature "$TMP/out"
"$REAL_GIT" -C "$TMP/guards/source" checkout -q main
printf 'ahead\n' >"$TMP/guards/source/ahead.txt"
"$REAL_GIT" -C "$TMP/guards/source" add ahead.txt
"$REAL_GIT" -C "$TMP/guards/source" commit -qm ahead
"$REAL_GIT" -C "$TMP/guards/source" checkout -q --detach HEAD~1
run_rejected "$TMP/guards/source"
grep -q 'detached HEAD at the main tip required' "$TMP/out"
"$REAL_GIT" -C "$TMP/guards/source" checkout -q main
printf 'version = "9.9.9"\n' >"$TMP/guards/source/crates/cli/Cargo.toml"
"$REAL_GIT" -C "$TMP/guards/source" add crates/cli/Cargo.toml
"$REAL_GIT" -C "$TMP/guards/source" commit -qm version
run_rejected "$TMP/guards/source"
grep -q 9.9.9 "$TMP/out"; grep -q 1.2.3 "$TMP/out"
"$REAL_GIT" -C "$TMP/guards/source" rm -q crates/cli/Cargo.toml
"$REAL_GIT" -C "$TMP/guards/source" commit -qm missing-version
run_rejected "$TMP/guards/source"
mkdir -p "$TMP/guards/source/crates/cli"
printf 'version = "1.2.3"\n' >"$TMP/guards/source/crates/cli/Cargo.toml"
printf 'private\n' >"$TMP/guards/source/note.private.md"
"$REAL_GIT" -C "$TMP/guards/source" add -A
"$REAL_GIT" -C "$TMP/guards/source" commit -qm private
run_rejected "$TMP/guards/source"
grep -q "private notes" "$TMP/out"
"$REAL_GIT" -C "$TMP/guards/source" rm -q note.private.md
printf '%s %s %s\n' 'BEGIN' 'RSA' 'PRIVATE KEY' >"$TMP/guards/source/secret.txt"
"$REAL_GIT" -C "$TMP/guards/source" add -A
"$REAL_GIT" -C "$TMP/guards/source" commit -qm secret
run_rejected "$TMP/guards/source"
grep -q 'LEAK: possible secret material detected:' "$TMP/out"
if (cd "$TMP/guards/source" && "$PUBLISH" --tag v1.2.3) >/dev/null 2>&1; then exit 1; fi
if (cd "$TMP/guards/source" && "$PUBLISH" --execute) >/dev/null 2>&1; then exit 1; fi
if (cd "$TMP/guards/source" && "$PUBLISH" --candidate) >/dev/null 2>&1; then exit 1; fi
if (cd "$TMP/guards/source" && "$PUBLISH" --candidate --execute --tag v1.2.3) >/dev/null 2>&1; then exit 1; fi
echo 'publish-public probes passed'
