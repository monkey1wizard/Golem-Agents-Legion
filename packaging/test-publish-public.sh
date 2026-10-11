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
mkdir -p "$TMP/bin"
cat >"$TMP/bin/git" <<'WRAPPER'
#!/usr/bin/env bash
args=("$@")
command_name=
for arg in "${args[@]}"; do
  case "$arg" in push|ls-remote|get-url) command_name=$arg; break ;; esac
done
if [[ "$command_name" == push || "$command_name" == ls-remote ]]; then
  for ((i=0; i<${#args[@]}; i++)); do
    if [[ "${args[$i]}" == github || "${args[$i]}" == https://github.com/monkey1wizard/Golem-Agents-Legion.git ]]; then args[$i]="$PUBLISH_TEST_RECEIVER"; fi
  done
fi
if [[ "$command_name" == push ]]; then
  [[ " ${args[*]} " == *" --atomic "* && " ${args[*]} " == *" +refs/heads/main:refs/heads/main "* && " ${args[*]} " == *" refs/tags/v1.2.3:refs/tags/v1.2.3 "* ]] || {
    echo 'push did not contain the required atomic main and non-force tag refspecs' >&2; exit 98;
  }
fi
if [[ "$command_name" == push && "${PUBLISH_TEST_TAG_RACE:-0}" == 1 ]]; then
  "$PUBLISH_TEST_REAL_GIT" --git-dir="$PUBLISH_TEST_RECEIVER" update-ref refs/tags/v1.2.3 "$PUBLISH_TEST_RACE_WINNER"
  unset PUBLISH_TEST_TAG_RACE
  exit 1
fi
if [[ "$command_name" == ls-remote && "${PUBLISH_TEST_EXISTING_TAG:-0}" == 1 ]]; then exit 0; fi
if [[ "$command_name" == push && "${PUBLISH_TEST_ATOMIC_REJECT:-0}" == 1 ]]; then
  [[ " ${args[*]} " == *" --atomic "* ]] || { echo 'push omitted --atomic' >&2; exit 98; }
  exit 1
fi
if [[ "$command_name" == ls-remote && "${PUBLISH_TEST_LOOKUP_FAIL:-0}" == 1 ]]; then exit 128; fi
if [[ "$command_name" == get-url && "${PUBLISH_TEST_EFFECTIVE_LOOKUP_FAIL:-0}" == 1 ]]; then exit 128; fi
exec "$PUBLISH_TEST_REAL_GIT" "${args[@]}"
WRAPPER
chmod +x "$TMP/bin/git"
export PUBLISH_TEST_REAL_GIT=$REAL_GIT PATH="$TMP/bin:$PATH"

init_repo() {
  local name=$1
  mkdir -p "$TMP/$name/source" "$TMP/$name/receiver"
  "$REAL_GIT" init -q -b main "$TMP/$name/source"
  "$REAL_GIT" -C "$TMP/$name/source" config user.name Fixture
  "$REAL_GIT" -C "$TMP/$name/source" config user.email fixture@example.invalid
  "$REAL_GIT" -C "$TMP/$name/source" config commit.gpgsign false
  "$REAL_GIT" -C "$TMP/$name/source" config core.hooksPath /dev/null
  "$REAL_GIT" -C "$TMP/$name/source" config filter.gal-config.clean 'sed s/personal/canonical/'
  "$REAL_GIT" -C "$TMP/$name/source" config filter.gal-config.smudge 'sed s/canonical/personal/'
  mkdir -p "$TMP/$name/source/.github/workflows" "$TMP/$name/source/docs/plans" "$TMP/$name/source/.dev/plans"
  printf 'private controller\n' >"$TMP/$name/source/.gitlab-ci.yml"
  printf 'canonical\n' >"$TMP/$name/source/config.toml"
  printf 'config.toml filter=gal-config\n' >"$TMP/$name/source/.gitattributes"
  printf 'private\n' >"$TMP/$name/source/.dev/plans/secret.md"
  printf 'private\n' >"$TMP/$name/source/docs/plans/secret.md"
  printf 'controller\n' >"$TMP/$name/source/.github/workflows/publish-public.yml"
  printf 'release\n' >"$TMP/$name/source/.github/workflows/release.yml"
  "$REAL_GIT" -C "$TMP/$name/source" add -A
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
  [[ "$after_main" == "$before_main" && "$after_tag" == "$before_tag" ]] || { echo "rejected publication changed receiver refs: main $before_main -> $after_main; tag $before_tag -> $after_tag" >&2; exit 1; }
}

# Invalid destinations, removed flags, and non-main targets reject without writes.
init_repo rejected
before=$(ref refs/heads/main)
"$REAL_GIT" -C "$TMP/rejected/source" remote set-url github https://evilgithub.com/monkey1wizard/Golem-Agents-Legion.git
if (cd "$TMP/rejected/source" && "$PUBLISH" --execute) >/dev/null 2>&1; then exit 1; fi
[[ "$(ref refs/heads/main)" == "$before" ]]
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
if (cd "$TMP/rejected/source" && "$PUBLISH" --execute) >/dev/null 2>&1; then exit 1; fi
[[ "$(ref refs/heads/main)" == "$before" ]]
"$REAL_GIT" -C "$TMP/rejected/source" config --unset remote.github.pushurl
for rewrite in insteadOf pushInsteadOf; do
  "$REAL_GIT" -C "$TMP/rejected/source" config "url.https://evil.example/.${rewrite}" https://github.com/monkey1wizard/Golem-Agents-Legion.git
  run_rejected "$TMP/rejected/source"
  "$REAL_GIT" -C "$TMP/rejected/source" config --unset-all "url.https://evil.example/.${rewrite}"
done
for option in --include-project-md; do
  if (cd "$TMP/rejected/source" && "$PUBLISH" "$option") >/dev/null 2>&1; then exit 1; fi
done
if (cd "$TMP/rejected/source" && "$PUBLISH" --branch release) >/dev/null 2>&1; then exit 1; fi

# Success publishes an orphan snapshot with matching refs, canonical content, and no excluded paths.
init_repo success
if ! (cd "$TMP/success/source" && "$PUBLISH" --execute --tag v1.2.3) >"$TMP/success.log" 2>&1; then
  cat "$TMP/success.log" >&2; exit 1
fi
P=$(ref refs/heads/main)
[[ -n "$P" && "$P" == "$(ref refs/tags/v1.2.3)" ]]
printf 'receiver evidence: main=%s tag=%s\n' "$P" "$(ref refs/tags/v1.2.3)"
echo 'export tree evidence:'
"$REAL_GIT" --git-dir="$PUBLISH_TEST_RECEIVER" ls-tree -r --name-only "$P"
[[ "$("$REAL_GIT" --git-dir="$PUBLISH_TEST_RECEIVER" rev-list --parents -n 1 "$P" | wc -w | tr -d ' ')" == 1 ]]
[[ "$("$REAL_GIT" --git-dir="$PUBLISH_TEST_RECEIVER" show "$P:config.toml")" == canonical ]]
for path in .dev/plans/secret.md .gitlab-ci.yml .github/workflows/publish-public.yml; do
  if "$REAL_GIT" --git-dir="$PUBLISH_TEST_RECEIVER" cat-file -e "$P:$path" 2>/dev/null; then echo "excluded path leaked: $path" >&2; exit 1; fi
done
"$REAL_GIT" --git-dir="$PUBLISH_TEST_RECEIVER" cat-file -e "$P:.github/workflows/release.yml"

# Existing tags and lookup failures preserve observed refs.
PUBLISH_TEST_EXISTING_TAG=1; export PUBLISH_TEST_EXISTING_TAG
run_rejected "$TMP/success/source"
unset PUBLISH_TEST_EXISTING_TAG
PUBLISH_TEST_LOOKUP_FAIL=1; export PUBLISH_TEST_LOOKUP_FAIL
run_rejected "$TMP/success/source"
unset PUBLISH_TEST_LOOKUP_FAIL

# An atomic receive rejection and a competing tag retain old refs and the winning tag object.
init_repo atomic
PUBLISH_TEST_RECEIVER="$TMP/atomic/receiver"; export PUBLISH_TEST_RECEIVER
PUBLISH_TEST_ATOMIC_REJECT=1; export PUBLISH_TEST_ATOMIC_REJECT
run_rejected "$TMP/atomic/source"
unset PUBLISH_TEST_ATOMIC_REJECT

init_repo race
PUBLISH_TEST_RECEIVER="$TMP/race/receiver"; export PUBLISH_TEST_RECEIVER
RACE_TAG=$("$REAL_GIT" -C "$TMP/race/source" rev-parse HEAD)
"$REAL_GIT" --git-dir="$PUBLISH_TEST_RECEIVER" fetch -q "$TMP/race/source" HEAD
export PUBLISH_TEST_RACE_WINNER="$RACE_TAG"
export PUBLISH_TEST_TAG_RACE=1
if (cd "$TMP/race/source" && "$PUBLISH" --execute --tag v1.2.3) >"$TMP/race.log" 2>&1; then echo 'expected tag-race rejection' >&2; exit 1; fi
[[ -z "$(ref refs/heads/main)" ]]
[[ "$(ref refs/tags/v1.2.3)" == "$RACE_TAG" ]]

# Unsafe retained directories and dry-run tags reject before any push.
init_repo keep
PUBLISH_TEST_RECEIVER="$TMP/keep/receiver"; export PUBLISH_TEST_RECEIVER
mkdir -p "$TMP/occupied"; touch "$TMP/occupied/file"
if (cd "$TMP/keep/source" && "$PUBLISH" --keep-snapshot "$TMP/occupied") >/dev/null 2>&1; then exit 1; fi
ln -s "$TMP/occupied" "$TMP/symlink-keep"
if (cd "$TMP/keep/source" && "$PUBLISH" --keep-snapshot "$TMP/symlink-keep") >/dev/null 2>&1; then exit 1; fi
if (cd "$TMP/keep/source" && "$PUBLISH" --tag v1.2.3) >/dev/null 2>&1; then exit 1; fi
if (cd "$TMP/keep/source" && "$PUBLISH" --execute) >/dev/null 2>&1; then exit 1; fi
EMPTY_KEEP="$TMP/retained"
(cd "$TMP/keep/source" && "$PUBLISH" --keep-snapshot "$EMPTY_KEEP") >"$TMP/keep.log" 2>&1 || { cat "$TMP/keep.log" >&2; exit 1; }
echo 'retained snapshot tree evidence:'
ls -la "$EMPTY_KEEP"
[[ "$(cat "$EMPTY_KEEP/config.toml")" == canonical ]]
[[ ! -e "$EMPTY_KEEP/.gitlab-ci.yml" && ! -e "$EMPTY_KEEP/.dev/plans/secret.md" ]]
[[ -d "$EMPTY_KEEP/.git" ]]
[[ -z "$(ref refs/heads/main)" ]]
echo 'publish-public probes passed'
