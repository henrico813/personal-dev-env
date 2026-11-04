# Commands for a roadmap run

The snippets work in zsh and bash. Variables, arrays and functions
are lost between tool calls, so each PR keeps them in a run folder:
`env` holds the variables and loads `fns.sh`. Start every tool call
after the setup with `. <run folder>/env`, using the path the setup
printed. Each snippet stops with `exit 1` on the first failure; never
run the lines after a failed one by hand.

## Contents

- Write a design digest
- Set up a PR's run folder
- Build the PR
- Apply a fix round
- Put back the bugs
- Ship
- Push a later round
- Print a PR's status line
- Base a child on an unbuilt parent
- Move a child's base
- Carry a parent's commit to a child

## Write a design digest

Before a PR's first planner starts, write its design digest in the
run's shared folder, beside `rules.md`. A script writes it, so it
drops no passage that names a term. List every design note, the
design-edits note among them, and each term the PR adds or changes,
such as a file, type, function, command, flag or error code. Name
each section to copy whole, such as the Dictionary:

```sh
G='<shared folder>/design-<PR name>.md'
E='<design-edits note, or the one design note>'
set -- '<design note>' '<design note>'
printf '%s\n' '<term>' '<term>' > "$G.terms" || exit 1
printf '%s\n' 'Dictionary' > "$G.sections" || exit 1
rg -n -F -C 2 --heading -f "$G.terms" -- "$@" > "$G.passages" ||
  { echo "no passage matched, or a note is missing"; exit 1; }
# Copy each design-edits entry that a matched passage links to.
link='\[\[([^]|#]*)#([^]|\\]+)'
rg -o -N --no-filename -r '$1#$2' "$link" "$G.passages" |
  awk -F '#' -v e="$(basename "$E" .md)" '$1 == e { print $2 }' \
  >> "$G.sections"
# Print each listed section and its subsections, with line numbers.
awk '
NR == FNR { want[$0]; next }
FNR == 1 { lvl = 0 }
/^#+ / {
  n = index($0, " ") - 1
  if (lvl && n <= lvl) lvl = 0
  if (!lvl && (substr($0, n + 2) in want)) {
    lvl = n
    print "\n" FILENAME
  }
}
lvl { print FNR ":" $0 }
' "$G.sections" "$@" > "$G.whole" || exit 1
cat "$G.whole" "$G.passages" > "$G" || exit 1
wc -l "$G"
```

Each passage keeps its note and line number, so a reader can read
around it with `sed -n`. To add a term, run the whole snippet again.

## Set up a PR's run folder

The folder is under `~/.local/state`, so it survives a reboot.
`PARENT` is the branch this PR stacks on: the default branch, or the
parent PR's branch. `P` is the plan file. The setup runs
`planner key` on it before making the folder and stops when the file
has no key. `K` holds that key, such as `abc-12`, and goes into the
run folder's name and `BR`, so one search for the key finds the
branch, its worktree and the run folder. `BR` is this PR's branch,
`<type>/$K-<words>`: a `git-messages` commit type, the key, then
lowercase words joined by hyphens, at most 48 characters after the
type. Words after the key are required: `feat/abc-12-approvals-file`,
never `feat/abc-12`. When the repository sets `pde.branchPattern`,
set `BR` to a branch that starts with the ticket key the plan names,
in the form the pattern asks for, with words after it, and ask the
user when the plan names none; `K` and the run folder still come
from `planner key`. `CHECK` is the plan's test and
lint command. Keep each value in single quotes, since vault paths
have spaces; use double quotes for a value that holds a single quote
or `$K`.

```sh
S=${XDG_STATE_HOME:-$HOME/.local/state}
P='<plan file>'
K=$(planner key "$P") || exit 1
mkdir -p "$S" && D=$(mktemp -d "$S/roadmap-run.$K.XXXXXX") || exit 1
printf 'D=%q\n. "$D/fns.sh"\n' "$D" > "$D/env"
cat > "$D/fns.sh" <<'FNS'
# Set a variable here and save it for later tool calls; a later value
# wins.
keep() { eval "$1=\$2"; printf '%s=%q\n' "$1" "$2" >> "$D/env"; }

# Print the hash of the tree the plan makes through STEP. The tree is
# written through a scratch index, so no checkout changes.
plan_tree() {
  local plan=$1 repo=$2 base=$3 step=$4 tmp git_dir rc
  tmp=$(mktemp -d) || return 1
  if ! planner export "$plan" --repo "$repo" --base-commit "$base" \
    --out "$tmp/tree" --through "$step" >/dev/null; then
    rm -rf "$tmp"
    return 1
  fi
  git_dir=$(git -C "$repo" rev-parse --path-format=absolute --git-common-dir)
  (
    cd "$tmp/tree" &&
      GIT_INDEX_FILE="$tmp/index" git --git-dir "$git_dir" --work-tree . add -A -f . &&
      GIT_INDEX_FILE="$tmp/index" git --git-dir "$git_dir" --work-tree . write-tree
  )
  rc=$?
  rm -rf "$tmp"
  return $rc
}

# Put back one bug, run the test command, and restore the file.
# Returns 1 when the test command passes, since no test caught the
# bug, and 2 when FILE is gone or OLD does not appear exactly once.
plant() {
  local file=$1 old=$2 new=$3 backup rc
  shift 3
  backup=$(mktemp "$D/plant.XXXXXX") || return 1
  cp "$file" "$backup" || { rm "$backup"; return 2; }
  python3 - "$file" "$old" "$new" <<'EOF' || { rm "$backup"; return 2; }
import sys
path, old, new = sys.argv[1:]
text = open(path).read()
assert text.count(old) == 1, "old text must appear exactly once"
open(path, "w").write(text.replace(old, new))
EOF
  "$@"
  rc=$?
  cp "$backup" "$file" && rm -f "$backup"
  if [ "$rc" -eq 0 ]; then
    echo "plant: no test failed for $file"
    return 1
  fi
  echo "plant: tests failed as expected, exit $rc"
}

# Print the lines added plus deleted in REPO between FROM and TO.
changed() {
  local out
  out=$(git -C "$1" diff --numstat "$2" "$3") || return 1
  printf '%s\n' "$out" | awk '{n += $1 + $2} END {print n + 0}'
}

# Run function step_NAME of build.sh or ship.sh in a subshell, with
# its output added to LOG. The step's mark holds the plan's hash, so a
# rerun skips it until the plan changes. A step that exits 3 ran but
# leaves no mark. Any other failure prints one STOPPED line and ends
# the script.
run_step() {
  local log=$1 name=$2 h rc
  h=$(git hash-object "$P") ||
    { echo "STOPPED at $name: cannot read $P"; exit 1; }
  [ "$(cat "$D/done.$name" 2>/dev/null)" = "$h" ] && return 0
  echo "== $name $(date -u +%FT%TZ)" >> "$log"
  ( . "$D/env" && "step_$name" ) >> "$log" 2>&1
  rc=$?
  case $rc in
    0) echo "$h" > "$D/done.$name" ;;
    3) ;;
    *) echo "STOPPED at $name: $(tail -n 1 "$log")"; exit 1 ;;
  esac
}

# Replace the first line of status.txt and print it.
st() {
  touch "$D/status.txt" &&
    { printf '%s\n' "$1"; sed 1d "$D/status.txt"; } > "$D/status.new" &&
    mv "$D/status.new" "$D/status.txt" && printf '%s\n' "$1"
}

# Print the roadmap's status line for this PR, ending with NEXT, and
# write it as the first line of status.txt.
status_line() (
  { [ -n "$W" ] && [ "$W" != null ]; } ||
    { st "$BR: not built; next: $NEXT"; exit $?; }
  cd "$W" || exit 1
  git fetch -q "$REMOTE" || exit 1
  head=$(git rev-parse HEAD) || exit 1
  out=$(git ls-remote "$REMOTE" "refs/heads/$BR") || exit 1
  pushed=$(printf '%s' "$out" | cut -f 1)
  same=no
  [ "$head" = "$pushed" ] && same=yes
  mb=$(git merge-base "$REMOTE/$PARENT" HEAD) || exit 1
  n=$(changed . "$mb" HEAD) || exit 1
  req=$(cat "$D/request.result" 2>/dev/null) || req="not filed"
  line="$BR: pushed ${pushed:-none}; local equals pushed: $same;"
  line="$line $n changed lines against $PARENT; request: $req"
  st "$line; next: $NEXT"
)
FNS
. "$D/env"
keep R '<repository root>'
keep P "$P"
keep B '<full commit from the plan>'
keep REMOTE '<remote, such as origin>'
keep PARENT '<parent branch>'
keep BR "<type>/$K-<words>"
keep CHECK '<test and lint command>'
keep M "$HOME/.agents/skills/git-messages/scripts/msg"
echo "run folder: $D"
```

Write the run folder's path on the roadmap page next to the PR.

## Build the PR

Write `$D/MSG<step>` for each `### <step>.` heading in the plan,
following `git-messages`. Then run this block. It saves `build.sh` in
the run folder and runs it:

```sh
. <run folder>/env
cat > "$D/build.sh" <<'SH'
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
SH
bash "$D/build.sh"
```

It prints `built <branch> in <worktree>: <n> changed lines`, or one
line `STOPPED at <step>: <reason>`. The full output is in `build.log`
in the run folder. To resume, fix the cause and run
`bash <run folder>/build.sh`; it skips each step it finished for this
version of the plan.

- `check`: start a new planner with planner's errors from
  `build.log`, then rerun.
- `lint`: fix the message, then rerun.
- Any other step means a stale plan or a broken tool. Do not retry or
  edit files: stop this PR and its children, put the error on the
  roadmap page as a blocking question, and keep building other
  stacks.

A revised plan makes build.sh repeat every step. Once the branch holds
a build, build.sh stops at `worktree`; bring a revision in with Apply
a fix round instead.

## Apply a fix round

After the planner revises the plan, commit its whole tree as one fix
commit. Read the step list again, since the revision may add steps.
Write `$D/MSGfix` first:

```sh
. <run folder>/env
{ [ -n "$W" ] && [ "$W" != null ] && cd "$W"; } ||
  { echo "no worktree: '$W'"; exit 1; }
python3 "$M" fmt "$D/MSGfix" >/dev/null || exit 1
python3 "$M" lint "$D/MSGfix" || exit 1
planner check "$P" --repo "$R" --json-errors >/dev/null || exit 1
rg -o '^### ([0-9]+)\.' -r '$1' "$P" > "$D/steps" || exit 1
t=$(plan_tree "$P" "$R" "$B" "$(tail -n 1 "$D/steps")") || exit 1
c=$(git -C "$W" commit-tree "$t" -p HEAD -F "$D/MSGfix") || exit 1
git -C "$W" merge -q --ff-only "$c" || exit 1
n=$(changed "$W" "$B" HEAD) || exit 1
echo "changed lines: $n"
```

After the first push, run Push a later round right away. A fix that
stays local is not done. The revised plan no longer matches the marks
that build.sh and ship.sh left, so a later ship.sh run repeats every
step.

## Put back the bugs

This runs every `plant` line in the plan and in the run folder's
reports, one at a time, and writes one `pass`, `FAIL` or `stale`
line per `plant` line to `$D/plants.txt`, ending with `done`. Their
output goes to `$D/plants.log`. A line that passed before is skipped,
so a later round runs only new or changed lines:

```sh
. <run folder>/env
{ [ -n "$W" ] && [ "$W" != null ] && cd "$W"; } ||
  { echo "no worktree: '$W'"; exit 1; }
rg -N --no-filename --sort path --max-depth 1 -g '*.md' '^plant ' \
  "$P" "$D" > "$D/plants" || { echo "no plant lines found"; exit 1; }
touch "$D/plants.passed" && : > "$D/plants.txt" || exit 1
n=0
while IFS= read -r line <&3; do
  n=$((n + 1))
  if grep -qxF -e "$line" "$D/plants.passed"; then
    echo "skip $n: passed before" >> "$D/plants.txt"
    continue
  fi
  echo "## $n: $line" >> "$D/plants.log"
  eval "$line" < /dev/null >> "$D/plants.log" 2>&1
  case $? in
    0) r=pass ;;
    2) r=stale ;;
    *) r=FAIL ;;
  esac
  [ -z "$(git status --porcelain)" ] || {
    echo "worktree not clean after line $n" >> "$D/plants.txt"
    exit 1
  }
  echo "$r $n: $line" >> "$D/plants.txt"
  [ "$r" != pass ] || printf '%s\n' "$line" >> "$D/plants.passed"
done 3< "$D/plants"
echo done >> "$D/plants.txt"
cat "$D/plants.txt"
```

A `FAIL` line means no test caught that bug. A `stale` line means
its file is gone or its old text no longer appears once, because a
fix round changed it. When a `stale` line's bug still matters, write
a new `plant` line for it in a `.md` file in the run folder. Send only
`FAIL` lines to a planner.

## Ship

Run this after the final pass. Write the PR text to `$D/FILE` first,
so it can give the final pass's real numbers. The block saves
`ship.sh` in the run folder and runs it. ship.sh squashes only before
the branch's first push:

```sh
. <run folder>/env
cat > "$D/ship.sh" <<'SH'
# Check that the branch holds the plan's tree, lint the PR text,
# squash onto the parent, push, file the PR request and print the
# status line. Rerun to resume or to read a waiting request.
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
# again after it expires.
step_request() {
  grep -q '^exit 0 ' "$D/request.result" 2>/dev/null && exit 0
  if [ ! -e "$D/request.sh" ] ||
    grep -q '^exit 2 ' "$D/request.result" 2>/dev/null; then
    tail -n +3 "$D/FILE" > "$D/request.body" || exit 1
    {
      printf 'cd %q || exit 1\n' "$W"
      printf 'pde-gh-write pr create --base %q --title %q' \
        "$PARENT" "$(head -n 1 "$D/FILE")"
      printf ' --body-file %q\n' "$D/request.body"
      printf 'rc=$?\n'
      printf 'echo "exit $rc at $(date -u +%%FT%%TZ)" > %q\n' \
        "$D/request.result"
      printf 'exit $rc\n'
    } > "$D/request.sh" || exit 1
  fi
  bash "$D/request.sh"
  rc=$?
  case $rc in
    0) exit 0 ;;
    3) echo "waiting for the user's approval"; exit 3 ;;
    4) echo "declined; park this PR and its children"; exit 1 ;;
    6) echo "interrupted, never retried; ask the user"; exit 1 ;;
    2) echo "refused; fix the command, then rerun"; exit 1 ;;
    *) echo "exit $rc from gh; put its output on the roadmap page"
       exit 1 ;;
  esac
}

run_step "$log" tree
run_step "$log" text
run_step "$log" push
run_step "$log" request
# A later round while the request waited left newer PR text in BODY.
if ! grep -q '^exit 0 ' "$D/request.result" 2>/dev/null; then
  keep NEXT 'rerun ship.sh to read the request'
elif [ -e "$D/BODY" ] && ! cmp -s "$D/BODY" "$D/request.body"; then
  keep NEXT 'update the PR text with pde-gh-write pr edit unless done'
else
  keep NEXT 'queue the comment review'
fi
status_line 2>> "$log" ||
  { echo "STOPPED at status: see $log"; exit 1; }
SH
bash "$D/ship.sh"
```

It prints the PR's status line, or one line
`STOPPED at <step>: <reason>`. The full output is in `ship.log` in
the run folder. To resume, fix the cause and run
`bash <run folder>/ship.sh`. It checks the tree and lints the PR
text on every run, and skips each other step it finished for this
version of the plan.

- `tree`: the branch lacks the plan's last revision. Run Apply a fix
  round, then rerun.
- `text`: fix `$D/FILE`, then rerun.
- `push`: follow the reason. A failed check is never minor. When the
  parent moved, run Carry a parent's commit to a child, then rerun.
- `request`: the reason gives the PR request's result, below.

Put `bash <run folder>/request.sh` on the roadmap page. Rerunning it
reads the result:

- `0`, or `already ran <id>, exit 0`: the PR is open.
- `3`: it waits for the user; say so once and move on. ship.sh still
  prints the status line.
- `4`, `declined <id>`: park this PR and its children.
- `6`, `interrupted <id>`: it is never retried; ask the user.
- `2`: the command was refused; fix it, then rerun ship.sh, which
  writes `request.sh` again.
- Anything else is from `gh`; put its output on the roadmap page.

A request expires four hours after it is filed. Rerunning the script
after that files a fresh request. ship.sh keeps the request's first
text, reruns the saved script while the request waits, and skips it
once the PR is open, so a later round never files a second PR
request.

## Push a later round

After the first push, each change adds a commit, and ship.sh stops at
`push`. Update `$D/FILE`, then push the commit with this:

```sh
. <run folder>/env
python3 "$M" fmt "$D/FILE" >/dev/null || exit 1
python3 "$M" lint --pr "$D/FILE" || exit 1
tail -n +3 "$D/FILE" > "$D/BODY" || exit 1
{ [ -n "$W" ] && [ "$W" != null ] && cd "$W"; } ||
  { echo "no worktree: '$W'"; exit 1; }
[ "$(git symbolic-ref -q --short HEAD)" = "$BR" ] ||
  { echo "not on $BR"; exit 1; }
[ -z "$(git status --porcelain)" ] ||
  { echo "worktree not clean"; exit 1; }
sh -c "$CHECK" || { echo "checks failed; not pushing"; exit 1; }
git fetch -q "$REMOTE" || exit 1
git merge-base --is-ancestor "$REMOTE/$PARENT" HEAD ||
  { echo "$PARENT moved; carry its commit to this PR first"; exit 1; }
git push -u "$REMOTE" "HEAD:refs/heads/$BR" || exit 1
```

Then update the PR text with
`pde-gh-write pr edit NUMBER --body-file "$D/BODY"`, and print the
status line. While the PR request waits, there is no PR to edit yet;
update its text once the PR opens. ship.sh's status line then names
`pr edit` as the next step.

## Print a PR's status line

Write the roadmap's status line for a PR from this output, never by
hand. Set NEXT to the PR's next step, such as `skill reviews` or
`rerun request.sh`, so a new session can resume from the roadmap
page. `status_line` also writes the line as the first line of
`$D/status.txt` and keeps the lines below it, so the status line that
ship.sh prints reaches the file too:

```sh
. <run folder>/env
keep NEXT '<next step>'
status_line
```

`local equals pushed: no` means a commit is not pushed: push it
before anything else, and never deploy it.

## Base a child on an unbuilt parent

A child planned before its parent is built needs a base commit with
the parent's planned tree. In the parent's run folder:

```sh
. <parent's run folder>/env
pt=$(plan_tree "$P" "$R" "$B" "$(tail -n 1 "$D/steps")") || exit 1
git -C "$R" commit-tree "$pt" -p "$B" -m "Planned tree of $(basename "$P")"
```

Give the printed commit to the child's planner as its base. No branch
points at it, so `git gc` may delete it after two weeks.

## Move a child's base

After the parent ships, run this in the child's run folder. It sets
the child plan's `Base commit:` line to the parent's final commit,
saves that commit, and runs `planner check`:

```sh
. <child's run folder>/env
nb='<final commit of the parent>'
nb=$(git -C "$R" rev-parse --verify "$nb^{commit}") || exit 1
sed -i "0,/^Base commit: .*/s//Base commit: $nb/" "$P" || exit 1
grep -qx "Base commit: $nb" "$P" ||
  { echo "no Base commit: line in $P"; exit 1; }
keep B "$nb"
planner check "$P" --repo "$R" --json-errors >/dev/null ||
  { echo "plan does not apply on $nb"; exit 1; }
```

If the check passes, the child's diffs apply unchanged. If it fails,
the parent changed after the child was written: start a new planner
for the child with the errors.

## Carry a parent's commit to a child

After a parent pushes a new commit, run this in each child's run
folder, in stack order, pushing each child before the next. It makes
a merge commit whose tree `planner export` writes from the child's
plan on the pushed parent:

```sh
. <child's run folder>/env
{ [ -n "$W" ] && [ "$W" != null ] && cd "$W"; } ||
  { echo "no worktree: '$W'"; exit 1; }
[ "$(git symbolic-ref -q --short HEAD)" = "$BR" ] ||
  { echo "not on $BR"; exit 1; }
[ -z "$(git status --porcelain)" ] || { echo "worktree not clean"; exit 1; }
git fetch -q "$REMOTE" || exit 1
nb=$(git rev-parse "$REMOTE/$PARENT") || exit 1
planner check "$P" --repo "$R" --base-commit "$nb" --json-errors >/dev/null ||
  { echo "plan does not apply on $PARENT; park this PR"; exit 1; }
t=$(plan_tree "$P" "$R" "$nb" "$(tail -n 1 "$D/steps")") || exit 1
c=$(git commit-tree "$t" -p HEAD -p "$nb" -m "chore: merge the parent branch") ||
  exit 1
git merge -q --ff-only "$c" || exit 1
sed -i "0,/^Base commit: .*/s//Base commit: $nb/" "$P" || exit 1
grep -qx "Base commit: $nb" "$P" ||
  { echo "no Base commit: line in $P"; exit 1; }
keep B "$nb"
```

Then rerun ship.sh before the branch's first push, or run Push a
later round after it; both run the plan's checks. A failed
`planner check` is a conflict: never resolve it by hand.
