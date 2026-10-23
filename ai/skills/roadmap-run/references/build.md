# Commands for a roadmap run

The snippets work in zsh and bash. Variables, arrays and functions
are lost between tool calls, so each PR keeps them in a run folder:
`env` holds the variables and loads `fns.sh`. Start every tool call
after the setup with `. <run folder>/env`, using the path the setup
printed. Each snippet stops with `exit 1` on the first failure; never
run the lines after a failed one by hand.

## Contents

- Set up a PR's run folder
- Make each step's patch
- Write and lint the commit messages
- Build with Vibe
- Resume after a failed step
- Apply a fix round
- Plant a bug
- Ship
- File the PR request
- Base a child on an unbuilt parent
- Move a child's base

## Set up a PR's run folder

The folder is under `~/.local/state`, so it survives a reboot.
`MODEL` is the model the user named for Vibe workers, or the one
their notes give. `PARENT` is the branch this PR stacks on: the
default branch, or `vibe/<parent key>`. `CHECK` is the plan's test
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
FNS
. "$D/env"
keep R '<repository root>'
keep P '<plan file>'
keep B '<full commit from the plan>'
keep KEY '<new Vibe key, such as agent-commands>'
keep MODEL '<provider/model for Vibe workers>'
keep REMOTE '<remote, such as origin>'
keep PARENT '<parent branch>'
keep CHECK '<test and lint command>'
keep M "$HOME/.agents/skills/git-messages/scripts/msg"
keep START "$B"
keep NEW_KEY 1
echo "run folder: $D"
```

Write the run folder's path on the roadmap page next to the PR.

## Make each step's patch

```sh
. <run folder>/env
planner check "$P" --repo "$R" --json-errors >/dev/null || exit 1
rg -o '^### ([0-9]+)\.' -r '$1' "$P" > "$D/steps" || exit 1
prev=$(git -C "$R" rev-parse "$B^{tree}") || exit 1
for s in $(cat "$D/steps"); do
  t=$(plan_tree "$P" "$R" "$B" "$s") || { echo "export failed: step $s"; exit 1; }
  git -C "$R" diff --binary --no-ext-diff --no-color "$prev" "$t" \
    > "$D/step$s.patch" || exit 1
  echo "$t" > "$D/tree$s"
  prev=$t
done
git -C "$R" diff --shortstat "$B" "$prev"
```

The last line is the PR's size. Over 700 changed lines, stop and
split the plan.

## Write and lint the commit messages

Write `$D/MSG<step>` for each step, following `git-messages`, and
the step prompts `$D/prompt<step>.md` from
[prompts.md](prompts.md). Lint before any build, and check that every
file the build reads exists:

```sh
. <run folder>/env
for s in $(cat "$D/steps"); do
  for f in "$D/MSG$s" "$D/prompt$s.md" "$D/step$s.patch" "$D/tree$s"; do
    [ -s "$f" ] || { echo "missing: $f"; exit 1; }
  done
  python3 "$M" fmt "$D/MSG$s" >/dev/null || exit 1
  python3 "$M" lint "$D/MSG$s" || { echo "lint failed: step $s"; exit 1; }
done
```

## Build with Vibe

Check the key first, as `implement-plan` says. Vibe makes the branch
`vibe/$KEY` and its worktree; do not make them yourself. Only the
first run of a new key takes `--base`:

```sh
. <run folder>/env
for s in $(cat "$D/steps"); do
  [ "$s" -lt "${FROM:-0}" ] && continue
  base=()
  if [ "$NEW_KEY" = 1 ]; then base=(--base "$START"); fi
  vibe run --key "$KEY" "${base[@]}" \
    --prompt-file "$D/prompt$s.md" --input "$D/step$s.patch" \
    --model "$MODEL" --commit-message "$(cat "$D/MSG$s")" \
    --stderr-level warn > "$D/run$s.json"
  st=$(jq -r '.status + " " + (.persistence_error // "none")' "$D/run$s.json")
  if [ "$st" != "completed none" ]; then
    echo "step $s: $st. Stop and park this PR; see $D/run$s.json"
    exit 1
  fi
  if [ "$NEW_KEY" = 1 ] &&
    [ "$(jq -r .pre_run_commit "$D/run$s.json")" != "$START" ]; then
    echo "step $s: Vibe did not start from $START"
    exit 1
  fi
  W=$(jq -r .worktree "$D/run$s.json")
  keep W "$W"
  keep NEW_KEY 0
  if [ "$(git -C "$W" rev-parse 'HEAD^{tree}')" != "$(cat "$D/tree$s")" ]; then
    echo "step $s: branch tree differs from the plan"
    exit 1
  fi
  git -C "$W" rev-parse HEAD > "$D/commit$s"
done
```

Then run the plan's checks in `$W`: `(cd "$W" && sh -c "$CHECK")`.

When a step stops, follow `implement-plan`'s status rules. Where they
need a recovery decision, do not reset, clean, retry or go on: park
this PR and its children, put `$D/run<step>.json` and its status on
the roadmap page as a blocking question, and keep building other
stacks.

## Resume after a failed step

Once the user decides how to recover step N, build the rest under a
new key that starts at the last step whose tree matched. Use
`commit<N-1>`, or `$B` when N is the first step:

```sh
. <run folder>/env
keep KEY '<new Vibe key>'
keep START "$(cat "$D/commit<N-1>")"
keep FROM '<N>'
keep NEW_KEY 1
```

Then run the build loop again. The PR's branch is now `vibe/<new key>`.

## Apply a fix round

After the planner revises the plan, apply the whole difference as one
commit. Read the step list again, since the revision may add steps:

```sh
. <run folder>/env
planner check "$P" --repo "$R" --json-errors >/dev/null || exit 1
rg -o '^### ([0-9]+)\.' -r '$1' "$P" > "$D/steps" || exit 1
t=$(plan_tree "$P" "$R" "$B" "$(tail -n 1 "$D/steps")") || exit 1
echo "$t" > "$D/treefix"
git -C "$W" diff --binary --no-ext-diff --no-color HEAD "$t" > "$D/fix.patch"
```

Write `$D/MSGfix` and `$D/promptfix.md` naming `fix.patch`, then run
it. The key exists, so there is no `--base`:

```sh
. <run folder>/env
python3 "$M" fmt "$D/MSGfix" >/dev/null || exit 1
python3 "$M" lint "$D/MSGfix" || exit 1
vibe run --key "$KEY" --prompt-file "$D/promptfix.md" \
  --input "$D/fix.patch" --model "$MODEL" \
  --commit-message "$(cat "$D/MSGfix")" --stderr-level warn > "$D/runfix.json"
st=$(jq -r '.status + " " + (.persistence_error // "none")' "$D/runfix.json")
[ "$st" = "completed none" ] || { echo "fix: $st. Park this PR"; exit 1; }
[ "$(git -C "$W" rev-parse 'HEAD^{tree}')" = "$(cat "$D/treefix")" ] ||
  { echo "fix: branch tree differs from the plan"; exit 1; }
```

## Plant a bug

Put back one bug a reviewer found. A test must fail, and the
worktree must be clean after:

```sh
. <run folder>/env
cd "$W" || exit 1
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
[ "$(git symbolic-ref -q --short HEAD)" = "vibe/$KEY" ] ||
  { echo "not on vibe/$KEY"; exit 1; }
[ -z "$(git status --porcelain)" ] || { echo "worktree not clean"; exit 1; }
sh -c "$CHECK" || { echo "checks failed; not pushing"; exit 1; }
git fetch -q "$REMOTE" || exit 1
git merge-base --is-ancestor "$REMOTE/$PARENT" HEAD ||
  { echo "$PARENT moved; move this plan's base and rebuild"; exit 1; }
git ls-remote --exit-code --heads "$REMOTE" "vibe/$KEY" >/dev/null
rc=$?
[ "$rc" = 2 ] || { echo "vibe/$KEY is already pushed or ls-remote failed"; exit 1; }
git reset -q --soft "$REMOTE/$PARENT" || exit 1
git commit -q -F "$D/FILE" || exit 1
git push --force-with-lease -u "$REMOTE" "HEAD:refs/heads/vibe/$KEY" || exit 1
```

A later round, after the first push, adds a commit: use the same
script, but replace everything from `git ls-remote` on with
`git push -u "$REMOTE" "HEAD:refs/heads/vibe/$KEY"`, then update the
PR text with `pde-gh-write pr edit`.

## File the PR request

This saves the request as a script, so the identical command can run
again later:

```sh
. <run folder>/env
tail -n +3 "$D/FILE" > "$D/BODY" || exit 1
{
  printf 'cd %q || exit 1\n' "$W"
  printf 'pde-gh-write pr create --base %q --title %q --body-file %q\n' \
    "$PARENT" "$(head -n 1 "$D/FILE")" "$D/BODY"
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
child's diffs apply unchanged; save the commit with `keep B` and
`keep START` in the child's run folder. If it fails, the parent
changed after the child was written: send the errors to the child's
planner.
