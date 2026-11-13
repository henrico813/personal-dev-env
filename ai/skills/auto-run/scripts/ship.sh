# Check that the branch holds the plan's tree, lint the PR text,
# squash onto the parent, push, file the PR request and print the
# status line. Rerun to resume or to read a waiting request.
# Before the first request, it stops after the push until `tested`
# names the pushed commit.
. "$(dirname "$0")/env" || exit 1
log=$D/ship.log

# Exits 3 when the trees match, so every run checks again.
step_tree() {
  { [ -n "$W" ] && [ "$W" != null ]; } ||
    { echo "no worktree: '$W'"; exit 1; }
  last=$(rg -o '^### ([0-9]+)\.' -r '$1' "$P" | tail -n 1)
  t=$(plan_tree "$P" "$R" "$B" "$last") ||
    { echo "export failed: step $last"; exit 1; }
  [ "$(git -C "$W" rev-parse 'HEAD^{tree}')" = "$t" ] ||
    { echo "branch tree differs from the plan"; exit 1; }
  exit 3
}

# Exits 3 after a clean lint, so every run lints FILE again.
step_text() {
  python3 "$M" fmt "$D/FILE" >/dev/null || exit 1
  python3 "$M" lint --pr "$D/FILE" ||
    { echo "lint failed: $D/FILE"; exit 1; }
  exit 3
}

# A push that worked before its mark was written counts as done.
# CHECK runs again, since fix commits came after build.sh.
step_push() {
  cd "$W" || exit 1
  [ "$(git symbolic-ref -q --short HEAD)" = "$BR" ] ||
    { echo "not on $BR"; exit 1; }
  [ -z "$(git status --porcelain)" ] ||
    { echo "worktree not clean"; exit 1; }
  git fetch -q "$REMOTE" || exit 1
  out=$(git ls-remote "$REMOTE" "refs/heads/$BR") || exit 1
  pushed=$(printf '%s' "$out" | cut -f 1)
  [ "$pushed" = "$(git rev-parse HEAD)" ] &&
    { echo "already pushed"; exit 0; }
  [ -z "$pushed" ] ||
    { echo "$BR is already pushed; use Push a later round"; exit 1; }
  sh -c "$CHECK" || { echo "checks failed; not pushing"; exit 1; }
  git merge-base --is-ancestor "$REMOTE/$PARENT" HEAD ||
    { echo "$PARENT moved; carry its commit to this PR first"
      exit 1; }
  git reset -q --soft "$REMOTE/$PARENT" || exit 1
  git commit -q -F "$D/FILE" || exit 1
  git push --force-with-lease -u "$REMOTE" "HEAD:refs/heads/$BR" ||
    exit 1
}

# Saves the request as request.sh, with its own body file, so the
# identical command can run again later. A changed command or body is
# a new request, so both are written only before the first filing or
# after a refusal. A run cut off before its result reruns the saved
# request. Exit 3 leaves no mark, so a rerun reads the result. Once
# the PR is open, a rerun skips the saved request, which would run
# again after it expires. Writing the request also writes BODY, since
# FILE is the newest text. After a decline or an interruption it files
# nothing, since an expired request would come back as a new one.
step_request() {
  grep -q '^exit 0 ' "$D/request.result" 2>/dev/null && exit 0
  if grep -q '^exit [46] ' "$D/request.result" 2>/dev/null; then
    echo "declined or interrupted before; filing nothing; ask the hub"
    exit 1
  fi
  [ -n "$RUN" ] || { echo "RUN is empty; set it with keep"; exit 1; }
  if [ ! -e "$D/request.sh" ] ||
    grep -q '^exit 2 ' "$D/request.result" 2>/dev/null; then
    tail -n +3 "$D/FILE" > "$D/request.body" || exit 1
    cp "$D/request.body" "$D/BODY" || exit 1
    {
      printf 'cd %q || exit 1\n' "$W"
      printf 'pde-gh-write pr create --base %q --title %q' \
        "$PARENT" "$(head -n 1 "$D/FILE")"
      printf ' --body-file %q 2> %q\n' \
        "$D/request.body" "$D/request.err"
      printf 'rc=$?\n'
      printf 'cat %q >&2\n' "$D/request.err"
      printf 'echo "exit $rc at $(date -u +%%FT%%TZ)" > %q\n' \
        "$D/request.result"
      printf 'exit $rc\n'
    } > "$D/request.sh" || exit 1
  fi
  rm -f "$D/request.err"
  bash "$D/request.sh"
  rc=$?
  # pde-gh-write prints this line only for a newly saved request. The
  # auto hub counts each request's expiry from its line in the log, and
  # workers keep requests a minute apart by the last line.
  grep -q '^pde-gh-write: waiting for approval' "$D/request.err" \
    2>/dev/null &&
    printf '%s %s %s\n' "$(date -u +%FT%TZ)" "$BR" "$D" \
      >> "$RUN/requests.log"
  case $rc in
    0) exit 0 ;;
    3) echo "waiting for the user's approval"; exit 3 ;;
    4) echo "declined; park this PR and its children"; exit 1 ;;
    6) echo "interrupted, never retried; ask the hub"; exit 1 ;;
    2) echo "refused; fix the command, then rerun"; exit 1 ;;
    *) echo "exit $rc from gh; put its output on the item's page"
       exit 1 ;;
  esac
}

run_step "$log" tree
run_step "$log" text
run_step "$log" push
# The first request waits until the pushed commit is tried and its
# result is in tested, as pr-loop.md's step 7 says.
if [ ! -e "$D/request.sh" ] &&
  ! grep -qF "$(git -C "$W" rev-parse HEAD)" "$D/tested" 2>/dev/null
then
  keep NEXT \
    'try the pushed commit, then add its result to FILE and tested'
  status_line 2>> "$log" ||
    { echo "STOPPED at status: see $log"; exit 1; }
  exit 0
fi
run_step "$log" request
# A later round while the request waited left newer PR text in BODY.
if ! grep -q '^exit 0 ' "$D/request.result" 2>/dev/null; then
  keep NEXT 'rerun ship.sh to read the request'
elif [ -e "$D/BODY" ] && ! cmp -s "$D/BODY" "$D/request.body"; then
  keep NEXT 'pr edit the PR text once the hub says the user reviewed it'
else
  keep NEXT 'queue the comment review'
fi
status_line 2>> "$log" ||
  { echo "STOPPED at status: see $log"; exit 1; }
