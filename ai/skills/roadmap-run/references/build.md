# Commands for a roadmap run

The snippets work in zsh and bash. Variables, arrays and functions
are lost between tool calls, so each PR keeps them in a run folder:
`env` holds the variables and loads `fns.sh`. Start every tool call
after the setup with `. <run folder>/env`, using the path the setup
printed. Each snippet stops with `exit 1` on the first failure; never
run the lines after a failed one by hand.

## Contents

- Check the comment-review file
- Set up a PR's run folder
- Write and lint the commit messages
- Build the commits
- Make the PR's worktree
- Apply a fix round
- Plant a bug
- Ship
- File the PR request
- Print a PR's status line
- Base a child on an unbuilt parent
- Move a child's base
- Carry a parent's commit to a child

## Check the comment-review file

Before the run, check each repository's main checkout. Never create
the file; ask the user about every line this prints:

```sh
rc=0
for r in '<repository root>'; do
  m=$(git -C "$r" rev-parse --path-format=absolute --git-common-dir) || exit 1
  m=$(dirname "$m")
  if [ ! -f "$m/.code-comments.json" ]; then
    echo "missing: $m/.code-comments.json"; rc=1
  elif ! git -C "$m" check-ignore -q .code-comments.json; then
    echo "not ignored: $m/.code-comments.json"; rc=1
  fi
done
exit $rc
```

## Set up a PR's run folder

The folder is under `~/.local/state`, so it survives a reboot.
`PARENT` is the branch this PR stacks on: the default branch, or the
parent PR's branch. `BR` is this PR's branch, named
`<type>/<words>`: a `git-messages` commit type, then lowercase words
joined by hyphens, at most 48 characters. `CHECK` is the plan's test
and lint command. Keep each value in single quotes, since vault paths
have spaces; use double quotes for a value that holds a single quote.

```sh
S=${XDG_STATE_HOME:-$HOME/.local/state}
mkdir -p "$S" && D=$(mktemp -d "$S/roadmap-run.XXXXXX") || exit 1
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
# Fails when the test command passes, since no test caught the bug.
plant() {
  local file=$1 old=$2 new=$3 backup rc
  shift 3
  backup=$(mktemp "$D/plant.XXXXXX") || return 1
  cp "$file" "$backup" || return 1
  python3 - "$file" "$old" "$new" <<'EOF' || { rm -f "$backup"; return 1; }
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
FNS
. "$D/env"
keep R '<repository root>'
keep P '<plan file>'
keep B '<full commit from the plan>'
keep REMOTE '<remote, such as origin>'
keep PARENT '<parent branch>'
keep BR '<branch, such as feat/agent-commands>'
keep CHECK '<test and lint command>'
keep M "$HOME/.agents/skills/git-messages/scripts/msg"
echo "run folder: $D"
```

Write the run folder's path on the roadmap page next to the PR.

## Write and lint the commit messages

Write `$D/MSG<step>` for each `### <step>.` heading in the plan,
following `git-messages`. Lint them before the build:

```sh
. <run folder>/env
planner check "$P" --repo "$R" --json-errors >/dev/null || exit 1
rg -o '^### ([0-9]+)\.' -r '$1' "$P" > "$D/steps" || exit 1
for s in $(cat "$D/steps"); do
  [ -s "$D/MSG$s" ] || { echo "missing: $D/MSG$s"; exit 1; }
  python3 "$M" fmt "$D/MSG$s" >/dev/null || exit 1
  python3 "$M" lint "$D/MSG$s" || { echo "lint failed: step $s"; exit 1; }
done
```

## Build the commits

Each step's commit has the tree `planner export` makes through that
step, on the commit of the step before, starting at `$B`:

```sh
. <run folder>/env
prev=$B
for s in $(cat "$D/steps"); do
  t=$(plan_tree "$P" "$R" "$B" "$s") || { echo "export failed: step $s"; exit 1; }
  echo "$t" > "$D/tree$s"
  prev=$(git -C "$R" commit-tree "$t" -p "$prev" -F "$D/MSG$s") || exit 1
  echo "$prev" > "$D/commit$s"
done
n=$(changed "$R" "$B" "$prev") || exit 1
echo "changed lines: $n"
```

## Make the PR's worktree

`wt` makes the branch `$BR` at `$B`; the merge then moves it to the
last step's commit:

```sh
. <run folder>/env
last=$(tail -n 1 "$D/steps")
wt -C "$R" switch --create "$BR" --base "$B" --no-cd --format=json \
  > "$D/wt.json" || exit 1
W=$(jq -r .path "$D/wt.json")
{ [ -n "$W" ] && [ "$W" != null ]; } || { echo "no worktree in $D/wt.json"; exit 1; }
keep W "$W"
git -C "$W" merge -q --ff-only "$(cat "$D/commit$last")" || exit 1
[ "$(git -C "$W" rev-parse 'HEAD^{tree}')" = "$(cat "$D/tree$last")" ] ||
  { echo "branch tree differs from the plan"; exit 1; }
```

Then run the plan's checks in `$W`, with Ship's worktree guard:

```sh
. <run folder>/env
{ [ -n "$W" ] && [ "$W" != null ] && cd "$W"; } ||
  { echo "no worktree: '$W'"; exit 1; }
sh -c "$CHECK" || exit 1
```

A failed build command means a stale plan or a broken tool. Do not
retry or edit files: stop this PR and its children, put the error on
the roadmap page as a blocking question, and keep building other
stacks.

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

After the first push, run Ship's later round right away. A fix that
stays local is not done.

## Plant a bug

Put back one bug a reviewer found. A test must fail, and the
worktree must be clean after:

```sh
. <run folder>/env
{ [ -n "$W" ] && [ "$W" != null ] && cd "$W"; } ||
  { echo "no worktree: '$W'"; exit 1; }
plant internal/x.go 'if a {' 'if false && a {' go test ./internal/... || exit 1
[ -z "$(git status --porcelain)" ] || { echo "worktree not clean"; exit 1; }
```

Read the output: a build error also fails the command, but only a
failed test proves the bug is caught.

## Ship

Write the PR text to `$D/FILE` first. Squash only before the branch's
first push; the script refuses a branch already on the remote:

```sh
. <run folder>/env
python3 "$M" fmt "$D/FILE" >/dev/null || exit 1
python3 "$M" lint --pr "$D/FILE" || exit 1
{ [ -n "$W" ] && [ "$W" != null ] && cd "$W"; } ||
  { echo "no worktree: '$W'"; exit 1; }
[ "$(git symbolic-ref -q --short HEAD)" = "$BR" ] ||
  { echo "not on $BR"; exit 1; }
[ -z "$(git status --porcelain)" ] || { echo "worktree not clean"; exit 1; }
sh -c "$CHECK" || { echo "checks failed; not pushing"; exit 1; }
git fetch -q "$REMOTE" || exit 1
git merge-base --is-ancestor "$REMOTE/$PARENT" HEAD ||
  { echo "$PARENT moved; move this plan's base and rebuild"; exit 1; }
git ls-remote --exit-code --heads "$REMOTE" "$BR" >/dev/null
rc=$?
[ "$rc" = 2 ] || { echo "$BR is already pushed or ls-remote failed"; exit 1; }
git reset -q --soft "$REMOTE/$PARENT" || exit 1
git commit -q -F "$D/FILE" || exit 1
git push --force-with-lease -u "$REMOTE" "HEAD:refs/heads/$BR" || exit 1
```

A later round, after the first push, adds a commit: use the same
script, but replace everything from `git ls-remote` on with
`git push -u "$REMOTE" "HEAD:refs/heads/$BR"`, then update the
PR text with `pde-gh-write pr edit NUMBER --body-file "$D/BODY"`, and
print the status line.

## File the PR request

This saves the request as a script, so the identical command can run
again later. Each run writes its exit code to `$D/request.result`:

```sh
. <run folder>/env
{ [ -n "$W" ] && [ "$W" != null ]; } || { echo "no worktree: '$W'"; exit 1; }
tail -n +3 "$D/FILE" > "$D/BODY" || exit 1
{
  printf 'cd %q || exit 1\n' "$W"
  printf 'pde-gh-write pr create --base %q --title %q --body-file %q\n' \
    "$PARENT" "$(head -n 1 "$D/FILE")" "$D/BODY"
  printf 'rc=$?\necho "exit $rc at $(date -u +%%FT%%TZ)" > %q\nexit $rc\n' \
    "$D/request.result"
} > "$D/request.sh"
bash "$D/request.sh"
echo "exit $?"
```

Put `bash <run folder>/request.sh` on the roadmap page. Rerunning it
reads the result:

- `0`, or `already ran <id>, exit 0`: the PR is open.
- `3`: it waits for the user; say so once and move on.
- `4`, `declined <id>`: park this PR and its children.
- `6`, `interrupted <id>`: it is never retried; ask the user.
- `2`: the command was refused; fix it, rewrite the script, run it.
- Anything else is from `gh`; put its output on the roadmap page.

A request expires four hours after it is filed. Rerunning the script
after that files a fresh request.

## Print a PR's status line

Write the roadmap's status line for a PR from this output, never by
hand:

```sh
. <run folder>/env
{ [ -n "$W" ] && [ "$W" != null ]; } || { echo "$BR: not built"; exit 0; }
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
echo "$BR: pushed ${pushed:-none}; local equals pushed: $same;" \
  "$n changed lines against $PARENT; request: $req"
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

After the parent ships, set the child plan's `Base commit:` line to
the parent's final commit and run `planner check`. If it passes, the
child's diffs apply unchanged; save the commit with `keep B` in the
child's run folder. If it fails, the parent changed after the child
was written: send the errors to the child's planner.

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
keep B "$nb"
```

Then set the plan's `Base commit:` line to `$nb`, run the plan's
checks, and run Ship's later round. A failed `planner check` is a
conflict: never resolve it by hand.
