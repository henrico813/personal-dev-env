# Check the plan, lint the commit messages, commit each step's tree on
# the step before, make the worktree and run CHECK. Rerun to resume.
. "$(dirname "$0")/env" || exit 1
log=$D/build.log

step_check() {
  planner check "$P" --repo "$R" --json-errors >/dev/null ||
    { echo "planner check failed; its errors are in build.log"
      exit 1; }
  rg -o '^### ([0-9]+)\.' -r '$1' "$P" > "$D/steps" || exit 1
}

step_lint() {
  for s in $(cat "$D/steps"); do
    [ -s "$D/MSG$s" ] || { echo "missing: $D/MSG$s"; exit 1; }
    python3 "$M" fmt "$D/MSG$s" >/dev/null || exit 1
    python3 "$M" lint "$D/MSG$s" ||
      { echo "lint failed: step $s"; exit 1; }
  done
}

# Starts at $B on every run, since commit-tree makes new hashes.
step_build() {
  prev=$B
  for s in $(cat "$D/steps"); do
    t=$(plan_tree "$P" "$R" "$B" "$s") ||
      { echo "export failed: step $s"; exit 1; }
    echo "$t" > "$D/tree$s"
    prev=$(git -C "$R" commit-tree "$t" -p "$prev" -F "$D/MSG$s") ||
      exit 1
    echo "$prev" > "$D/commit$s"
  done
}

# Reuses the branch's worktree when an earlier run made it.
step_worktree() {
  last=$(tail -n 1 "$D/steps")
  W=$(wt -C "$R" list --format=json | jq -r --arg b "$BR" \
    '.items[] | select(.branch == $b) | .worktree.path // empty')
  if [ -z "$W" ]; then
    wt -C "$R" switch --create "$BR" --base "$B" --no-cd \
      --format=json > "$D/wt.json" || exit 1
    W=$(jq -r .path "$D/wt.json")
  fi
  { [ -n "$W" ] && [ "$W" != null ]; } ||
    { echo "no worktree for $BR"; exit 1; }
  keep W "$W"
  git -C "$W" merge -q --ff-only "$(cat "$D/commit$last")" ||
    { echo "$BR holds another build; use Apply a fix round"; exit 1; }
  t=$(cat "$D/tree$last")
  [ "$(git -C "$W" rev-parse 'HEAD^{tree}')" = "$t" ] ||
    { echo "branch tree differs from the plan"; exit 1; }
}

# ship.sh runs CHECK again, after the fix commits.
step_test() {
  { [ -n "$W" ] && [ "$W" != null ] && cd "$W"; } ||
    { echo "no worktree: '$W'"; exit 1; }
  sh -c "$CHECK" || { echo "CHECK failed in $W"; exit 1; }
}

run_step "$log" check
run_step "$log" lint
run_step "$log" build
run_step "$log" worktree
run_step "$log" test
. "$D/env"
echo "built $BR in $W: $(changed "$W" "$B" HEAD) changed lines"
